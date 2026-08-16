use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::error::GameError;

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorInfo {
    pub zh: String,
    pub link: String,
}

#[derive(Debug, Clone, Default)]
pub struct ErrorMapper {
    map: HashMap<String, ErrorInfo>,
}

impl ErrorMapper {
    pub fn load(path: &Path) -> Result<Self, GameError> {
        let content = std::fs::read_to_string(path)?;
        let map: HashMap<String, ErrorInfo> = toml::from_str(&content)
            .map_err(|e| GameError::TomlParse(path.display().to_string(), e.to_string()))?;
        Ok(Self { map })
    }

    pub fn lookup(&self, code: &str) -> Option<&ErrorInfo> {
        self.map.get(code)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// 最小兜底表：assets 缺失或映射不全时保证常见错误仍有中文提示
    pub fn default_fallback() -> Self {
        let map = HashMap::from([
            (
                "E0308".to_string(),
                ErrorInfo { zh: "类型不匹配：表达式的实际类型与期望类型不一致".into(), link: "https://doc.rust-lang.org/error_codes/E0308.html".into() },
            ),
            (
                "E0382".to_string(),
                ErrorInfo { zh: "使用了已移动的值：所有权已转移，无法再使用原变量".into(), link: "https://doc.rust-lang.org/error_codes/E0382.html".into() },
            ),
            (
                "E0502".to_string(),
                ErrorInfo { zh: "同时存在不可变借用与可变借用，Rust 不允许".into(), link: "https://doc.rust-lang.org/error_codes/E0502.html".into() },
            ),
            (
                "E0596".to_string(),
                ErrorInfo { zh: "无法以可变方式借用：变量需要声明为 mut".into(), link: "https://doc.rust-lang.org/error_codes/E0596.html".into() },
            ),
            (
                "E0106".to_string(),
                ErrorInfo { zh: "缺少生命周期标注：需要为引用显式标注生命周期".into(), link: "https://doc.rust-lang.org/error_codes/E0106.html".into() },
            ),
        ]);
        Self { map }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
[E0308]
zh = "类型不匹配：表达式的实际类型与期望类型不一致"
link = "https://doc.rust-lang.org/error_codes/E0308.html"

[E0502]
zh = "同时存在不可变借用与可变借用，Rust 不允许"
link = "https://doc.rust-lang.org/error_codes/E0502.html"
"#;

    #[test]
    fn load_and_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("errors.toml");
        std::fs::write(&p, FIXTURE).unwrap();
        let m = ErrorMapper::load(&p).unwrap();
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
}
