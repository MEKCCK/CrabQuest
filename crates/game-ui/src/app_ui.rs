use egui_macroquad::egui;
use game_core::app::{ChapterMapData, FeedbackData, GameApp, GameFlow, Input, LevelData, MenuData, Screen};
use game_core::editor::{tokenize, TokenKind};
use game_core::error::GameError;
use game_core::level::LevelTier;
use game_core::ui::UiBackend;
use macroquad::prelude::*;

/// JetBrains Maple Mono（内嵌，SIL OFL 许可）——覆盖 CJK 统一表意区，保证中文正常渲染
const MAPLE_FONT: &[u8] = include_bytes!("../assets/JetBrainsMapleMono-Regular.ttf");

/// 把中文字体安装进 egui 字体系统：插入 Proportional / Monospace 家族首位，
/// 中文与拉丁字符都用它渲染，缺失字形（如部分 emoji）回退到 egui 默认字体。
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "jetbrains_maple_mono".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(MAPLE_FONT)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        list.insert(0, "jetbrains_maple_mono".to_owned());
    }
    ctx.set_fonts(fonts);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Busy {
    None,
    Show(Input), // 显示「编译中」一帧
    Do(Input),   // 下一帧真正执行提交
}

pub struct GameUi {
    code_buf: String,
    last_level_id: Option<String>,
    /// 选择题当前选中的选项（0-based；非 quiz 关恒为 None）
    quiz_sel: Option<usize>,
    busy: Busy,
    quit: bool,
}

impl GameUi {
    pub fn new() -> Self {
        Self { code_buf: String::new(), last_level_id: None, quiz_sel: None, busy: Busy::None, quit: false }
    }

    /// 把 code_buf 回写进 app（TextEdit 每次改动后调用）
    fn sync_code(&mut self, app: &mut GameApp) {
        app.set_code(self.code_buf.clone());
    }

    fn act(&mut self, app: &mut GameApp, input: Input) {
        match app.handle(input) {
            Ok(GameFlow::Quit) => self.quit = true,
            Ok(GameFlow::Continue) => {}
            Err(e) => eprintln!("[ui] 错误: {e}"),
        }
    }

    fn key(ctx: &egui::Context, k: egui::Key) -> bool {
        ctx.input(|i| i.key_pressed(k))
    }

    fn draw(&mut self, ctx: &egui::Context, app: &mut GameApp) {
        let screen = app.screen().clone();
        // 进入新关卡时同步 code_buf 与选择题选中态
        if let Screen::Level(d) = &screen {
            if self.last_level_id.as_deref() != Some(d.level.id.as_str()) {
                self.code_buf = d.code.clone();
                self.quiz_sel = None;
                self.last_level_id = Some(d.level.id.clone());
            }
        } else {
            self.last_level_id = None;
        }
        match self.busy {
            Busy::Show(input) => {
                self.draw_busy(ctx);
                self.busy = Busy::Do(input);
                return;
            }
            Busy::Do(input) => {
                self.draw_busy(ctx);
                self.busy = Busy::None;
                self.act(app, input);
                return;
            }
            Busy::None => {}
        }
        match screen {
            Screen::Menu(m) => self.draw_menu(ctx, app, &m),
            Screen::ChapterMap(m) => self.draw_map(ctx, app, &m),
            Screen::Level(d) => self.draw_level(ctx, app, &d),
            Screen::Feedback(f) => self.draw_feedback(ctx, app, &f),
        }
    }

    fn draw_busy(&self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("⏳ 编译中，请稍候...");
                ui.label("（首次编译可能较慢）");
            });
        });
    }

    fn draw_menu(&mut self, ctx: &egui::Context, app: &mut GameApp, m: &MenuData) {
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        if Self::key(ctx, egui::Key::ArrowUp) {
            self.act(app, Input::Up);
        }
        if Self::key(ctx, egui::Key::ArrowDown) {
            self.act(app, Input::Down);
        }
        if Self::key(ctx, egui::Key::Enter) {
            self.act(app, Input::Enter);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.heading(egui::RichText::new("🦀 Rust 学习游戏").size(44.0));
                ui.add_space(8.0);
                ui.label("闯关学 Rust：修好代码，让编译器闭嘴，让程序输出正确结果。");
                ui.add_space(30.0);
                let mut clicked: Option<usize> = None;
                let mut idx = 0;
                if m.can_continue {
                    if ui.selectable_label(m.selected == 0, "▶ 继续游戏").clicked() {
                        clicked = Some(idx);
                    }
                    idx += 1;
                }
                if ui.selectable_label(m.selected == idx, "🆕 新游戏").clicked() {
                    clicked = Some(idx);
                }
                idx += 1;
                if ui.selectable_label(m.selected == idx, "🚪 退出").clicked() {
                    clicked = Some(idx);
                }
                if let Some(target) = clicked {
                    self.select_and_enter(app, target);
                }
                ui.add_space(20.0);
                ui.label(egui::RichText::new("↑↓ 选择 · Enter 确认 · Esc 退出").weak());
            });
        });
    }

    fn select_and_enter(&mut self, app: &mut GameApp, target: usize) {
        loop {
            let cur = match app.screen() {
                Screen::Menu(m) => m.selected,
                _ => break,
            };
            if cur < target {
                self.act(app, Input::Down);
            } else if cur > target {
                self.act(app, Input::Up);
            } else {
                break;
            }
        }
        self.act(app, Input::Enter);
    }

    fn draw_map(&mut self, ctx: &egui::Context, app: &mut GameApp, m: &ChapterMapData) {
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        if Self::key(ctx, egui::Key::ArrowUp) {
            self.act(app, Input::Up);
        }
        if Self::key(ctx, egui::Key::ArrowDown) {
            self.act(app, Input::Down);
        }
        if Self::key(ctx, egui::Key::Enter) {
            self.act(app, Input::Enter);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🗺 关卡地图");
            ui.label("按 L0 → L4 顺序推进，解锁前一关后才能进入下一关。");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut clicked: Option<usize> = None;
                for (i, entry) in m.entries.iter().enumerate() {
                    let (icon, state_str) = match entry.state {
                        game_core::save::LevelState::Passed => ("✅", "已通关"),
                        game_core::save::LevelState::Unlocked => ("🔓", "可挑战"),
                        game_core::save::LevelState::Locked => ("🔒", "未解锁"),
                    };
                    let tier = match entry.level.tier {
                        LevelTier::L0 => "L0 入门",
                        LevelTier::L1 => "L1 所有权",
                        LevelTier::L2 => "L2 集合/错误",
                        LevelTier::L3 => "L3 生命周期/trait",
                        LevelTier::L4 => "L4 挑战",
                    };
                    let text = format!("{icon} {}. {}（{tier}）{state_str}", i + 1, entry.level.title);
                    if ui.selectable_label(m.selected == i, text).clicked() {
                        clicked = Some(i);
                    }
                }
                if let Some(target) = clicked {
                    // 移动选中到 target 再确认
                    loop {
                        let cur = match app.screen() {
                            Screen::ChapterMap(mm) => mm.selected,
                            _ => break,
                        };
                        if cur < target {
                            self.act(app, Input::Down);
                        } else if cur > target {
                            self.act(app, Input::Up);
                        } else {
                            break;
                        }
                    }
                    self.act(app, Input::Enter);
                }
            });
        });
    }

    fn draw_level(&mut self, ctx: &egui::Context, app: &mut GameApp, d: &LevelData) {
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(&d.level.title);
                ui.label(format!(
                    "L{} · {}/{} · XP {} · 连击 {}x",
                    d.level.tier.order(),
                    d.index + 1,
                    d.total,
                    d.xp,
                    d.combo
                ));
            });
            ui.label(&d.level.description);
            if let Some((text, cur, total)) = d.visible_hint() {
                ui.add_space(4.0);
                let label = if total > 1 {
                    format!("💡 提示 {cur}/{total}: {text}")
                } else {
                    format!("💡 {text}")
                };
                ui.colored_label(egui::Color32::from_rgb(255, 200, 80), label);
            }
            ui.separator();
            if d.level.kind == "quiz" {
                self.draw_quiz_body(ui, d);
            } else {
                self.draw_code_body(ui, app);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if d.level.kind == "quiz" {
                    let can_submit = self.quiz_sel.is_some();
                    if ui.add_enabled(can_submit, egui::Button::new("✅ 提交选择")).clicked() {
                        if let Some(sel) = self.quiz_sel {
                            self.busy = Busy::Show(Input::SubmitQuiz(sel as u32));
                        }
                    }
                    if !can_submit {
                        ui.label(egui::RichText::new("请先选择一个选项").weak());
                    }
                } else if ui.button("▶ 提交运行").clicked() {
                    self.busy = Busy::Show(Input::Submit);
                }
                let hint_label = match d.visible_hint() {
                    Some((_, cur, total)) if total > 1 => format!("💡 提示 {cur}/{total}"),
                    _ => "💡 提示".to_owned(),
                };
                if ui.button(hint_label).clicked() {
                    self.act(app, Input::Hint);
                }
                if d.level.kind != "quiz" && ui.button("↺ 重置代码").clicked() {
                    self.act(app, Input::Reset);
                    self.last_level_id = None; // 下一帧重新同步 starter_code
                }
            });
        });
    }

    /// 普通关：可编辑代码编辑器（语法高亮 + 行号 gutter）
    fn draw_code_body(&mut self, ui: &mut egui::Ui, app: &mut GameApp) {
        ui.horizontal(|ui| {
            // 行号 gutter（对齐用 monospace 字体）
            let line_count = self.code_buf.lines().count().max(1);
            let gutter = (1..=line_count).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
            ui.add(
                egui::Label::new(
                    egui::RichText::new(gutter).monospace().color(egui::Color32::from_rgb(120, 120, 120)),
                )
                .selectable(false),
            );
            // 编辑器
            let mut layouter = |ui: &egui::Ui, text: &str, _wrap_width: f32| {
                let mut job = egui::text::LayoutJob::default();
                for span in tokenize(text) {
                    job.append(
                        &text[span.start..span.end],
                        0.0,
                        egui::TextFormat {
                            font_id: egui::FontId::monospace(14.0),
                            color: color_for(span.kind),
                            ..Default::default()
                        },
                    );
                }
                ui.fonts(|f| f.layout_job(job))
            };
            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.code_buf)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(20)
                    .desired_width(f32::INFINITY)
                    .layouter(&mut layouter),
            );
            if resp.changed() {
                self.sync_code(app);
            }
        });
    }

    /// quiz 关：只读展示代码 + 选项列表（玩家不可编辑展示代码）
    fn draw_quiz_body(&mut self, ui: &mut egui::Ui, d: &LevelData) {
        ui.label(egui::RichText::new("📜 展示代码（只读，可选中复制）").weak());
        let line_count = d.level.starter_code.lines().count().max(1);
        let gutter = (1..=line_count).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(gutter).monospace().color(egui::Color32::from_rgb(120, 120, 120)),
                    )
                    .selectable(false),
                );
                ui.add(
                    egui::Label::new(egui::RichText::new(&d.level.starter_code).monospace())
                        .selectable(true),
                );
            });
        });
        ui.add_space(10.0);
        ui.label(egui::RichText::new("🤔 程序运行后会输出什么？").strong());
        for (i, opt) in d.level.options.iter().enumerate() {
            if ui.selectable_label(self.quiz_sel == Some(i), format!("{}. {}", i + 1, opt)).clicked() {
                self.quiz_sel = Some(i);
            }
        }
    }

    fn draw_feedback(&mut self, ctx: &egui::Context, app: &mut GameApp, f: &FeedbackData) {
        if Self::key(ctx, egui::Key::Enter) {
            self.act(app, Input::Enter);
            return;
        }
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(30.0);
            if f.passed {
                ui.vertical_centered(|ui| {
                    ui.heading(
                        egui::RichText::new("✅ 通关！")
                            .color(egui::Color32::from_rgb(90, 220, 130))
                            .size(40.0),
                    );
                    ui.add_space(8.0);
                    ui.label(format!("获得 {} XP，已自动保存进度", f.xp_gained));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("按 Enter 进入下一关").weak());
                });
            } else {
                ui.heading(egui::RichText::new("❌ 未通过").color(egui::Color32::from_rgb(240, 90, 90)));
                ui.separator();
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for line in &f.feedback {
                        ui.label(egui::RichText::new(line).color(egui::Color32::from_rgb(235, 190, 190)));
                        ui.add_space(6.0);
                    }
                });
                ui.separator();
                ui.label(egui::RichText::new("按 Enter 返回编辑继续修改，Esc 回地图").weak());
            }
        });
    }
}

impl Default for GameUi {
    fn default() -> Self {
        Self::new()
    }
}

fn color_for(kind: TokenKind) -> egui::Color32 {
    match kind {
        TokenKind::Keyword => egui::Color32::from_rgb(86, 156, 214),
        TokenKind::Comment => egui::Color32::from_rgb(106, 153, 85),
        TokenKind::String => egui::Color32::from_rgb(206, 145, 120),
        TokenKind::Number => egui::Color32::from_rgb(181, 206, 168),
        TokenKind::Normal => egui::Color32::from_rgb(220, 220, 220),
    }
}

impl UiBackend for GameUi {
    async fn run(&mut self, app: &mut GameApp) -> Result<(), GameError> {
        let mut fonts_installed = false;
        loop {
            clear_background(Color::from_rgba(30, 30, 30, 255));
            egui_macroquad::ui(|ctx| {
                if !fonts_installed {
                    install_fonts(ctx);
                    fonts_installed = true;
                }
                self.draw(ctx, app);
            });
            egui_macroquad::draw();
            if self.quit {
                break;
            }
            next_frame().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::app::{GameApp, Input, Screen};
    use game_core::engine::Engine;
    use game_core::level::{parse_levels, LevelSet};
    use game_core::sandbox::DevSandbox;
    use game_core::validate::mapper::ErrorMapper;

    fn test_app() -> GameApp {
        let levels = parse_levels(
            "[[level]]\nid = \"t\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(1); }\"\nsource = \"x\"\n",
        )
        .unwrap();
        let engine = Engine::new(
            LevelSet { levels },
            Default::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        GameApp::new(engine)
    }

    #[test]
    fn renders_menu_headless() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Esc).unwrap(); // 进入菜单
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(matches!(app.screen(), Screen::Menu(_)));
    }

    #[test]
    fn renders_level_and_syncs_code_buffer() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap(); // 进入关卡
        assert!(ui.code_buf.is_empty());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(ui.code_buf.contains("fn main"), "code_buf 未同步: {}", ui.code_buf);
        assert!(matches!(app.screen(), Screen::Level(_)));
    }

    #[test]
    fn editor_edit_syncs_back_to_app() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        // 模拟 TextEdit 改动后 UI 回写
        ui.code_buf = "fn main() { println!(\"edited\"); }".into();
        ui.sync_code(&mut app);
        match app.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"edited\"); }"),
            other => panic!("expected Level, got {other:?}"),
        }
    }

    #[test]
    fn renders_quiz_level_with_options() {
        let levels = parse_levels(
            r#"
[[level]]
id = "q"
title = "q"
tier = "l4"
kind = "quiz"
description = "d"
starter_code = "fn main() { print!(\"1\"); }"
options = ["0", "1", "编译错误", "不确定"]
answer_index = 1
source = "s"
"#,
        )
        .unwrap();
        let engine = Engine::new(
            LevelSet { levels },
            Default::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        let mut app = GameApp::new(engine);
        let mut ui = GameUi::new();
        // 进入关卡并渲染 quiz 界面（只读展示代码 + 选项）不崩溃
        let ctx = egui::Context::default();
        app.handle(Input::Enter).unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(matches!(app.screen(), Screen::Level(_)));
        // 展示代码同步进缓冲区但不渲染为可编辑 TextEdit（只读展示），quiz_sel 初始未选中
        assert_eq!(ui.code_buf, "fn main() { print!(\"1\"); }");
        assert_eq!(ui.quiz_sel, None);
    }
}
