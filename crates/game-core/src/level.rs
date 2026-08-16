use serde::Deserialize;
use std::path::Path;
use std::collections::HashSet;

use crate::error::GameError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LevelTier {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl LevelTier {
    pub fn order(&self) -> u8 {
        match self {
            LevelTier::L0 => 0,
            LevelTier::L1 => 1,
            LevelTier::L2 => 2,
            LevelTier::L3 => 3,
            LevelTier::L4 => 4,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Level {
    pub id: String,
    pub title: String,
    pub tier: LevelTier,
    pub description: String,
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub hints: Vec<String>,
    #[serde(default)]
    pub starter_code: String,
    #[serde(default)]
    pub expect_output: String,
    #[serde(default)]
    pub allow_compile_fail: bool,
    #[serde(default)]
    pub expect_error_code: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct LevelFile {
    level: Vec<Level>,
}

/// 解析一份 TOML 内容（可能含多个 [[level]]），供 load 与测试复用
pub fn parse_levels(content: &str) -> Result<Vec<Level>, GameError> {
    let file: LevelFile = toml::from_str(content)
        .map_err(|e| GameError::TomlParse("关卡内容".into(), e.to_string()))?;
    Ok(file.level)
}

#[derive(Debug, Clone, Default)]
pub struct LevelSet {
    pub levels: Vec<Level>,
}

impl LevelSet {
    /// 从目录加载全部关卡 TOML，按文件名排序形成线性关卡顺序
    pub fn load(dir: &Path) -> Result<Self, GameError> {
        if !dir.exists() {
            return Err(GameError::LevelDirNotFound(dir.display().to_string()));
        }
        let mut files: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |x| x == "toml"))
            .collect();
        files.sort();
        let mut levels = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for f in files {
            let content = std::fs::read_to_string(&f)?;
            let parsed = parse_levels(&content)
                .map_err(|e| match e {
                    GameError::TomlParse(_, msg) => {
                        GameError::TomlParse(f.display().to_string(), msg)
                    }
                    other => other,
                })?;
            for lvl in parsed {
                if !seen.insert(lvl.id.clone()) {
                    return Err(GameError::DuplicateLevelId(lvl.id.clone()));
                }
                levels.push(lvl);
            }
        }
        if levels.is_empty() {
            return Err(GameError::LevelDirNotFound(dir.display().to_string()));
        }
        Ok(Self { levels })
    }

    pub fn get(&self, id: &str) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const LEVEL_TOML: &str = r#"
[[level]]
id = "l0-hello"
title = "你好，变量"
tier = "l0"
description = "修复代码，使其编译并输出预期结果"
hint = "变量需要 let 声明"
starter_code = "fn main() { x = 5; println!(\"x has the value {}\", x); }"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l1-move"
title = "所有权转移"
tier = "l1"
description = "理解 move 语义"
starter_code = "fn main() { let s = String::from(\"hi\"); take(s); println!(\"{}\", s); } fn take(x: String) {}"
expect_output = ""
allow_compile_fail = true
expect_error_code = "E0382"
source = "rustlings"
"#;

    #[test]
    fn parse_levels_ok() {
        let levels = parse_levels(LEVEL_TOML).unwrap();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].id, "l0-hello");
        assert_eq!(levels[0].tier, LevelTier::L0);
        assert_eq!(levels[0].expect_output, "x has the value 5");
        assert!(!levels[0].allow_compile_fail);
        assert_eq!(levels[1].tier, LevelTier::L1);
        assert!(levels[1].allow_compile_fail);
        assert_eq!(levels[1].expect_error_code, "E0382");
    }

    #[test]
    fn parse_levels_invalid_tier_fails() {
        let bad = LEVEL_TOML.replace("tier = \"l0\"", "tier = \"l9\"");
        assert!(parse_levels(&bad).is_err());
    }

    #[test]
    fn parse_levels_malformed_fails() {
        assert!(parse_levels("not toml at all [[[[").is_err());
    }

    #[test]
    fn tier_order() {
        assert!(LevelTier::L0.order() < LevelTier::L1.order());
        assert!(LevelTier::L4.order() == 4);
    }

    #[test]
    fn level_set_load_sorted_and_duplicate_detected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("02-second.toml"), LEVEL_TOML).unwrap();
        std::fs::write(
            dir.path().join("01-first.toml"),
            "[[level]]\nid = \"first\"\ntitle = \"t\"\ntier = \"l2\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nsource = \"x\"\n",
        )
        .unwrap();
        let set = LevelSet::load(dir.path()).unwrap();
        assert_eq!(set.len(), 3);
        assert_eq!(set.levels[0].id, "first"); // 按文件名排序
        assert!(set.get("first").is_some());
        assert!(set.get("nope").is_none());
    }

    #[test]
    fn level_set_duplicate_id_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.toml"), LEVEL_TOML).unwrap();
        std::fs::write(dir.path().join("b.toml"), LEVEL_TOML).unwrap();
        assert!(matches!(LevelSet::load(dir.path()), Err(GameError::DuplicateLevelId(_))));
    }

    #[test]
    fn level_set_missing_dir_fails() {
        assert!(matches!(
            LevelSet::load(&PathBuf::from("/nonexistent/rlg-levels")),
            Err(GameError::LevelDirNotFound(_))
        ));
    }
}
