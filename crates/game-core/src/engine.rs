use crate::error::GameError;
use crate::level::{Level, LevelSet};
use crate::sandbox::{CompileOutcome, Sandbox};
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
        if level.kind == "quiz" {
            return Err(GameError::LevelKindMismatch(
                "该关卡是选择题（kind=quiz），请通过 submit_quiz 提交选项".into(),
            ));
        }

        let result = validate(&level, code, &self.mapper, self.sandbox.as_ref())?;

        match &result {
            Validation::Pass => self.record_pass(&level, idx),
            Validation::Fail { .. } => self.record_fail(&level),
        }
        Ok(result)
    }

    /// 选择题（kind=quiz）提交：不提交代码，提交选项索引（0-based）。
    /// 提交前校验展示代码可编译；answer 与 answer_index 相等即通关，
    /// 通关/失败的 XP/combo/状态记账与普通关完全一致。
    pub fn submit_quiz(&mut self, answer: u32) -> Result<Validation, GameError> {
        let idx = self
            .current
            .ok_or_else(|| GameError::LevelNotFound("无当前关卡".into()))?;
        let level = self
            .level_set
            .levels
            .get(idx)
            .cloned()
            .ok_or_else(|| GameError::LevelNotFound(format!("index {idx}")))?;
        if level.kind != "quiz" {
            return Err(GameError::LevelKindMismatch(
                "该关卡不是选择题（kind=code），请通过 submit 提交代码".into(),
            ));
        }
        let n = level.options.len();
        if (answer as usize) >= n {
            return Err(GameError::QuizAnswerOutOfRange { index: answer, len: n });
        }
        // 展示代码须可编译（加载期不编译，提交前实测一次）
        match self.sandbox.compile(&level.starter_code)? {
            CompileOutcome::Failed { .. } => {
                return Err(GameError::LevelDataInvalid(
                    level.id.clone(),
                    "quiz 关展示代码无法编译".into(),
                ));
            }
            CompileOutcome::Success { .. } => {}
        }
        if level.answer_index == Some(answer) {
            self.record_pass(&level, idx);
            Ok(Validation::Pass)
        } else {
            self.record_fail(&level);
            Ok(Validation::Fail {
                feedback: vec![format!(
                    "回答错误：输出与你选择的选项「{}」不符。再读一遍展示代码，必要时查看提示。",
                    level.options[answer as usize]
                )],
            })
        }
    }

    /// 通关记账：XP/combo/max_combo/状态/完成时间 + 解锁下一关（普通关与 quiz 关共用）
    fn record_pass(&mut self, level: &Level, idx: usize) {
        self.save.xp += XP_PER_PASS;
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

    /// 失败记账：combo 清零/错误计数/attempts 递增（普通关与 quiz 关共用）
    fn record_fail(&mut self, level: &Level) {
        self.save.combo = 0;
        self.save.total_errors += 1;
        let entry = self
            .save
            .level_states
            .entry(level.id.clone())
            .or_insert_with(LevelProgress::default);
        entry.attempts += 1;
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

    #[test]
    fn expect_panic_level_submit_loop() {
        // expect_panic 关：触发指定 panic → 通关记账（XP/状态）；未触发 → 失败计错
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"l2-panics\"\ntitle = \"制造越界 panic\"\ntier = \"l2\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nexpect_panic = \"index out of bounds\"\nsource = \"自编\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        e.new_game();
        e.start_level(0).unwrap();
        // 失败：编译通过但未 panic
        let ok = "fn main() { println!(\"fine\"); }";
        assert!(matches!(e.submit(ok).unwrap(), Validation::Fail { .. }));
        assert_eq!(e.save.total_errors, 1);
        assert_eq!(e.save.level_states.get("l2-panics").unwrap().attempts, 1);
        // 通过：触发 index out of bounds
        let panicking = "fn main() { let v = vec![1, 2, 3]; println!(\"{}\", v[3]); }";
        assert_eq!(e.submit(panicking).unwrap(), Validation::Pass);
        assert_eq!(e.save.xp, XP_PER_PASS);
        assert_eq!(e.save.level_states.get("l2-panics").unwrap().state, LevelState::Passed);
    }

    /// 013-mutable-zst：零大小类型（ZST），两个可变引用可指向同一地址，指针比较相等 → 输出 1
    const QUIZ_ZST_CODE: &str = r#"struct S;

fn main() {
    let [x, y] = &mut [S, S];
    let eq = x as *mut S == y as *mut S;
    print!("{}", eq as u8);
}
"#;

    fn quiz_engine() -> Engine {
        let set = LevelSet {
            levels: parse_levels(&format!(
                r#"
[[level]]
id = "l0-hello"
title = "hello"
tier = "l0"
description = "d"
starter_code = "fn main() {{ x = 5; println!(\"x has the value {{}}\", x); }}"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l4-mutable-zst"
title = "可变零大小类型"
tier = "l4"
kind = "quiz"
description = "d"
starter_code = '''{starter}'''
options = ["0", "1", "编译错误", "不确定"]
answer_index = 1
source = "rust-quiz (questions/013-mutable-zst.rs, CC BY-SA 4.0，解释自写)"
"#,
                starter = QUIZ_ZST_CODE
            ))
            .unwrap(),
        };
        Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()))
    }

    #[test]
    fn quiz_correct_answer_passes_with_xp_and_unlocks_next() {
        let mut e = quiz_engine();
        e.new_game();
        // 先通过第一关解锁 quiz 关
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        assert_eq!(e.save.xp, XP_PER_PASS);
        e.start_level(1).unwrap();
        assert_eq!(e.submit_quiz(1).unwrap(), Validation::Pass);
        assert_eq!(e.save.xp, XP_PER_PASS * 2);
        assert_eq!(e.save.combo, 2);
        assert_eq!(e.save.max_combo, 2);
        let p = e.save.level_states.get("l4-mutable-zst").unwrap();
        assert_eq!(p.state, LevelState::Passed);
        assert_eq!(p.attempts, 1);
        assert!(p.completed_at.is_some());
        assert_eq!(e.save.total_errors, 0, "选对不应计入错误");
    }

    #[test]
    fn quiz_wrong_answer_fails_resets_combo_and_counts_error() {
        let mut e = quiz_engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        assert_eq!(e.save.combo, 1);
        e.start_level(1).unwrap();
        let res = e.submit_quiz(0).unwrap();
        assert!(matches!(res, Validation::Fail { .. }));
        match res {
            Validation::Fail { feedback } => assert!(
                feedback[0].contains("回答错误"),
                "失败反馈应提示选错: {}",
                feedback[0]
            ),
            _ => unreachable!(),
        }
        assert_eq!(e.save.combo, 0);
        assert_eq!(e.save.total_errors, 1);
        let p = e.save.level_states.get("l4-mutable-zst").unwrap();
        assert_eq!(p.attempts, 1);
        assert_eq!(p.state, LevelState::Unlocked, "选错不改变关卡状态");
        assert!(p.completed_at.is_none());
    }

    #[test]
    fn quiz_out_of_range_answer_rejected() {
        let mut e = quiz_engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        e.start_level(1).unwrap();
        for bad in [4, 99] {
            assert!(
                matches!(e.submit_quiz(bad), Err(GameError::QuizAnswerOutOfRange { index, len }) if index == bad && len == 4),
                "越界选项 {bad} 应被拒绝"
            );
        }
        // 越界提交不记账
        assert_eq!(e.save.total_errors, 0);
        assert_eq!(e.save.level_states.get("l4-mutable-zst").unwrap().attempts, 0);
    }

    #[test]
    fn quiz_level_rejects_code_submit() {
        let mut e = quiz_engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        e.start_level(1).unwrap();
        assert!(matches!(
            e.submit("fn main() { println!(\"bypass\"); }"),
            Err(GameError::LevelKindMismatch(_))
        ));
    }

    #[test]
    fn submit_quiz_on_code_level_rejected() {
        let mut e = quiz_engine();
        e.new_game();
        e.start_level(0).unwrap();
        assert!(matches!(
            e.submit_quiz(0),
            Err(GameError::LevelKindMismatch(_))
        ));
    }
}
