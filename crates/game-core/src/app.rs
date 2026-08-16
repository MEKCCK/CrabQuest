use crate::engine::{Engine, XP_PER_PASS};
use crate::error::GameError;
use crate::level::Level;
use crate::save::{LevelState, SaveData};
use crate::validate::Validation;

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
    pub xp: u32,
    pub combo: u32,
    pub total: usize,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct FeedbackData {
    pub passed: bool,
    pub level_id: String,
    pub feedback: Vec<String>,
    pub xp_gained: u32,
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
            xp: self.engine.save.xp,
            combo: self.engine.save.combo,
            total: self.engine.level_set.len(),
            index,
            level,
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
                    Validation::Pass => {
                        self.screen = Screen::Feedback(FeedbackData {
                            passed: true,
                            level_id: d.level.id.clone(),
                            feedback: Vec::new(),
                            xp_gained: XP_PER_PASS,
                        });
                    }
                    Validation::Fail { feedback } => {
                        self.screen = Screen::Feedback(FeedbackData {
                            passed: false,
                            level_id: d.level.id.clone(),
                            feedback,
                            xp_gained: 0,
                        });
                    }
                }
            }
            Input::Hint => {
                if let Screen::Level(cur) = &mut self.screen {
                    cur.show_hint = !cur.show_hint;
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
                    self.screen = Screen::Level(prev);
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
                assert_eq!(f.xp_gained, XP_PER_PASS);
            }
            other => panic!("expected Feedback, got {:?}", other),
        }
        // 回车 → 自动进入下一关
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.level.id, "l1-move"),
            other => panic!("expected Level l1-move, got {:?}", other),
        }
    }

    #[test]
    fn fail_keeps_code_and_returns_to_level() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        // 写错代码：输出不符
        a.set_code("fn main() { println!(\"wrong\"); }".into());
        a.handle(Input::Submit).unwrap();
        match a.screen() {
            Screen::Feedback(f) => {
                assert!(!f.passed);
                assert!(!f.feedback.is_empty());
            }
            other => panic!("expected Feedback fail, got {:?}", other),
        }
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"wrong\"); }"), // 代码保留
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
