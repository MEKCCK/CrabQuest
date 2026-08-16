use crate::engine::{hint_unlock_state, Engine};
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
    /// P2-08：复习关卡说明回血（engine.review_lore）
    ReviewLore,
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
    /// P4-26：自定义章节起始下标（= 内置关卡数）。`entries[..custom_start]` 为内置章节，
    /// `entries[custom_start..]` 为「自定义关卡」独立章节；无自定义关卡时 == entries.len()。
    pub custom_start: usize,
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
    /// P2-11：参考答案（最后一条 hint = 解法级修复代码）是否已二次确认展示
    pub reference_revealed: bool,
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
    /// P3-19：错误卡「第 N 行」点击 → 编辑器光标跳转目标（0-based 行号）。
    /// UI 在应用到 TextEdit 光标后调用 `take_focus_line` 清除（一次性，避免重渲染反复跳回）。
    pub focus_line: Option<usize>,
    /// P4-26：自定义关卡加载失败列表（已格式化为「自定义关卡 X 加载失败：原因」）。
    /// 启动日志已打印；地图页顶部以警示框呈现（游戏内提示）。
    pub custom_load_errors: Vec<String>,
}

impl GameApp {
    pub fn new(engine: Engine) -> Self {
        Self::with_custom_load_errors(engine, Vec::new())
    }

    /// P4-26：携带自定义关卡加载错误构造应用（游戏内提示；无错误时等价于 `new`）。
    pub fn with_custom_load_errors(mut engine: Engine, custom_load_errors: Vec<String>) -> Self {
        engine.unlock_first();
        let screen = Self::build_map(&engine, 0);
        Self { engine, screen, last_level: None, focus_line: None, custom_load_errors }
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
        Screen::ChapterMap(ChapterMapData {
            selected,
            entries,
            custom_start: engine.builtin_count,
        })
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
            reference_revealed: false,
        };
        self.last_level = Some(d.clone());
        // P3-19：新进入关卡清除上次跳转目标（不同关的行号无意义）
        self.focus_line = None;
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

    /// P3-19：错误卡「第 N 行」点击 → 记录跳转目标（0-based 行）并切回编辑器屏幕。
    /// `one_based` 为 ErrorCard.line（rustc 1-based 行号）；越界行由编辑器端钳制到文件末尾。
    /// 已在编辑器内（底部固定面板）时仅更新目标，不改屏幕；在 Feedback 屏时按
    /// handle_feedback 的返回路径重建 Level 并保留反馈面板（底部固定不消失）。
    pub fn jump_to_line(&mut self, one_based: u32) {
        self.focus_line = Some(one_based.saturating_sub(1) as usize);
        if let Screen::Feedback(f) = self.screen.clone() {
            if let Some(mut prev) = self.last_level.clone() {
                prev.feedback = Some(f);
                self.screen = Screen::Level(prev);
            }
        }
    }

    /// P3-19：UI 在把光标应用到 TextEdit 后取走跳转目标（一次性：
    /// 清除后重渲染不会反复跳回同一行）。
    pub fn take_focus_line(&mut self) -> Option<usize> {
        self.focus_line.take()
    }

    /// P2-11：二次确认「查看答案」→ 展示参考答案（最后一条 hint = 解法级修复代码）
    /// 并记录查看（engine.reveal_hint，幂等；零成本：不扣心/XP、不改 fail_count）。
    /// 拒绝路径不发此调用：弹窗关闭且不展示任何代码。
    pub fn reveal_reference(&mut self) {
        if let Screen::Level(d) = &mut self.screen {
            d.reference_revealed = true;
            if let Some(last) = d.level.hints.len().checked_sub(1) {
                self.engine.reveal_hint(&d.level.id, last as u32);
            }
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
                        // P4-26：下一关标题只在同章节内取（内置末尾/自定义末尾 → None）
                        let unlocked_next = self
                            .engine
                            .next_in_chapter(d.index)
                            .and_then(|n| self.engine.level_set.levels.get(n))
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
                    } else if !cur.level.hint_unlock.is_empty() {
                        // P2-11 联动模式：失败联动自动推进替代手动逐级——按键只开关面板；
                        // 打开面板 = 查看当前自动展开的提示（记录 hints_used，幂等；零成本）
                        cur.show_hint = !cur.show_hint;
                        if cur.show_hint {
                            let fail_count = self
                                .engine
                                .save
                                .level_states
                                .get(&cur.level.id)
                                .map(|p| p.fail_count)
                                .unwrap_or(0);
                            if let Some(st) = hint_unlock_state(
                                cur.level.hints.len(),
                                &cur.level.hint_unlock,
                                fail_count,
                            ) {
                                self.engine.reveal_hint(&cur.level.id, st.expanded as u32);
                            }
                        }
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
                // P2-09：查看 hint 计为活跃行为（同日幂等）
                self.engine.touch_activity();
            }
            Input::ReviewLore => {
                if let Screen::Level(cur) = &mut self.screen {
                    // 复习回血（引擎层幂等：每关每局一次）；成功后刷新反馈面板心数
                    self.engine.review_lore(&cur.level.id);
                    if let Some(fb) = &mut cur.feedback {
                        fb.hearts = self.engine.save.hearts;
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
                    // P4-26：同章节内推进（内置末尾/自定义末尾 → 回地图，不跨章节）
                    match self.engine.next_in_chapter(idx) {
                        Some(next) => {
                            self.engine.start_level(next)?;
                            self.screen = self.build_level(next)?;
                        }
                        None => {
                            self.screen = Self::build_map(&self.engine, 0);
                        }
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
                assert_eq!(f.hearts, 4, "初始 3 心 + 通关回血 1 → 4");
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

    // ===== P2-08/09/10：hearts / streak / achievements（app 层接线）=====

    #[test]
    fn review_lore_from_level_heals_once() {
        let mut a = app();
        a.handle(Input::Enter).unwrap(); // 进入第一关
        // 失败一次 → 3 → 2
        a.set_code("fn main() { println!(\"wrong\"); }".into());
        a.handle(Input::Submit).unwrap();
        assert_eq!(a.engine.save.hearts, 2);
        a.handle(Input::Enter).unwrap(); // 回编辑（带反馈面板）
        assert_eq!(a.engine.save.hearts, 2);
        // 复习回血 → 3，且反馈面板心数同步刷新
        a.handle(Input::ReviewLore).unwrap();
        assert_eq!(a.engine.save.hearts, 3);
        match a.screen() {
            Screen::Level(d) => {
                assert!(d.feedback.is_some(), "返回编辑后反馈面板保留");
                assert_eq!(d.feedback.as_ref().unwrap().hearts, 3, "面板心数应刷新");
            }
            other => panic!("expected Level, got {:?}", other),
        }
        // 幂等：再次复习不回血
        a.handle(Input::ReviewLore).unwrap();
        assert_eq!(a.engine.save.hearts, 3);
        assert!(a.engine.save.completed_steps.contains("l0-hello:lore"));
    }

    #[test]
    fn hint_press_counts_as_activity() {
        let mut a = app();
        assert_eq!(a.engine.save.streak_days, 0);
        a.handle(Input::Enter).unwrap(); // 进入关卡
        a.handle(Input::Hint).unwrap();
        assert_eq!(a.engine.save.streak_days, 1, "查看 hint 计为活跃");
        assert!(a.engine.save.last_played_date.is_some());
        // 同日再次 hint → 幂等
        a.handle(Input::Hint).unwrap();
        assert_eq!(a.engine.save.streak_days, 1);
    }

    #[test]
    fn submit_blocked_at_zero_hearts() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        a.engine.save.hearts = 0;
        let result = a.handle(Input::Submit);
        assert!(matches!(result, Err(GameError::NoHearts)));
        // 编辑仍可用（0 心不禁编辑）
        a.set_code("fn main() { println!(\"edited\"); }".into());
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"edited\"); }"),
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

    // ===== P2-11：hint 失败联动（app 层接线）=====

    const LEVELS_HINTS_UNLOCK: &str = r#"
[[level]]
id = "h-unlock"
title = "unlock-hint"
tier = "l1"
description = "d"
hints = ["第一级提示", "第二级提示", "第三级提示"]
hint_unlock = [1, 3, 5]
starter_code = "fn main() { println!(\"x has the value {}\", 5); }"
expect_output = "x has the value 5"
source = "rustlings"
"#;

    fn unlock_hint_app() -> GameApp {
        let set = LevelSet { levels: parse_levels(LEVELS_HINTS_UNLOCK).unwrap() };
        let engine = Engine::new(set, Default::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        GameApp::new(engine)
    }

    fn set_fail_count(a: &mut GameApp, level_id: &str, fail_count: u32) {
        a.engine
            .save
            .level_states
            .entry(level_id.into())
            .or_default()
            .fail_count = fail_count;
    }

    #[test]
    fn hint_input_unlock_mode_toggles_and_records_expanded() {
        let mut a = unlock_hint_app();
        a.handle(Input::Enter).unwrap(); // 进入关卡
        // fc=1：expanded=0 → 打开面板记录 hint[0]
        set_fail_count(&mut a, "h-unlock", 1);
        a.handle(Input::Hint).unwrap();
        let d = level_screen(&a);
        assert!(d.show_hint, "联动模式按键打开面板");
        assert_eq!(
            a.engine.save.level_states.get("h-unlock").unwrap().hints_used,
            vec![0],
            "打开面板 = 查看当前自动展开的提示"
        );
        // 再按 → 关闭面板
        a.handle(Input::Hint).unwrap();
        assert!(!level_screen(&a).show_hint);
        // 重复打开：幂等（已记录不重复）
        a.handle(Input::Hint).unwrap();
        assert_eq!(a.engine.save.level_states.get("h-unlock").unwrap().hints_used, vec![0]);
    }

    #[test]
    fn hint_input_unlock_mode_high_fail_records_last_hint() {
        let mut a = unlock_hint_app();
        a.handle(Input::Enter).unwrap();
        // fc=4：expanded=最后一条（hint[2]）→ 打开面板记录 hint[2]
        set_fail_count(&mut a, "h-unlock", 4);
        a.handle(Input::Hint).unwrap();
        assert_eq!(
            a.engine.save.level_states.get("h-unlock").unwrap().hints_used,
            vec![2],
            "fc≥4 打开面板记录最后一条"
        );
    }

    #[test]
    fn hint_input_manual_stepping_unchanged_without_hint_unlock() {
        // 回归：无 hint_unlock 时手动逐级行为与 P1 完全一致（hint_step 步进）
        let mut a = hint_app();
        a.handle(Input::Enter).unwrap();
        a.handle(Input::Hint).unwrap();
        assert_eq!(level_screen(&a).visible_hint(), Some(("第一级提示", 1, 3)));
        a.handle(Input::Hint).unwrap();
        assert_eq!(level_screen(&a).visible_hint(), Some(("第二级提示", 2, 3)));
        a.handle(Input::Hint).unwrap();
        assert_eq!(level_screen(&a).visible_hint(), Some(("第三级提示", 3, 3)));
        a.handle(Input::Hint).unwrap();
        assert!(!level_screen(&a).show_hint);
        assert!(a.engine.save.level_states.get("h-multi").map(|p| p.hints_used.is_empty()).unwrap_or(true),
            "手动模式不写 hints_used（保持 P1 行为，不影响 no_hint_perfect）");
    }

    #[test]
    fn reveal_reference_shows_last_hint_and_records_view() {
        let mut a = unlock_hint_app();
        a.handle(Input::Enter).unwrap();
        set_fail_count(&mut a, "h-unlock", 4);
        // 确认前：未展示、未记录
        assert!(!level_screen(&a).reference_revealed);
        a.reveal_reference();
        let d = level_screen(&a);
        assert!(d.reference_revealed, "确认后标记展示参考答案");
        assert_eq!(
            a.engine.save.level_states.get("h-unlock").unwrap().hints_used,
            vec![2],
            "参考答案 = 最后一条 hint（解法级修复代码）"
        );
        // 幂等：重复确认不重复记录
        a.reveal_reference();
        assert_eq!(a.engine.save.level_states.get("h-unlock").unwrap().hints_used, vec![2]);
    }

    #[test]
    fn reveal_reference_zero_cost() {
        let mut a = unlock_hint_app();
        a.handle(Input::Enter).unwrap();
        set_fail_count(&mut a, "h-unlock", 4);
        let hearts = a.engine.save.hearts;
        let xp = a.engine.save.xp;
        a.reveal_reference();
        assert_eq!(a.engine.save.hearts, hearts, "参考答案不扣心");
        assert_eq!(a.engine.save.xp, xp, "参考答案不扣 XP");
        assert_eq!(a.engine.save.level_states.get("h-unlock").unwrap().fail_count, 4, "fail_count 不变");
    }

    // ===== P3-19：行号跳转编辑器（app 层状态）=====

    #[test]
    fn line_click_returns_to_level_with_focus_and_panel() {
        let mut a = app();
        a.handle(Input::Enter).unwrap(); // 进入第一关
        // 制造失败反馈
        a.set_code("fn main() { println!(\"wrong\"); }".into());
        a.handle(Input::Submit).unwrap();
        assert!(matches!(a.screen(), Screen::Feedback(_)));
        // 点击「第 4 行」→ 回到编辑器 + 光标目标 0-based 3 + 反馈面板保留
        a.jump_to_line(4);
        assert_eq!(a.focus_line, Some(3), "1-based 行号转 0-based");
        match a.screen() {
            Screen::Level(d) => {
                assert!(d.feedback.is_some(), "跳转后面板保留（底部固定）");
                assert_eq!(d.code, "fn main() { println!(\"wrong\"); }", "代码保留");
            }
            other => panic!("expected Level, got {other:?}"),
        }
    }

    #[test]
    fn line_click_in_editor_sets_focus_only() {
        let mut a = app();
        a.handle(Input::Enter).unwrap(); // 已在编辑器
        a.jump_to_line(5);
        assert_eq!(a.focus_line, Some(4));
        assert!(matches!(a.screen(), Screen::Level(_)), "编辑器内点击不切屏");
    }

    #[test]
    fn line_none_not_clickable_no_state_change() {
        // line=None（EUNKNOWN 无 --> 行）：不设置任何跳转状态（UI 侧不渲染可点击行号）
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        // 模拟 UI 只在 card.line.is_some() 时调用 jump_to_line——这里验证零调用路径等价
        assert_eq!(a.focus_line, None);
        assert!(matches!(a.screen(), Screen::Level(_)));
    }

    #[test]
    fn consecutive_line_clicks_switch_focus() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        a.jump_to_line(2); // 第 2 行
        assert_eq!(a.focus_line, Some(1));
        a.jump_to_line(9); // 第 9 行
        assert_eq!(a.focus_line, Some(8), "连续点击不同错误 → 光标目标切换");
        // 越界行（超出代码行数）：目标仍记录，编辑器端钳制到文件末尾
        a.jump_to_line(u32::MAX);
        assert_eq!(a.focus_line, Some(u32::MAX as usize - 1));
    }

    #[test]
    fn take_focus_line_clears_after_apply() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        a.jump_to_line(3);
        assert_eq!(a.take_focus_line(), Some(2), "UI 应用后取走目标");
        assert_eq!(a.focus_line, None, "一次性：清除后不反复跳回");
        assert_eq!(a.take_focus_line(), None);
    }

    #[test]
    fn build_level_resets_focus_line_and_reference() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        a.focus_line = Some(5);
        a.handle(Input::Esc).unwrap(); // 回地图
        a.handle(Input::Enter).unwrap(); // 重进关卡（build_level）
        assert_eq!(a.focus_line, None, "重进关卡清除跳转目标");
        match a.screen() {
            Screen::Level(d) => assert!(!d.reference_revealed, "重进关卡参考答案未确认"),
            other => panic!("expected Level, got {other:?}"),
        }
    }

    // ===== P4-26：自定义关卡章节（地图分区 + 游玩闭环）=====

    const CUSTOM_LEVEL_TOML: &str = r#"
[[level]]
id = "c1-hello"
title = "自定义·你好"
tier = "l0"
description = "d"
starter_code = "fn main() { println!(\"custom ok\"); }"
expect_output = "custom ok"
source = "community"
"#;

    fn custom_app() -> GameApp {
        let builtin = LevelSet { levels: parse_levels(LEVELS).unwrap() };
        let custom = parse_levels(CUSTOM_LEVEL_TOML).unwrap();
        let engine = Engine::with_custom_levels(
            builtin,
            custom,
            Default::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        GameApp::new(engine)
    }

    #[test]
    fn map_splits_builtin_and_custom_chapters() {
        let a = custom_app();
        match a.screen() {
            Screen::ChapterMap(m) => {
                assert_eq!(m.entries.len(), 3);
                assert_eq!(m.custom_start, 2, "内置 2 关在前，自定义从下标 2 开始");
                assert_eq!(m.entries[2].level.id, "c1-hello");
                // 自定义关卡无存档时显示为可挑战（Unlocked），不是未解锁
                assert_eq!(m.entries[2].state, LevelState::Unlocked);
            }
            other => panic!("expected ChapterMap, got {other:?}"),
        }
    }

    #[test]
    fn custom_level_plays_compile_compare_save_roundtrip() {
        let mut a = custom_app();
        // 从地图导航到自定义关卡（Down×2 → Enter）
        a.handle(Input::Down).unwrap();
        a.handle(Input::Down).unwrap();
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => {
                assert_eq!(d.level.id, "c1-hello");
                assert_eq!(d.index, 2, "自定义关卡全局索引 2");
            }
            other => panic!("expected Level c1-hello, got {other:?}"),
        }
        // 编译 + 输出比对：正确代码直接通过
        a.handle(Input::Submit).unwrap();
        match a.screen() {
            Screen::Feedback(f) => {
                assert!(f.passed, "自定义关卡输出比对应通过");
                assert!(f.errors.is_empty() && f.expectation.is_none() && f.panic.is_none());
            }
            other => panic!("expected Feedback, got {other:?}"),
        }
        // 存档落盘 + 回读：自定义进度保留（存档隔离的「可存」侧）
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("save.toml");
        crate::save::save(a.save_ref(), &p).unwrap();
        let loaded = crate::save::load(&p).unwrap();
        assert_eq!(loaded.level_states.get("c1-hello").unwrap().state, LevelState::Passed);
        // 成就/rank 侧不触发
        assert!(a.engine.save.achievements.is_empty(), "自定义通关不触发成就");
        assert_eq!(a.engine.builtin_completed_count(), 0);
        // 通关后 Enter：自定义章节末尾 → 回地图（不越界）
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::ChapterMap(m) => {
                assert_eq!(m.entries[2].state, LevelState::Passed, "回地图后自定义关显示已通关");
            }
            other => panic!("expected ChapterMap, got {other:?}"),
        }
    }

    #[test]
    fn custom_load_errors_carried_into_app() {
        let builtin = LevelSet { levels: parse_levels(LEVELS).unwrap() };
        let engine = Engine::new(
            builtin,
            Default::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        let errs = vec!["自定义关卡 bad.toml 加载失败：TOML 解析失败：xxx".to_string()];
        let a = GameApp::with_custom_load_errors(engine, errs.clone());
        assert_eq!(a.custom_load_errors, errs);
        // 无错误时 new 等价于空错误列表
        let plain = GameApp::new(Engine::new(
            LevelSet { levels: parse_levels(LEVELS).unwrap() },
            Default::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        ));
        assert!(plain.custom_load_errors.is_empty());
    }
}
