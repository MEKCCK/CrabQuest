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

        // P2-08：0 心禁提交（引擎层兜底；不编译、不扣 XP，UI 应已禁用按钮）
        if self.save.hearts == 0 {
            return Err(GameError::NoHearts);
        }

        let result = validate(&level, code, &self.mapper, self.sandbox.as_ref())?;

        let mut xp_gained = 0;
        // P2-10：本次通关上下文（id, fail_count, hints_used 是否为空），
        // 供 check_achievements 判定完美类成就（Fail 分支为 None）
        let mut just_passed: Option<(String, u32, bool)> = None;
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
                // 提前取本次通关上下文（id, fail_count, hints_used 为空），
                // 结束 entry 借用后再做后续自借用（解锁下一关 / touch_activity）
                just_passed = Some((
                    level.id.clone(),
                    entry.fail_count,
                    entry.hints_used.is_empty(),
                ));
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
                // P2-08：通关回血 +1（cap 5）；P2-09：通关 = 活跃
                self.save.hearts = (self.save.hearts + 1).min(5);
                self.touch_activity();
            }
            Validation::Fail { errors, .. } => {
                self.save.combo = 0;
                self.save.total_errors += 1;
                let entry = self
                    .save
                    .level_states
                    .entry(level.id.clone())
                    .or_insert_with(LevelProgress::default);
                entry.attempts += 1;
                entry.fail_count += 1;
                // P2-08：失败扣心（floor 0）；Boss 失败不扣（显式标注或命中已知 Boss 表）
                if !(level.is_boss || crate::achievements::is_boss_level_id(&level.id)) {
                    self.save.hearts = self.save.hearts.saturating_sub(1);
                }
                // P2-10：记录本次见到的错误码（不同错误码去重累计，≥10 种解锁收藏家）
                for card in errors {
                    self.save.seen_error_codes.insert(card.code.clone());
                }
            }
        }
        // P2-10：统一成就检查（纯函数，HashSet 幂等）
        let newly = crate::achievements::check_achievements(&crate::achievements::AchievementCheck {
            level_states: &self.save.level_states,
            completed_steps: &self.save.completed_steps,
            combo: self.save.combo,
            seen_error_codes: &self.save.seen_error_codes,
            total_levels: self.level_set.len(),
            already: &self.save.achievements,
            just_passed: just_passed
                .as_ref()
                .map(|(id, fail_count, hints_empty)| (id.as_str(), *fail_count, *hints_empty)),
        });
        for id in newly {
            self.save.achievements.insert(id);
        }
        Ok(match result {
            Validation::Pass { .. } => Validation::Pass { xp_gained },
            other => other,
        })
    }

    /// P2-08：复习关卡说明回血（每关每局一次，幂等）。
    /// 返回是否实际回了 1 心（首次复习且心 <5 时 true；满心或已复习过 → false）。
    /// 复习也算活跃行为（P2-09 streak）。
    pub fn review_lore(&mut self, level_id: &str) -> bool {
        let key = format!("{level_id}:lore");
        if self.save.completed_steps.contains(&key) {
            return false;
        }
        let healed = self.save.hearts < 5;
        if healed {
            self.save.hearts += 1;
        }
        self.save.completed_steps.insert(key);
        self.touch_activity();
        healed
    }

    /// P2-09：活跃一次（通关 / 查看 hint / 复习回血共用钩子）。
    /// 同日幂等（streak 纯函数 touch_streak 判定），更新 last_played_date 为今天。
    pub fn touch_activity(&mut self) {
        let today = crate::streak::today_str();
        let (streak, date) = crate::streak::touch_streak(
            self.save.streak_days,
            self.save.last_played_date.as_deref(),
            &today,
        );
        self.save.streak_days = streak;
        self.save.last_played_date = Some(date);
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

    // ===== P2-08：hearts =====

    #[test]
    fn initial_hearts_is_3() {
        let mut e = engine();
        e.new_game();
        assert_eq!(e.save.hearts, 3);
    }

    #[test]
    fn fail_deducts_heart_floor_zero() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        e.submit(bad).unwrap();
        assert_eq!(e.save.hearts, 2);
        e.submit(bad).unwrap();
        assert_eq!(e.save.hearts, 1);
        e.submit(bad).unwrap();
        assert_eq!(e.save.hearts, 0);
        // 0 心后提交被引擎拦截（NoHearts），心数保持 0
        assert!(matches!(e.submit(bad), Err(GameError::NoHearts)));
        assert_eq!(e.save.hearts, 0);
    }

    #[test]
    fn zero_hearts_rejects_submit_without_state_change() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.save.hearts = 0;
        let before_xp = e.save.xp;
        let before_attempts = e.save.level_states.get("l0-hello").map(|p| p.attempts).unwrap_or(0);
        assert!(matches!(e.submit("fn main() { println!(\"x has the value {}\", 5); }"), Err(GameError::NoHearts)));
        // 0 心拦截：不扣 XP、不计尝试、不改变状态
        assert_eq!(e.save.xp, before_xp);
        assert_eq!(e.save.level_states.get("l0-hello").map(|p| p.attempts).unwrap_or(0), before_attempts);
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().state, LevelState::Unlocked);
    }

    #[test]
    fn pass_restores_heart_capped_at_5() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        let good = "fn main() { println!(\"x has the value {}\", 5); }";
        // 先扣到 2：3 → 2
        e.submit(bad).unwrap();
        assert_eq!(e.save.hearts, 2);
        // 通关 +1：2 → 3
        e.submit(good).unwrap();
        assert_eq!(e.save.hearts, 3);
        // 重复通关继续 +1 至 cap 5
        e.submit(good).unwrap();
        assert_eq!(e.save.hearts, 4);
        e.submit(good).unwrap();
        assert_eq!(e.save.hearts, 5);
        e.submit(good).unwrap();
        assert_eq!(e.save.hearts, 5, "cap 5：满心后再通关不再增加");
    }

    #[test]
    fn boss_fail_keeps_hearts() {
        let mut e = boss_engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        for _ in 0..3 {
            e.submit(bad).unwrap();
        }
        assert_eq!(e.save.hearts, 3, "Boss 失败不扣心");
    }

    #[test]
    fn boss_id_fallback_keeps_hearts_even_without_flag() {
        // 已知 Boss id 表兜底：即使数据未标注 is_boss，失败也不扣心（P3-17 数据未落地前）
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"l1-clone\"\ntitle = \"boss\"\ntier = \"l1\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(1); }\"\nexpect_output = \"1\"\nsource = \"x\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"wrong\"); }").unwrap();
        assert_eq!(e.save.hearts, 3);
    }

    #[test]
    fn review_lore_heals_once_per_level() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 失败一次 → 2 心
        e.submit("fn main() { println!(\"wrong\"); }").unwrap();
        assert_eq!(e.save.hearts, 2);
        // 复习回血 → 3 心，且记 lore 标记
        assert!(e.review_lore("l0-hello"));
        assert_eq!(e.save.hearts, 3);
        assert!(e.save.completed_steps.contains("l0-hello:lore"));
        // 幂等：同关再复习不回血、不加标记次数
        assert!(!e.review_lore("l0-hello"));
        assert_eq!(e.save.hearts, 3);
        // 其他关的 lore 标记独立
        assert!(!e.save.completed_steps.contains("l1-move:lore"));
    }

    #[test]
    fn review_lore_full_hearts_no_gain_but_marked() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 满心（5）时复习：不回血但写入标记（之后扣心也无法再回该关的血）
        e.save.hearts = 5;
        assert!(!e.review_lore("l0-hello"));
        assert_eq!(e.save.hearts, 5);
        assert!(e.save.completed_steps.contains("l0-hello:lore"));
        e.submit("fn main() { println!(\"wrong\"); }").unwrap();
        assert_eq!(e.save.hearts, 4);
        assert!(!e.review_lore("l0-hello"), "已复习过 → 不回血");
        assert_eq!(e.save.hearts, 4);
    }

    // ===== P2-09：streak =====

    #[test]
    fn first_pass_touches_streak() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        assert_eq!(e.save.streak_days, 1);
        let today = crate::streak::today_str();
        assert_eq!(e.save.last_played_date.as_deref(), Some(today.as_str()));
    }

    #[test]
    fn same_day_activity_is_idempotent() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let good = "fn main() { println!(\"x has the value {}\", 5); }";
        e.submit(good).unwrap();
        let date = e.save.last_played_date.clone();
        // 同日再次通关 + 复习：streak 不变（幂等）
        e.submit(good).unwrap();
        assert_eq!(e.save.streak_days, 1);
        assert_eq!(e.save.last_played_date, date);
        e.review_lore("l0-hello");
        assert_eq!(e.save.streak_days, 1);
        assert_eq!(e.save.last_played_date, date);
    }

    #[test]
    fn yesterday_active_increments_streak() {
        // 直接构造「昨天活跃」存档（streak 纯逻辑在 streak.rs 单测锁定，这里验钩子接线）
        let mut e = engine();
        e.new_game();
        e.save.streak_days = 3;
        let today = crate::streak::today_str();
        let yesterday = crate::streak::previous_day(&today).expect("today 必有昨天");
        e.save.last_played_date = Some(yesterday.clone());
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        assert_eq!(e.save.streak_days, 4, "昨日活跃 → +1");
        assert_eq!(e.save.last_played_date.as_deref(), Some(today.as_str()));
    }

    #[test]
    fn hint_view_touches_streak() {
        let mut e = engine();
        e.new_game();
        assert_eq!(e.save.streak_days, 0);
        // hint 查看走 app 层（Input::Hint → engine.touch_activity），这里直接验钩子
        e.touch_activity();
        assert_eq!(e.save.streak_days, 1);
        assert!(e.save.last_played_date.is_some());
    }

    // ===== P2-10：achievements =====

    #[test]
    fn first_pass_unlocks_first_steps_and_no_hint_perfect() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        assert!(e.save.achievements.contains("first_steps"));
        // 首通即完美且未看 hint → 无师自通
        assert!(e.save.achievements.contains("no_hint_perfect"));
        assert!(!e.save.achievements.contains("champion"));
    }

    #[test]
    fn fail_records_seen_error_codes() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 已知 E0382 错误（move 语义，与 allow_compile_fail 测试同源代码）
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        e.submit(code).unwrap();
        assert!(e.save.seen_error_codes.contains("E0382"), "seen_error_codes 应记录 E0382: {:?}", e.save.seen_error_codes);
        // 重复提交同码不重复计数
        e.submit(code).unwrap();
        assert_eq!(e.save.seen_error_codes.len(), 1);
    }

    #[test]
    fn boss_pass_unlocks_boss_slayer() {
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"l1-clone\"\ntitle = \"boss\"\ntier = \"l1\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(\\\"1\\\"); }\"\nis_boss = true\nexpect_output = \"1\"\nsource = \"x\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        e.new_game();
        e.start_level(0).unwrap();
        assert!(!e.save.achievements.contains("boss_slayer"));
        e.submit("fn main() { println!(\"1\"); }").unwrap();
        assert!(e.save.achievements.contains("boss_slayer"));
        assert!(e.save.achievements.contains("first_steps"));
        assert!(!e.save.achievements.contains("boss_all"), "单 Boss 不触发屠龙");
        assert!(e.save.achievements.contains("champion"), "单关总关数 1 → 通关即冠军");
    }

    #[test]
    fn all_four_bosses_unlock_boss_all() {
        let mut toml = String::new();
        for (i, id) in ["l1-clone", "l2-result", "l3-trait", "l4-lifetime-trap"].iter().enumerate() {
            toml.push_str(&format!(
                "[[level]]\nid = \"{id}\"\ntitle = \"boss{i}\"\ntier = \"l4\"\ndescription = \"d\"\nstarter_code = \"fn main() {{ println!(\\\"1\\\"); }}\"\nis_boss = true\nexpect_output = \"1\"\nsource = \"x\"\n"
            ));
        }
        let set = LevelSet { levels: parse_levels(&toml).unwrap() };
        let mut e = Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        e.new_game();
        for i in 0..4 {
            e.start_level(i).unwrap();
            e.submit("fn main() { println!(\"1\"); }").unwrap();
        }
        assert!(e.save.achievements.contains("boss_slayer"));
        assert!(e.save.achievements.contains("boss_all"));
        assert!(e.save.achievements.contains("champion"), "4 关全过 → 冠军");
    }

    #[test]
    fn never_give_up_after_ten_fails_then_pass() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        for _ in 0..10 {
            // 0 心拦截前先回血（模拟在别处通关回血，验证 fail_count 可累计到 10）
            if e.save.hearts == 0 {
                e.save.hearts = 3;
            }
            e.submit(bad).unwrap();
        }
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().fail_count, 10);
        // 0 心：复习回血 1 心后通过（0 心禁提交，但复习后心 > 0 可提交）
        e.save.hearts = 0;
        assert!(e.review_lore("l0-hello"), "0 心复习应回 1 心");
        assert_eq!(e.save.hearts, 1);
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        assert!(e.save.achievements.contains("never_give_up"), "失败 ≥10 次后通过 → 永不言弃");
    }

    #[test]
    fn achievements_idempotent_on_repeat() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let good = "fn main() { println!(\"x has the value {}\", 5); }";
        e.submit(good).unwrap();
        let first = e.save.achievements.clone();
        assert!(first.contains("first_steps"));
        e.submit(good).unwrap();
        e.submit(good).unwrap();
        // 重复通关不重复入账（HashSet 幂等，无新成就）
        assert_eq!(e.save.achievements, first);
    }
}
