//! Tier 2：真实 rustc 编译矩阵（P2-16 §2-§3，素材见 tests/fixtures/）。
//!
//! 遍历 fixtures/{errors,nocode,panic,dead_codes}/** 的真实代码，用 DevSandbox
//! （与生产管线同参数：`rustc --edition 2021` + 临时目录）编译/运行，断言错误码与
//! 行号（**只比 E 码与行号、不比文本**，抗 rustc 措辞漂移）。断言期望全部来自
//! expected.toml（line 为 rustc 1.97 实测值，L3-B2 §3）。
//!
//! 成本：L3-B2 预估 18 场景 ≈ 8-10s 串行 / 4 路并行 ≈ 3s（慢机基准）；
//! 本机（rustc 1.97）实测 4 路并行 ≈ 0.9s、串行 ≈ 1.7s，均远低于预算。
//!
//! # CI 策略
//! 本文件所有需要真实编译的测试都标 `#[ignore]`：本地 `cargo test -p game-core`
//! 默认跳过（依赖 rustc 可执行文件 + 约 3-8s，不宜进日常增量）；CI 用
//! `cargo test -p game-core -- --ignored` 全跑。`fixture_metadata_consistency`
//! 是纯静态检查（零 rustc），默认运行。

mod common;
use common::*;

use game_core::level::parse_levels;
use game_core::sandbox::{CompileOutcome, DevSandbox, RunOutcome, Sandbox};
use game_core::validate::error_parser::{sanitize_panic, IssueKind};
use game_core::validate::mapper::ErrorMapper;
use game_core::validate::{validate, Validation};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Instant;

/// expected.toml 解析出的断言目标
#[derive(Debug, Clone)]
enum Expect {
    /// errors/：首条错误码 + 首个 `-->` 行号
    Compile { code: String, line: u32 },
    /// nocode/：EUNKNOWN 兜底 + 行号 + 稳定消息子串
    NoCode { code: String, line: u32, message_contains: String },
    /// panic/：净化+分类后的分类 id + 行号 + 净化消息子串
    Panic { class: &'static str, line: u32, message_contains: String },
    /// dead_codes/：负面断言——此码不得出现在解析结果中（errors 为空或其它活跃码）
    DeadCode { not_emitted: String },
}

/// 单个 fixture 用例（fixture 目录自动遍历发现，新增 fixture 无需改测试代码）
struct FixtureCase {
    name: String,
    dir: PathBuf,
    expect: Expect,
}

/// 从 expected.toml 读取断言元数据（schema 见 tests/fixtures/README.md）
fn parse_expect(dir: &Path) -> Expect {
    let content = std::fs::read_to_string(dir.join("expected.toml"))
        .unwrap_or_else(|e| panic!("{}: 读取 expected.toml 失败: {e}", dir.display()));
    let m = parse_flat_toml(&content);
    let kind = m.get("kind").expect("expected.toml 缺 kind 字段").clone();
    match kind.as_str() {
        "compile" => Expect::Compile {
            code: m["code"].clone(),
            line: m["line"].parse().expect("line 应为整数"),
        },
        "nocode" => Expect::NoCode {
            code: m["code"].clone(),
            line: m["line"].parse().expect("line 应为整数"),
            message_contains: m["message_contains"].clone(),
        },
        "panic" => Expect::Panic {
            class: class_id(class_from_id(&m["classification"])),
            line: m["line"].parse().expect("line 应为整数"),
            message_contains: m["message_contains"].clone(),
        },
        "deadcode" => Expect::DeadCode { not_emitted: m["code"].clone() },
        other => panic!("未知 kind: {other}"),
    }
}

/// 遍历 fixtures/{errors,nocode,panic,dead_codes}/*/ 发现全部用例
fn discover() -> Vec<FixtureCase> {
    let root = fixtures_root();
    let mut cases = Vec::new();
    for cat in ["errors", "nocode", "panic", "dead_codes"] {
        let dir = root.join(cat);
        let mut subdirs: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("fixtures 目录 {dir:?} 不可读: {e}"))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subdirs.sort();
        for d in subdirs {
            let name = format!("{cat}/{}", d.file_name().unwrap().to_string_lossy());
            let expect = parse_expect(&d);
            cases.push(FixtureCase { name, dir: d, expect });
        }
    }
    assert!(!cases.is_empty(), "未发现任何 fixture（{root:?}）");
    cases
}

/// 断言 broken.rs 的编译/运行结果符合 expected.toml
fn run_case(case: &FixtureCase) -> Result<(), String> {
    let sb = DevSandbox::new();
    let broken = std::fs::read_to_string(case.dir.join("broken.rs"))
        .map_err(|e| format!("读取 broken.rs 失败: {e}"))?;
    let compile = sb.compile(&broken).map_err(|e| format!("编译错误: {e}"))?;

    match &case.expect {
        Expect::Compile { code, line } => match compile {
            CompileOutcome::Failed { errors } => {
                let first = errors.first().ok_or("编译失败但 errors 为空（解析器漏检）")?;
                if first.code != *code {
                    return Err(format!("首条码 {} != 期望 {code}（errors: {errors:?}）", first.code));
                }
                if first.line != Some(*line) {
                    return Err(format!("首条行 {:?} != 期望 {line}", first.line));
                }
                if first.kind != IssueKind::CompileCode {
                    return Err(format!("kind 应为 CompileCode，实际 {:?}", first.kind));
                }
                Ok(())
            }
            other => Err(format!("预期编译失败（{code}），实际 {other:?}")),
        },
        Expect::NoCode { code, line, message_contains } => match compile {
            CompileOutcome::Failed { errors } => {
                let first = errors.first().ok_or("编译失败但 errors 为空（解析器漏检）")?;
                if first.code != *code {
                    return Err(format!("首条码 {} != 期望 {code}（errors: {errors:?}）", first.code));
                }
                if first.kind != IssueKind::NoCode {
                    return Err(format!("无码错误 kind 应为 NoCode，实际 {:?}", first.kind));
                }
                if first.line != Some(*line) {
                    return Err(format!("首条行 {:?} != 期望 {line}", first.line));
                }
                if !first.message.contains(message_contains) {
                    return Err(format!("消息缺稳定子串 {message_contains:?}：{}", first.message));
                }
                Ok(())
            }
            other => Err(format!("预期无码编译失败，实际 {other:?}")),
        },
        Expect::Panic { class, line, message_contains } => {
            let binary = match compile {
                CompileOutcome::Success { binary } => binary,
                other => return Err(format!("panic fixture 应先编译成功，实际 {other:?}")),
            };
            let message = match sb.run(&binary).map_err(|e| format!("运行错误: {e}"))? {
                RunOutcome::Panic { message } => message,
                other => return Err(format!("预期 panic，实际 {other:?}")),
            };
            let sp = sanitize_panic(&message);
            if class_id(sp.class) != *class {
                return Err(format!("panic 分类 {} != 期望 {class}（净化后: {sp:?}）", class_id(sp.class)));
            }
            if sp.line != Some(*line) {
                return Err(format!("panic 行 {:?} != 期望 {line}（净化后: {sp:?}）", sp.line));
            }
            if !sp.message.contains(message_contains) {
                return Err(format!("净化消息缺子串 {message_contains:?}：{}", sp.message));
            }
            Ok(())
        }
        Expect::DeadCode { not_emitted } => {
            // 负面断言：死码不得被误报为活跃错误码
            match compile {
                CompileOutcome::Failed { errors } => {
                    if errors.iter().any(|e| e.code == *not_emitted) {
                        Err(format!("死码 {not_emitted} 被解析为活跃错误（errors: {errors:?}）"))
                    } else {
                        Ok(())
                    }
                }
                CompileOutcome::Success { .. } => Ok(()), // errors 为空：死码未发射
            }
        }
    }
}

/// 断言 fixed.rs 编译通过 + 运行成功（不比输出文本）
fn check_fixed(case: &FixtureCase) -> Result<(), String> {
    let fixed = std::fs::read_to_string(case.dir.join("fixed.rs"))
        .map_err(|e| format!("读取 fixed.rs 失败: {e}"))?;
    let sb = DevSandbox::new();
    let binary = match sb.compile(&fixed).map_err(|e| format!("编译错误: {e}"))? {
        CompileOutcome::Success { binary } => binary,
        other => return Err(format!("fixed.rs 应编译通过，实际 {other:?}")),
    };
    match sb.run(&binary).map_err(|e| format!("运行错误: {e}"))? {
        RunOutcome::Ok { .. } => Ok(()),
        RunOutcome::Panic { message } => Err(format!("fixed.rs 运行 panic: {message}")),
        RunOutcome::Timeout => Err("fixed.rs 运行超时".into()),
    }
}

/// 多路并行执行（4 路 ≈3-4s；串行 ≈8s），保持输入顺序收集结果。
/// 用 mpsc 通道回传（免锁），worker 取任务用原子计数器。
fn parallel_map<T: Send + Sync, F: Fn(&T) -> Result<(), String> + Sync + Send>(
    items: &[T],
    workers: usize,
    f: F,
) -> Vec<Result<(), String>> {
    let (tx, rx) = mpsc::channel::<(usize, Result<(), String>)>();
    let next = AtomicUsize::new(0);
    let next = &next; // 共享引用：所有 worker 闭包共同借用
    let f = &f; // F: Sync → &F 可跨线程共享
    let mut out: Vec<Result<(), String>> = (0..items.len()).map(|_| Ok(())).collect();
    std::thread::scope(|s| {
        for _ in 0..workers {
            let tx = tx.clone();
            s.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= items.len() {
                    break;
                }
                let r = f(&items[i]);
                let _ = tx.send((i, r));
            });
        }
        drop(tx);
        for (i, r) in rx.iter() {
            out[i] = r;
        }
    });
    out
}

// ============ 纯静态检查（零 rustc，默认运行） ============

#[test]
fn fixture_metadata_consistency() {
    let cases = discover();
    // 矩阵规模锁（18 场景 = errors 12 + nocode 2 + panic 2 + dead_codes 2）
    let count = |cat: &str| cases.iter().filter(|c| c.name.starts_with(&format!("{cat}/"))).count();
    assert_eq!(count("errors"), 12, "errors/ 应为 12（11 码 + 方案 A clippy 改写）");
    assert_eq!(count("nocode"), 2, "nocode/ 应为 2");
    assert_eq!(count("panic"), 2, "panic/ 应为 2");
    assert_eq!(count("dead_codes"), 2, "dead_codes/ 应为 2");
    assert_eq!(cases.len(), 18, "fixture 总数应为 18（新增需同步 README 计数）");

    for case in &cases {
        // 目录分类与 expected.toml kind 一致
        let cat = case.name.split('/').next().unwrap();
        let kind = match &case.expect {
            Expect::Compile { .. } => "compile",
            Expect::NoCode { .. } => "nocode",
            Expect::Panic { .. } => "panic",
            Expect::DeadCode { .. } => "deadcode",
        };
        let expect_kind = match cat {
            "errors" => "compile",
            "nocode" => "nocode",
            "panic" => "panic",
            "dead_codes" => "deadcode",
            _ => unreachable!(),
        };
        assert_eq!(kind, expect_kind, "{}: kind 与目录不符", case.name);

        // broken.rs 必须存在；errors/nocode/panic 必须成对提供 fixed.rs
        assert!(case.dir.join("broken.rs").exists(), "{}: 缺 broken.rs", case.name);
        match cat {
            "dead_codes" => assert!(
                !case.dir.join("fixed.rs").exists(),
                "{}: dead_codes 是负面断言，不需要 fixed.rs",
                case.name
            ),
            _ => assert!(case.dir.join("fixed.rs").exists(), "{}: 缺 fixed.rs", case.name),
        }
    }

    // 方案 A 样例关：level.toml 可解析为合法关卡，且 starter_code 与 broken.rs 逐字节一致
    let pa = fixtures_root().join("errors/E0308_clippy_approx_constant");
    let level_toml = std::fs::read_to_string(pa.join("level.toml")).expect("方案 A 样例关缺 level.toml");
    let levels = parse_levels(&level_toml).expect("level.toml 无法解析为关卡");
    assert_eq!(levels.len(), 1);
    let lv = &levels[0];
    assert!(lv.allow_compile_fail, "方案 A 样例关必须 allow_compile_fail = true");
    assert_eq!(lv.expect_error_code, "E0308");
    let broken = std::fs::read_to_string(pa.join("broken.rs")).unwrap();
    assert_eq!(lv.starter_code, broken, "样例关 starter_code 必须与 broken.rs 一致");
}

// ============ Tier 2：真实编译矩阵（#[ignore]，CI 全跑） ============

#[test]
#[ignore = "真实 rustc 编译矩阵（18 场景 ≈3-8s）：本地跳过，CI 用 -- --ignored 全跑"]
fn tier2_real_compile_matrix() {
    let cases = discover();
    let start = Instant::now();
    let results = parallel_map(&cases, 4, |c| {
        run_case(c).and_then(|_| {
            // dead_codes 无 fixed.rs（负面断言），其余断言修复版可编译运行
            if c.dir.join("fixed.rs").exists() {
                check_fixed(c)
            } else {
                Ok(())
            }
        })
    });
    let elapsed = start.elapsed();

    let mut failures: Vec<String> = Vec::new();
    for (case, r) in cases.iter().zip(results) {
        if let Err(e) = r {
            failures.push(format!("{}: {e}", case.name));
        }
    }
    assert!(
        failures.is_empty(),
        "Tier 2 失败 {} / {} 个（耗时 {elapsed:?}）：\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
    eprintln!("Tier 2 矩阵：{} 场景 / 4 路并行耗时 {elapsed:?}", cases.len());
}

#[test]
#[ignore = "方案 A 样例关全流程验证（真实编译）：本地跳过，CI 全跑"]
fn tier2_plan_a_clippy_level_end_to_end() {
    // 方案 A（P2-16 §4 / L3-B2 §1.3）：clippy lint（approx_constant 风格裸字面量）
    // 在裸 rustc 下不触发，改写为等价编译错误 `let pi: i32 = 3.14;` → E0308，
    // 以编译错误关形式收录。此测试用完整 validate() 流程验证样例关可用。
    let pa = fixtures_root().join("errors/E0308_clippy_approx_constant");
    let level_toml = std::fs::read_to_string(pa.join("level.toml")).unwrap();
    let levels = parse_levels(&level_toml).unwrap();
    let lv = &levels[0];
    let sb = DevSandbox::new();
    let mapper = ErrorMapper::default_fallback();

    // broken starter（clippy 改写版）→ allow_compile_fail 判定通过（首条码 E0308 命中）
    match validate(lv, &lv.starter_code, &mapper, &sb).unwrap() {
        Validation::Pass => {}
        other => panic!("broken starter 应判定通过，实际 {other:?}"),
    }

    // fixed 代码（类型修正 f64）→ allow_compile_fail 分支必须 Fail（“编译成功”）
    let fixed = std::fs::read_to_string(pa.join("fixed.rs")).unwrap();
    match validate(lv, &fixed, &mapper, &sb).unwrap() {
        Validation::Fail { feedback } => {
            assert!(feedback[0].contains("编译成功"), "应提示编译成功: {feedback:?}");
        }
        other => panic!("fixed 代码应判定 Fail，实际 {other:?}"),
    }
}
