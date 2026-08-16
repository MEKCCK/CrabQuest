use crate::engine::Engine;
use crate::error::GameError;
use crate::level::Level;
use crate::save::{LevelState, SaveData};
use crate::validate::{ErrorCard, OutputDiff, Validation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Enter,
    Esc,
    Submit,
    Hint,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameFlow {
    Continue,
    Quit,
}

#[derive(Debug, Clone)]
pub struct MenuData {
    pub selected: usize,
    pub can_continue: bool,
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub level: Level,
    pub state: LevelState,
}

#[derive(Debug, Clone)]
pub struct ChapterMapData {
    pub selected: usize,
    pub entries: Vec<MapEntry>,
}

#[derive(Debug, Clone)]
pub struct LevelData {
    pub level: Level,
    pub code: String,
    pub show_hint: bool,
    /// 多级提示当前展示到第几条（0-based；hints 为空时恒为 0）
    pub hint_step: usize,
    pub xp: u32,
    pub combo: u32,
    pub total: usize,
    pub index: usize,
    /// P1-03：返回编辑后底部固定的反馈面板（上次提交失败时 Some，底部固定保留）
    pub feedback: Option<FeedbackData>,
}

impl LevelData {
    /// 当前应显示的提示：hints 数组优先，为空回退单条 hint 字段。
    /// 返回 (文本, 当前第几条, 总条数)；未显示或两者皆空时返回 None。
    pub fn visible_hint(&self) -> Option<(&str, usize, usize)> {
        if !self.show_hint {
            return None;
        }
        if !self.level.hints.is_empty() {
            let idx = self.hint_step.min(self.level.hints.len() - 1);
            Some((&self.level.hints[idx], idx + 1, self.level.hints.len()))
        } else if !self.level.hint.is_empty() {
            Some((&self.level.hint, 1, 1))
        } else {
            None
        }
    }
}

/// 结构化反馈面板数据（P1-03 v3 §7.7）：失败分支按
/// errors（编译错误卡片）/ expectation（输出不符 diff）/ panic（运行崩溃）三选一填充。
#[derive(Debug, Clone)]
pub struct FeedbackData {
    pub passed: bool,
    pub level_id: String,
    pub xp_gained: u32,
    pub combo: u32,
    pub hearts: u32,
    pub errors: Vec<ErrorCard>,
    pub expectation: Option<OutputDiff>,
    /// panic 分支合成串：「❗ 程序运行崩溃（分类）\n净化消息」（UI 拆首行为标题，其余折叠）
    pub panic: Option<String>,
    pub unlocked_next: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Screen {
    Menu(MenuData),
    ChapterMap(ChapterMapData),
    Level(LevelData),
    Feedback(FeedbackData),
}

pub struct GameApp {
    pub engine: Engine,
    screen: Screen,
    last_level: Option<LevelData>,
}

impl GameApp {
    pub fn new(mut engine: Engine) -> Self {
        engine.unlock_first();
        let screen = Self::build_map(&engine, 0);
        Self { engine, screen, last_level: None }
    }

    fn build_map(engine: &Engine, selected: usize) -> Screen {
        let entries = engine
            .level_set
            .levels
            .iter()
            .map(|l| {
                let state = engine
                    .save
                    .level_states
                    .get(&l.id)
                    .map(|p| p.state)
                    .unwrap_or(LevelState::Locked);
                MapEntry { level: l.clone(), state }
            })
            .collect();
        Screen::ChapterMap(ChapterMapData { selected, entries })
    }

    fn build_menu(engine: &Engine) -> Screen {
        Screen::Menu(MenuData { selected: 0, can_continue: engine.can_continue() })
    }

    fn build_level(&mut self, index: usize) -> Result<Screen, GameError> {
        let level = self
            .engine
            .level_set
            .levels
            .get(index)
            .cloned()
            .ok_or_else(|| GameError::LevelNotFound(format!("index {index}")))?;
        let d = LevelData {
            code: level.starter_code.clone(),
            show_hint: false,
            hint_step: 0,
            xp: self.engine.save.xp,
            combo: self.engine.save.combo,
            total: self.engine.level_set.len(),
            index,
            level,
            feedback: None,
        };
        self.last_level = Some(d.clone());
        Ok(Screen::Level(d))
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn save_ref(&self) -> &SaveData {
        &self.engine.save
    }

    pub fn set_code(&mut self, code: String) {
        if let Screen::Level(d) = &mut self.screen {
            d.code = code;
            self.last_level = Some(d.clone());
        }
    }

    pub fn handle(&mut self, input: Input) -> Result<GameFlow, GameError> {
        match self.screen.clone() {
            Screen::Menu(m) => self.handle_menu(m, input),
            Screen::ChapterMap(m) => self.handle_map(m, input),
            Screen::Level(d) => self.handle_level(d, input),
            Screen::Feedback(f) => self.handle_feedback(f, input),
        }
    }

    fn handle_menu(&mut self, m: MenuData, input: Input) -> Result<GameFlow, GameError> {
        match input {
            Input::Up => {
                self.screen = Screen::Menu(MenuData { selected: m.selected.saturating_sub(1), ..m });
            }
            Input::Down => {
                let max = if m.can_continue { 2 } else { 1 };
                self.screen = Screen::Menu(MenuData { selected: (m.selected + 1).min(max), ..m });
            }
            Input::Enter => {
                if m.can_continue {
                    match m.selected {
                        0 => self.screen = Self::build_map(&self.engine, 0),
                        1 => {
                            self.engine.new_game();
                            self.screen = Self::build_map(&self.engine, 0);
                        }
                        _ => return Ok(GameFlow::Quit),
                    }
                } else {
                    match m.selected {
                        0 => {
                            self.engine.new_game();
                            self.screen = Self::build_map(&self.engine, 0);
                        }
                        _ => return Ok(GameFlow::Quit),
                    }
                }
            }
            Input::Esc => return Ok(GameFlow::Quit),
            _ => {}
        }
        Ok(GameFlow::Continue)
    }

    fn handle_map(&mut self, m: ChapterMapData, input: Input) -> Result<GameFlow, GameError> {
        match input {
            Input::Up => {
                self.screen = Screen::ChapterMap(ChapterMapData { selected: m.selected.saturating_sub(1), ..m });
            }
            Input::Down => {
                let max = m.entries.len().saturating_sub(1);
                self.screen = Screen::ChapterMap(ChapterMapData { selected: (m.selected + 1).min(max), ..m });
            }
            Input::Enter => {
                self.engine.start_level(m.selected)?;
                self.screen = self.build_level(m.selected)?;
            }
            Input::Esc => self.screen = Self::build_menu(&self.engine),
            _ => {}
        }
        Ok(GameFlow::Continue)
    }

    fn handle_level(&mut self, d: LevelData, input: Input) -> Result<GameFlow, GameError> {
        match input {
            Input::Submit => {
                let result = self.engine.submit(&d.code)?;
                match result {
                    Validation::Pass { xp_gained } => {
                        let unlocked_next = self
                            .engine
                            .level_set
                            .levels
                            .get(d.index + 1)
                            .map(|l| l.title.clone());
                        self.screen = Screen::Feedback(FeedbackData {
                            passed: true,
                            level_id: d.level.id.clone(),
                            xp_gained,
                            combo: self.engine.save.combo,
                            hearts: self.engine.save.hearts,
                            errors: Vec::new(),
                            expectation: None,
                            panic: None,
                            unlocked_next,
                        });
                    }
                    Validation::Fail { errors, expectation, panic } => {
                        self.screen = Screen::Feedback(FeedbackData {
                            passed: false,
                            level_id: d.level.id.clone(),
                            xp_gained: 0,
                            combo: self.engine.save.combo,
                            hearts: self.engine.save.hearts,
                            errors,
                            expectation,
                            // P1-03：panic 合成「标题\n净化消息」（UI 拆首行为标题，其余折叠）
                            panic: panic.map(|p| format!("❗ 程序运行崩溃（{}）\n{}", p.class_zh, p.message)),
                            unlocked_next: None,
                        });
                    }
                }
            }
            Input::Hint => {
                if let Screen::Level(cur) = &mut self.screen {
                    if cur.level.hints.is_empty() {
                        // 无多级提示：保持原有开关行为
                        cur.show_hint = !cur.show_hint;
                    } else if !cur.show_hint {
                        // 首次按下：显示第一条
                        cur.show_hint = true;
                        cur.hint_step = 0;
                    } else if cur.hint_step + 1 < cur.level.hints.len() {
                        // 逐级揭示下一条
                        cur.hint_step += 1;
                    } else {
                        // 最后一条后再按：关闭提示
                        cur.show_hint = false;
                        cur.hint_step = 0;
                    }
                    self.last_level = Some(cur.clone());
                }
            }
            Input::Reset => {
                if let Screen::Level(cur) = &mut self.screen {
                    cur.code = cur.level.starter_code.clone();
                    self.last_level = Some(cur.clone());
                }
            }
            Input::Esc => self.screen = Self::build_map(&self.engine, 0),
            _ => {}
        }
        Ok(GameFlow::Continue)
    }

    fn handle_feedback(&mut self, f: FeedbackData, input: Input) -> Result<GameFlow, GameError> {
        match input {
            Input::Enter => {
                if f.passed {
                    let idx = self.engine.current.unwrap_or(0);
                    let next = idx + 1;
                    if next < self.engine.level_set.len() {
                        self.engine.start_level(next)?;
                        self.screen = self.build_level(next)?;
                    } else {
                        self.screen = Self::build_map(&self.engine, 0);
                    }
                } else if let Some(prev) = self.last_level.clone() {
                    // P1-03：返回编辑器，反馈面板底部固定保留
                    let mut lvl = prev;
                    lvl.feedback = Some(f);
                    self.screen = Screen::Level(lvl);
                } else {
                    self.screen = Self::build_map(&self.engine, 0);
                }
            }
            Input::Esc => self.screen = Self::build_map(&self.engine, 0),
            _ => {}
        }
        Ok(GameFlow::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::level::{parse_levels, LevelSet};
    use crate::sandbox::DevSandbox;
    use crate::save::LevelState;
    use crate::validate::mapper::ErrorMapper;

    const LEVELS: &str = r#"
[[level]]
id = "l0-hello"
title = "hello"
tier = "l0"
description = "d"
starter_code = "fn main() { println!(\"x has the value {}\", 5); }"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l1-move"
title = "move"
tier = "l1"
description = "d"
starter_code = "fn main() { let s = String::from(\"hi\"); take(s); println!(\"{}\", s); } fn take(x: String) {}"
expect_output = ""
source = "rustlings"
"#;

    fn app() -> GameApp {
        let set = LevelSet { levels: parse_levels(LEVELS).unwrap() };
        let engine = Engine::new(set, Default::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        GameApp::new(engine)
    }

    fn menu_selected(a: &GameApp) -> usize {
        match a.screen() {
            Screen::Menu(m) => m.selected,
            _ => panic!("not menu"),
        }
    }

    #[test]
    fn starts_in_chapter_map() {
        let a = app();
        match a.screen() {
            Screen::ChapterMap(m) => {
                assert_eq!(m.entries.len(), 2);
                assert_eq!(m.entries[0].state, LevelState::Unlocked);
                assert_eq!(m.entries[1].state, LevelState::Locked);
            }
            other => panic!("expected ChapterMap, got {:?}", other),
        }
    }

    #[test]
    fn esc_to_menu_then_quit() {
        let mut a = app();
        assert_eq!(a.handle(Input::Esc).unwrap(), GameFlow::Continue);
        let sel = menu_selected(&a);
        assert_eq!(sel, 0);
        // 无进度：菜单只有 [新游戏][退出]
        assert_eq!(a.handle(Input::Esc).unwrap(), GameFlow::Quit);
    }

    #[test]
    fn enter_level_submit_pass_feedback_then_next() {
        let mut a = app();
        a.handle(Input::Enter).unwrap(); // 进入第一关
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"x has the value {}\", 5); }"),
            other => panic!("expected Level, got {:?}", other),
        }
        a.handle(Input::Submit).unwrap();
        match a.screen() {
            Screen::Feedback(f) => {
                assert!(f.passed);
                // 首通 + 完美（首次提交即通过）→ 25 + 10（engine award_xp 实算值）
                assert_eq!(f.xp_gained, crate::engine::XP_PASS + crate::engine::XP_PERFECT);
                assert_eq!(f.combo, 1, "通过后连击应为 1");
                assert_eq!(f.hearts, 3, "初始心数 3");
                assert_eq!(f.unlocked_next.as_deref(), Some("move"), "应解锁下一关标题");
                assert!(f.errors.is_empty() && f.expectation.is_none() && f.panic.is_none());
            }
            other => panic!("expected Feedback, got {:?}", other),
        }
        // 回车 → 自动进入下一关
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => {
                assert_eq!(d.level.id, "l1-move");
                assert!(d.feedback.is_none(), "新关卡不应携带旧反馈面板");
            }
            other => panic!("expected Level l1-move, got {:?}", other),
        }
    }

    #[test]
    fn fail_keeps_code_and_panel_returns_to_level() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        // 写错代码：输出不符
        a.set_code("fn main() { println!(\"wrong\"); }".into());
        a.handle(Input::Submit).unwrap();
        match a.screen() {
            Screen::Feedback(f) => {
                assert!(!f.passed);
                assert!(f.expectation.is_some(), "输出不符应携带 OutputDiff");
                assert!(f.errors.is_empty() && f.panic.is_none());
                assert_eq!(f.xp_gained, 0);
            }
            other => panic!("expected Feedback fail, got {:?}", other),
        }
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => {
                assert_eq!(d.code, "fn main() { println!(\"wrong\"); }"); // 代码保留
                assert!(d.feedback.is_some(), "返回编辑后反馈面板应保留（底部固定）");
            }
            other => panic!("expected Level, got {:?}", other),
        }
    }

    #[test]
    fn panel_cleared_when_leaving_level_to_map() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        a.set_code("fn main() { println!(\"wrong\"); }".into());
        a.handle(Input::Submit).unwrap();
        a.handle(Input::Enter).unwrap(); // 回编辑（带面板）
        match a.screen() {
            Screen::Level(d) => assert!(d.feedback.is_some()),
            other => panic!("expected Level, got {:?}", other),
        }
        a.handle(Input::Esc).unwrap(); // 回地图
        match a.screen() {
            Screen::ChapterMap(_) => {}
            other => panic!("expected ChapterMap, got {:?}", other),
        }
        // 重新进同一关：面板不应残留（新会话）
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => assert!(d.feedback.is_none(), "重进关卡面板应清除"),
            other => panic!("expected Level, got {:?}", other),
        }
    }

    #[test]
    fn reset_restores_starter_code() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        a.set_code("fn main() { println!(\"whatever\"); }".into());
        a.handle(Input::Reset).unwrap();
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"x has the value {}\", 5); }"),
            other => panic!("expected Level, got {:?}", other),
        }
    }

    #[test]
    fn hint_toggles() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => assert!(!d.show_hint),
            other => panic!("expected Level, got {:?}", other),
        }
        a.handle(Input::Hint).unwrap();
        match a.screen() {
            Screen::Level(d) => assert!(d.show_hint),
            other => panic!("expected Level, got {:?}", other),
        }
    }

    const LEVELS_HINTS: &str = r#"
[[level]]
id = "h-multi"
title = "multi-hint"
tier = "l1"
description = "d"
hint = "兜底提示"
hints = ["第一级提示", "第二级提示", "第三级提示"]
starter_code = "fn main() { println!(\"x has the value {}\", 5); }"
expect_output = "x has the value 5"
source = "rustlings"
"#;

    fn hint_app() -> GameApp {
        let set = LevelSet { levels: parse_levels(LEVELS_HINTS).unwrap() };
        let engine = Engine::new(set, Default::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        GameApp::new(engine)
    }

    fn level_screen(a: &GameApp) -> LevelData {
        match a.screen() {
            Screen::Level(d) => d.clone(),
            other => panic!("expected Level, got {:?}", other),
        }
    }

    #[test]
    fn hint_steps_through_levels_then_closes() {
        let mut a = hint_app();
        a.handle(Input::Enter).unwrap(); // 进入关卡

        // 初始：未显示
        let d = level_screen(&a);
        assert!(!d.show_hint);
        assert_eq!(d.visible_hint(), None);

        // 首次按下 → 第一条
        a.handle(Input::Hint).unwrap();
        let d = level_screen(&a);
        assert_eq!(d.visible_hint(), Some(("第一级提示", 1, 3)));

        // 逐级揭示
        a.handle(Input::Hint).unwrap();
        assert_eq!(level_screen(&a).visible_hint(), Some(("第二级提示", 2, 3)));
        a.handle(Input::Hint).unwrap();
        assert_eq!(level_screen(&a).visible_hint(), Some(("第三级提示", 3, 3)));

        // 最后一条再按 → 关闭
        a.handle(Input::Hint).unwrap();
        let d = level_screen(&a);
        assert!(!d.show_hint);
        assert_eq!(d.visible_hint(), None);
    }

    const LEVELS_SINGLE_HINT: &str = r#"
[[level]]
id = "s-single"
title = "single-hint"
tier = "l1"
description = "d"
hint = "唯一提示"
starter_code = "fn main() { println!(\"x has the value {}\", 5); }"
expect_output = "x has the value 5"
source = "rustlings"
"#;

    fn single_hint_app() -> GameApp {
        let set = LevelSet { levels: parse_levels(LEVELS_SINGLE_HINT).unwrap() };
        let engine = Engine::new(set, Default::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        GameApp::new(engine)
    }

    #[test]
    fn hint_without_hints_toggles_single_hint() {
        let mut a = single_hint_app(); // 只有 hint 字段、无 hints 数组
        a.handle(Input::Enter).unwrap();
        assert_eq!(level_screen(&a).visible_hint(), None);
        a.handle(Input::Hint).unwrap();
        assert_eq!(level_screen(&a).visible_hint(), Some(("唯一提示", 1, 1)));
        a.handle(Input::Hint).unwrap();
        assert_eq!(level_screen(&a).visible_hint(), None);
    }

    #[test]
    fn menu_new_game_flow() {
        let mut a = app();
        // 先通关一关制造进度
        a.handle(Input::Enter).unwrap();
        a.handle(Input::Submit).unwrap();
        a.handle(Input::Enter).unwrap();
        // 回到地图再 Esc 到菜单
        a.handle(Input::Esc).unwrap();
        a.handle(Input::Esc).unwrap();
        assert_eq!(menu_selected(&a), 0);
        // 继续游戏（Enter）回地图
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::ChapterMap(m) => assert!(m.entries[0].state == LevelState::Passed),
            other => panic!("expected ChapterMap, got {:?}", other),
        }
    }
}
