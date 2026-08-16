use crate::error::GameError;
use crate::level::{Level, LevelSet};
use crate::sandbox::Sandbox;
use crate::save::{LevelProgress, LevelState, SaveData};
use crate::validate::mapper::ErrorMapper;
use crate::validate::{validate, Validation};

pub const XP_PER_PASS: u32 = 20;

pub struct Engine {
    pub level_set: LevelSet,
    pub save: SaveData,
    pub current: Option<usize>,
    pub mapper: ErrorMapper,
    pub sandbox: Box<dyn Sandbox>,
}

impl Engine {
    pub fn new(
        level_set: LevelSet,
        save: SaveData,
        mapper: ErrorMapper,
        sandbox: Box<dyn Sandbox>,
    ) -> Self {
        Self { level_set, save, current: None, mapper, sandbox }
    }

    pub fn new_game(&mut self) {
        self.save = SaveData::default();
        self.current = None;
        // 预置全部关卡状态底图（默认 Locked），保证线性解锁前后 map 中均存在每关条目
        for lvl in &self.level_set.levels {
            self.save
                .level_states
                .entry(lvl.id.clone())
                .or_insert_with(LevelProgress::default);
        }
        self.unlock_first();
    }

    pub fn unlock_first(&mut self) {
        if let Some(first) = self.level_set.levels.first() {
            let p = self
                .save
                .level_states
                .entry(first.id.clone())
                .or_insert_with(LevelProgress::default);
            p.state = LevelState::Unlocked;
        }
    }

    pub fn start_level(&mut self, index: usize) -> Result<(), GameError> {
        let level = self
            .level_set
            .levels
            .get(index)
            .ok_or_else(|| GameError::LevelNotFound(format!("index {index}")))?;
        let state = self
            .save
            .level_states
            .get(&level.id)
            .map(|p| p.state)
            .unwrap_or(LevelState::Locked);
        if state == LevelState::Locked {
            return Err(GameError::LevelLocked(level.id.clone()));
        }
        self.current = Some(index);
        Ok(())
    }

    pub fn submit(&mut self, code: &str) -> Result<Validation, GameError> {
        let idx = self
            .current
            .ok_or_else(|| GameError::LevelNotFound("无当前关卡".into()))?;
        let level = self
            .level_set
            .levels
            .get(idx)
            .cloned()
            .ok_or_else(|| GameError::LevelNotFound(format!("index {idx}")))?;

        let result = validate(&level, code, &self.mapper, self.sandbox.as_ref())?;

        match &result {
            Validation::Pass => {
                self.save.xp += XP_PER_PASS;
                self.save.combo += 1;
                self.save.max_combo = self.save.max_combo.max(self.save.combo);
                let entry = self
                    .save
                    .level_states
                    .entry(level.id.clone())
                    .or_insert_with(|| LevelProgress {
                        state: LevelState::Unlocked,
                        attempts: 0,
                        completed_at: None,
                    });
                entry.state = LevelState::Passed;
                entry.attempts += 1;
                entry.completed_at = Some(unix_secs());
                if let Some(next) = self.level_set.levels.get(idx + 1) {
                    let n = self
                        .save
                        .level_states
                        .entry(next.id.clone())
                        .or_insert_with(LevelProgress::default);
                    if n.state == LevelState::Locked {
                        n.state = LevelState::Unlocked;
                    }
                }
            }
            Validation::Fail { .. } => {
                self.save.combo = 0;
                self.save.total_errors += 1;
                let entry = self
                    .save
                    .level_states
                    .entry(level.id.clone())
                    .or_insert_with(LevelProgress::default);
                entry.attempts += 1;
            }
        }
        Ok(result)
    }

    pub fn current_level(&self) -> Option<&Level> {
        self.current.and_then(|i| self.level_set.levels.get(i))
    }

    pub fn can_continue(&self) -> bool {
        self.save.xp > 0
            || self.save.level_states.values().any(|p| p.state == LevelState::Passed)
    }

    pub fn save_ref(&self) -> &SaveData {
        &self.save
    }
}

fn unix_secs() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{parse_levels, LevelSet, LevelTier};
    use crate::save::{LevelState, SaveData};
    use crate::sandbox::DevSandbox;
    use crate::validate::mapper::ErrorMapper;
    use crate::validate::Validation;

    const LEVELS: &str = r#"
[[level]]
id = "l0-hello"
title = "hello"
tier = "l0"
description = "d"
starter_code = "fn main() { x = 5; println!(\"x has the value {}\", x); }"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l1-move"
title = "move"
tier = "l1"
description = "d"
starter_code = "fn main() { let s = String::from(\"hi\"); take(s); println!(\"{}\", s); } fn take(x: String) {}"
expect_output = "hi"
source = "rustlings"
"#;

    fn engine() -> Engine {
        let set = LevelSet { levels: parse_levels(LEVELS).unwrap() };
        Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()))
    }

    #[test]
    fn new_game_unlocks_first() {
        let mut e = engine();
        e.new_game();
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().state, LevelState::Unlocked);
        assert_eq!(e.save.level_states.get("l1-move").unwrap().state, LevelState::Locked);
    }

    #[test]
    fn locked_level_rejected() {
        let mut e = engine();
        e.new_game();
        assert!(matches!(e.start_level(1), Err(GameError::LevelLocked(_))));
    }

    #[test]
    fn pass_updates_xp_combo_and_unlocks_next() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        assert_eq!(e.submit(code).unwrap(), Validation::Pass);
        assert_eq!(e.save.xp, XP_PER_PASS);
        assert_eq!(e.save.combo, 1);
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().state, LevelState::Passed);
        assert_eq!(e.save.level_states.get("l1-move").unwrap().state, LevelState::Unlocked);
        assert!(e.save.level_states.get("l0-hello").unwrap().completed_at.is_some());
    }

    #[test]
    fn fail_resets_combo_and_counts_error() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 先通关一关拿到 combo
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        e.submit(code).unwrap();
        assert_eq!(e.save.combo, 1);
        // 然后在 l1-move 上故意写错
        e.start_level(1).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        assert!(matches!(e.submit(bad).unwrap(), Validation::Fail { .. }));
        assert_eq!(e.save.combo, 0);
        assert_eq!(e.save.total_errors, 1);
        assert_eq!(e.save.level_states.get("l1-move").unwrap().attempts, 1);
        // 失败不改变关卡状态
        assert_eq!(e.save.level_states.get("l1-move").unwrap().state, LevelState::Unlocked);
    }

    #[test]
    fn allow_compile_fail_level_passes_with_right_error() {
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"l1-bug\"\ntitle = \"制造错误\"\ntier = \"l1\"\ndescription = \"d\"\nstarter_code = \"\"\nallow_compile_fail = true\nexpect_error_code = \"E0382\"\nsource = \"rust-quiz\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        assert_eq!(e.submit(code).unwrap(), Validation::Pass);
    }

    #[test]
    fn can_continue_after_progress() {
        let mut e = engine();
        assert!(!e.can_continue());
        e.new_game();
        assert!(!e.can_continue());
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        assert!(e.can_continue());
    }
}
