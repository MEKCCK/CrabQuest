use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::error::GameError;

/// 严重度：P0 主线必踩（UI 常驻）/ P1 扩展路线（默认）/ P2 低优先
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Severity {
    P0,
    P1,
    P2,
}

impl Default for Severity {
    fn default() -> Self {
        Severity::P1
    }
}

/// 知识概念标签（12 类，v3 §5.2）：name|ownership|borrow|lifetime|type|trait|macro|module|match|variable|function|panic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Concept {
    Name,
    Ownership,
    Borrow,
    Lifetime,
    Type,
    Trait,
    Macro,
    Module,
    Match,
    Variable,
    Function,
    Panic,
}

/// 错误码卡片信息（errors.toml schema v2，v3 §5.2）。
///
/// 兼容规则：新增字段全部 `#[serde(default)]`，旧格式（仅 zh/link）文件可直接加载；
/// 缺省值：severity=P1、link_zh/fix/example/concept=None（部分填充的卡片仍可渲染）。
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorInfo {
    /// 中文解释（必填，≤60 字）；离线可用
    pub zh: String,
    /// 官方错误码页（必填）：https://doc.rust-lang.org/error_codes/E0xxx.html
    pub link: String,
    /// 中文概念页（可选，rustwiki.org 白名单；UI 中文链接优先）
    #[serde(default)]
    pub link_zh: Option<String>,
    /// 严重度（可选，默认 P1）
    #[serde(default)]
    pub severity: Severity,
    /// 知识概念标签（可选）
    #[serde(default)]
    pub concept: Option<Concept>,
    /// 一句话修复方向（可选，≤40 字）
    #[serde(default)]
    pub fix: Option<String>,
    /// 最小示例代码（可选，≤8 行；TOML 三引号字面量，最小复现 + 一处修复标注）
    #[serde(default)]
    pub example: Option<String>,
}

/// `[fallback]` 段：无 E 码错误（EUNKNOWN）与未收录码的兜底文案。
/// 单段不分码；旧文件缺省不报错（`load` 只在出现该段时填充）。
#[derive(Debug, Clone, Deserialize)]
pub struct FallbackInfo {
    pub zh: String,
    pub link: String,
}

/// 错误码映射：活跃条目表 + 兜底段 + 死码登记段（E0412/E0504 从活跃表移除，登记于此）。
#[derive(Debug, Clone, Default)]
pub struct ErrorMapper {
    map: HashMap<String, ErrorInfo>,
    fallback: Option<FallbackInfo>,
    deprecated: HashMap<String, String>,
}

impl ErrorMapper {
    /// 解析 errors.toml。
    ///
    /// 顶层结构：`[CODE]` 表 → 活跃条目；`[fallback]` → 兜底段；`[deprecated]` → 死码登记。
    /// 用 `toml::Value` 逐键分发，避免 struct+flatten 静默吞掉未知表（旧文件只有 [CODE] 时同样可解析）。
    pub fn load(path: &Path) -> Result<Self, GameError> {
        let content = std::fs::read_to_string(path)?;
        let value: toml::Value = toml::from_str(&content)
            .map_err(|e| GameError::TomlParse(path.display().to_string(), e.to_string()))?;
        let table = value.as_table().ok_or_else(|| {
            GameError::TomlParse(path.display().to_string(), "顶层必须是表（[CODE]/[fallback]/[deprecated]）".into())
        })?;

        let mut map = HashMap::new();
        let mut fallback = None;
        let mut deprecated = HashMap::new();

        for (key, val) in table {
            match key.as_str() {
                "fallback" => {
                    fallback = Some(
                        FallbackInfo::deserialize(val.clone()).map_err(|e| {
                            GameError::TomlParse(path.display().to_string(), format!("[fallback]: {e}"))
                        })?,
                    );
                }
                "deprecated" => {
                    deprecated = HashMap::<String, String>::deserialize(val.clone()).map_err(|e| {
                        GameError::TomlParse(path.display().to_string(), format!("[deprecated]: {e}"))
                    })?;
                }
                _ => {
                    let info = ErrorInfo::deserialize(val.clone()).map_err(|e| {
                        GameError::TomlParse(path.display().to_string(), format!("[{key}]: {e}"))
                    })?;
                    map.insert(key.clone(), info);
                }
            }
        }
        Ok(Self { map, fallback, deprecated })
    }

    pub fn lookup(&self, code: &str) -> Option<&ErrorInfo> {
        self.map.get(code)
    }

    /// 遍历全部活跃条目（测试 / 工具用）
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ErrorInfo)> {
        self.map.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// `[fallback]` 兜底文案（无 E 码 / 未收录码）
    pub fn fallback(&self) -> Option<&FallbackInfo> {
        self.fallback.as_ref()
    }

    /// 死码登记原因；`Some` = 该码已不再由 rustc 发射（不得用于关卡设计）
    pub fn deprecated_reason(&self, code: &str) -> Option<&str> {
        self.deprecated.get(code).map(String::as_str)
    }

    pub fn is_deprecated(&self, code: &str) -> bool {
        self.deprecated.contains_key(code)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// 最小兜底表：assets 缺失或映射不全时保证常见错误仍有中文提示。
    /// 覆盖全部 P0 码（E0425/E0596/E0382/E0106/E0599/E0597 + E0282/E0384/E0594/E0621/E0601）+ EUNKNOWN
    /// （v3 §5.2 兼容规则），并内置 [fallback] 兜底段。
    pub fn default_fallback() -> Self {
        let entry = |zh: &str, code: &str, severity: Severity, concept: Concept| ErrorInfo {
            zh: zh.into(),
            link: format!("https://doc.rust-lang.org/error_codes/{code}.html"),
            link_zh: None,
            severity,
            concept: Some(concept),
            fix: None,
            example: None,
        };
        let map = HashMap::from([
            (
                "E0308".to_string(),
                entry("类型不匹配：表达式的实际类型与期望类型不一致", "E0308", Severity::P1, Concept::Type),
            ),
            (
                "E0382".to_string(),
                entry("使用了已移动的值：所有权已转移，无法再使用原变量", "E0382", Severity::P0, Concept::Ownership),
            ),
            (
                "E0502".to_string(),
                entry("同时存在不可变借用与可变借用，Rust 不允许", "E0502", Severity::P1, Concept::Borrow),
            ),
            (
                "E0596".to_string(),
                entry("无法以可变方式借用：变量需要声明为 mut", "E0596", Severity::P0, Concept::Borrow),
            ),
            (
                "E0106".to_string(),
                entry("缺少生命周期标注：需要为引用显式标注生命周期", "E0106", Severity::P0, Concept::Lifetime),
            ),
            (
                "E0425".to_string(),
                entry("找不到名字：变量、函数或类型未定义，或不在当前作用域内", "E0425", Severity::P0, Concept::Name),
            ),
            (
                "E0599".to_string(),
                entry("没有该方法：类型上不存在这个方法的实现（或未实现对应 trait）", "E0599", Severity::P0, Concept::Trait),
            ),
            (
                "E0597".to_string(),
                entry("借用活得不够久：被引用的值比引用先被释放（生命周期问题）", "E0597", Severity::P0, Concept::Lifetime),
            ),
            (
                "E0282".to_string(),
                entry("类型推断失败：编译器无法从上下文猜出变量类型", "E0282", Severity::P0, Concept::Type),
            ),
            (
                "E0384".to_string(),
                entry("不能给不可变变量重新赋值：let 声明的变量默认不可变", "E0384", Severity::P0, Concept::Variable),
            ),
            (
                "E0594".to_string(),
                entry("不能修改只读借用背后的值：& 引用是只读的", "E0594", Severity::P0, Concept::Borrow),
            ),
            (
                "E0621".to_string(),
                entry("函数签名需显式生命周期：返回值可能来自未标注生命周期的参数", "E0621", Severity::P0, Concept::Lifetime),
            ),
            (
                "E0601".to_string(),
                entry("二进制程序缺少入口函数 main：程序不知道从哪里开始执行", "E0601", Severity::P0, Concept::Function),
            ),
            (
                "EUNKNOWN".to_string(),
                ErrorInfo {
                    zh: "这是一个编译错误（rustc 未提供错误码）。请对照报错原文，检查最近的改动（如 println! 格式参数、语法拼写）".into(),
                    link: "https://doc.rust-lang.org/error_codes/index.html".into(),
                    link_zh: None,
                    severity: Severity::P1,
                    concept: None,
                    fix: None,
                    example: None,
                },
            ),
        ]);
        Self {
            map,
            fallback: Some(FallbackInfo {
                zh: "这是一个编译错误（rustc 未提供错误码）。请对照报错原文，检查最近的改动（如 println! 格式参数、语法拼写）".into(),
                link: "https://doc.rust-lang.org/error_codes/index.html".into(),
            }),
            deprecated: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 旧格式兼容：仅 zh/link，无任何 v2 字段/段落
    const OLD_FORMAT: &str = r#"
[E0308]
zh = "类型不匹配：表达式的实际类型与期望类型不一致"
link = "https://doc.rust-lang.org/error_codes/E0308.html"

[E0502]
zh = "同时存在不可变借用与可变借用，Rust 不允许"
link = "https://doc.rust-lang.org/error_codes/E0502.html"
"#;

    /// v2 全字段：新字段 + [fallback] + [deprecated]（E0412 已移出活跃表）
    const V2_FORMAT: &str = r#"
[E0382]
zh = "使用了已移动的值：所有权已转移给别的变量/函数，原变量不能再使用"
link = "https://doc.rust-lang.org/error_codes/E0382.html"
link_zh = "https://rustwiki.org/zh-CN/book/ch04-01-what-is-ownership.html"
severity = "P0"
concept = "ownership"
fix = "改用引用 &s（只借用）或 s.clone()，或调整使用顺序"
example = '''
let s1 = String::from("hi");
let s2 = &s1;            // 借用：s1 仍可用
println!("{} {}", s1, s2);
'''

[E0282]
zh = "类型推断失败：编译器无法从上下文猜出变量类型"
link = "https://doc.rust-lang.org/error_codes/E0282.html"
concept = "type"

[fallback]
zh = "编译出错但没有标准错误码。请对照面板中的原文与行号，逐行检查最近的改动。"
link = "https://doc.rust-lang.org/error_codes/index.html"

[deprecated]
E0412 = "该错误码已不再由 rustc 发射（找不到类型现报 E0425），请勿用于关卡设计"
"#;

    fn load_str(content: &str) -> ErrorMapper {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("errors.toml");
        std::fs::write(&p, content).unwrap();
        ErrorMapper::load(&p).unwrap()
    }

    #[test]
    fn load_and_lookup() {
        let m = load_str(OLD_FORMAT);
        assert_eq!(m.lookup("E0308").unwrap().zh, "类型不匹配：表达式的实际类型与期望类型不一致");
        assert!(m.lookup("E9999").is_none());
    }

    #[test]
    fn missing_file_is_default_empty() {
        // 调用方用 unwrap_or_default 兜底，这里验证 default 行为
        let m = ErrorMapper::default();
        assert!(m.is_empty());
    }

    #[test]
    fn fallback_has_common_codes() {
        let m = ErrorMapper::default_fallback();
        assert!(m.lookup("E0308").is_some());
        assert!(m.lookup("E0382").is_some());
        assert!(m.lookup("E0502").is_some());
        assert!(m.lookup("E0596").is_some());
        assert!(m.lookup("E0106").is_some());
    }

    #[test]
    fn fallback_has_eunknown() {
        // P1-01：无 E 码错误必须有兜底中文文案
        let m = ErrorMapper::default_fallback();
        let e = m.lookup("EUNKNOWN").expect("EUNKNOWN 必须存在于兜底表");
        assert!(e.zh.contains("编译错误"), "zh: {}", e.zh);
        assert!(e.link.starts_with("https://"));
    }

    /// 验收 a：旧格式（仅 zh/link）不加字段也能被 v2 解析器正常读取，且缺省值正确
    #[test]
    fn old_format_parses_with_defaults() {
        let m = load_str(OLD_FORMAT);
        let info = m.lookup("E0308").expect("旧格式条目必须可解析");
        assert_eq!(info.severity, Severity::P1, "缺省 severity 应为 P1");
        assert!(info.link_zh.is_none());
        assert!(info.concept.is_none());
        assert!(info.fix.is_none());
        assert!(info.example.is_none());
        assert!(m.fallback().is_none(), "旧文件无 [fallback] 段");
        assert!(!m.is_deprecated("E0308"));
        assert_eq!(m.deprecated_reason("E0308"), None);
    }

    /// 验收 b：v2 字段全部可解析，部分缺省字段按缺省值回退
    #[test]
    fn v2_fields_parse_with_partial_defaults() {
        let m = load_str(V2_FORMAT);
        let full = m.lookup("E0382").expect("E0382 条目");
        assert_eq!(full.severity, Severity::P0);
        assert_eq!(full.concept, Some(Concept::Ownership));
        assert_eq!(
            full.link_zh.as_deref(),
            Some("https://rustwiki.org/zh-CN/book/ch04-01-what-is-ownership.html")
        );
        assert!(full.fix.as_deref().unwrap().contains("clone"));
        assert!(full.example.as_deref().unwrap().contains("let s2 = &s1;"));
        // 未写 severity 的条目回退 P1
        let partial = m.lookup("E0282").expect("E0282 条目");
        assert_eq!(partial.severity, Severity::P1);
        assert_eq!(partial.concept, Some(Concept::Type));
        assert!(partial.fix.is_none());
    }

    /// 验收 c：`[fallback]` / `[deprecated]` 独立结构解析
    #[test]
    fn fallback_and_deprecated_sections_parse() {
        let m = load_str(V2_FORMAT);
        let fb = m.fallback().expect("[fallback] 段必须解析");
        assert!(fb.zh.contains("标准错误码"));
        assert!(fb.link.starts_with("https://"));
        let reason = m.deprecated_reason("E0412").expect("E0412 必须登记在 [deprecated]");
        assert!(reason.contains("E0425"), "登记原因应注明替代码: {reason}");
        assert!(m.is_deprecated("E0412"));
    }

    /// 验收 d：死码（E0412/E0504）不在活跃查找中
    #[test]
    fn deprecated_codes_absent_from_active_lookups() {
        let m = load_str(V2_FORMAT);
        assert!(m.lookup("E0412").is_none(), "死码不得出现在活跃映射");
        assert!(m.lookup("E0504").is_none());
        assert!(m.is_deprecated("E0412"));
    }

    /// 验收 e：default_fallback 覆盖全部 P0 码 + EUNKNOWN
    #[test]
    fn default_fallback_covers_all_p0_and_eunknown() {
        let m = ErrorMapper::default_fallback();
        // A 组（现有关卡触发 6 码）+ B 组（新增 P0 5 码），共 11 个 P0
        for code in [
            "E0425", "E0596", "E0382", "E0106", "E0599", "E0597",
            "E0282", "E0384", "E0594", "E0621", "E0601",
        ] {
            let info = m.lookup(code).unwrap_or_else(|| panic!("兜底表缺少 P0 码 {code}"));
            assert_eq!(info.severity, Severity::P0, "{code} 应为 P0");
            assert!(!info.zh.is_empty());
            assert!(info.link.starts_with("https://"));
        }
        assert!(m.lookup("EUNKNOWN").is_some(), "兜底表必须含 EUNKNOWN");
        assert!(m.fallback().is_some(), "default_fallback 也应内置 [fallback] 兜底段");
    }

    /// 内容校验工具：新条目 zh ≤60 / fix ≤40 / example ≤8 行（真实 assets 由 crab-quest-data 集成测试断言）
    #[test]
    fn word_count_limits_hold_in_v2_fixture() {
        let m = load_str(V2_FORMAT);
        for (code, info) in m.iter() {
            assert!(info.zh.chars().count() <= 60, "{code} zh 超 60 字: {}", info.zh);
            if let Some(fix) = &info.fix {
                assert!(fix.chars().count() <= 40, "{code} fix 超 40 字: {fix}");
            }
            if let Some(example) = &info.example {
                assert!(example.lines().count() <= 8, "{code} example 超 8 行");
            }
        }
    }
}
