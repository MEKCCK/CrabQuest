//! Tier 3：关卡回归（P2-16 §3）——55 关 starter 编译断言 + 01-l0-print 反馈非空锁。
//!
//! 从 `assets/levels/*.toml` **动态读取** starter_code（不复制代码进测试，防素材
//! 漂移；T3 中文字面量修订也不影响），用 DevSandbox 真实编译，断言每关 starter 的
//! 期望画像：首条错误码（rustc 输出顺序，L3-B2 §1.1）/ 编译成功 / 编译成功+运行
//! panic 分类。期望表源自 docs-review/L3-B2-parser.md §3 的 15 fixture 实测
//! （rustc 1.97）+ 各 verification-*.md 实测；**不锁行号**（素材微调会漂），只锁
//! 错误码与运行行为。
//!
//! 01-l0-print 单独经 `validate()` 全流程断言反馈非空（锁死空反馈 bug，P1-01 /
//! L3-B2 §1.5：format 参数错误无 E 码，解析器必须产出 EUNKNOWN）。
//!
//! 本文件为默认运行的回归测试（非 #[ignore]）；成本 ≈ 55 次编译 + 数次运行
//! ≈ 15-60s（Tier3 全量画像，属正常耗时）。

mod common;
use common::*;

use game_core::level::LevelSet;
use game_core::sandbox::{CompileOutcome, DevSandbox, RunOutcome, Sandbox};
use game_core::validate::error_parser::{sanitize_panic, IssueKind};
use game_core::validate::mapper::ErrorMapper;
use game_core::validate::{validate, Validation};

/// 每关 starter 的期望画像。
/// 期望码 = 该关 starter 编译失败时的**首条**错误码（rustc 输出顺序）；
/// EUNKNOWN = 无 E 码编译错误（走兜底，L3-B2 §1.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterExpect {
    /// 预期编译失败，首条错误码 = code（"EUNKNOWN" = 无码错误）
    CompileFail { code: &'static str },
    /// 预期编译成功 + 正常运行（输出与 expect_output 不符由关卡设计保证）
    Compiles,
    /// 预期编译成功 + 运行 panic，分类 = class（L3-B2 §1.4 分类表 id）
    CompilesThenPanic { class: &'static str },
}

/// 期望表：按关卡 id 推导自 L3-B2 §3 的 fixture 实测（15 存量关 × rustc 1.97）+ 各
/// verification-*.md 实测（新增 40 关：T3 rustlings / T4 100ex+rust-quiz / T5 quiz / T6
/// expect_panic / T7 22 关）。新增关卡必须在此补充条目，否则 `expect_for` panic（防静默漏测）。
fn expect_for(id: &str) -> StarterExpect {
    match id {
        // 00-l0-hello：x 未声明 → E0425（F1）
        "l0-hello" => StarterExpect::CompileFail { code: "E0425" },
        // 01-l0-print：println! 格式参数数量错误，无 E 码 → EUNKNOWN（F12）
        "l0-print" => StarterExpect::CompileFail { code: "EUNKNOWN" },
        // 02-l0-function：call_me 未定义 → E0425
        "l0-function" => StarterExpect::CompileFail { code: "E0425" },
        // 03-l0-loop：编译通过，输出 10 ≠ 期望 15
        "l0-loop" => StarterExpect::Compiles,
        // 04-l1-move：fill_vec 参数非 mut → E0596（F2）
        "l1-move" => StarterExpect::CompileFail { code: "E0596" },
        // 05-l1-borrow：calculate_length 拿走所有权 → E0382（F3）
        "l1-borrow" => StarterExpect::CompileFail { code: "E0382" },
        // 06-l1-mut-borrow：多码场景，rustc 输出顺序首条 E0382（L3-B2 §1.1 实测）
        "l1-mut-borrow" => StarterExpect::CompileFail { code: "E0382" },
        // 07-l1-clone：s2 = s1 后使用 s1 → E0382
        "l1-clone" => StarterExpect::CompileFail { code: "E0382" },
        // 08-l2-vec：编译通过，运行 v[3] 越界 panic → array_index_oob（F14）
        "l2-vec" => StarterExpect::CompilesThenPanic { class: "array_index_oob" },
        // 09-l2-option：编译通过，输出 none ≠ 期望 3
        "l2-option" => StarterExpect::Compiles,
        // 10-l2-result（现 24-l2-result）：编译通过，运行 unwrap(Err) panic → unwrap_result_err
        // （starter 内嵌 FAIL_MSG 中文字面量，P1-06 修订后仍编译成功，仅运行 panic）
        "l2-result" => StarterExpect::CompilesThenPanic { class: "unwrap_result_err" },
        // 11-l3-lifetime：longest 缺生命周期标注 → E0106（F4）
        "l3-lifetime" => StarterExpect::CompileFail { code: "E0106" },
        // 12-l3-trait：Rectangle 未实现 area → E0599（F5）
        "l3-trait" => StarterExpect::CompileFail { code: "E0599" },
        // 13-l4-drop-order：编译通过，输出顺序不符（无编译/运行错误）
        "l4-drop-order" => StarterExpect::Compiles,
        // 14-l4-lifetime-trap：借用悬垂 → E0597（F6）
        "l4-lifetime-trap" => StarterExpect::CompileFail { code: "E0597" },
        // ---- 以下为本分支新增 18 关（T3 rustlings 8 + T4 100ex/rust-quiz 8 + T5 quiz 1 + T6 expect_panic 1）----
        // 04-l0-integers：u8/u32 混乘 → E0308（verification-100ex-quiz.md 实测）
        "l0-integers" => StarterExpect::CompileFail { code: "E0308" },
        // 20-l1-ownership-ticket：访问器按值接收 self，二次调用已移动 → E0382（100ex 实测）
        "l1-ownership-ticket" => StarterExpect::CompileFail { code: "E0382" },
        // 27-l2-saturating：编译通过，运行期 factorial(20) 溢出 panic → integer_overflow（100ex 实测）
        "l2-saturating" => StarterExpect::CompilesThenPanic { class: "integer_overflow" },
        // 47-l4-lazy-map：编译通过，输出 123101 ≠ 期望 112031（rust-quiz 实测）
        "l4-lazy-map" => StarterExpect::Compiles,
        // 48-l4-fnptr：allow_compile_fail 关，starter 可编译输出 0（rust-quiz 实测）
        "l4-fnptr" => StarterExpect::Compiles,
        // 50-l4-drop-underscore：编译通过，输出 12 ≠ 期望 21（rust-quiz 实测）
        "l4-drop-underscore" => StarterExpect::Compiles,
        // 51-l4-lifetime-ext：编译通过，输出 0101 ≠ 期望 1001（rust-quiz 实测）
        "l4-lifetime-ext" => StarterExpect::Compiles,
        // 52-l4-fnmut-copy：参数无 Copy 约束，二次调用已移动 → E0382（rust-quiz 实测）
        "l4-fnmut-copy" => StarterExpect::CompileFail { code: "E0382" },
        // 49-l4-mutable-zst：quiz 关，展示代码可编译 + 正常运行输出 "1"（verification-quiz.md 实测）
        "l4-mutable-zst" => StarterExpect::Compiles,
        // 54-l2-panics：expect_panic 关，broken 版编译通过但不 panic（verification-panics.md 实测）
        "l2-panics" => StarterExpect::Compiles,
        // 14-l1-move2：use of moved value → E0382（verification-rustlings.md 实测）
        "l1-move2" => StarterExpect::CompileFail { code: "E0382" },
        // 15-l1-move3：cannot borrow as mutable → E0596（rustlings 实测）
        "l1-move3" => StarterExpect::CompileFail { code: "E0596" },
        // 25-l2-errors3：main 内 `?` 但 main 返回 () → E0277（rustlings 实测）
        "l2-errors3" => StarterExpect::CompileFail { code: "E0277" },
        // 26-l2-errors2：Result 不能与整数相乘 → E0369（rustlings 实测）
        "l2-errors2" => StarterExpect::CompileFail { code: "E0369" },
        // 28-l2-errors4：broken 编译通过（负数/零被放行），逻辑/运行期修复关（rustlings 实测）
        "l2-errors4" => StarterExpect::Compiles,
        // 29-l2-vecs2：broken 编译通过（循环体为空输出 []），逻辑/运行期修复关（rustlings 实测）
        "l2-vecs2" => StarterExpect::Compiles,
        // 36-l3-lifetime3：missing lifetime specifier ×2 → E0106（rustlings 实测）
        "l3-lifetime3" => StarterExpect::CompileFail { code: "E0106" },
        // 37-l3-lifetime1：missing lifetime specifier → E0106（rustlings 实测）
        "l3-lifetime1" => StarterExpect::CompileFail { code: "E0106" },
        // ---- 以下为 T7（P4-25 Wave 3a）新增 22 关（L0 5 + L1 5 + L2 4 + L3 7 + L4 1，verification-T7-*.md 实测）----
        // 05-l0-variables2：let x; 无初始值，类型无法推断 → E0283（verification-T7-l0.md 实测）
        "l0-variables2" => StarterExpect::CompileFail { code: "E0283" },
        // 06-l0-if：函数体为空返回 ()，期望 i32 → E0308（T7-l0 实测）
        "l0-if" => StarterExpect::CompileFail { code: "E0308" },
        // 07-l0-primitives：is_evening 未定义 → E0425（T7-l0 实测）
        "l0-primitives" => StarterExpect::CompileFail { code: "E0425" },
        // 08-l0-functions2：call_me 缺实参 → E0061（T7-l0 实测）
        "l0-functions2" => StarterExpect::CompileFail { code: "E0061" },
        // 09-l0-boss：total 未定义 → E0425（T7-l0 实测；is_boss=false 普通综合关）
        "l0-boss" => StarterExpect::CompileFail { code: "E0425" },
        // 16-l1-strings：函数体返回 &str，签名要求 String → E0308（T7-l1 实测）
        "l1-strings" => StarterExpect::CompileFail { code: "E0308" },
        // 17-l1-structs：初始化缺 blue 字段 → E0063（T7-l1 实测）
        "l1-structs" => StarterExpect::CompileFail { code: "E0063" },
        // 18-l1-options1：函数体空返回 ()，要求 Option<u16> → E0308（T7-l1 实测）
        "l1-options1" => StarterExpect::CompileFail { code: "E0308" },
        // 19-l1-enums：找不到 Resize 变体 → E0599（T7-l1 实测）
        "l1-enums" => StarterExpect::CompileFail { code: "E0599" },
        // 21-l1-boss：note 未声明 mut 却可变借用 → E0596（T7-l1 实测）
        "l1-boss" => StarterExpect::CompileFail { code: "E0596" },
        // 30-l2-hashmap：basket 未定义 → E0425（T7-l2 实测）
        "l2-hashmap" => StarterExpect::CompileFail { code: "E0425" },
        // 31-l2-strings2：String 传入 &str 形参 → E0308（T7-l2 实测）
        "l2-strings2" => StarterExpect::CompileFail { code: "E0308" },
        // 32-l2-match：match 未覆盖 West 分支 → E0004（T7-l2 实测）
        "l2-match" => StarterExpect::CompileFail { code: "E0004" },
        // 33-l2-boss：match None 分支返回 &str，期望 u32 → E0308（T7-l2 实测）
        "l2-boss" => StarterExpect::CompileFail { code: "E0308" },
        // 38-l3-generics：Vec 元素类型无法推断 → E0282（T7-l3 实测）
        "l3-generics" => StarterExpect::CompileFail { code: "E0282" },
        // 39-l3-traits1：impl 缺 append_bar 方法 → E0046（T7-l3 实测）
        "l3-traits1" => StarterExpect::CompileFail { code: "E0046" },
        // 40-l3-iterators：空函数体返回 ()，要求 u64 → E0308（T7-l3 实测）
        "l3-iterators" => StarterExpect::CompileFail { code: "E0308" },
        // 41-l3-iterators2：返回 Map 迭代器与 Vec<String> 不符 → E0308（T7-l3 实测）
        "l3-iterators2" => StarterExpect::CompileFail { code: "E0308" },
        // 42-l3-conversions：f64 与 usize 不能相除 → E0277（T7-l3 实测）
        "l3-conversions" => StarterExpect::CompileFail { code: "E0277" },
        // 43-l3-enums3：逻辑修复关，broken 编译通过（process 方法体为空，输出全部初始状态）
        "l3-enums3" => StarterExpect::Compiles,
        // 44-l3-boss：Item<T> 的 name 字段缺生命周期标注 → E0106（T7-l3 实测）
        "l3-boss" => StarterExpect::CompileFail { code: "E0106" },
        // 53-l4-boss：add 接收 &self 却 push 可变 → E0596（T7-l4 实测）
        "l4-boss" => StarterExpect::CompileFail { code: "E0596" },
        other => panic!("期望表未覆盖关卡 {other}：新增关卡必须补充期望画像"),
    }
}

/// 单关断言：编译 starter → 校验画像；编译失败时 errors 必须非空
/// （errors 为空会触发 validate 的 FALLBACK 硬兜底文案，仍非空反馈，但说明解析器
/// 漏检，应视为失败——L3-B2 §1.5 硬兜底是最后防线，不是常态）。
fn check_level(lv: &game_core::level::Level, sb: &DevSandbox) -> Result<(), String> {
    let exp = expect_for(&lv.id);
    let compile = sb.compile(&lv.starter_code).map_err(|e| format!("编译错误: {e}"))?;
    match (exp, compile) {
        (StarterExpect::CompileFail { code }, CompileOutcome::Failed { errors }) => {
            let first = errors.first().ok_or("编译失败但 errors 为空（解析器漏检）")?;
            if first.code != code {
                return Err(format!("首条码 {} != 期望 {code}（errors: {errors:?}）", first.code));
            }
            if code == "EUNKNOWN" && first.kind != IssueKind::NoCode {
                return Err(format!("EUNKNOWN 的 kind 应为 NoCode，实际 {:?}", first.kind));
            }
            Ok(())
        }
        (StarterExpect::CompileFail { code }, CompileOutcome::Success { .. }) => {
            Err(format!("预期编译失败（{code}），实际编译成功"))
        }
        (StarterExpect::Compiles, CompileOutcome::Success { binary }) => match sb.run(&binary) {
            Ok(RunOutcome::Ok { .. }) => Ok(()),
            Ok(RunOutcome::Panic { message }) => {
                Err(format!("预期正常运行，实际 panic: {}", sanitize_panic(&message).message))
            }
            Ok(RunOutcome::Timeout) => Err("运行超时".into()),
            Err(e) => Err(format!("运行错误: {e}")),
        },
        (StarterExpect::Compiles, CompileOutcome::Failed { errors }) => {
            Err(format!("预期编译成功，实际编译失败 {errors:?}"))
        }
        (StarterExpect::CompilesThenPanic { class }, CompileOutcome::Success { binary }) => {
            match sb.run(&binary) {
                Ok(RunOutcome::Panic { message }) => {
                    let sp = sanitize_panic(&message);
                    if class_id(sp.class) != class {
                        Err(format!("panic 分类 {} != 期望 {class}（净化后: {sp:?}）", class_id(sp.class)))
                    } else {
                        Ok(())
                    }
                }
                Ok(other) => Err(format!("预期 panic，实际 {other:?}")),
                Err(e) => Err(format!("运行错误: {e}")),
            }
        }
        (StarterExpect::CompilesThenPanic { .. }, CompileOutcome::Failed { errors }) => {
            Err(format!("预期编译成功，实际编译失败 {errors:?}"))
        }
    }
}

/// Tier 3 主断言：55 关 starter 编译画像（错误码 / 编译成功 / panic 分类）。
#[test]
fn tier3_levels_starter_error_codes() {
    let set = LevelSet::load(&assets_levels_dir()).expect("加载 assets/levels 失败");
    assert_eq!(
        set.len(),
        55,
        "关卡数应为 55（15 存量 + T3 8 + T4 8 + T5 quiz 1 + T6 expect_panic 1 + T7 22；新增关卡必须同步更新 expect_for 期望表）"
    );
    let sb = DevSandbox::new();
    let mut failures: Vec<String> = Vec::new();
    for lv in &set.levels {
        if let Err(e) = check_level(lv, &sb) {
            failures.push(format!("{}: {e}", lv.id));
        }
    }
    assert!(
        failures.is_empty(),
        "Tier 3 失败 {} / {} 关：\n{}",
        failures.len(),
        set.len(),
        failures.join("\n")
    );
}

/// 锁死空反馈 bug（P1-01 / L3-B2 §1.5）：
/// 01-l0-print starter 的 format 参数错误**无 E 码**，解析器必须产出 EUNKNOWN，
/// validate() 反馈必须非空（历史 bug：空白反馈面板，现网级回归）。
#[test]
fn tier3_l0_print_feedback_non_empty() {
    let set = LevelSet::load(&assets_levels_dir()).expect("加载 assets/levels 失败");
    let lv = set.get("l0-print").expect("缺少 l0-print 关卡");
    let sb = DevSandbox::new();

    // 1) 真实编译：首条错误必须是无码错误 EUNKNOWN（kind=NoCode）
    match sb.compile(&lv.starter_code).unwrap() {
        CompileOutcome::Failed { errors } => {
            let first = errors.first().expect("编译失败但 errors 为空：解析器漏检（将触发 FALLBACK）");
            assert_eq!(first.code, "EUNKNOWN", "应解析出 EUNKNOWN: {errors:?}");
            assert_eq!(first.kind, IssueKind::NoCode, "kind 应为 NoCode: {errors:?}");
        }
        other => panic!("01-l0-print starter 应编译失败，实际 {other:?}"),
    }

    // 2) 全流程 validate：反馈非空且走「无码编译错误」分支（EUNKNOWN 或 fallback 均可，
    //    但绝不能空白）
    match validate(lv, &lv.starter_code, &ErrorMapper::default_fallback(), &sb).unwrap() {
        Validation::Fail { feedback } => {
            assert!(!feedback.is_empty(), "禁止空反馈（锁死空白反馈面板 bug）");
            assert!(!feedback[0].trim().is_empty(), "反馈首条不能是空白串");
            assert!(
                feedback[0].contains("编译错误") || feedback[0].contains("无法解析"),
                "应展示无码错误文案或硬兜底文案: {feedback:?}"
            );
        }
        Validation::Pass => panic!("broken starter 不应 Pass"),
    }
}
