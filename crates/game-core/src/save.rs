use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::GameError;

/// 当前支持的存档版本。升级 schema 时 +1 并新增迁移函数（迁移链连续）。
pub const CURRENT_SAVE_VERSION: u32 = 1;

/// 无 version 字段的旧存档（当前线上形状）读出版本 0，进入迁移链。
fn default_version() -> u32 {
    0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LevelState {
    #[default]
    Locked,
    Unlocked,
    Passed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelProgress {
    #[serde(default)]
    pub state: LevelState,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub completed_at: Option<String>,
    /// 单次通关最快用时（毫秒），engine.submit 通过分支记录
    #[serde(default)]
    pub best_time_ms: Option<u64>,
    /// 看过的 hint 序号（0-based）；is_empty() = 未看过 hint
    #[serde(default)]
    pub hints_used: Vec<u32>,
}

impl Default for LevelProgress {
    fn default() -> Self {
        Self {
            state: LevelState::Locked,
            attempts: 0,
            completed_at: None,
            best_time_ms: None,
            hints_used: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BossProgress {
    #[serde(default)]
    pub defeated: bool,
    #[serde(default)]
    pub best_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveData {
    /// 存档版本：旧存档（无 version 字段）由 default_version() 读出 0
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub player_name: String,
    #[serde(default)]
    pub xp: u32,
    #[serde(default)]
    pub combo: u32,
    #[serde(default)]
    pub max_combo: u32,
    #[serde(default)]
    pub total_errors: u32,
    /// 起始心数 3（serde default 给 0，须由迁移链显式补 3，不得用 default 兜）
    #[serde(default)]
    pub hearts: u32,
    #[serde(default)]
    pub streak_days: u32,
    /// 上次活跃日 "yyyy-mm-dd"
    #[serde(default)]
    pub last_played_date: Option<String>,
    /// XP-once 去重集合，格式 "{level_id}:pass"
    #[serde(default)]
    pub completed_steps: HashSet<String>,
    #[serde(default)]
    pub achievements: HashSet<String>,
    #[serde(default)]
    pub practice_unlock_all: bool,
    #[serde(default)]
    pub victory_celebrated: bool,
    #[serde(default)]
    pub level_states: HashMap<String, LevelProgress>,
    #[serde(default)]
    pub boss_states: HashMap<String, BossProgress>,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: CURRENT_SAVE_VERSION,
            player_name: String::new(),
            xp: 0,
            combo: 0,
            max_combo: 0,
            total_errors: 0,
            hearts: 3,
            streak_days: 0,
            last_played_date: None,
            completed_steps: HashSet::new(),
            achievements: HashSet::new(),
            practice_unlock_all: false,
            victory_celebrated: false,
            level_states: HashMap::new(),
            boss_states: HashMap::new(),
        }
    }
}

pub fn load(path: &Path) -> Result<SaveData, GameError> {
    if !path.exists() {
        return Ok(SaveData::default());
    }
    let content = std::fs::read_to_string(path)?;
    let data: SaveData =
        toml::from_str(&content).map_err(|e| GameError::CorruptSave(e.to_string()))?;
    let (data, migrated) = migrate(data)?;
    if migrated {
        // 迁移后立即回写落盘：迁移结果在进程崩溃前已持久化
        save(&data, path)?;
    }
    Ok(data)
}

/// 迁移链（纯函数）：按 version 逐级升到 CURRENT_SAVE_VERSION。
/// 返回 (迁移后数据, 是否发生迁移)；version > CURRENT → fail-fast Err，
/// 调用方不写回，原文件不被修改、不产生 .bak。
fn migrate(data: SaveData) -> Result<(SaveData, bool), GameError> {
    if data.version > CURRENT_SAVE_VERSION {
        return Err(GameError::CorruptSave(format!(
            "存档版本 {} 高于游戏版本 {}，请升级游戏",
            data.version, CURRENT_SAVE_VERSION
        )));
    }
    let mut data = data;
    let mut migrated = false;
    while data.version < CURRENT_SAVE_VERSION {
        data = match data.version {
            0 => migrate_v0_to_v1(data),
            v => unreachable!("存档版本链不连续：v{} 无迁移函数", v),
        };
        migrated = true;
    }
    Ok((data, migrated))
}

/// v0 → v1 迁移（纯函数）。v0 特征：无 version 字段（读出 0），其余字段可能齐全。
fn migrate_v0_to_v1(mut data: SaveData) -> SaveData {
    data.version = CURRENT_SAVE_VERSION;
    // serde default 给 0，须显式补起始心 3
    data.hearts = 3;
    // 无法回推历史活跃日，从零开始（不惩罚）
    data.streak_days = 0;
    data.last_played_date = None;
    // 老玩家通关记录 → XP-once 语义不丢：仅 state==Passed 的关卡进集合
    data.completed_steps = data
        .level_states
        .iter()
        .filter(|(_, p)| p.state == LevelState::Passed)
        .map(|(id, _)| format!("{id}:pass"))
        .collect();
    // player_name / achievements / practice_unlock_all / victory_celebrated /
    // boss_states / best_time_ms / hints_used 由 serde(default) 兜底，无需显式处理
    data
}

pub fn save(data: &SaveData, path: &Path) -> Result<(), GameError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    let content =
        toml::to_string_pretty(data).map_err(|e| GameError::CorruptSave(e.to_string()))?;
    std::fs::write(&tmp, content)?;
    // 写入前把旧档复制为 .bak（人工恢复通道）；备份失败不阻断原子写
    if path.exists() {
        let _ = std::fs::copy(path, path.with_extension("toml.bak"));
    }
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

    /// v0 真实形状：当前线上代码（无 version 字段）的实际输出
    const V0_REAL: &str = r#"xp = 120
combo = 3
max_combo = 5
total_errors = 7

[level_states."l0-hello"]
state = "passed"
attempts = 2
completed_at = "1720000000"

[level_states."l1-move"]
state = "unlocked"
attempts = 0
"#;

    /// v0 含三态关卡：passed / unlocked / locked
    const V0_THREE_STATES: &str = r#"xp = 10

[level_states."l0-hello"]
state = "passed"
attempts = 1

[level_states."l1-move"]
state = "unlocked"
attempts = 0

[level_states."l2-vec"]
state = "locked"
attempts = 0
"#;

    #[test]
    fn default_when_missing() {
        let p = temp_path("missing.toml");
        let _ = std::fs::remove_file(&p);
        let data = load(&p).unwrap();
        assert_eq!(data.version, CURRENT_SAVE_VERSION);
        assert_eq!(data.hearts, 3);
        assert_eq!(data.xp, 0);
        assert!(data.level_states.is_empty());
        assert!(data.completed_steps.is_empty());
    }

    #[test]
    fn migrate_v0_real_shape() {
        // ① v0 真实形状 → version==1、hearts==3、completed_steps 正确、未通关关保持 unlocked
        let p = temp_path("v0-real.toml");
        std::fs::write(&p, V0_REAL).unwrap();
        let data = load(&p).unwrap();
        assert_eq!(data.version, CURRENT_SAVE_VERSION);
        assert_eq!(data.hearts, 3);
        assert_eq!(data.streak_days, 0);
        assert_eq!(data.last_played_date, None);
        // 老字段保留
        assert_eq!(data.xp, 120);
        assert_eq!(data.combo, 3);
        assert_eq!(data.max_combo, 5);
        assert_eq!(data.total_errors, 7);
        // completed_steps 由 Passed 关推导，XP-once 语义不丢
        assert_eq!(data.completed_steps, ["l0-hello:pass"].into_iter().map(String::from).collect());
        // 未通关关保持 unlocked，不进 completed_steps
        let l1 = data.level_states.get("l1-move").unwrap();
        assert_eq!(l1.state, LevelState::Unlocked);
        assert_eq!(l1.attempts, 0);
        let l0 = data.level_states.get("l0-hello").unwrap();
        assert_eq!(l0.state, LevelState::Passed);
        assert_eq!(l0.attempts, 2);
    }

    #[test]
    fn future_version_fails_fast_and_keeps_file() {
        // ② version=99 → Err(CorruptSave) 且原文件未被修改（fail-fast 不写回、不建 .bak）
        let p = temp_path("v99.toml");
        let original = "version = 99\nxp = 5\n";
        std::fs::write(&p, original).unwrap();
        let err = load(&p).unwrap_err();
        assert!(matches!(err, GameError::CorruptSave(_)));
        assert!(err.to_string().contains("高于游戏版本"));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
        assert!(!p.with_extension("toml.bak").exists());
    }

    #[test]
    fn v1_roundtrip_all_fields() {
        // ③ v1 roundtrip 全字段相等
        let p = temp_path("v1-roundtrip.toml");
        let mut data = SaveData {
            version: CURRENT_SAVE_VERSION,
            player_name: "测试玩家".into(),
            xp: 120,
            combo: 3,
            max_combo: 5,
            total_errors: 7,
            hearts: 4,
            streak_days: 3,
            last_played_date: Some("2026-08-16".into()),
            completed_steps: ["l0-hello:pass"].into_iter().map(String::from).collect(),
            achievements: ["first_steps"].into_iter().map(String::from).collect(),
            practice_unlock_all: true,
            victory_celebrated: true,
            ..SaveData::default()
        };
        data.level_states.insert(
            "l0-hello".into(),
            LevelProgress {
                state: LevelState::Passed,
                attempts: 2,
                completed_at: Some("1720000000".into()),
                best_time_ms: Some(42000),
                hints_used: vec![0, 1],
            },
        );
        data.boss_states.insert(
            "l1-clone".into(),
            BossProgress { defeated: true, best_attempts: 2 },
        );
        save(&data, &p).unwrap();
        let loaded = load(&p).unwrap();
        assert_eq!(loaded, data);
        // 原子写：临时文件应不存在
        assert!(!p.with_extension("toml.tmp").exists());
    }

    #[test]
    fn completed_steps_only_passed() {
        // ④ completed_steps 仅 Passed 进集合（unlocked/locked 不入）
        let p = temp_path("v0-three-states.toml");
        std::fs::write(&p, V0_THREE_STATES).unwrap();
        let data = load(&p).unwrap();
        assert_eq!(data.completed_steps, ["l0-hello:pass"].into_iter().map(String::from).collect());
        assert_eq!(data.level_states.get("l1-move").unwrap().state, LevelState::Unlocked);
        assert_eq!(data.level_states.get("l2-vec").unwrap().state, LevelState::Locked);
    }

    #[test]
    fn minimal_toml_migrates_to_full_v1() {
        // ⑤ 极简 TOML（仅 xp=5）→ 迁移成完整 v1 不报错
        let p = temp_path("minimal.toml");
        std::fs::write(&p, "xp = 5\n").unwrap();
        let data = load(&p).unwrap();
        assert_eq!(data.version, CURRENT_SAVE_VERSION);
        assert_eq!(data.xp, 5);
        assert_eq!(data.hearts, 3);
        assert_eq!(data.combo, 0);
        assert!(data.level_states.is_empty());
        assert!(data.completed_steps.is_empty());
        assert!(data.boss_states.is_empty());
        assert_eq!(data.player_name, "");
    }

    #[test]
    fn save_creates_bak_with_old_content() {
        // save() 写入前把旧档复制为 .bak，内容为旧档
        let p = temp_path("bak.toml");
        std::fs::write(&p, "xp = 1\n").unwrap();
        let data = SaveData { xp: 2, ..SaveData::default() };
        save(&data, &p).unwrap();
        let bak = p.with_extension("toml.bak");
        assert!(bak.exists());
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), "xp = 1\n");
        // 新档已写入
        let loaded = load(&p).unwrap();
        assert_eq!(loaded.xp, 2);
    }

    #[test]
    fn migration_persisted_immediately() {
        // 迁移后立即落盘：load 返回后磁盘文件已是 v1（崩溃前迁移结果已持久化）
        let p = temp_path("migrate-persist.toml");
        std::fs::write(&p, V0_REAL).unwrap();
        let data = load(&p).unwrap();
        assert_eq!(data.version, CURRENT_SAVE_VERSION);
        let on_disk = std::fs::read_to_string(&p).unwrap();
        assert!(on_disk.contains("version = 1"));
        assert!(on_disk.contains("hearts = 3"));
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
