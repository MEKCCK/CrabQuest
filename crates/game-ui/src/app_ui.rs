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
    /// 首次进入编辑器时显示一行「中文请复制粘贴」弱提示（IME 不可用，仅显示一次）
    ime_hint_shown: bool,
}

impl GameUi {
    pub fn new() -> Self {
        Self {
            code_buf: String::new(),
            last_level_id: None,
            quiz_sel: None,
            busy: Busy::None,
            quit: false,
            ime_hint_shown: false,
        }
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
    /// P1-06：layouter 尊重 TextEdit 传入的 wrap_width（超宽中文行换行不截断），
    /// 代码区与行号 gutter 共用 TextStyle::Monospace（字号一致，禁止硬编码）。
    fn draw_code_body(&mut self, ui: &mut egui::Ui, app: &mut GameApp) {
        if !self.ime_hint_shown {
            // 弱提示：IME 不可用（egui-miniquad 无 IME 通道），中文只能粘贴
            ui.label(egui::RichText::new("代码编辑器不支持中文输入法，中文内容请复制粘贴").weak());
            self.ime_hint_shown = true;
        }
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
            // 编辑器（行号 gutter 与代码区共用 TextStyle::Monospace，字号一致）
            let font_id = egui::TextStyle::Monospace.resolve(ui.style());
            let mut layouter = move |ui: &egui::Ui, text: &str, wrap_width: f32| {
                let job = code_layout_job(text, wrap_width, &font_id);
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

/// 构建编辑器代码区的着色 LayoutJob。
/// `wrap_width` 是 TextEdit 传入的换行宽度（面板宽）：超宽行（如无空格的长中文）必须在此换行，
/// 否则横向溢出被截断；`font_id` 必须来自 `TextStyle::Monospace.resolve(style)`，
/// 与行号 gutter（`.monospace()`）同源，保证行号与代码字号一致。
/// 注意：tokenize 会跳过空白与标点，这里必须把「间隙」也以 Normal 格式补进 job，
/// 否则 galley 文本与代码不一致，换行宽度与光标/选区映射都会错位。
fn code_layout_job(text: &str, wrap_width: f32, font_id: &egui::FontId) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let mut cursor = 0;
    let normal = egui::TextFormat {
        font_id: font_id.clone(),
        color: color_for(TokenKind::Normal),
        ..Default::default()
    };
    for span in tokenize(text) {
        if span.start > cursor {
            job.append(&text[cursor..span.start], 0.0, normal.clone());
        }
        job.append(
            &text[span.start..span.end],
            0.0,
            egui::TextFormat {
                font_id: font_id.clone(),
                color: color_for(span.kind),
                ..Default::default()
            },
        );
        cursor = span.end;
    }
    if cursor < text.len() {
        job.append(&text[cursor..], 0.0, normal);
    }
    job
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
    /// P1-06：layouter 必须尊重 TextEdit 传入的 wrap_width，超宽中文行（无空格）要换行。
    #[test]
    fn layouter_wraps_wide_cjk_line_at_wrap_width() {
        let ctx = egui::Context::default();
        let resolved = egui::TextStyle::Monospace.resolve(&egui::Style::default());
        // 无空格的长中文行：若不换行会横向溢出面板
        let cjk = "中文长行内容必须换行不能截断".repeat(20);
        let job = code_layout_job(&cjk, 100.0, &resolved);
        assert_eq!(
            job.wrap.max_width, 100.0,
            "LayoutJob 必须使用传入的 wrap_width，而不是 INFINITY"
        );
        let mut galley = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            galley = Some(ctx.fonts(|f| f.layout_job(job.clone())));
        });
        let galley = galley.expect("布局应在 run 期间完成");
        assert!(galley.rows.len() > 1, "100px 宽的 CJK 长行应换行成多行，实际 {} 行", galley.rows.len());

        // 旧行为（wrap_width = INFINITY）回归对照：只有一行
        let job_inf = code_layout_job(&cjk, f32::INFINITY, &resolved);
        let mut galley_inf = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            galley_inf = Some(ctx.fonts(|f| f.layout_job(job_inf.clone())));
        });
        assert_eq!(galley_inf.unwrap().rows.len(), 1, "INFINITY 宽度下 CJK 长行应保持单行");
    }

    /// P1-06：代码区字号必须来自 TextStyle::Monospace（与行号 gutter 同源），禁止硬编码 14/12 混用。
    #[test]
    fn layouter_uses_style_monospace_font_id() {
        let resolved = egui::TextStyle::Monospace.resolve(&egui::Style::default());
        assert_eq!(resolved.size, 12.0, "egui 默认 Monospace 字号应为 12.0");
        let job = code_layout_job("fn main() {}", 200.0, &resolved);
        for section in &job.sections {
            assert_eq!(
                section.format.font_id, resolved,
                "代码区每个 span 的字号必须等于 TextStyle::Monospace 解析值（与行号一致）"
            );
        }
    }

    /// P1-06：编辑器实际绘制时，代码区与行号 gutter 字号一致（端到端，防调用点回归硬编码 14）。
    #[test]
    fn drawn_editor_uses_same_font_size_as_gutter() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        let expected = egui::TextStyle::Monospace.resolve(&egui::Style::default()).size;
        let mut code_sizes: Vec<f32> = Vec::new();
        let mut gutter_sizes: Vec<f32> = Vec::new();
        for clipped in &out.shapes {
            if let egui::Shape::Text(t) = &clipped.shape {
                let text = t.galley.text();
                let sizes: Vec<f32> = t.galley.job.sections.iter().map(|s| s.format.font_id.size).collect();
                if text.contains("fn main") {
                    code_sizes.extend(sizes);
                } else if !text.is_empty() && text.chars().all(|c| c.is_ascii_digit() || c == '\n') {
                    gutter_sizes.extend(sizes);
                }
            }
        }
        assert!(!code_sizes.is_empty(), "应绘制出代码区文本");
        assert!(!gutter_sizes.is_empty(), "应绘制出行号 gutter");
        assert!(
            code_sizes.iter().all(|&s| s == expected) && gutter_sizes.iter().all(|&s| s == expected),
            "代码区 {code_sizes:?} 与 gutter {gutter_sizes:?} 字号不一致（期望 {expected}）"
        );
    }

    /// P1-06：首次进入编辑器显示「中文请复制粘贴」弱提示，且只显示一次。
    #[test]
    fn ime_hint_shown_once_on_first_level_entry() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap(); // 进入关卡
        let out1 = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(ui.ime_hint_shown, "首次绘制关卡后应标记提示已显示");
        assert!(
            shapes_contain_text(&out1.shapes, "复制粘贴"),
            "首次进入编辑器应显示「中文请复制粘贴」提示"
        );
        let out2 = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            !shapes_contain_text(&out2.shapes, "复制粘贴"),
            "提示只应在首次进入时显示一次"
        );
    }

    fn shapes_contain_text(shapes: &[egui::epaint::ClippedShape], needle: &str) -> bool {
        shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Text(t) => t.galley.text().contains(needle),
            _ => false,
        })
    }

}
