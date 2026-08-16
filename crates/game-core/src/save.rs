use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::GameError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LevelState {
    #[default]
    Locked,
    Unlocked,
    Passed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelProgress {
    #[serde(default)]
    pub state: LevelState,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub completed_at: Option<String>,
}

impl Default for LevelProgress {
    fn default() -> Self {
        Self { state: LevelState::Locked, attempts: 0, completed_at: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SaveData {
    #[serde(default)]
    pub xp: u32,
    #[serde(default)]
    pub combo: u32,
    #[serde(default)]
    pub max_combo: u32,
    #[serde(default)]
    pub level_states: HashMap<String, LevelProgress>,
    #[serde(default)]
    pub total_errors: u32,
}

pub fn load(path: &Path) -> Result<SaveData, GameError> {
    if !path.exists() {
        return Ok(SaveData::default());
    }
    let content = std::fs::read_to_string(path)?;
    toml::from_str(&content).map_err(|e| GameError::CorruptSave(e.to_string()))
}

pub fn save(data: &SaveData, path: &Path) -> Result<(), GameError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    let content =
        toml::to_string_pretty(data).map_err(|e| GameError::CorruptSave(e.to_string()))?;
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rlg-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn default_when_missing() {
        let p = temp_path("missing.toml");
        let _ = std::fs::remove_file(&p);
        let data = load(&p).unwrap();
        assert_eq!(data.xp, 0);
        assert!(data.level_states.is_empty());
    }

    #[test]
    fn roundtrip() {
        let p = temp_path("roundtrip.toml");
        let mut data = SaveData::default();
        data.xp = 120;
        data.combo = 3;
        data.level_states.insert(
            "l0-hello".into(),
            LevelProgress { state: LevelState::Passed, attempts: 2, completed_at: Some("1720000000".into()) },
        );
        save(&data, &p).unwrap();
        let loaded = load(&p).unwrap();
        assert_eq!(loaded.xp, 120);
        assert_eq!(loaded.combo, 3);
        let prog = loaded.level_states.get("l0-hello").unwrap();
        assert_eq!(prog.state, LevelState::Passed);
        assert_eq!(prog.attempts, 2);
        // 原子写：临时文件应不存在
        assert!(!p.with_extension("toml.tmp").exists());
    }

    #[test]
    fn corrupt_file_returns_corrupt_save() {
        let p = temp_path("corrupt.toml");
        std::fs::write(&p, "not a toml [[[").unwrap();
        assert!(matches!(load(&p), Err(GameError::CorruptSave(_))));
    }

    #[test]
    fn state_serde_lowercase() {
        // toml 0.8 顶层不接受裸字符串值，用包装结构验证 lowercase 反序列化
        #[derive(serde::Deserialize)]
        struct Wrapper {
            state: LevelState,
        }
        let w: Wrapper = toml::from_str("state = \"passed\"").unwrap();
        assert_eq!(w.state, LevelState::Passed);
    }
}
