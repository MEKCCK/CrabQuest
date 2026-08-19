//! 共享测试工具（Tier 2 / Tier 3 共用）。
//!
//! - 极简 flat TOML 读取：expected.toml 只有 `key = value` 标量行（无嵌套/数组），
//!   手写解析避免给 crab-quest-core 加 dev-dependency。
//! - panic 分类 id 双向映射（与 src/validate/error_parser.rs 的 8 类对应）。
//! - 15 关 starter 期望画像表 + 通用 check_level（Tier 3 与 bwrap 集成测试共用，
//!   单一事实源防漂移；沙盒实现无关）。
//! - 仓库路径解析：测试二进制 CWD 由 cargo 决定，统一用 CARGO_MANIFEST_DIR 定位。
#![allow(dead_code)] // 本模块被每个测试二进制独立编译，各自只用到子集

use crab_quest_core::level::Level;
use crab_quest_core::sandbox::{CompileOutcome, RunOutcome, Sandbox};
use crab_quest_core::validate::error_parser::{sanitize_panic, IssueKind, PanicClass};
use std::collections::HashMap;
use std::path::PathBuf;

/// crates/crab-quest-core/ 根（cargo 注入的 MANIFEST_DIR）
pub fn crate_root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 由 cargo 注入"))
}

/// tests/fixtures/ 根
pub fn fixtures_root() -> PathBuf {
    crate_root().join("tests/fixtures")
}

/// 仓库根 assets/levels/（Tier 3 关卡素材，动态读取）
pub fn assets_levels_dir() -> PathBuf {
    crate_root().join("../../assets/levels")
}

/// 读取一份 flat TOML 为 key→value（值去引号；注释/空行忽略）。
/// 仅支持 expected.toml 的标量 schema，不做通用 TOML 解析。
pub fn parse_flat_toml(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = t.split_once('=') {
            map.insert(
                k.trim().to_string(),
                v.trim().trim_matches('"').trim().to_string(),
            );
        }
    }
    map
}

/// panic 分类 id → PanicClass（与 error_parser.rs 的 8 类关键词表一致）
pub fn class_from_id(id: &str) -> PanicClass {
    match id {
        "array_index_oob" => PanicClass::ArrayIndexOob,
        "unwrap_option_none" => PanicClass::UnwrapOptionNone,
        "unwrap_result_err" => PanicClass::UnwrapResultErr,
        "parse_failure" => PanicClass::ParseFailure,
        "integer_overflow" => PanicClass::IntegerOverflow,
        "divide_by_zero" => PanicClass::DivideByZero,
        "explicit_panic" => PanicClass::ExplicitPanic,
        "alloc_failure" => PanicClass::AllocFailure,
        "generic" => PanicClass::Generic,
        other => panic!("未知 classification id: {other}（见 L3-B2 §1.4 分类表）"),
    }
}

/// PanicClass → 分类 id
pub fn class_id(c: PanicClass) -> &'static str {
    match c {
        PanicClass::ArrayIndexOob => "array_index_oob",
        PanicClass::UnwrapOptionNone => "unwrap_option_none",
        PanicClass::UnwrapResultErr => "unwrap_result_err",
        PanicClass::ParseFailure => "parse_failure",
        PanicClass::IntegerOverflow => "integer_overflow",
        PanicClass::DivideByZero => "divide_by_zero",
        PanicClass::ExplicitPanic => "explicit_panic",
        PanicClass::AllocFailure => "alloc_failure",
        PanicClass::Generic => "generic",
    }
}

/// 每关 starter 的期望画像。
/// 期望码 = 该关 starter 编译失败时的**首条**错误码（rustc 输出顺序）；
/// EUNKNOWN = 无 E 码编译错误（走兜底，L3-B2 §1.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarterExpect {
    /// 预期编译失败，首条错误码 = code（"EUNKNOWN" = 无码错误）
    CompileFail { code: &'static str },
    /// 预期编译成功 + 正常运行（输出与 expect_output 不符由关卡设计保证）
    Compiles,
    /// 预期编译成功 + 运行 panic，分类 = class（L3-B2 §1.4 分类表 id）
    CompilesThenPanic { class: &'static str },
}

/// 期望表：按关卡 id 推导自 L3-B2 §3 的 fixture 实测（15 关 × rustc 1.97）。
/// 新增关卡必须在此补充条目，否则 `starter_expect` panic（防静默漏测）。
pub fn starter_expect(id: &str) -> StarterExpect {
    match id {
        // 00-l0-hello：x 未声明 → E0425（F1）
        "l0-hello-world" => StarterExpect::Compiles,
        "l0-hello" => StarterExpect::CompileFail { code: "E0425" },
        // 01-l0-print：println! 格式参数数量错误，无 E 码 → EUNKNOWN（F12）
        "l0-print" => StarterExpect::CompileFail { code: "EUNKNOWN" },
        // 02-l0-function：call_me 未定义 → E0425
        "l0-function" => StarterExpect::CompileFail { code: "E0425" },
        // 03-l0-loop：编译通过，输出 10 ≠ 期望 15
        "l0-loop" => StarterExpect::Compiles,
        "l0-integers" => StarterExpect::CompileFail { code: "E0308" },
        "l0-variables2" => StarterExpect::CompileFail { code: "E0283" },
        "l0-if" => StarterExpect::CompileFail { code: "E0308" },
        "l0-primitives" => StarterExpect::CompileFail { code: "E0425" },
        "l0-functions2" => StarterExpect::CompileFail { code: "E0061" },
        "l0-boss" => StarterExpect::CompileFail { code: "E0425" },
        // 04-l1-move：fill_vec 参数非 mut → E0596（F2）
        "l1-move" => StarterExpect::CompileFail { code: "E0596" },
        // 05-l1-borrow：calculate_length 拿走所有权 → E0382（F3）
        "l1-borrow" => StarterExpect::CompileFail { code: "E0382" },
        // 06-l1-mut-borrow：多码场景，rustc 输出顺序首条 E0382（L3-B2 §1.1 实测）
        "l1-mut-borrow" => StarterExpect::CompileFail { code: "E0382" },
        // 07-l1-clone：s2 = s1 后使用 s1 → E0382
        "l1-clone" => StarterExpect::CompileFail { code: "E0382" },
        "l1-move2" => StarterExpect::CompileFail { code: "E0382" },
        "l1-move3" => StarterExpect::CompileFail { code: "E0596" },
        "l1-strings" => StarterExpect::CompileFail { code: "E0308" },
        "l1-structs" => StarterExpect::CompileFail { code: "E0063" },
        "l1-options1" => StarterExpect::CompileFail { code: "E0308" },
        "l1-enums" => StarterExpect::CompileFail { code: "E0599" },
        "l1-ownership-ticket" => StarterExpect::CompileFail { code: "E0382" },
        "l1-boss" => StarterExpect::CompileFail { code: "E0596" },
        // 08-l2-vec：编译通过，运行 v[3] 越界 panic → array_index_oob（F14）
        "l2-vec" => StarterExpect::CompilesThenPanic {
            class: "array_index_oob",
        },
        // 09-l2-option：编译通过，输出 none ≠ 期望 3
        "l2-option" => StarterExpect::Compiles,
        // 10-l2-result：main 返回 () 时 `?` 无法展开 → E0277
        // （P3-17 重构 starter 为 `result?` 后仅 1 个预期码；内嵌 FAIL_MSG
        // 中文字面量，P1-06 修订后仍编译失败，仅报 E0277）
        "l2-result" => StarterExpect::CompilesThenPanic {
            class: "unwrap_result_err",
        },
        "l2-errors3" => StarterExpect::CompileFail { code: "E0277" },
        "l2-errors2" => StarterExpect::CompileFail { code: "E0369" },
        "l2-saturating" => StarterExpect::CompilesThenPanic {
            class: "integer_overflow",
        },
        "l2-errors4" => StarterExpect::Compiles,
        "l2-vecs2" => StarterExpect::Compiles,
        "l2-hashmap" => StarterExpect::CompileFail { code: "E0425" },
        "l2-strings2" => StarterExpect::CompileFail { code: "E0308" },
        "l2-match" => StarterExpect::CompileFail { code: "E0004" },
        "l2-boss" => StarterExpect::CompileFail { code: "E0308" },
        // 11-l3-lifetime：longest 缺生命周期标注 → E0106（F4）
        "l3-lifetime" => StarterExpect::CompileFail { code: "E0106" },
        // 12-l3-trait：Rectangle 未实现 area → E0599（F5）
        "l3-trait" => StarterExpect::CompileFail { code: "E0599" },
        "l3-lifetime3" => StarterExpect::CompileFail { code: "E0106" },
        "l3-lifetime1" => StarterExpect::CompileFail { code: "E0106" },
        "l3-generics" => StarterExpect::CompileFail { code: "E0282" },
        "l3-traits1" => StarterExpect::CompileFail { code: "E0046" },
        "l3-iterators" => StarterExpect::CompileFail { code: "E0308" },
        "l3-iterators2" => StarterExpect::CompileFail { code: "E0308" },
        "l3-conversions" => StarterExpect::CompileFail { code: "E0277" },
        "l3-enums3" => StarterExpect::Compiles,
        "l3-boss" => StarterExpect::CompileFail { code: "E0106" },
        // 13-l4-drop-order：编译通过，输出顺序不符（无编译/运行错误）
        "l4-drop-order" => StarterExpect::Compiles,
        // 14-l4-lifetime-trap：借用悬垂 → E0597（F6）
        "l4-lifetime-trap" => StarterExpect::CompileFail { code: "E0597" },
        "l4-lazy-map" => StarterExpect::Compiles,
        "l4-fnptr" => StarterExpect::Compiles,
        "l4-mutable-zst" => StarterExpect::Compiles,
        "l4-drop-underscore" => StarterExpect::Compiles,
        "l4-lifetime-ext" => StarterExpect::Compiles,
        "l4-fnmut-copy" => StarterExpect::CompileFail { code: "E0382" },
        "l4-boss" => StarterExpect::CompileFail { code: "E0596" },
        "l2-panics" => StarterExpect::Compiles,
        "l1-tuple-partial-move" => StarterExpect::CompileFail { code: "E0382" },
        "l1-str-param" => StarterExpect::CompileFail { code: "E0308" },
        "l2-closure-count" => StarterExpect::CompileFail { code: "E0596" },
        "l3-generic-largest" => StarterExpect::CompileFail { code: "E0369" },
        "l3-trait-object" => StarterExpect::CompileFail { code: "E0782" },
        "l4-closure-move" => StarterExpect::CompileFail { code: "E0382" },
        other => panic!("期望表未覆盖关卡 {other}：新增关卡必须补充期望画像"),
    }
}

/// 单关断言（沙盒实现无关：Tier 3 的 DevSandbox 与 P4-24 的 BwrapSandbox
/// 共用同一期望表，验证 bwrap 隔离环境下编译/运行结果与 DevSandbox 一致）。
/// 编译失败时 errors 必须非空（errors 为空会触发 validate 的 FALLBACK 硬兜底
/// 文案，仍非空反馈，但说明解析器漏检，应视为失败——L3-B2 §1.5 硬兜底是
/// 最后防线，不是常态）。
pub fn check_level<S: Sandbox>(lv: &Level, sb: &S) -> Result<(), String> {
    let exp = starter_expect(&lv.id);
    let compile = sb
        .compile(&lv.starter_code)
        .map_err(|e| format!("编译错误: {e}"))?;
    match (exp, compile) {
        (StarterExpect::CompileFail { code }, CompileOutcome::Failed { errors }) => {
            let first = errors
                .first()
                .ok_or("编译失败但 errors 为空（解析器漏检）")?;
            if first.code != code {
                return Err(format!(
                    "首条码 {} != 期望 {code}（errors: {errors:?}）",
                    first.code
                ));
            }
            if code == "EUNKNOWN" && first.kind != IssueKind::NoCode {
                return Err(format!(
                    "EUNKNOWN 的 kind 应为 NoCode，实际 {:?}",
                    first.kind
                ));
            }
            Ok(())
        }
        (StarterExpect::CompileFail { code }, CompileOutcome::Success { .. }) => {
            Err(format!("预期编译失败（{code}），实际编译成功"))
        }
        (StarterExpect::Compiles, CompileOutcome::Success { binary }) => match sb.run(&binary) {
            Ok(RunOutcome::Ok { .. }) => Ok(()),
            Ok(RunOutcome::Panic { message }) => Err(format!(
                "预期正常运行，实际 panic: {}",
                sanitize_panic(&message).message
            )),
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
                        Err(format!(
                            "panic 分类 {} != 期望 {class}（净化后: {sp:?}）",
                            class_id(sp.class)
                        ))
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
