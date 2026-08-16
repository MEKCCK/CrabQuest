use crate::error::GameError;
use crate::level::{Level, LevelSet};
use crate::sandbox::Sandbox;
use crate::save::{LevelProgress, LevelState, SaveData};
use crate::validate::mapper::ErrorMapper;
use crate::validate::{validate, Validation};

/// XP 定价表（v3 §7.2，替换旧 XP_PER_PASS=20）：
/// 首次通关（普通关）+25；完美通关（fail_count==0）+10；连击加成（通过后 combo>=3）+5；
/// Boss 首通 ≤4 次尝试 +50；Boss 首通 >4 次尝试 +30。重复通关 +0（combo 仍更新）。
/// 单关上限：普通 25+10+5=40；Boss 50+10+5=65。
pub const XP_PASS: u32 = 25;
pub const XP_PERFECT: u32 = 10;
pub const XP_COMBO: u32 = 5;
pub const XP_BOSS: u32 = 50;
pub const XP_BOSS_FALLBACK: u32 = 30;

/// XP 一次制分档纯函数（v3 §7.2）：四步累加，重复通关一律 +0。
///
/// - `is_first_pass`：`completed_steps` 无 `"{level_id}:pass"` 记录；
/// - `is_boss`：Boss 关替换 base 档位（≤4 次尝试 +50 / >4 次尝试 +30）；
/// - `fail_count`：该关失败提交次数，==0 且首通 → 完美 +10；
/// - `combo_after_pass`：通过后 combo 值（v3「通过后 combo ≥ 3」→ 取累加后值）；
/// - `attempts_at_pass`：通关时该关累计提交次数（含本次通过，总提交数 = fail + 通过）。
///
/// 返回本次应得 XP（已钳制单关上限：普通 40 / Boss 65）。
pub fn award_xp(
    is_first_pass: bool,
    is_boss: bool,
    fail_count: u32,
    combo_after_pass: u32,
    attempts_at_pass: u32,
) -> u32 {
    if !is_first_pass {
        return 0;
    }
    let mut xp = if is_boss {
        if attempts_at_pass <= 4 {
            XP_BOSS
        } else {
            XP_BOSS_FALLBACK
        }
    } else {
        XP_PASS
    };
    if fail_count == 0 {
        xp += XP_PERFECT;
    }
    if combo_after_pass >= 3 {
        xp += XP_COMBO;
    }
    // 单关上限保险钳制（当前定价天然不越界，防止未来新增加成越限）
    let cap = if is_boss {
        XP_BOSS + XP_PERFECT + XP_COMBO
    } else {
        XP_PASS + XP_PERFECT + XP_COMBO
    };
    xp.min(cap)
}

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

        let mut xp_gained = 0;
        match &result {
            Validation::Pass { .. } => {
                let pass_key = format!("{}:pass", level.id);
                let first_pass = !self.save.completed_steps.contains(&pass_key);
                self.save.combo += 1;
                self.save.max_combo = self.save.max_combo.max(self.save.combo);
                let entry = self
                    .save
                    .level_states
                    .entry(level.id.clone())
                    .or_insert_with(|| LevelProgress {
                        state: LevelState::Unlocked,
                        ..LevelProgress::default()
                    });
                entry.state = LevelState::Passed;
                entry.attempts += 1;
                entry.completed_at = Some(unix_secs());
                // XP 一次制分档：实际奖励随 Validation::Pass 返回（v3 §7.2）
                xp_gained = award_xp(
                    first_pass,
                    level.is_boss,
                    entry.fail_count,
                    self.save.combo, // 通过后 combo（v3「通过后 combo ≥ 3」）
                    entry.attempts,  // 通关时累计提交次数（含本次通过）
                );
                self.save.xp += xp_gained;
                if first_pass {
                    self.save.completed_steps.insert(pass_key);
                }
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
                entry.fail_count += 1;
            }
        }
        Ok(match result {
            Validation::Pass { .. } => Validation::Pass { xp_gained },
            other => other,
        })
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
    use crate::level::{parse_levels, LevelSet};
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
        // 首通 + 完美（首次提交即通过）→ 25 + 10 = 35
        assert_eq!(e.submit(code).unwrap(), Validation::Pass { xp_gained: XP_PASS + XP_PERFECT });
        assert_eq!(e.save.xp, XP_PASS + XP_PERFECT);
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
        assert!(matches!(e.submit(code).unwrap(), Validation::Pass { .. }));
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

    // ---- P1-05：XP 一次制分档 + rank（v3 §7.2/§7.3）----

    fn boss_engine() -> Engine {
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"boss\"\ntitle = \"boss\"\ntier = \"l4\"\ndescription = \"d\"\nstarter_code = \"\"\nis_boss = true\nexpect_output = \"ok\"\nsource = \"rust-quiz\"\n",
            )
            .unwrap(),
        };
        Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()))
    }

    #[test]
    fn first_pass_awards_25_base() {
        // 首次通关：+25 base（无 perfect/combo 时）
        assert_eq!(award_xp(true, false, 1, 1, 2), XP_PASS);
        assert_eq!(award_xp(true, false, 1, 2, 2), XP_PASS);
    }

    #[test]
    fn repeat_pass_awards_zero_but_combo_still_updates() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        assert_eq!(e.submit(code).unwrap(), Validation::Pass { xp_gained: XP_PASS + XP_PERFECT });
        assert_eq!(e.save.xp, XP_PASS + XP_PERFECT);
        assert!(e.save.completed_steps.contains("l0-hello:pass"));
        // 重复通关同一关：+0 XP，combo 仍更新（练习价值保留）
        assert_eq!(e.submit(code).unwrap(), Validation::Pass { xp_gained: 0 });
        assert_eq!(e.save.xp, XP_PASS + XP_PERFECT);
        assert_eq!(e.save.combo, 2);
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().attempts, 2);
    }

    #[test]
    fn perfect_pass_awards_10() {
        // 完美通关（首次提交即通过，fail_count == 0）：+10
        assert_eq!(award_xp(true, false, 0, 1, 1), XP_PASS + XP_PERFECT);
        // 失败过再通过：无 perfect
        assert_eq!(award_xp(true, false, 1, 1, 2), XP_PASS);
    }

    #[test]
    fn combo_3_or_more_awards_5() {
        // 连击加成：首通且通过后 combo >= 3 → +5（v3「通过后 combo ≥ 3」，取累加后值）
        assert_eq!(award_xp(true, false, 1, 3, 3), XP_PASS + XP_COMBO);
        assert_eq!(award_xp(true, false, 0, 3, 1), XP_PASS + XP_PERFECT + XP_COMBO);
        // combo 2 时无加成
        assert_eq!(award_xp(true, false, 0, 2, 1), XP_PASS + XP_PERFECT);
    }

    #[test]
    fn single_level_cap_normal_40() {
        // 普通关单关上限 40 = 25 + 10 + 5（全加成可叠加且不越上限）
        let gained = award_xp(true, false, 0, 3, 1);
        assert_eq!(gained, XP_PASS + XP_PERFECT + XP_COMBO);
        assert_eq!(gained, 40);
    }

    #[test]
    fn boss_first_pass_4_attempts_50() {
        // Boss 首通 ≤4 次尝试 → +50（替换 base；perfect/combo 照常叠加）
        assert_eq!(award_xp(true, true, 3, 1, 4), XP_BOSS);
        assert_eq!(award_xp(true, true, 0, 3, 1), XP_BOSS + XP_PERFECT + XP_COMBO);
        // Boss 单关上限 65 = 50 + 10 + 5
        assert_eq!(XP_BOSS + XP_PERFECT + XP_COMBO, 65);
    }

    #[test]
    fn boss_first_pass_over_4_attempts_30() {
        // Boss 首通 >4 次尝试 → +30 惩罚档
        assert_eq!(award_xp(true, true, 4, 1, 5), XP_BOSS_FALLBACK);
    }

    #[test]
    fn boss_level_attempts_drive_tier_via_submit() {
        // 集成：4 次提交（3 败 1 过）→ +50；5 次提交（4 败 1 过）→ +30
        let mut e = boss_engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        for _ in 0..3 {
            e.submit(bad).unwrap();
        }
        assert_eq!(e.submit("fn main() { println!(\"ok\"); }").unwrap(), Validation::Pass { xp_gained: XP_BOSS });
        let p = e.save.level_states.get("boss").unwrap();
        assert_eq!(p.fail_count, 3);
        assert_eq!(p.attempts, 4);

        let mut e2 = boss_engine();
        e2.new_game();
        e2.start_level(0).unwrap();
        for _ in 0..4 {
            e2.submit(bad).unwrap();
        }
        assert_eq!(e2.submit("fn main() { println!(\"ok\"); }").unwrap(), Validation::Pass { xp_gained: XP_BOSS_FALLBACK });
        let p = e2.save.level_states.get("boss").unwrap();
        assert_eq!(p.fail_count, 4);
        assert_eq!(p.attempts, 5);
        assert_eq!(e2.save.xp, XP_BOSS_FALLBACK);
    }

    #[test]
    fn fail_increments_fail_count_attempts_is_total() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        e.submit(bad).unwrap();
        e.submit(bad).unwrap();
        let p = e.save.level_states.get("l0-hello").unwrap();
        assert_eq!(p.fail_count, 2);
        assert_eq!(p.attempts, 2);
        // 通过：attempts 含本次（3），fail_count 保持 2 → 无 perfect、combo 1 < 3 无连击
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        assert_eq!(e.submit(code).unwrap(), Validation::Pass { xp_gained: XP_PASS });
        let p = e.save.level_states.get("l0-hello").unwrap();
        assert_eq!(p.fail_count, 2);
        assert_eq!(p.attempts, 3);
        assert_eq!(e.save.xp, XP_PASS);
    }

    #[test]
    fn rank_does_not_unlock_levels() {
        // rank 只解锁元内容：关卡线性解锁链不受 rank 影响（v3 §7.3）
        let mut e = engine();
        e.new_game();
        // 伪造 R10 存档：15 关 Passed
        for i in 0..15 {
            let id = format!("fake{i}");
            e.save
                .level_states
                .insert(id, LevelProgress { state: LevelState::Passed, ..LevelProgress::default() });
        }
        assert_eq!(crate::rank::rank_for(e.save.completed_count()).level, 10);
        // l1-move 仍 Locked → 拒绝进入（解锁只看 level_states.state）
        assert!(matches!(e.start_level(1), Err(GameError::LevelLocked(_))));
    }
}
