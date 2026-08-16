//! 共享测试工具（Tier 2 / Tier 3 共用）。
//!
//! - 极简 flat TOML 读取：expected.toml 只有 `key = value` 标量行（无嵌套/数组），
//!   手写解析避免给 game-core 加 dev-dependency。
//! - panic 分类 id 双向映射（与 src/validate/error_parser.rs 的 8 类对应）。
//! - 仓库路径解析：测试二进制 CWD 由 cargo 决定，统一用 CARGO_MANIFEST_DIR 定位。
#![allow(dead_code)] // 本模块被每个测试二进制独立编译，各自只用到子集

use game_core::validate::error_parser::PanicClass;
use std::collections::HashMap;
use std::path::PathBuf;

/// crates/game-core/ 根（cargo 注入的 MANIFEST_DIR）
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
            map.insert(k.trim().to_string(), v.trim().trim_matches('"').trim().to_string());
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
