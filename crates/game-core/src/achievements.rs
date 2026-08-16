//! 成就系统（P2-10，v3 §7.4）：10 个静态成就，HashSet 存档、无 XP 奖励。
//! `check_achievements` 为纯函数（只读存档状态快照 → 本次新解锁的成就 id 列表），
//! 触发点挂在 engine.submit 的 Pass/Fail 分支后统一调用；重复入账由 HashSet 幂等。

use crate::save::LevelProgress;
use std::collections::{HashMap, HashSet};

/// 已知的 4 个 Boss 关 id（assets/levels 07-l1-clone / 10-l2-result / 12-l3-trait /
/// 14-l4-lifetime-trap；当前数据尚未标注 is_boss，按 id 表兜底判定）。
pub const BOSS_LEVEL_IDS: [&str; 4] = ["l1-clone", "l2-result", "l3-trait", "l4-lifetime-trap"];

/// 成就静态表：id → 中文名（v3 §7.4 成就表，顺序即图鉴顺序）。
pub const ACHIEVEMENTS: [(&str, &str); 10] = [
    ("first_steps", "初出茅庐"),
    ("no_hint_perfect", "无师自通"),
    ("combo_5", "连击 5"),
    ("combo_10", "连击 10"),
    ("owner_guard", "所有权卫士"),
    ("boss_slayer", "斩将"),
    ("boss_all", "屠龙"),
    ("error_collector", "错误收藏家"),
    ("never_give_up", "永不言弃"),
    ("champion", "冠军"),
];

/// 成就 id → 中文名（未知 id → None）。
pub fn achievement_name(id: &str) -> Option<&'static str> {
    ACHIEVEMENTS.iter().find(|(k, _)| *k == id).map(|(_, n)| *n)
}

/// id 是否命中已知 Boss 表。
pub fn is_boss_level_id(id: &str) -> bool {
    BOSS_LEVEL_IDS.contains(&id)
}

/// `check_achievements` 的只读入参（存档快照 + 本次通过上下文；纯数据，禁止传 Engine）。
pub struct AchievementCheck<'a> {
    /// 全部关卡状态（champion / boss 判定按 state==Passed 推导）
    pub level_states: &'a HashMap<String, LevelProgress>,
    /// 一次制通关标记 "{level_id}:pass"（first_steps 判定）
    pub completed_steps: &'a HashSet<String>,
    /// 当前连击（通过后累加值；combo_5 / combo_10 判定）
    pub combo: u32,
    /// 累计见过的不同错误码（error_collector 判定）
    pub seen_error_codes: &'a HashSet<String>,
    /// 关卡总数（champion = 全部通关）
    pub total_levels: usize,
    /// 已入账成就（幂等：已满足但已入账 → 不重复返回）
    pub already: &'a HashSet<String>,
    /// 本次 Pass 分支刚通关的关卡：(关卡 id, 该关失败次数, 该关 hints_used 是否为空)
    /// Fail 分支传 None。完美类成就（no_hint_perfect / owner_guard / never_give_up）
    /// 必须在通关时刻取该关的 fail_count / hints_used 判定。
    pub just_passed: Option<(&'a str, u32, bool)>,
}

/// 纯函数：从存档快照计算「已满足但尚未入账」的成就 id 列表（新解锁）。
///
/// 各成就条件（v3 §7.4 成就表）：
/// - first_steps：任意关卡首次通关（completed_steps 非空）；
/// - no_hint_perfect：首次提交即通过（fail_count==0）且该关未看过 hint；
/// - combo_5 / combo_10：通过后 combo 达 5 / 10；
/// - owner_guard：l1-move 完美通关（fail_count==0）；
/// - boss_slayer / boss_all：击败任意 / 全部 4 个 Boss；
/// - error_collector：seen_error_codes ≥ 10 种不同错误码；
/// - never_give_up：单关失败 ≥10 次后仍通过；
/// - champion：全部关卡通关。
pub fn check_achievements(c: &AchievementCheck<'_>) -> Vec<String> {
    let passed: HashSet<&str> = c
        .level_states
        .iter()
        .filter(|(_, p)| p.state == crate::save::LevelState::Passed)
        .map(|(id, _)| id.as_str())
        .collect();
    let passed_count = passed.len();
    let boss_passed = BOSS_LEVEL_IDS.iter().filter(|id| passed.contains(*id)).count();

    let mut newly = Vec::new();
    let mut unlock = |id: &str, ok: bool| {
        if ok && !c.already.contains(id) {
            newly.push(id.to_string());
        }
    };

    unlock("first_steps", !c.completed_steps.is_empty());
    unlock("combo_5", c.combo >= 5);
    unlock("combo_10", c.combo >= 10);
    unlock("boss_slayer", boss_passed >= 1);
    unlock("boss_all", boss_passed == BOSS_LEVEL_IDS.len());
    unlock("error_collector", c.seen_error_codes.len() >= 10);
    unlock("champion", passed_count >= c.total_levels);

    if let Some((id, fail_count, hints_empty)) = c.just_passed {
        let perfect = fail_count == 0;
        unlock("no_hint_perfect", perfect && hints_empty);
        unlock("owner_guard", perfect && id == "l1-move");
        unlock("never_give_up", fail_count >= 10);
    }

    newly
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{LevelProgress, LevelState};

    fn progress(state: LevelState) -> LevelProgress {
        LevelProgress { state, ..LevelProgress::default() }
    }

    fn check(
        level_states: &HashMap<String, LevelProgress>,
        completed_steps: &HashSet<String>,
        combo: u32,
        seen: &HashSet<String>,
        total: usize,
        already: &HashSet<String>,
        just_passed: Option<(&str, u32, bool)>,
    ) -> Vec<String> {
        check_achievements(&AchievementCheck {
            level_states,
            completed_steps,
            combo,
            seen_error_codes: seen,
            total_levels: total,
            already,
            just_passed,
        })
    }

    fn empty() -> (HashMap<String, LevelProgress>, HashSet<String>, HashSet<String>, HashSet<String>) {
        (HashMap::new(), HashSet::new(), HashSet::new(), HashSet::new())
    }

    #[test]
    fn first_steps_unlocks_on_first_pass() {
        let (mut states, mut steps, seen, already) = empty();
        states.insert("l0-hello".into(), progress(LevelState::Passed));
        steps.insert("l0-hello:pass".into());
        let ids = check(&states, &steps, 1, &seen, 2, &already, Some(("l0-hello", 0, true)));
        assert!(ids.contains(&"first_steps".to_string()));
        // 未通关（completed_steps 空）不触发
        let (s2, st2, _, _) = empty();
        let ids = check(&s2, &st2, 0, &seen, 2, &already, None);
        assert!(!ids.contains(&"first_steps".to_string()));
    }

    #[test]
    fn no_hint_perfect_requires_empty_hints() {
        let (mut states, mut steps, seen, already) = empty();
        states.insert("l0-hello".into(), progress(LevelState::Passed));
        steps.insert("l0-hello:pass".into());
        // 首次提交即通过且未看 hint → 解锁
        let ids = check(&states, &steps, 1, &seen, 1, &already, Some(("l0-hello", 0, true)));
        assert!(ids.contains(&"no_hint_perfect".to_string()));
        // 看过 hint（hints_used 非空）→ 即使一次通过也不解锁（断言顺序：hint 在通过前看过）
        let ids = check(&states, &steps, 1, &seen, 1, &already, Some(("l0-hello", 0, false)));
        assert!(!ids.contains(&"no_hint_perfect".to_string()));
        // 失败过再通过（fail_count > 0）→ 非「首次提交即通过」
        let ids = check(&states, &steps, 1, &seen, 1, &already, Some(("l0-hello", 1, true)));
        assert!(!ids.contains(&"no_hint_perfect".to_string()));
        // Fail 分支（just_passed=None）不触发
        let ids = check(&states, &steps, 1, &seen, 1, &already, None);
        assert!(!ids.contains(&"no_hint_perfect".to_string()));
    }

    #[test]
    fn combo_5_and_combo_10() {
        let (states, steps, seen, already) = empty();
        assert!(check(&states, &steps, 5, &seen, 1, &already, None).contains(&"combo_5".to_string()));
        assert!(check(&states, &steps, 10, &seen, 1, &already, None).contains(&"combo_10".to_string()));
        // combo 4 只满足 5 不满足 10
        let ids = check(&states, &steps, 4, &seen, 1, &already, None);
        assert!(!ids.contains(&"combo_5".to_string()) && !ids.contains(&"combo_10".to_string()));
        let ids = check(&states, &steps, 9, &seen, 1, &already, None);
        assert!(ids.contains(&"combo_5".to_string()) && !ids.contains(&"combo_10".to_string()));
    }

    #[test]
    fn owner_guard_on_l1_move_perfect() {
        let (mut states, mut steps, seen, already) = empty();
        states.insert("l1-move".into(), progress(LevelState::Passed));
        steps.insert("l1-move:pass".into());
        // l1-move 完美通关 → 解锁
        let ids = check(&states, &steps, 1, &seen, 1, &already, Some(("l1-move", 0, false)));
        assert!(ids.contains(&"owner_guard".to_string()));
        // 失败过 → 非完美
        let ids = check(&states, &steps, 1, &seen, 1, &already, Some(("l1-move", 2, false)));
        assert!(!ids.contains(&"owner_guard".to_string()));
        // 其他关卡完美通过不触发
        let ids = check(&states, &steps, 1, &seen, 1, &already, Some(("l0-hello", 0, false)));
        assert!(!ids.contains(&"owner_guard".to_string()));
    }

    #[test]
    fn boss_slayer_and_boss_all() {
        let (mut states, steps, seen, already) = empty();
        // 击败 1 个 Boss → slayer；未集齐 → 无 all
        states.insert("l1-clone".into(), progress(LevelState::Passed));
        let ids = check(&states, &steps, 1, &seen, 5, &already, None);
        assert!(ids.contains(&"boss_slayer".to_string()) && !ids.contains(&"boss_all".to_string()));
        // 全部 4 个 Boss → all
        for b in BOSS_LEVEL_IDS {
            states.insert(b.to_string(), progress(LevelState::Passed));
        }
        let ids = check(&states, &steps, 1, &seen, 5, &already, None);
        assert!(ids.contains(&"boss_slayer".to_string()) && ids.contains(&"boss_all".to_string()));
        // 未击败任何 Boss → 都不触发
        let (s2, st2, _, _) = empty();
        let ids = check(&s2, &st2, 1, &seen, 5, &already, None);
        assert!(!ids.contains(&"boss_slayer".to_string()) && !ids.contains(&"boss_all".to_string()));
    }

    #[test]
    fn error_collector_requires_ten_distinct_codes() {
        let (states, steps, already) = (HashMap::new(), HashSet::new(), HashSet::new());
        let nine: HashSet<String> = (0..9).map(|i| format!("E{i:04}")).collect();
        let ids = check(&states, &steps, 0, &nine, 1, &already, None);
        assert!(!ids.contains(&"error_collector".to_string()));
        let ten: HashSet<String> = (0..10).map(|i| format!("E{i:04}")).collect();
        let ids = check(&states, &steps, 0, &ten, 1, &already, None);
        assert!(ids.contains(&"error_collector".to_string()));
    }

    #[test]
    fn never_give_up_after_ten_fails() {
        let (mut states, mut steps, seen, already) = empty();
        states.insert("l0-hello".into(), progress(LevelState::Passed));
        steps.insert("l0-hello:pass".into());
        // 失败 9 次后通过 → 不触发
        let ids = check(&states, &steps, 1, &seen, 1, &already, Some(("l0-hello", 9, true)));
        assert!(!ids.contains(&"never_give_up".to_string()));
        // 失败 10 次后通过 → 触发
        let ids = check(&states, &steps, 1, &seen, 1, &already, Some(("l0-hello", 10, true)));
        assert!(ids.contains(&"never_give_up".to_string()));
    }

    #[test]
    fn champion_requires_all_levels_passed() {
        let (mut states, steps, seen, already) = empty();
        // 2 关总关卡：过 1 关不触发
        states.insert("l0-hello".into(), progress(LevelState::Passed));
        let ids = check(&states, &steps, 1, &seen, 2, &already, None);
        assert!(!ids.contains(&"champion".to_string()));
        // 全过 → 触发
        states.insert("l1-move".into(), progress(LevelState::Passed));
        let ids = check(&states, &steps, 1, &seen, 2, &already, None);
        assert!(ids.contains(&"champion".to_string()));
    }

    #[test]
    fn already_unlocked_not_repeated() {
        // HashSet 幂等：已入账的成就不再出现在新解锁列表
        let (mut states, mut steps, seen, _) = empty();
        for b in BOSS_LEVEL_IDS {
            states.insert(b.to_string(), progress(LevelState::Passed));
        }
        states.insert("l0-hello".into(), progress(LevelState::Passed));
        steps.insert("l0-hello:pass".into());
        let already: HashSet<String> = ["first_steps", "boss_all", "champion", "combo_10"]
            .into_iter()
            .map(String::from)
            .collect();
        let ids = check(&states, &steps, 10, &seen, 5, &already, None);
        assert!(!ids.contains(&"first_steps".to_string()));
        assert!(!ids.contains(&"boss_all".to_string()));
        assert!(!ids.contains(&"champion".to_string()));
        assert!(!ids.contains(&"combo_10".to_string()));
        // 未入账的仍在列表（boss_slayer / combo_5）
        assert!(ids.contains(&"boss_slayer".to_string()));
        assert!(ids.contains(&"combo_5".to_string()));
    }

    #[test]
    fn achievement_table_has_ten_entries_with_names() {
        assert_eq!(ACHIEVEMENTS.len(), 10);
        for (id, name) in ACHIEVEMENTS {
            assert_eq!(achievement_name(id), Some(name));
            assert!(!id.is_empty() && !name.is_empty());
        }
        assert_eq!(achievement_name("nope"), None);
        assert!(is_boss_level_id("l1-clone"));
        assert!(!is_boss_level_id("l0-hello"));
    }
}
