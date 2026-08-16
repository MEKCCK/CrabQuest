use egui_macroquad::egui;
use game_core::app::{ChapterMapData, FeedbackData, GameApp, GameFlow, Input, LevelData, MenuData, Screen};
use game_core::editor::{tokenize, TokenKind};
use game_core::error::GameError;
use game_core::level::LevelTier;
use game_core::ui::UiBackend;
use game_core::validate::{ErrorCard, OutputDiff};
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
    Show, // 显示「编译中」一帧
    Do,   // 下一帧真正执行提交
}

pub struct GameUi {
    code_buf: String,
    last_level_id: Option<String>,
    busy: Busy,
    quit: bool,
    /// 首次进入编辑器时显示一行「中文请复制粘贴」弱提示（IME 不可用，仅显示一次）
    ime_hint_shown: bool,
    /// P1-03：离线标志（启动时对 rustwiki.org 做 ≤3s HEAD 探测并缓存；测试可注入）
    offline: bool,
    /// P1-03：toast 提示（离线点击链接等），3 秒后自动消失
    toast: Option<(String, std::time::Instant)>,
}

impl GameUi {
    pub fn new() -> Self {
        Self {
            code_buf: String::new(),
            last_level_id: None,
            busy: Busy::None,
            quit: false,
            ime_hint_shown: false,
            offline: false,
            toast: None,
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
        // 进入新关卡时同步 code_buf
        if let Screen::Level(d) = &screen {
            if self.last_level_id.as_deref() != Some(d.level.id.as_str()) {
                self.code_buf = d.code.clone();
                self.last_level_id = Some(d.level.id.clone());
            }
        } else {
            self.last_level_id = None;
        }
        match self.busy {
            Busy::Show => {
                self.draw_busy(ctx);
                self.busy = Busy::Do;
                return;
            }
            Busy::Do => {
                self.draw_busy(ctx);
                self.busy = Busy::None;
                self.act(app, Input::Submit);
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
        // P1-03：返回编辑后反馈面板底部固定保留（TopBottomPanel::bottom）
        if let Some(fb) = &d.feedback {
            egui::TopBottomPanel::bottom("feedback_panel")
                .resizable(false)
                .show(ctx, |ui| {
                    self.draw_feedback_panel(ui, fb, false);
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            // P2-08/09：心数与连续游玩日实时读取引擎存档（复习回血后即时刷新）
            let hearts = app.save_ref().hearts;
            let streak = app.save_ref().streak_days;
            ui.horizontal(|ui| {
                ui.heading(&d.level.title);
                let mut stats = format!(
                    "L{} · {}/{} · XP {} · 连击 {}x · ❤️ {}",
                    d.level.tier.order(),
                    d.index + 1,
                    d.total,
                    d.xp,
                    d.combo,
                    hearts
                );
                if streak > 0 {
                    stats.push_str(&format!(" · 🔥 连续 {streak} 天"));
                }
                ui.label(stats);
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
            ui.separator();
            // P2-08：0 心禁提交（按钮置灰 + 复习引导）；编辑不禁止
            let can_submit = hearts > 0;
            ui.horizontal(|ui| {
                if ui.add_enabled(can_submit, egui::Button::new("▶ 提交运行")).clicked() {
                    self.busy = Busy::Show;
                }
                let hint_label = match d.visible_hint() {
                    Some((_, cur, total)) if total > 1 => format!("💡 提示 {cur}/{total}"),
                    _ => "💡 提示".to_owned(),
                };
                if ui.button(hint_label).clicked() {
                    self.act(app, Input::Hint);
                }
                if ui.button("↺ 重置代码").clicked() {
                    self.act(app, Input::Reset);
                    self.last_level_id = None; // 下一帧重新同步 starter_code
                }
                if hearts < 5 {
                    if ui.button("📖 复习关卡说明 +1❤").clicked() {
                        self.act(app, Input::ReviewLore);
                    }
                }
            });
            if !can_submit {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 80),
                    "❤️ 已空：复习关卡说明可回 1 心",
                );
            }
        });
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
            if f.passed {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.heading(
                        egui::RichText::new("✅ 通关！")
                            .color(egui::Color32::from_rgb(90, 220, 130))
                            .size(40.0),
                    );
                    ui.add_space(8.0);
                    ui.label(format!("获得 {} XP · 连击 {}x · ❤️ {}", f.xp_gained, f.combo, f.hearts));
                    ui.add_space(8.0);
                    match &f.unlocked_next {
                        Some(title) => {
                            ui.label(egui::RichText::new(format!("🔓 下一关已解锁：{title}")).size(16.0));
                        }
                        None => {
                            ui.label(egui::RichText::new("🏆 全部通关！").size(16.0));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("已自动保存进度").weak());
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("按 Enter 进入下一关").weak());
                });
            } else {
                ui.add_space(12.0);
                self.draw_feedback_panel(ui, f, true);
            }
        });
    }

    /// P1-03 失败反馈面板主体（Feedback 屏与编辑器底部固定区共用）。
    /// `show_nav_hint`：Feedback 屏提示「Enter 返回编辑 / Esc 回地图」，编辑器内不重复。
    fn draw_feedback_panel(&mut self, ui: &mut egui::Ui, f: &FeedbackData, show_nav_hint: bool) {
        // 防挫败语气（v3 §7.6）：「❌ 未通过」→「🔧 还差一点」
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("🔧 还差一点").color(egui::Color32::from_rgb(255, 180, 80)));
            ui.label(egui::RichText::new(format!("❤️ {}", f.hearts)).weak());
        });
        self.draw_toast(ui);
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // ①-⑤ 编译错误卡片：第一条默认展开，其余折叠「还有 N 个错误」逐个展开
                if !f.errors.is_empty() {
                    self.draw_error_card(ui, &f.errors[0]);
                    if f.errors.len() > 1 {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("还有 {} 个错误（点击展开）", f.errors.len() - 1))
                                .weak(),
                        );
                        for (i, card) in f.errors.iter().enumerate().skip(1) {
                            egui::CollapsingHeader::new(
                                egui::RichText::new(self.error_head(card)).monospace(),
                            )
                            .id_salt(("err", i))
                            .default_open(false)
                            .show(ui, |ui| {
                                self.draw_error_card_body(ui, card);
                            });
                        }
                    }
                }
                // ⑥ panic 卡片：分类标题 + 折叠净化消息
                if let Some(p) = &f.panic {
                    ui.add_space(6.0);
                    self.draw_panic_card(ui, p);
                }
                // ⑦ 输出不符两栏 diff
                if let Some(d) = &f.expectation {
                    ui.add_space(6.0);
                    Self::draw_output_diff(ui, d);
                }
            });
        ui.separator();
        if show_nav_hint {
            ui.label(egui::RichText::new("按 Enter 返回编辑继续修改，Esc 回地图").weak());
        }
    }

    /// 单张错误卡（默认完全展开）：① 徽章+行号+标题 ② zh 展开 ③ 怎么改折叠 ④ 原文折叠 ⑤ 链接
    fn draw_error_card(&mut self, ui: &mut egui::Ui, card: &ErrorCard) {
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(52, 42, 40))
            .corner_radius(4)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(&card.code)
                            .monospace()
                            .strong()
                            .color(egui::Color32::from_rgb(255, 180, 100))
                            .background_color(egui::Color32::from_rgb(72, 48, 22)),
                    );
                    if let Some(line) = card.line {
                        ui.label(
                            egui::RichText::new(format!("第 {line} 行"))
                                .color(egui::Color32::from_rgb(205, 205, 215)),
                        );
                    }
                    ui.label(egui::RichText::new(Self::card_title(card)).strong());
                });
                self.draw_error_card_body(ui, card);
            });
    }

    /// 卡片折叠区身体：② zh 默认展开 ③「怎么改」折叠 ④「rustc 原文」折叠 ⑤ 链接（离线降级）
    fn draw_error_card_body(&mut self, ui: &mut egui::Ui, card: &ErrorCard) {
        // ② 中文解释默认展开（第一屏）
        ui.label(&card.zh);
        // ③「怎么改」默认折叠（fix + example + 相关 hint 序号）
        if !card.fix.is_empty() || card.example.is_some() {
            egui::CollapsingHeader::new("怎么改")
                .default_open(false)
                .show(ui, |ui| {
                    if !card.fix.is_empty() {
                        ui.label(&card.fix);
                    }
                    if let Some(ex) = &card.example {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(ex)
                                    .monospace()
                                    .color(egui::Color32::from_rgb(150, 205, 155)),
                            )
                            .wrap(),
                        );
                    }
                    if let Some(hi) = card.hint_index {
                        ui.label(format!("💡 与提示 {} 相关", hi + 1));
                    }
                });
        }
        // ④ rustc 原文默认折叠（CollapsingHeader，monospace 灰字）
        if !card.summary.is_empty() {
            egui::CollapsingHeader::new("rustc 原文")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&card.summary)
                                .monospace()
                                .color(egui::Color32::from_rgb(150, 150, 158)),
                        )
                        .wrap(),
                    );
                });
        }
        // ⑤ 链接（P1-03 离线降级）
        self.draw_card_link(ui, card);
    }

    /// ⑥ panic 卡片：分类标题 + 折叠净化消息（v3 §7.7）
    fn draw_panic_card(&mut self, ui: &mut egui::Ui, p: &str) {
        let (title, body) = p
            .split_once('\n')
            .map(|(t, b)| (t, b.trim()))
            .unwrap_or((p, ""));
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(66, 30, 30))
            .corner_radius(4)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(title)
                        .strong()
                        .color(egui::Color32::from_rgb(255, 150, 130)),
                );
                if !body.is_empty() {
                    egui::CollapsingHeader::new("原始信息")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(body)
                                        .monospace()
                                        .color(egui::Color32::from_rgb(185, 175, 178)),
                                )
                                .wrap(),
                            );
                        });
                }
            });
    }

    /// ⑦ 输出不符两栏 diff：期望 vs 实际，逐行着色（相同行中性色，差异行分别高亮）
    fn draw_output_diff(ui: &mut egui::Ui, d: &OutputDiff) {
        ui.label(egui::RichText::new("输出不符合要求").strong());
        let exp: Vec<&str> = d.expected.lines().collect();
        let act: Vec<&str> = d.actual.lines().collect();
        let rows = exp.len().max(act.len()).max(1);
        egui::Grid::new("output_diff")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("期望输出")
                        .color(egui::Color32::from_rgb(140, 215, 150))
                        .strong(),
                );
                ui.label(
                    egui::RichText::new("实际输出")
                        .color(egui::Color32::from_rgb(245, 145, 145))
                        .strong(),
                );
                ui.end_row();
                for i in 0..rows {
                    let e = exp.get(i).copied().unwrap_or("");
                    let a = act.get(i).copied().unwrap_or("");
                    let same = e == a;
                    let ec = if same {
                        egui::Color32::from_rgb(205, 215, 205)
                    } else {
                        egui::Color32::from_rgb(120, 215, 135)
                    };
                    let ac = if same {
                        egui::Color32::from_rgb(225, 200, 200)
                    } else {
                        egui::Color32::from_rgb(255, 130, 130)
                    };
                    ui.add(egui::Label::new(egui::RichText::new(e).monospace().color(ec)).wrap());
                    ui.add(egui::Label::new(egui::RichText::new(a).monospace().color(ac)).wrap());
                    ui.end_row();
                }
            });
    }

    /// ⑤ 链接：online → hyperlink；offline → 灰字提示（点击弹 toast，不崩溃）
    fn draw_card_link(&mut self, ui: &mut egui::Ui, card: &ErrorCard) {
        if let Some(link) = &card.link {
            if self.offline {
                let resp = ui.label(
                    egui::RichText::new("当前离线：概念已内置在讲解卡片中")
                        .weak()
                        .color(egui::Color32::from_rgb(140, 140, 148)),
                );
                if resp.clicked() {
                    self.set_toast("无法打开在线教材（当前离线）");
                }
            } else {
                ui.hyperlink_to("📖 概念详解 ↗", link.as_str());
            }
        }
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), std::time::Instant::now()));
    }

    fn draw_toast(&mut self, ui: &mut egui::Ui) {
        if let Some((msg, at)) = &self.toast {
            if at.elapsed().as_secs() < 3 {
                ui.colored_label(egui::Color32::from_rgb(255, 200, 80), format!("🔔 {msg}"));
            } else {
                self.toast = None;
            }
        }
    }

    /// 折叠头行（v3 §7.7 ①）：`E0308 · 第 3 行 · 类型不匹配`
    fn error_head(&self, card: &ErrorCard) -> String {
        match card.line {
            Some(l) => format!("{} · 第 {l} 行 · {}", card.code, Self::card_title(card)),
            None => format!("{} · {}", card.code, Self::card_title(card)),
        }
    }

    /// 卡片中文标题：取 zh 第一个「：」前的短语（如「类型不匹配」），无冒号则整段。
    fn card_title(card: &ErrorCard) -> &str {
        let t = card.zh.split('：').next().unwrap_or(&card.zh).trim();
        if t.is_empty() {
            &card.zh
        } else {
            t
        }
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
        // P1-03 链接降级：启动时探测一次在线状态（≤3s，缓存 offline 标志）。
        // 设计选择：不引入 HTTP/TLS 依赖（egui-macroquad 栈无网络库），
        // 纯 TCP 80 端口 HEAD，任何 HTTP 响应即在线。
        self.offline = !probe_online();
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

/// P1-03 链接降级探测：对 rustwiki.org:80 发 HTTP/1.0 HEAD，3 秒超时。
/// 设计选择：不加 HTTP/TLS 依赖，纯 TCP 头探测——任何 HTTP 状态行即视为在线；
/// 结果缓存到 `GameUi::offline`，仅启动时执行一次。
fn probe_online() -> bool {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let addr = match "rustwiki.org:80".to_socket_addrs().ok().and_then(|mut it| it.next()) {
        Some(a) => a,
        None => return false,
    };
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
    if stream
        .write_all(b"HEAD / HTTP/1.0\r\nHost: rustwiki.org\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 64];
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => String::from_utf8_lossy(&buf[..n]).starts_with("HTTP/"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::app::{GameApp, Input, Screen};
    use game_core::engine::Engine;
    use game_core::level::{parse_levels, LevelSet};
    use game_core::sandbox::DevSandbox;
    use game_core::save::SaveData;
    use game_core::validate::mapper::ErrorMapper;

    fn test_app() -> GameApp {
        test_app_with_save(SaveData::default())
    }

    fn test_app_with_save(save: SaveData) -> GameApp {
        let levels = parse_levels(
            "[[level]]\nid = \"t\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(1); }\"\nsource = \"x\"\n",
        )
        .unwrap();
        let engine = Engine::new(
            LevelSet { levels },
            save,
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

    // ===== P2-08/09：hearts / streak（headless）=====

    #[test]
    fn zero_hearts_disables_submit_and_shows_review_message() {
        let ctx = egui::Context::default();
        let save = SaveData { hearts: 0, streak_days: 3, ..SaveData::default() };
        let mut app = test_app_with_save(save);
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap(); // 进入关卡
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out.shapes, "❤️ 已空：复习关卡说明可回 1 心"),
            "0 心应显示复习引导文案"
        );
        assert!(shapes_contain_text(&out.shapes, "▶ 提交运行"), "提交按钮仍在（add_enabled 置灰）");
        assert!(shapes_contain_text(&out.shapes, "连续 3 天"), "状态栏应显示连续天数");
        // 0 心绘制不触发提交（busy 保持 None）
        assert_eq!(ui.busy, Busy::None);
    }

    #[test]
    fn review_button_visible_below_full_hearts_only() {
        // 心 < 5 → 复习按钮可见；心 > 0 不显示 0 心提示
        let ctx = egui::Context::default();
        let mut app = test_app_with_save(SaveData { hearts: 3, ..SaveData::default() });
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(shapes_contain_text(&out.shapes, "复习关卡说明"), "心 <5 显示复习按钮");
        assert!(!shapes_contain_text(&out.shapes, "❤️ 已空"), "心 >0 不显示 0 心提示");

        // 满心（cap 5）→ 复习按钮隐藏（无可回）
        let mut app2 = test_app_with_save(SaveData { hearts: 5, ..SaveData::default() });
        let mut ui2 = GameUi::new();
        app2.handle(Input::Enter).unwrap();
        let out2 = ctx.run(egui::RawInput::default(), |ctx| {
            ui2.draw(ctx, &mut app2);
        });
        assert!(!shapes_contain_text(&out2.shapes, "复习关卡说明"), "满心隐藏复习按钮");
    }

    #[test]
    fn streak_hidden_when_zero_days() {
        let ctx = egui::Context::default();
        let mut app = test_app_with_save(SaveData { hearts: 3, streak_days: 0, ..SaveData::default() });
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(!shapes_contain_text(&out.shapes, "连续 0 天"), "未活跃不显示连续天数");
        assert!(shapes_contain_text(&out.shapes, "❤️ 3"), "头部应显示心数");
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

    // ===== P1-03 结构化反馈面板（headless）=====

    fn card(code: &str, line: Option<u32>, zh: &str, fix: &str, link: Option<&str>) -> ErrorCard {
        ErrorCard {
            code: code.into(),
            line,
            summary: format!("rustc summary {code}"),
            zh: zh.into(),
            fix: fix.into(),
            example: None,
            link: link.map(str::to_owned),
            hint_index: None,
        }
    }

    fn fail_fb(errors: Vec<ErrorCard>) -> FeedbackData {
        FeedbackData {
            passed: false,
            level_id: "t".into(),
            xp_gained: 0,
            combo: 0,
            hearts: 3,
            errors,
            expectation: None,
            panic: None,
            unlocked_next: None,
        }
    }

    fn draw_panel(ctx: &egui::Context, game_ui: &mut GameUi, fb: &FeedbackData) -> egui::FullOutput {
        ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                game_ui.draw_feedback_panel(ui, fb, true);
            });
        })
    }

    /// 多错误：第一条默认展开，其余折叠「还有 N 个错误」且逐个可展开；怎么改/原文默认折叠
    #[test]
    fn feedback_panel_expands_only_first_error() {
        let ctx = egui::Context::default();
        let fb = fail_fb(vec![
            card("E0425", Some(2), "找不到名字：变量、函数或类型未定义", "检查拼写或先定义", Some("https://doc.rust-lang.org/error_codes/E0425.html")),
            card("E0308", Some(5), "类型不匹配：表达式的实际类型与期望类型不一致", "", None),
        ]);
        let mut game_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut game_ui, &fb);
        assert!(shapes_contain_text(&out.shapes, "🔧 还差一点"), "失败语气应为「还差一点」");
        assert!(shapes_contain_text(&out.shapes, "找不到名字"), "第一条 zh 应默认展开");
        // ① 头行组件：徽章 + 行号 + 中文标题（分开渲染）
        assert!(shapes_contain_text(&out.shapes, "E0425"), "错误码徽章缺失");
        assert!(shapes_contain_text(&out.shapes, "第 2 行"), "行号缺失");
        assert!(shapes_contain_text(&out.shapes, "还有 1 个错误"), "「还有 N 个错误」提示缺失");
        assert!(shapes_contain_text(&out.shapes, "E0308 · 第 5 行 · 类型不匹配"), "第二条折叠头缺失");
        assert!(
            !shapes_contain_text(&out.shapes, "类型不匹配：表达式的实际类型与期望类型不一致"),
            "第二条 zh 应折叠隐藏"
        );
        assert!(!shapes_contain_text(&out.shapes, "检查拼写或先定义"), "「怎么改」应默认折叠");
        assert!(!shapes_contain_text(&out.shapes, "rustc summary"), "rustc 原文应默认折叠");
    }

    /// panic 分支：分类标题可见，原始信息折叠
    #[test]
    fn feedback_panel_panic_card_title_with_folded_message() {
        let ctx = egui::Context::default();
        let mut fb = fail_fb(Vec::new());
        fb.panic = Some(
            "❗ 程序运行崩溃（索引越界）\nmain.rs:3:21: index out of bounds: the len is 3 but the index is 3"
                .into(),
        );
        let mut game_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut game_ui, &fb);
        assert!(shapes_contain_text(&out.shapes, "程序运行崩溃（索引越界）"), "panic 分类标题缺失");
        assert!(!shapes_contain_text(&out.shapes, "index out of bounds"), "净化消息应默认折叠");
    }

    /// 输出不符：两栏 diff（期望/实际表头可见）
    #[test]
    fn feedback_panel_output_diff_two_columns() {
        let ctx = egui::Context::default();
        let mut fb = fail_fb(Vec::new());
        fb.expectation = Some(OutputDiff {
            expected: "a\nb".into(),
            actual: "a\nc".into(),
        });
        let mut game_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut game_ui, &fb);
        assert!(shapes_contain_text(&out.shapes, "期望输出"), "diff 期望列表头缺失");
        assert!(shapes_contain_text(&out.shapes, "实际输出"), "diff 实际列表头缺失");
    }

    /// 无 E 码错误 → fallback 兜底卡（非空白）正常渲染
    #[test]
    fn feedback_panel_eunknown_fallback_card_non_blank() {
        let ctx = egui::Context::default();
        let fb = fail_fb(vec![card(
            "EUNKNOWN",
            Some(2),
            "这是一个编译错误（rustc 未提供错误码）。请对照报错原文，检查最近的改动",
            "",
            None,
        )]);
        let mut game_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut game_ui, &fb);
        assert!(shapes_contain_text(&out.shapes, "EUNKNOWN"), "兜底卡徽章缺失");
        assert!(shapes_contain_text(&out.shapes, "编译错误"), "兜底卡 zh 非空白");
    }

    /// 链接降级：offline → 灰字提示（不渲染可点链接）；online → 可点链接
    #[test]
    fn feedback_link_degrades_offline() {
        let ctx = egui::Context::default();
        let fb = fail_fb(vec![card(
            "E0425",
            None,
            "找不到名字：变量、函数或类型未定义",
            "",
            Some("https://doc.rust-lang.org/error_codes/E0425.html"),
        )]);
        let mut offline_ui = GameUi::new();
        offline_ui.offline = true;
        let out = draw_panel(&ctx, &mut offline_ui, &fb);
        assert!(
            shapes_contain_text(&out.shapes, "当前离线：概念已内置在讲解卡片中"),
            "离线灰字提示缺失"
        );
        assert!(!shapes_contain_text(&out.shapes, "概念详解"), "离线不应渲染可点链接");

        let mut online_ui = GameUi::new();
        let out2 = draw_panel(&ctx, &mut online_ui, &fb);
        assert!(shapes_contain_text(&out2.shapes, "概念详解"), "在线应渲染链接");
    }

    /// 面板在返回编辑器后保留（底部固定）：真实提交失败 → Enter 回编辑 → 绘制出面板
    #[test]
    fn feedback_panel_persists_after_return_to_editor() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap(); // 进入关卡
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        }); // 同步 code_buf
        app.handle(Input::Submit).unwrap(); // starter 编译失败 → Fail
        match app.screen() {
            Screen::Feedback(f) => assert!(!f.passed),
            other => panic!("expected Feedback, got {other:?}"),
        }
        app.handle(Input::Enter).unwrap(); // 返回编辑（面板应保留）
        assert!(matches!(app.screen(), Screen::Level(_)));
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out.shapes, "🔧 还差一点"),
            "返回编辑后面板应底部固定保留"
        );
    }

    /// toast：离线点击链接后设置 toast 文案，下一次绘制可见（3 秒内）
    #[test]
    fn feedback_link_offline_click_sets_toast() {
        let mut game_ui = GameUi::new();
        game_ui.offline = true;
        game_ui.set_toast("无法打开在线教材（当前离线）");
        let ctx = egui::Context::default();
        let fb = fail_fb(vec![card(
            "E0425",
            None,
            "找不到名字",
            "",
            Some("https://doc.rust-lang.org/error_codes/E0425.html"),
        )]);
        let out = draw_panel(&ctx, &mut game_ui, &fb);
        assert!(
            shapes_contain_text(&out.shapes, "无法打开在线教材"),
            "toast 应绘制出来"
        );
    }
}
