use eframe::egui;
use crab_quest_core::app::{
    ChapterMapData, FeedbackData, GameApp, GameFlow, Input, LevelData, MenuData, Screen,
};
use crab_quest_core::editor::{tokenize, TokenKind};
use crab_quest_core::engine::{
    boss_hint_lock_remaining, boss_hint_locked, hint_unlock_state, HintUnlockState,
};
use crab_quest_core::error::GameError;
use crab_quest_core::level::LevelTier;
use crab_quest_core::save;
use crab_quest_core::validate::{ErrorCard, OutputDiff};

use crate::icons::{Icon, IconLibrary};

/// JetBrains Maple Mono（内嵌，SIL OFL 许可）——覆盖 CJK 统一表意区，保证中文正常渲染；
/// Monospace 家族主字体（代码区等宽），同时作为 Proportional 家族的 CJK 兜底。
const MAPLE_FONT: &[u8] = include_bytes!("../assets/JetBrainsMapleMono-Regular.ttf");

/// Noto Sans SC（SIL OFL 1.1；Google Fonts 官方源下载后由 pyftsubset 子集化到游戏用字，
/// 约 300KB。来源 URL、许可全文与复现命令见 crates/crab-quest-ui/scripts/font_subset.py 与
/// assets/NotoSansSC-OFL.txt）——Proportional 家族主字体（标题/描述/正文无衬线中文）。
const NOTO_SANS_SC: &[u8] = include_bytes!("../assets/NotoSansSC-Regular.ttf");

/// Noto Sans Symbols 2（SIL OFL 1.1）——随游戏内置的 UI 符号回退字体，
/// 不依赖玩家系统恰好安装某款符号字体。
const NOTO_SANS_SYMBOLS: &[u8] = include_bytes!("../assets/NotoSansSymbols2-Regular.ttf");

/// P3-19：编辑器 TextEdit 的持久化 id salt（光标状态跨帧/跨布局保持，
/// 行号跳转先写 TextEditState 再绘制，焦点行才能落在目标行首）
const EDITOR_ID_SALT: &str = "code_editor";
/// 编辑器在紧凑窗口中的最低高度；常规布局中会占据主区其余空间。
const EDITOR_MIN_HEIGHT: f32 = 260.0;
/// 编辑器下方失败反馈抽屉的限高，不能挤掉主要代码工作区。
const FEEDBACK_DRAWER_MAX_HEIGHT: f32 = 176.0;

// 六屏统一视觉语言：深靛蓝底、半透明感的分层面板、青蓝描边与少量暖色强调。
/// 根画布透明；实际可读内容都落在下方半透明亚克力面板中。
const INK: egui::Color32 = egui::Color32::TRANSPARENT;
fn surface() -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(23, 35, 66, 185) }
fn surface_raised() -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(35, 50, 88, 150) }
fn border() -> egui::Color32 { egui::Color32::from_rgba_unmultiplied(120, 157, 210, 76) }
const CYAN: egui::Color32 = egui::Color32::from_rgb(113, 214, 238);
const MINT: egui::Color32 = egui::Color32::from_rgb(118, 222, 175);
const GOLD: egui::Color32 = egui::Color32::from_rgb(255, 209, 112);
const DANGER: egui::Color32 = egui::Color32::from_rgb(255, 147, 158);

fn glass_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(surface())
        .stroke(egui::Stroke::new(0.5_f32, border()))
        .corner_radius(1)
        .inner_margin(egui::Margin::symmetric(16, 14))
}

fn raised_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(surface_raised())
        .stroke(egui::Stroke::new(0.5_f32, border()))
        .corner_radius(1)
        .inner_margin(egui::Margin::symmetric(12, 10))
}

fn apply_visual_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.panel_fill = INK;
    style.visuals.window_fill = surface();
    style.visuals.extreme_bg_color = INK;
    style.visuals.faint_bg_color = surface_raised();
    style.visuals.widgets.noninteractive.bg_fill = surface();
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5_f32, border());
    style.visuals.widgets.inactive.bg_fill = surface_raised();
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(0.5_f32, border());
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgba_unmultiplied(65, 95, 145, 155);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgba_unmultiplied(75, 120, 160, 175);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(41, 105, 137);
    style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, CYAN);
    style.spacing.button_padding = egui::vec2(10.0, 7.0);
    style.spacing.item_spacing = egui::vec2(9.0, 9.0);
    ctx.set_style(style);
}

/// P3-20 双字体方案：
/// - Proportional = [noto_sans_sc, jetbrains_maple_mono, noto_sans_symbols, egui 默认]——标题/描述/正文用
///   无衬线 Noto Sans SC，maple 兜底 CJK（其全量表意区覆盖防 Noto 子集缺字）；
/// - Monospace = [jetbrains_maple_mono, egui 默认]——代码区保持等宽（不变）。
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto_sans_sc".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(NOTO_SANS_SC)),
    );
    fonts.font_data.insert(
        "jetbrains_maple_mono".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(MAPLE_FONT)),
    );
    fonts.font_data.insert(
        "noto_sans_symbols".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(NOTO_SANS_SYMBOLS)),
    );
    {
        let proportional = fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default();
        proportional.insert(0, "noto_sans_sc".to_owned());
        proportional.insert(1, "jetbrains_maple_mono".to_owned());
        proportional.insert(2, "noto_sans_symbols".to_owned());
    }
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains_maple_mono".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("noto_sans_symbols".to_owned());
    ctx.set_fonts(fonts);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Busy {
    None,
    Show, // 显示「编译中」一帧
    Do,   // 下一帧真正执行提交
}

/// P3-18：通关庆祝动画（v3 §7.7 正反馈最小集：每项 ≤1s，总 ≈1s）。
/// 帧驱动 + 墙钟兜底：stage 每满 12 帧（≈0.2s @60fps）或 300ms 推进，
/// 4 次推进 ≈ 0.8s ≤ 1s；内容累积展示（到达后保持），不阻塞 Enter/Esc。
#[derive(Debug, Clone)]
struct Celebration {
    /// 当前已展示阶段：0=未开始；1..=4 依次 = XP/进度条、下一关🔓（或🏆）、
    /// 已自动保存、❤️+1；5 = 全部完成（导航提示全程可见）
    stage: usize,
    frames: u32,
    since: std::time::Instant,
}

impl Celebration {
    const STAGE_FRAMES: u32 = 12;
    const STAGE_MAX_MS: u64 = 300;

    fn new() -> Self {
        Self {
            stage: 0,
            frames: 0,
            since: std::time::Instant::now(),
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    /// 每帧推进一次；全部阶段展示完（stage ≥ 5）返回 true。
    fn tick(&mut self) -> bool {
        if self.stage >= 5 {
            return true;
        }
        self.frames += 1;
        if self.frames >= Self::STAGE_FRAMES
            || self.since.elapsed().as_millis() as u64 >= Self::STAGE_MAX_MS
        {
            self.stage += 1;
            self.frames = 0;
            self.since = std::time::Instant::now();
        }
        self.stage >= 5
    }

    /// 当前阶段内进度 0..1（XP 数字跳动 / ProgressBar 增长）
    fn progress(&self) -> f32 {
        (self.frames as f32 / Self::STAGE_FRAMES as f32).min(1.0)
    }
}

pub struct GameUi {
    /// 本地 Tabler 图标纹理缓存（无网络/无外部文件依赖）。
    icons: IconLibrary,
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
    /// P2-11：参考答案二次确认弹窗是否打开（「先自己试试？」[再想想 / 查看答案]）
    ref_dialog: bool,
    /// 编辑器下方错误抽屉是否展开；关闭时不给它预留大块高度，也不截获滚轮。
    feedback_drawer_open: bool,
    /// P3-18：通关庆祝动画状态（Feedback 通关屏驱动；离开/换关重置）
    celebration: Celebration,
    /// P3-18：庆祝动画绑定的反馈关卡 id（同关重玩重新播放）
    celebration_level: Option<String>,
    /// 存档路径：关卡通过后立即落盘，庆典中也显示实际位置。
    save_path: Option<std::path::PathBuf>,
}

impl GameUi {
    pub fn new() -> Self {
        Self {
            icons: IconLibrary::default(),
            code_buf: String::new(),
            last_level_id: None,
            busy: Busy::None,
            quit: false,
            ime_hint_shown: false,
            offline: false,
            toast: None,
            ref_dialog: false,
            feedback_drawer_open: false,
            celebration: Celebration::new(),
            celebration_level: None,
            save_path: None,
        }
    }

    /// 注入存档位置；成功通关时立即写入，无需等待窗口关闭。
    pub fn set_save_path(&mut self, path: impl Into<std::path::PathBuf>) {
        self.save_path = Some(path.into());
    }

    /// 把 code_buf 回写进 app（TextEdit 每次改动后调用）
    fn sync_code(&mut self, app: &mut GameApp) {
        app.set_code(self.code_buf.clone());
    }

    fn act(&mut self, app: &mut GameApp, input: Input) {
        let submitted = input == Input::Submit;
        match app.handle(input) {
            Ok(GameFlow::Quit) => self.quit = true,
            Ok(GameFlow::Continue) => {
                if submitted && matches!(app.screen(), Screen::Feedback(f) if f.passed) {
                    if let Some(path) = &self.save_path {
                        if let Err(e) = crab_quest_core::save::save(app.save_ref(), path) {
                            eprintln!("通关自动存档失败: {e}");
                        }
                    }
                }
            }
            Err(e) => eprintln!("[ui] 错误: {e}"),
        }
    }

    fn key(ctx: &egui::Context, k: egui::Key) -> bool {
        ctx.input(|i| i.key_pressed(k))
    }

    fn draw(&mut self, ctx: &egui::Context, app: &mut GameApp) {
        apply_visual_theme(ctx);
        let screen = app.screen().clone();
        // P3-18：离开通关反馈屏 → 重置庆祝动画（再次通关重新播放）
        if !matches!(&screen, Screen::Feedback(f) if f.passed) {
            self.celebration.reset();
            self.celebration_level = None;
        }
        if !matches!(&screen, Screen::Level(d) if d.feedback.is_some()) {
            self.feedback_drawer_open = false;
        }
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
            Screen::Stats(s) => self.draw_stats(ctx, app, &s),
        }
    }

    fn draw_busy(&self, ctx: &egui::Context) {
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(INK)).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(110.0);
                glass_frame().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(egui::RichText::new("编译中，请稍候...").color(CYAN));
                        ui.label(egui::RichText::new("正在为你检查这段 Rust 代码").weak());
                    });
                });
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

        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(INK)).show(ctx, |ui| {
            ui.add_space(76.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("RUST QUEST · 学习路径").size(14.0).color(CYAN));
                ui.heading(egui::RichText::new("🦀 Rust 学习游戏").size(44.0).color(GOLD));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("闯关学 Rust：修好代码，让编译器闭嘴，让程序输出正确结果。").weak());
                ui.add_space(28.0);
                glass_frame().show(ui, |ui| {
                    ui.set_min_width(350.0);
                    let mut clicked: Option<usize> = None;
                    let mut idx = 0;
                    if m.can_continue {
                        if ui.add_sized([320.0, 38.0], egui::Button::new("▶ 继续游戏").selected(m.selected == 0)).clicked() {
                            clicked = Some(idx);
                        }
                        idx += 1;
                    }
                    if ui.add_sized([320.0, 38.0], egui::Button::new("🆕 新游戏").selected(m.selected == idx)).clicked() {
                        clicked = Some(idx);
                    }
                    idx += 1;
                    if ui.add_sized([320.0, 38.0], egui::Button::new("🚪 退出").selected(m.selected == idx)).clicked() {
                        clicked = Some(idx);
                    }
                    if let Some(target) = clicked {
                        self.select_and_enter(app, target);
                    }
                });
                ui.add_space(18.0);
                ui.label(egui::RichText::new("↑↓ 选择 · Enter 确认 · Esc 退出").weak().color(CYAN));
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

        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(INK)).show(ctx, |ui| {
            glass_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("LEARNING MAP").size(12.0).color(CYAN));
                        ui.horizontal(|ui| {
                            self.icons.show(ui, Icon::Map, 24.0, GOLD);
                            ui.heading(egui::RichText::new("关卡地图").color(GOLD));
                        });
                        ui.label(egui::RichText::new("按 L0 → L4 顺序推进，解锁前一关后才能进入下一关。").weak());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if app.stats_accessible() {
                            self.icons.show(ui, Icon::Achievement, 20.0, CYAN);
                            if ui.button("统计与成就").clicked() {
                                self.act(app, Input::OpenStats);
                            }
                        }
                    });
                });
            });
            // P4-26：自定义关卡加载失败 → 游戏内提示（启动日志已另行打印，游戏不崩溃）
            if !app.custom_load_errors.is_empty() {
                ui.add_space(10.0);
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(69, 37, 61))
                    .stroke(egui::Stroke::new(0.5_f32, DANGER))
                    .corner_radius(1)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("⚠️ 自定义关卡加载失败：")
                                .strong()
                                .color(egui::Color32::from_rgb(255, 150, 130)),
                        );
                        for e in &app.custom_load_errors {
                            ui.label(
                                egui::RichText::new(e)
                                    .color(egui::Color32::from_rgb(225, 170, 160)),
                            );
                        }
                    });
            }
            ui.add_space(12.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let mut clicked: Option<usize> = None;
                // 内置章节（线性推进；进度/成就/段位以本节为准）
                for (i, entry) in m.entries.iter().enumerate().take(m.custom_start) {
                    let text = Self::map_entry_text(entry, i + 1);
                    let fill = match entry.state {
                        crab_quest_core::save::LevelState::Passed => egui::Color32::from_rgb(27, 73, 67),
                        crab_quest_core::save::LevelState::Unlocked => egui::Color32::from_rgb(35, 66, 99),
                        crab_quest_core::save::LevelState::Locked => egui::Color32::from_rgb(25, 33, 58),
                    };
                    let node = egui::Button::new(egui::RichText::new(text).size(15.0))
                        .fill(fill)
                        .stroke(egui::Stroke::new(if m.selected == i { 2.0_f32 } else { 1.0_f32 }, if m.selected == i { CYAN } else { border() }))
                        .corner_radius(1);
                    if ui.add_sized([ui.available_width() - 4.0, 42.0], node).clicked() {
                        clicked = Some(i);
                    }
                }
                // P4-26：自定义章节独立显示（仅当存在自定义关卡时出现）
                if m.custom_start < m.entries.len() {
                    ui.add_space(8.0);
                    glass_frame().show(ui, |ui| {
                    ui.heading(egui::RichText::new("自定义关卡").color(CYAN));
                        ui.label(egui::RichText::new("自定义关卡进度独立保存，不影响内置成就与段位。").weak());
                    });
                    for (i, entry) in m.entries.iter().enumerate().skip(m.custom_start) {
                        let text = Self::map_entry_text(entry, i - m.custom_start + 1);
                        let node = egui::Button::new(egui::RichText::new(text).size(15.0))
                            .fill(surface_raised())
                            .stroke(egui::Stroke::new(if m.selected == i { 2.0_f32 } else { 1.0_f32 }, if m.selected == i { CYAN } else { border() }))
                            .corner_radius(1);
                        if ui.add_sized([ui.available_width() - 4.0, 42.0], node).clicked() {
                            clicked = Some(i);
                        }
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

    /// P4-26：地图条目文案（图标 + 章节内序号 + 标题 + 难度层 + 状态）
    fn map_entry_text(entry: &crab_quest_core::app::MapEntry, number: usize) -> String {
        let (icon, state_str) = match entry.state {
            crab_quest_core::save::LevelState::Passed => ("[完成]", "已通关"),
            crab_quest_core::save::LevelState::Unlocked => ("[可玩]", "可挑战"),
            crab_quest_core::save::LevelState::Locked => ("[锁定]", "未解锁"),
        };
        let tier = match entry.level.tier {
            LevelTier::L0 => "L0 入门",
            LevelTier::L1 => "L1 所有权",
            LevelTier::L2 => "L2 集合/错误",
            LevelTier::L3 => "L3 生命周期/trait",
            LevelTier::L4 => "L4 挑战",
        };
        format!(
            "{icon} {number}. {}（{tier}）{state_str}",
            entry.level.title
        )
    }

    /// P3-18：统计页（R9 解锁）：段位进度 + 心/XP + 各关尝试/best_time_ms + 成就图鉴。
    fn draw_stats(
        &mut self,
        ctx: &egui::Context,
        app: &mut GameApp,
        s: &crab_quest_core::app::StatsData,
    ) {
        if Self::key(ctx, egui::Key::Escape) || Self::key(ctx, egui::Key::Enter) {
            self.act(app, Input::Esc);
            return;
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(INK)).show(ctx, |ui| {
            glass_frame().show(ui, |ui| {
                ui.label(egui::RichText::new("PROGRESS OVERVIEW").size(12.0).color(CYAN));
                ui.horizontal_wrapped(|ui| {
                    self.icons.show(ui, Icon::Achievement, 24.0, GOLD);
                    ui.heading(egui::RichText::new("统计与成就").color(GOLD));
                    ui.label(egui::RichText::new(format!(
                        "段位 {}（R{}）  ·  {}/{} 关  ·  XP {}  ·  生命 {}",
                        s.rank.title, s.rank.level, s.completed, s.total, s.xp, s.hearts
                    )).color(CYAN));
                });
                ui.add(egui::ProgressBar::new(s.completed as f32 / s.total.max(1) as f32)
                    .fill(MINT)
                    .text(format!("已完成 {}/{}", s.completed, s.total)));
            });
            ui.add_space(12.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("各关记录").strong().color(CYAN));
                    for (i, e) in s.entries.iter().enumerate() {
                        let (icon, state_str) = match e.progress.state {
                            crab_quest_core::save::LevelState::Passed => ("[完成]", "已通关"),
                            crab_quest_core::save::LevelState::Unlocked => ("[可玩]", "可挑战"),
                            crab_quest_core::save::LevelState::Locked => ("[锁定]", "未解锁"),
                        };
                        let best = e
                            .progress
                            .best_time_ms
                            .map(|ms| format!("{ms} ms"))
                            .unwrap_or_else(|| "—".into());
                        raised_frame().show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(format!("{icon} {}. {}", i + 1, e.level.title)).strong());
                                ui.label(egui::RichText::new(format!("L{} · {state_str}", e.level.tier.order())).color(CYAN));
                                ui.label(egui::RichText::new(format!("尝试 {} · 失败 {} · 最快 {best}", e.progress.attempts, e.progress.fail_count)).weak());
                            });
                        });
                    }
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("成就图鉴").strong().color(GOLD));
                    for (id, name, unlocked) in &s.achievements {
                        let _ = id;
                        raised_frame().show(ui, |ui| {
                            if *unlocked {
                                ui.label(egui::RichText::new(format!("🏅 {name}  ✓")).color(GOLD));
                            } else {
                                ui.label(egui::RichText::new(format!("[未获得] {name}")).weak());
                            }
                        });
                    }
                });
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Esc / Enter 返回地图").weak().color(CYAN));
        });
    }

    fn draw_level(&mut self, ctx: &egui::Context, app: &mut GameApp, d: &LevelData) {
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        // 操作栏始终占据窗口最下沿；失败反馈作为编辑器下方紧凑抽屉，避免挤压主区。
        egui::TopBottomPanel::bottom("level_actions")
            .resizable(false)
            .show(ctx, |ui| self.draw_level_actions(ui, app, d));
        egui::SidePanel::left("level_sidebar")
            .resizable(true)
            .default_width(238.0)
            .width_range(190.0..=320.0)
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(13, 22, 48)))
            .show(ctx, |ui| self.draw_level_sidebar(ui, app, d));
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(INK)).show(ctx, |ui| {
            // P2-08/09：心数与连续游玩日实时读取引擎存档（复习回血后即时刷新）
            let hearts = app.save_ref().hearts;
            let streak = app.save_ref().streak_days;
            glass_frame().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.icons.show(ui, Icon::Code, 24.0, GOLD);
                    ui.heading(egui::RichText::new(&d.level.title).color(GOLD));
                    ui.label(egui::RichText::new(format!("L{} · {}/{}", d.level.tier.order(), d.index + 1, d.total)).color(CYAN));
                    self.icons.show(ui, Icon::Xp, 17.0, MINT);
                    ui.label(egui::RichText::new(d.xp.to_string()).color(MINT));
                    self.icons.show(ui, Icon::Combo, 17.0, GOLD);
                    ui.label(egui::RichText::new(format!("{}x", d.combo)).color(GOLD));
                    self.icons.show(ui, Icon::Heart, 17.0, DANGER);
                    ui.label(egui::RichText::new(hearts.to_string()).color(DANGER));
                    if streak > 0 { ui.label(egui::RichText::new(format!("连续 {streak} 天")).color(CYAN)); }
                });
                ui.label(egui::RichText::new(&d.level.description).weak());
            });
            ui.add_space(10.0);
            // P2-11：提示面板——hint_unlock 非空走失败联动（自动推进替代手动逐级），
            // 为空保持 P1 手动逐级揭示（visible_hint / hint_step，旧 TOML 零改动）
            let fail_count = app
                .save_ref()
                .level_states
                .get(&d.level.id)
                .map(|p| p.fail_count)
                .unwrap_or(0);
            // P3-17：Boss 关提示门控（v3 §7.5 提示默认禁用，fail_count ≥ 5 解锁兜底）。
            // 错误码解释卡不受影响（反馈面板照常显示，教学核心不豁免）。
            if boss_hint_locked(d.level.is_boss, fail_count) {
                ui.add_space(4.0);
                ui.colored_label(
                    egui::Color32::from_rgb(180, 160, 120),
                    format!(
                        "🔒 Boss 关提示已禁用（再失败 {} 次解锁）",
                        boss_hint_lock_remaining(fail_count)
                    ),
                );
            } else if !d.level.hint_unlock.is_empty() {
                if let Some(st) =
                    hint_unlock_state(d.level.hints.len(), &d.level.hint_unlock, fail_count)
                {
                    self.draw_unlock_hints(ui, app, d, st);
                }
            } else if let Some((text, cur, total)) = d.visible_hint() {
                ui.add_space(4.0);
                let label = if total > 1 {
                    format!("💡 提示 {cur}/{total}: {text}")
                } else {
                    format!("💡 {text}")
                };
                ui.colored_label(egui::Color32::from_rgb(255, 200, 80), label);
            }
            ui.add_space(6.0);
            if d.level.kind != "quiz" && !self.ime_hint_shown {
                // 弱提示：IME 不可用（egui-miniquad 无 IME 通道），中文只能粘贴
                ui.label(
                    egui::RichText::new("代码编辑器不支持中文输入法，中文内容请复制粘贴").weak(),
                );
                self.ime_hint_shown = true;
            }
            if d.level.kind == "quiz" {
                ui.label(egui::RichText::new("请选择一个答案：").strong());
                let mut selected = d.quiz_answer;
                for (index, option) in d.level.options.iter().enumerate() {
                    if ui
                        .radio_value(&mut selected, Some(index as u32), option)
                        .changed()
                    {
                        self.act(app, Input::SelectQuizAnswer(index as u32));
                    }
                }
            } else {
                raised_frame().show(ui, |ui| {
                    ui.label(egui::RichText::new("CODE WORKBENCH").size(12.0).color(CYAN));
                    // 让主编辑器取得剩余高度；仅为紧凑反馈抽屉预留有限空间。
                    let reserve = if d.feedback.is_some() && self.feedback_drawer_open {
                        FEEDBACK_DRAWER_MAX_HEIGHT + 36.0
                    } else if d.feedback.is_some() {
                        58.0
                    } else {
                        0.0
                    };
                    let editor_height = (ui.available_height() - reserve).max(EDITOR_MIN_HEIGHT);
                    // P3-19：编辑器（行号 gutter + 代码区；支持行号跳转光标）
                    self.draw_editor(ui, app, editor_height);
                });
                if let Some(fb) = &d.feedback {
                    ui.add_space(8.0);
                    self.draw_level_feedback_drawer(ui, app, fb);
                }
            }
        });
    }

    /// 代码关的左侧任务栏：保留主编辑区空间，同时始终显示当前任务与进度信息。
    fn draw_level_sidebar(&mut self, ui: &mut egui::Ui, app: &mut GameApp, d: &LevelData) {
        ui.add_space(8.0);
        glass_frame().show(ui, |ui| {
            ui.label(egui::RichText::new("CURRENT MISSION").size(11.0).color(CYAN));
            ui.horizontal(|ui| {
                self.icons.show(ui, if d.level.is_boss { Icon::Boss } else { Icon::Code }, 21.0, GOLD);
                ui.heading(egui::RichText::new(format!("L{} · 第 {} 关", d.level.tier.order(), d.index + 1)).color(GOLD));
            });
            ui.separator();
            ui.label(egui::RichText::new("任务目标").strong());
            ui.label(egui::RichText::new(&d.level.description).weak());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                self.icons.show(ui, Icon::Xp, 17.0, MINT);
                ui.label(egui::RichText::new(format!("XP {}", d.xp)).color(MINT));
                self.icons.show(ui, Icon::Combo, 17.0, GOLD);
                ui.label(egui::RichText::new(format!("{}x", d.combo)).color(GOLD));
            });
            ui.horizontal(|ui| {
                self.icons.show(ui, Icon::Heart, 18.0, DANGER);
                ui.label(egui::RichText::new(format!("{} / 5", app.save_ref().hearts)).color(DANGER));
            });
        });
        ui.add_space(10.0);
        raised_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                self.icons.show(ui, Icon::Hint, 17.0, CYAN);
                ui.label(egui::RichText::new("快捷操作").strong().color(CYAN));
            });
            ui.label(egui::RichText::new("提交、提示与重置固定在窗口底栏。\n错误卡可点击行号，直接回到对应代码行。").weak());
        });
    }

    /// 固定在关卡窗口底部的操作栏。提交、提示与重置在长代码/长反馈下仍始终可达。
    fn draw_level_actions(&mut self, ui: &mut egui::Ui, app: &mut GameApp, d: &LevelData) {
        let hearts = app.save_ref().hearts;
        let can_submit = hearts > 0;
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(35, 40, 50))
            .stroke(egui::Stroke::new(0.5_f32, border()))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.icons.show(ui, Icon::Play, 18.0, MINT);
                    if ui
                        .add_enabled(
                            can_submit,
                            egui::Button::new(if d.level.kind == "quiz" {
                                "▶ 提交答案"
                            } else {
                                "▶ 提交运行"
                            })
                            .fill(egui::Color32::from_rgb(43, 103, 77)),
                        )
                        .clicked()
                    {
                        self.busy = Busy::Show;
                    }
                    let hint_label = if !d.level.hint_unlock.is_empty() {
                        "提示".to_owned()
                    } else {
                        match d.visible_hint() {
                            Some((_, cur, total)) if total > 1 => format!("提示 {cur}/{total}"),
                            _ => "提示".to_owned(),
                        }
                    };
                    self.icons.show(ui, Icon::Hint, 18.0, GOLD);
                    if ui.button(hint_label).clicked() {
                        self.act(app, Input::Hint);
                    }
                    if ui
                        .button(if d.level.kind == "quiz" {
                            "↺ 清除选择"
                        } else {
                            "↺ 重置代码"
                        })
                        .clicked()
                    {
                        self.act(app, Input::Reset);
                        self.last_level_id = None; // 下一帧重新同步 starter_code
                    }
                    if hearts < 5 && ui.button("复习关卡说明 +1 生命").clicked() {
                        self.act(app, Input::ReviewLore);
                    }
                    if !can_submit {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 190, 100),
                            "生命已空：复习关卡说明可回 1 点",
                        );
                    }
                });
            });
    }

    /// P3-19：编辑器主体（行号 gutter + 代码区）。包装在垂直 ScrollArea 中：
    /// 行号跳转时 `ui.scroll_to_rect` 才能把光标行滚进可视区；高度由主区动态分配。
    fn draw_editor(&mut self, ui: &mut egui::Ui, app: &mut GameApp, editor_height: f32) {
        // 底部固定面板会改变 CentralPanel 的作用域路径；编辑器 id 必须与布局解耦，
        // 否则光标/滚动状态会在面板出现时丢失。
        let edit_id = egui::Id::new(EDITOR_ID_SALT);
        // P3-19：一次性跳转目标（应用后由 take 清除，重渲染不会反复跳回）
        let jump_line = app.take_focus_line();
        if let Some(line) = jump_line {
            // 先把光标写入 TextEditState（egui 持久化状态），本帧绘制即落在目标行首；
            // 同时请求焦点让光标绘制/闪烁并参与事件处理（events 从该光标起步）
            let ccursor = line_start_ccursor(&self.code_buf, line);
            let mut st = egui::TextEdit::load_state(ui.ctx(), edit_id).unwrap_or_default();
            st.cursor
                .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
            st.store(ui.ctx(), edit_id);
            ui.memory_mut(|m| m.request_focus(edit_id));
        }
        egui::ScrollArea::vertical()
            .max_height(editor_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // 行号 gutter（对齐用 monospace 字体）
                    let line_count = self.code_buf.lines().count().max(1);
                    let gutter = (1..=line_count)
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(gutter)
                                .monospace()
                                .color(egui::Color32::from_rgb(120, 120, 120)),
                        )
                        .selectable(false),
                    );
                    // 编辑器（行号 gutter 与代码区共用 TextStyle::Monospace，字号一致）
                    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                    let mut layouter = move |ui: &egui::Ui, text: &str, wrap_width: f32| {
                        let job = code_layout_job(text, wrap_width, &font_id);
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let output = egui::TextEdit::multiline(&mut self.code_buf)
                        .id(edit_id)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows((editor_height / 18.0).floor().max(8.0) as usize)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter)
                        .show(ui);
                    if output.response.changed() {
                        self.sync_code(app);
                    }
                    if jump_line.is_some() {
                        if let Some(cr) = output.cursor_range {
                            // 光标行滚动进可视区：galley 局部坐标 + galley 偏移 → 内容坐标
                            let rect = output
                                .galley
                                .pos_from_cursor(&cr.primary)
                                .translate(output.galley_pos.to_vec2());
                            ui.scroll_to_rect(rect, None);
                        }
                    }
                });
            });
    }

    /// P2-11：失败联动模式提示面板：列出已解锁提示，自动展开索引高亮；
    /// 失败 ≥4 次显示「查看参考答案」按钮（二次确认「先自己试试？」），
    /// 确认后才展示最后一条 hint（解法级修复代码）。查看零成本，确认前不展示代码。
    fn draw_unlock_hints(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut GameApp,
        d: &LevelData,
        st: HintUnlockState,
    ) {
        if d.show_hint {
            ui.add_space(4.0);
            for i in 0..st.unlocked {
                let text = format!(
                    "💡 提示 {}/{}: {}",
                    i + 1,
                    d.level.hints.len(),
                    d.level.hints[i]
                );
                if i == st.expanded {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 200, 80),
                        egui::RichText::new(text).strong(),
                    );
                } else {
                    ui.colored_label(egui::Color32::from_rgb(205, 180, 135), text);
                }
            }
        }
        if st.show_reference {
            ui.add_space(4.0);
            if d.reference_revealed {
                // 已二次确认：展示参考答案（最后一条 hint = 解法级修复代码）
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(45, 55, 42))
                    .corner_radius(1)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("📖 参考答案")
                                .strong()
                                .color(egui::Color32::from_rgb(140, 215, 150)),
                        );
                        if let Some(last) = d.level.hints.last() {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(last)
                                        .monospace()
                                        .color(egui::Color32::from_rgb(170, 210, 175)),
                                )
                                .wrap(),
                            );
                        }
                    });
            } else if ui.button("📖 查看参考答案").clicked() {
                self.ref_dialog = true;
            }
            // 二次确认（v3 §7.6：「先自己试试？」[再想想 / 查看答案]）——
            // 内联确认框（不用浮层 Window：headless 测试不可达，且内联更贴近编辑器上下文）
            if self.ref_dialog {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(58, 52, 34))
                    .corner_radius(1)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("先自己试试？").strong());
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("再想想").clicked() {
                                self.answer_reference(app, false);
                            }
                            if ui.button("查看答案").clicked() {
                                self.answer_reference(app, true);
                            }
                        });
                    });
            }
        }
    }

    /// P2-11：参考答案确认弹窗按钮：`confirm=false` 拒绝（关闭弹窗、不展示任何代码）；
    /// `confirm=true` 确认 → 展示参考答案并记录查看（零成本，engine.reveal_hint 幂等）。
    fn answer_reference(&mut self, app: &mut GameApp, confirm: bool) {
        self.ref_dialog = false;
        if confirm {
            app.reveal_reference();
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
        // 反馈操作同样固定在窗口下沿；中央内容在其上方滚动，不会被长错误卡遮挡。
        egui::TopBottomPanel::bottom("feedback_actions")
            .resizable(false)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(35, 40, 50))
                    .stroke(egui::Stroke::new(0.5_f32, border()))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            self.icons.show(ui, if f.passed { Icon::Passed } else { Icon::Code }, 19.0, if f.passed { MINT } else { GOLD });
                            let primary = if f.passed {
                                if f.unlocked_next.is_some() {
                                    "→ 进入下一关"
                                } else {
                                    "返回地图"
                                }
                            } else {
                                "← 返回编辑"
                            };
                            if ui
                                .add(
                                    egui::Button::new(primary)
                                        .fill(egui::Color32::from_rgb(43, 103, 77)),
                                )
                                .clicked()
                            {
                                self.act(app, Input::Enter);
                            }
                            self.icons.show(ui, Icon::Map, 18.0, CYAN);
                            if ui.button("地图").clicked() {
                                self.act(app, Input::Esc);
                            }
                            ui.label(egui::RichText::new("Enter 确认 · Esc 返回地图").weak());
                        });
                    });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            if f.passed {
                // P3-18：进入通关反馈 → 启动/继续帧驱动庆祝动画（Enter/Esc 随时可离开）
                if self.celebration_level.as_deref() != Some(f.level_id.as_str()) {
                    self.celebration.reset();
                    self.celebration_level = Some(f.level_id.clone());
                }
                self.celebration.tick();
                self.draw_celebration(ui, app, f);
            } else {
                ui.add_space(12.0);
                self.draw_feedback_panel(ui, app, f, true);
            }
        });
    }

    /// P3-18：通关庆祝最小集（v3 §7.7，每项 ≤1s、总 ≈1s，帧驱动无动画库）：
    /// ① XP 数字跳动 + ProgressBar 增长（+XP）→ ② 下一关标题 + 🔓 徽章
    /// （末关 →「🏆 全部通关！」一次性庆典 + 统计页入口）→ ③「已自动保存」+
    /// 首次显示存档路径 → ④ ❤️ +1 → ⑤ Enter/Esc 导航提示（全程可见）。
    /// 阶段内容累积展示（到达后保持），动画不阻塞按键。
    fn draw_celebration(&mut self, ui: &mut egui::Ui, app: &mut GameApp, f: &FeedbackData) {
        // stage 0（首帧未推进）按 1 展示：第 1 帧起 XP 数字即开始跳动
        let stage = self.celebration.stage.max(1);
        let prog = self.celebration.progress();
        ui.vertical_centered(|ui| {
            ui.add_space(28.0);
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(30, 48, 65))
                .stroke(egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(91, 163, 188)))
                .corner_radius(1)
                .inner_margin(egui::Margin::symmetric(26, 20))
                .show(ui, |ui| {
                    self.draw_celebration_sparkles(ui, stage, prog);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("关卡完成")
                                .size(17.0)
                                .strong()
                                .color(egui::Color32::from_rgb(148, 222, 236)),
                        );
                        ui.heading(
                            egui::RichText::new("做得漂亮！")
                                .color(egui::Color32::from_rgb(255, 218, 118))
                                .size(34.0),
                        );
                        // ① XP 数字跳动 + 带圆角容器的进度条
                        if stage >= 1 {
                            ui.add_space(14.0);
                            let xp_total = app.save_ref().xp;
                            let xp_prev = xp_total.saturating_sub(f.xp_gained);
                            ui.label(
                                // 结算数值必须始终是真实结果；只让进度条动画，
                                // 避免首帧把 “+35 XP” 与 “25 → 25” 同时显示。
                                egui::RichText::new(format!("XP {xp_prev}  →  {xp_total}"))
                                    .size(20.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(255, 222, 128)),
                            );
                            egui::Frame::NONE
                                .fill(egui::Color32::from_rgb(20, 32, 44))
                                .corner_radius(1)
                                .inner_margin(5.0)
                                .show(ui, |ui| {
                                    let frac = if f.xp_gained > 0 { prog } else { 0.0 };
                                    ui.add(
                                        egui::ProgressBar::new(frac)
                                            .fill(egui::Color32::from_rgb(104, 205, 151))
                                            .desired_width(300.0)
                                            .text(format!("+{} XP", f.xp_gained)),
                                    );
                                });
                        }
                        // ② 下一关标题 + 解锁徽章（末关显示统计入口）
                        if stage >= 2 {
                            ui.add_space(12.0);
                            if f.victory {
                                ui.label(
                                    egui::RichText::new("全部通关！")
                                        .size(23.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(255, 207, 95)),
                                );
                                if ui.button("📊 查看统计").clicked() {
                                    self.act(app, Input::OpenStats);
                                }
                            } else if let Some(title) = &f.unlocked_next {
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgb(37, 78, 68))
                                    .corner_radius(1)
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(format!("已解锁：{title}"))
                                                .strong()
                                                .color(egui::Color32::from_rgb(170, 235, 193)),
                                        );
                                    });
                            }
                        }
                        // ③ 保存、④回血：以紧凑状态胶囊呈现。
                        if stage >= 3 {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("✓ 进度已自动保存").weak());
                            if let Some(p) = &self.save_path {
                                ui.label(
                                    egui::RichText::new(p.display().to_string())
                                        .weak()
                                        .monospace()
                                        .size(12.0),
                                );
                            }
                        }
                        if stage >= 4 {
                            ui.add_space(6.0);
                            let heart_text = if f.hearts_gained > 0 {
                                format!("生命 +1  ·  当前 {}", f.hearts)
                            } else {
                                format!("生命已满  ·  {}/5", f.hearts)
                            };
                            ui.label(
                                egui::RichText::new(heart_text)
                                    .color(egui::Color32::from_rgb(255, 161, 178)),
                            );
                        }
                    });
                });
        });
    }

    /// 轻量的代码原生星光：随庆典阶段逐渐点亮，不引入外部动画资源。
    fn draw_celebration_sparkles(&self, ui: &mut egui::Ui, stage: usize, progress: f32) {
        let width = ui.available_width().max(1.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 24.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let glow = (stage as f32 / 5.0 + progress * 0.18).min(1.0);
        for (x, y, radius) in [(0.12, 13.0, 2.5), (0.30, 6.0, 1.8), (0.70, 7.0, 2.0), (0.88, 14.0, 3.0)] {
            let center = egui::pos2(rect.left() + rect.width() * x, rect.top() + y);
            let color = egui::Color32::from_rgba_unmultiplied(180, 235, 250, (150.0 * glow) as u8);
            painter.circle_filled(center, radius + glow * 1.5, color);
            painter.line_segment(
                [center - egui::vec2(radius * 2.0, 0.0), center + egui::vec2(radius * 2.0, 0.0)],
                egui::Stroke::new(1.0_f32, color),
            );
            painter.line_segment(
                [center - egui::vec2(0.0, radius * 2.0), center + egui::vec2(0.0, radius * 2.0)],
                egui::Stroke::new(1.0_f32, color),
            );
        }
    }

    /// 编辑器下方的失败反馈抽屉：默认只显示摘要；用户显式展开后才启用独立滚动。
    /// 这避免常驻嵌套 ScrollArea 截获滚轮，同时不在失败后留出大块空白或压扁编辑器。
    fn draw_level_feedback_drawer(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut GameApp,
        f: &FeedbackData,
    ) {
        raised_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("错误反馈").strong().color(GOLD));
                ui.label(egui::RichText::new("修改后可直接再次提交").weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.icons.show(ui, Icon::Heart, 16.0, DANGER);
                    ui.label(egui::RichText::new(f.hearts.to_string()).color(DANGER));
                });
            });
            let label = if self.feedback_drawer_open {
                "收起详细反馈"
            } else {
                "展开详细反馈"
            };
            if ui.button(label).clicked() {
                self.feedback_drawer_open = !self.feedback_drawer_open;
            }
            if self.feedback_drawer_open {
                egui::ScrollArea::vertical()
                    .id_salt(("level-feedback", &f.level_id))
                    .max_height(FEEDBACK_DRAWER_MAX_HEIGHT)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                            if let Some(card) = f.errors.first() {
                                self.draw_error_card(ui, app, card);
                            }
                            if f.errors.len() > 1 {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "还有 {} 个错误，可回到完整反馈页查看",
                                        f.errors.len() - 1
                                    ))
                                    .weak(),
                                );
                            }
                            if let Some(panic) = &f.panic {
                                self.draw_panic_card(ui, panic);
                            }
                            if let Some(diff) = &f.expectation {
                                Self::draw_output_diff(ui, diff);
                            }
                    });
            }
        });
    }

    /// P1-03 失败反馈面板主体（Feedback 屏与编辑器底部固定区共用）。
    /// `show_nav_hint`：Feedback 屏提示「Enter 返回编辑 / Esc 回地图」，编辑器内不重复。
    fn draw_feedback_panel(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut GameApp,
        f: &FeedbackData,
        show_nav_hint: bool,
    ) {
        // 防挫败语气（v3 §7.6）：「❌ 未通过」→「🔧 还差一点」
        glass_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("🔧 还差一点").color(GOLD));
                ui.label(egui::RichText::new("编译器已经给出了下一步线索").weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("❤️ {}", f.hearts)).color(DANGER));
                });
            });
        });
        self.draw_toast(ui);
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // ①-⑤ 编译错误卡片：第一条默认展开，其余折叠「还有 N 个错误」逐个展开
                if !f.errors.is_empty() {
                    self.draw_error_card(ui, app, &f.errors[0]);
                    if f.errors.len() > 1 {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "还有 {} 个错误（点击展开）",
                                f.errors.len() - 1
                            ))
                            .weak(),
                        );
                        for (i, card) in f.errors.iter().enumerate().skip(1) {
                            egui::CollapsingHeader::new(
                                egui::RichText::new(self.error_head(card)).monospace(),
                            )
                            .id_salt(("err", i))
                            .default_open(false)
                            .show(ui, |ui| {
                                // P3-19：展开后整卡渲染（行号同样可点击跳转）
                                self.draw_error_card(ui, app, card);
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

    /// 单张错误卡（默认完全展开）：① 徽章+行号+标题 ② zh 展开 ③ 怎么改折叠 ④ 原文折叠 ⑤ 链接。
    /// P3-19：行号以链接样式（蓝色 + 下划线）渲染，点击 → 编辑器光标跳到该行。
    fn draw_error_card(&mut self, ui: &mut egui::Ui, app: &mut GameApp, card: &ErrorCard) {
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(61, 43, 59))
            .stroke(egui::Stroke::new(0.5_f32, egui::Color32::from_rgba_unmultiplied(202, 120, 150, 100)))
            .corner_radius(1)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(&card.code)
                            .monospace()
                            .strong()
                            .color(GOLD)
                            .background_color(egui::Color32::from_rgb(83, 54, 49)),
                    );
                    if let Some(line) = card.line {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("第 {line} 行"))
                                    .color(egui::Color32::from_rgb(110, 170, 255))
                                    .underline(),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if resp.clicked() {
                            app.jump_to_line(line);
                        }
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
            .fill(egui::Color32::from_rgb(70, 38, 61))
            .stroke(egui::Stroke::new(0.5_f32, DANGER))
            .corner_radius(1)
            .inner_margin(12.0)
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

/// P3-19：把 0-based 行号转换为该行行首的字符索引（egui CCursor 用字符索引而非字节）。
/// 行号越界（≥ 行数）时钳制到文件末尾；空文件恒为 0。
/// 字节偏移恒为 UTF-8 字符边界（逐行 len 累加 + '\n'），切片安全。
fn line_start_ccursor(code: &str, line: usize) -> egui::text::CCursor {
    let mut byte = 0;
    for (i, l) in code.split('\n').enumerate() {
        if i == line {
            break;
        }
        byte += l.len() + 1; // 该行字节数 + '\n'
    }
    let byte = byte.min(code.len()); // 越界 → 文件末尾
    egui::text::CCursor::new(code[..byte].chars().count())
}

/// eframe::App 壳层：持有核心状态机 + UI 状态，由 eframe(winit) 驱动事件循环。
/// winit 原生支持 IME（中文输入法）——解决 egui-miniquad 无 IME 通道的问题，
/// 并统一 X11/Wayland/Windows/macOS 窗口行为（跨平台）。
pub struct CrabQuestApp {
    game: GameApp,
    ui: GameUi,
    fonts_installed: bool,
    online_probed: bool,
}

impl CrabQuestApp {
    pub fn new(game: GameApp, ui: GameUi) -> Self {
        Self { game, ui, fonts_installed: false, online_probed: false }
    }
}

impl eframe::App for CrabQuestApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 首帧安装中文字体（一次性，避免每帧重建字体图集）
        if !self.fonts_installed {
            install_fonts(ctx);
            self.fonts_installed = true;
        }
        // 首次进入时探测一次在线状态（≤3s，缓存 offline 标志），此后不再探测
        if !self.online_probed {
            self.ui.offline = !probe_online();
            self.online_probed = true;
        }
        let was_quit = self.ui.quit;
        self.ui.draw(ctx, &mut self.game);
        // 菜单「退出」→ 请求关闭窗口 → on_exit 落盘
        if self.ui.quit && !was_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(path) = self.ui.save_path.clone() {
            if let Err(e) = save::save(self.game.save_ref(), &path) {
                eprintln!("存档失败: {e}");
            }
        }
    }
}

/// P1-03 链接降级探测：对 rustwiki.org:80 发 HTTP/1.0 HEAD，3 秒超时。
/// 设计选择：不加 HTTP/TLS 依赖，纯 TCP 头探测——任何 HTTP 状态行即视为在线；
/// 结果缓存到 `GameUi::offline`，仅启动时执行一次。
fn probe_online() -> bool {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let addr = match "rustwiki.org:80"
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.next())
    {
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
    use crab_quest_core::app::{GameApp, Input, Screen};
    use crab_quest_core::engine::Engine;
    use crab_quest_core::level::{parse_levels, LevelSet};
    use crab_quest_core::sandbox::DevSandbox;
    use crab_quest_core::save::SaveData;
    use crab_quest_core::validate::mapper::ErrorMapper;

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
    fn desktop_theme_and_level_workbench_are_applied() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let out = ctx.run(egui::RawInput::default(), |ctx| ui.draw(ctx, &mut app));
        assert_eq!(ctx.style().visuals.panel_fill, INK, "全局面板应使用深靛蓝底色");
        assert!(shapes_contain_text(&out.shapes, "CURRENT MISSION"));
        assert!(shapes_contain_text(&out.shapes, "CODE WORKBENCH"));
    }

    #[test]
    fn acrylic_frames_are_translucent_with_hairline_dividers() {
        let glass = glass_frame();
        let raised = raised_frame();
        assert_eq!(INK.a(), 0, "根画布必须透明，不能以深色面板覆盖原生窗口");
        assert!(glass.fill.a() < 255 && raised.fill.a() < 255, "面板应为透明亚克力层");
        assert!(glass.stroke.width <= 0.5 && raised.stroke.width <= 0.5, "仅保留细分隔线");
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
        assert!(
            ui.code_buf.contains("fn main"),
            "code_buf 未同步: {}",
            ui.code_buf
        );
        assert!(matches!(app.screen(), Screen::Level(_)));
    }

    #[test]
    fn quiz_level_renders_choices_without_code_editor() {
        let ctx = egui::Context::default();
        let levels = parse_levels(
            "[[level]]\nid = \"q\"\ntitle = \"题目\"\ntier = \"l0\"\ndescription = \"选一个\"\nkind = \"quiz\"\noptions = [\"选项甲\", \"选项乙\"]\nanswer_index = 1\nsource = \"x\"\n",
        )
        .unwrap();
        let engine = Engine::new(
            LevelSet { levels },
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        let mut app = GameApp::new(engine);
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();

        let out = ctx.run(egui::RawInput::default(), |ctx| ui.draw(ctx, &mut app));
        assert!(shapes_contain_text(&out.shapes, "请选择一个答案"));
        assert!(shapes_contain_text(&out.shapes, "选项甲"));
        assert!(shapes_contain_text(&out.shapes, "选项乙"));
        assert!(shapes_contain_text(&out.shapes, "▶ 提交答案"));
        assert!(shapes_contain_text(&out.shapes, "↺ 清除选择"));
        assert!(
            !shapes_contain_text(&out.shapes, "代码编辑器不支持中文输入法"),
            "选择题不应渲染代码编辑器专属提示"
        );
    }

    /// P3-20：双字体安装不破坏无头环境——Proportional 用 Noto Sans SC 渲染中文正文，
    /// Monospace 保持 maple（等宽 + CJK 覆盖），弱提示在装字体后仍正常排版。
    #[test]
    fn install_fonts_dual_family_headless() {
        let ctx = egui::Context::default();
        // egui 字体系统需先跑一帧初始化（Context::run 前无字体可用）；set_fonts 在下一 pass 生效
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        install_fonts(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {}); // 应用新字体定义
        let prop_ok = ctx.fonts(|f| f.has_glyphs(&egui::FontId::proportional(16.0), "你好，变量"));
        let mono_ok =
            ctx.fonts(|f| f.has_glyphs(&egui::FontId::monospace(16.0), "let x = 5; fn main"));
        assert!(prop_ok, "Proportional 应能用 Noto Sans SC 渲染中文正文");
        assert!(mono_ok, "Monospace 应能用 maple 渲染代码");
        // 装字体后照常绘制关卡，弱提示文本不丢
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap(); // 进入关卡
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(ui.ime_hint_shown);
        assert!(
            shapes_contain_text(&out.shapes, "复制粘贴"),
            "装字体后首次进入编辑器仍应显示「中文请复制粘贴」提示"
        );
    }

    // ===== P2-08/09：hearts / streak（headless）=====

    #[test]
    fn zero_hearts_disables_submit_and_shows_review_message() {
        let ctx = egui::Context::default();
        let save = SaveData {
            hearts: 0,
            streak_days: 3,
            ..SaveData::default()
        };
        let mut app = test_app_with_save(save);
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap(); // 进入关卡
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out.shapes, "生命已空：复习关卡说明可回 1 点"),
            "0 心应显示复习引导文案"
        );
        assert!(
            shapes_contain_text(&out.shapes, "▶ 提交运行"),
            "提交按钮仍在（add_enabled 置灰）"
        );
        assert!(
            shapes_contain_text(&out.shapes, "连续 3 天"),
            "状态栏应显示连续天数"
        );
        // 0 心绘制不触发提交（busy 保持 None）
        assert_eq!(ui.busy, Busy::None);
    }

    #[test]
    fn passed_submission_persists_before_window_exit() {
        let mut app = test_app();
        let mut ui = GameUi::new();
        let path = std::env::temp_dir().join(format!(
            "rust-learning-crab-quest-ui-save-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        ui.set_save_path(&path);
        app.handle(Input::Enter).unwrap();
        app.set_code("fn main() {}".to_owned());
        ui.act(&mut app, Input::Submit);
        assert!(matches!(app.screen(), Screen::Feedback(f) if f.passed));
        let saved = std::fs::read_to_string(&path).expect("通关后应立即写入存档");
        assert!(saved.contains("t:pass"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn review_button_visible_below_full_hearts_only() {
        // 心 < 5 → 复习按钮可见；心 > 0 不显示 0 心提示
        let ctx = egui::Context::default();
        let mut app = test_app_with_save(SaveData {
            hearts: 3,
            ..SaveData::default()
        });
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out.shapes, "复习关卡说明"),
            "心 <5 显示复习按钮"
        );
        assert!(
            !shapes_contain_text(&out.shapes, "❤️ 已空"),
            "心 >0 不显示 0 心提示"
        );

        // 满心（cap 5）→ 复习按钮隐藏（无可回）
        let mut app2 = test_app_with_save(SaveData {
            hearts: 5,
            ..SaveData::default()
        });
        let mut ui2 = GameUi::new();
        app2.handle(Input::Enter).unwrap();
        let out2 = ctx.run(egui::RawInput::default(), |ctx| {
            ui2.draw(ctx, &mut app2);
        });
        assert!(
            !shapes_contain_text(&out2.shapes, "复习关卡说明"),
            "满心隐藏复习按钮"
        );
    }

    #[test]
    fn streak_hidden_when_zero_days() {
        let ctx = egui::Context::default();
        let mut app = test_app_with_save(SaveData {
            hearts: 3,
            streak_days: 0,
            ..SaveData::default()
        });
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            !shapes_contain_text(&out.shapes, "连续 0 天"),
            "未活跃不显示连续天数"
        );
        assert!(shapes_contain_text(&out.shapes, "3"), "头部应显示心数数值");
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
        assert!(
            galley.rows.len() > 1,
            "100px 宽的 CJK 长行应换行成多行，实际 {} 行",
            galley.rows.len()
        );

        // 旧行为（wrap_width = INFINITY）回归对照：只有一行
        let job_inf = code_layout_job(&cjk, f32::INFINITY, &resolved);
        let mut galley_inf = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            galley_inf = Some(ctx.fonts(|f| f.layout_job(job_inf.clone())));
        });
        assert_eq!(
            galley_inf.unwrap().rows.len(),
            1,
            "INFINITY 宽度下 CJK 长行应保持单行"
        );
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
        let expected = egui::TextStyle::Monospace
            .resolve(&egui::Style::default())
            .size;
        let mut code_sizes: Vec<f32> = Vec::new();
        let mut gutter_sizes: Vec<f32> = Vec::new();
        for clipped in &out.shapes {
            if let egui::Shape::Text(t) = &clipped.shape {
                let text = t.galley.text();
                let sizes: Vec<f32> = t
                    .galley
                    .job
                    .sections
                    .iter()
                    .map(|s| s.format.font_id.size)
                    .collect();
                if text.contains("fn main") {
                    code_sizes.extend(sizes);
                } else if !text.is_empty()
                    && text.chars().all(|c| c.is_ascii_digit() || c == '\n')
                    && t.galley.job.sections.iter().all(|s| {
                        s.format.color == egui::Color32::from_rgb(120, 120, 120)
                    })
                {
                    gutter_sizes.extend(sizes);
                }
            }
        }
        assert!(!code_sizes.is_empty(), "应绘制出代码区文本");
        assert!(!gutter_sizes.is_empty(), "应绘制出行号 gutter");
        assert!(
            code_sizes.iter().all(|&s| s == expected)
                && gutter_sizes.iter().all(|&s| s == expected),
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

    fn text_y(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<f32> {
        shapes.iter().find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(t) if t.galley.text().contains(needle) => Some(t.pos.y),
            _ => None,
        })
    }

    #[test]
    fn level_actions_stay_at_window_bottom() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| ui.draw(ctx, &mut app),
        );
        let submit_y = text_y(&out.shapes, "▶ 提交运行").expect("应渲染提交操作");
        assert!(
            submit_y > 620.0,
            "提交操作应固定在 700px 窗口的底部，而不是随内容滚动：y={submit_y}"
        );
    }

    #[test]
    fn passed_feedback_renders_celebration_card_and_bottom_actions() {
        let ctx = egui::Context::default();
        let levels = parse_levels(
            "[[level]]\nid = \"p\"\ntitle = \"p\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nsource = \"x\"\n",
        )
        .unwrap();
        let engine = Engine::new(
            LevelSet { levels },
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        let mut app = GameApp::new(engine);
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        app.handle(Input::Submit).unwrap();
        assert!(matches!(app.screen(), Screen::Feedback(f) if f.passed));
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| ui.draw(ctx, &mut app),
        );
        assert!(shapes_contain_text(&out.shapes, "关卡完成"));
        assert!(shapes_contain_text(&out.shapes, "做得漂亮！"));
        let action_y = text_y(&out.shapes, "返回地图").expect("应渲染固定反馈操作");
        assert!(action_y > 620.0, "反馈操作应固定在窗口底部：y={action_y}");
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
            hearts_gained: 0,
            errors,
            expectation: None,
            panic: None,
            unlocked_next: None,
            victory: false,
        }
    }

    fn draw_panel(
        ctx: &egui::Context,
        crab_quest_ui: &mut GameUi,
        fb: &FeedbackData,
    ) -> egui::FullOutput {
        ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crab_quest_ui.draw_feedback_panel(ui, &mut test_app(), fb, true);
            });
        })
    }

    /// 多错误：第一条默认展开，其余折叠「还有 N 个错误」且逐个可展开；怎么改/原文默认折叠
    #[test]
    fn feedback_panel_expands_only_first_error() {
        let ctx = egui::Context::default();
        let fb = fail_fb(vec![
            card(
                "E0425",
                Some(2),
                "找不到名字：变量、函数或类型未定义",
                "检查拼写或先定义",
                Some("https://doc.rust-lang.org/error_codes/E0425.html"),
            ),
            card(
                "E0308",
                Some(5),
                "类型不匹配：表达式的实际类型与期望类型不一致",
                "",
                None,
            ),
        ]);
        let mut crab_quest_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut crab_quest_ui, &fb);
        assert!(
            shapes_contain_text(&out.shapes, "🔧 还差一点"),
            "失败语气应为「还差一点」"
        );
        assert!(
            shapes_contain_text(&out.shapes, "找不到名字"),
            "第一条 zh 应默认展开"
        );
        // ① 头行组件：徽章 + 行号 + 中文标题（分开渲染）
        assert!(shapes_contain_text(&out.shapes, "E0425"), "错误码徽章缺失");
        assert!(shapes_contain_text(&out.shapes, "第 2 行"), "行号缺失");
        assert!(
            shapes_contain_text(&out.shapes, "还有 1 个错误"),
            "「还有 N 个错误」提示缺失"
        );
        assert!(
            shapes_contain_text(&out.shapes, "E0308 · 第 5 行 · 类型不匹配"),
            "第二条折叠头缺失"
        );
        assert!(
            !shapes_contain_text(&out.shapes, "类型不匹配：表达式的实际类型与期望类型不一致"),
            "第二条 zh 应折叠隐藏"
        );
        assert!(
            !shapes_contain_text(&out.shapes, "检查拼写或先定义"),
            "「怎么改」应默认折叠"
        );
        assert!(
            !shapes_contain_text(&out.shapes, "rustc summary"),
            "rustc 原文应默认折叠"
        );
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
        let mut crab_quest_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut crab_quest_ui, &fb);
        assert!(
            shapes_contain_text(&out.shapes, "程序运行崩溃（索引越界）"),
            "panic 分类标题缺失"
        );
        assert!(
            !shapes_contain_text(&out.shapes, "index out of bounds"),
            "净化消息应默认折叠"
        );
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
        let mut crab_quest_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut crab_quest_ui, &fb);
        assert!(
            shapes_contain_text(&out.shapes, "期望输出"),
            "diff 期望列表头缺失"
        );
        assert!(
            shapes_contain_text(&out.shapes, "实际输出"),
            "diff 实际列表头缺失"
        );
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
        let mut crab_quest_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut crab_quest_ui, &fb);
        assert!(
            shapes_contain_text(&out.shapes, "EUNKNOWN"),
            "兜底卡徽章缺失"
        );
        assert!(
            shapes_contain_text(&out.shapes, "编译错误"),
            "兜底卡 zh 非空白"
        );
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
        assert!(
            !shapes_contain_text(&out.shapes, "概念详解"),
            "离线不应渲染可点链接"
        );

        let mut online_ui = GameUi::new();
        let out2 = draw_panel(&ctx, &mut online_ui, &fb);
        assert!(
            shapes_contain_text(&out2.shapes, "概念详解"),
            "在线应渲染链接"
        );
    }

    /// 失败后返回编辑器：反馈作为代码区下方紧凑抽屉保留，不再占用底部整块空间。
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
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| ui.draw(ctx, &mut app),
        );
        assert!(
            shapes_contain_text(&out.shapes, "错误反馈"),
            "返回编辑后应保留紧凑错误反馈抽屉"
        );
        assert!(!ui.feedback_drawer_open, "抽屉默认关闭，不应常驻嵌套滚动区抢滚轮");
        let workbench_y = text_y(&out.shapes, "CODE WORKBENCH").expect("代码工作台应存在");
        let drawer_y = text_y(&out.shapes, "错误反馈").expect("错误抽屉应存在");
        assert!(drawer_y > workbench_y + 120.0, "错误抽屉应位于编辑器下方");
        assert!(drawer_y < 650.0, "错误抽屉不应被推到固定底栏外或留下整块空白");
    }

    /// toast：离线点击链接后设置 toast 文案，下一次绘制可见（3 秒内）
    #[test]
    fn feedback_link_offline_click_sets_toast() {
        let mut crab_quest_ui = GameUi::new();
        crab_quest_ui.offline = true;
        crab_quest_ui.set_toast("无法打开在线教材（当前离线）");
        let ctx = egui::Context::default();
        let fb = fail_fb(vec![card(
            "E0425",
            None,
            "找不到名字",
            "",
            Some("https://doc.rust-lang.org/error_codes/E0425.html"),
        )]);
        let out = draw_panel(&ctx, &mut crab_quest_ui, &fb);
        assert!(
            shapes_contain_text(&out.shapes, "无法打开在线教材"),
            "toast 应绘制出来"
        );
    }

    // ===== P3-19：行号跳转编辑器（headless）=====

    fn shapes_contain_underlined(shapes: &[egui::epaint::ClippedShape], needle: &str) -> bool {
        shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Text(t) => {
                t.galley.text().contains(needle)
                    && t.galley
                        .job
                        .sections
                        .iter()
                        .any(|s| s.format.underline.width > 0.0)
            }
            _ => false,
        })
    }

    #[test]
    fn error_line_rendered_as_clickable_link() {
        let ctx = egui::Context::default();
        // line = Some → 链接样式（下划线）行号
        let fb = fail_fb(vec![card("E0308", Some(2), "类型不匹配", "", None)]);
        let mut crab_quest_ui = GameUi::new();
        let out = draw_panel(&ctx, &mut crab_quest_ui, &fb);
        assert!(shapes_contain_text(&out.shapes, "第 2 行"), "行号应渲染");
        assert!(
            shapes_contain_underlined(&out.shapes, "第 2 行"),
            "行号应为可点击链接样式（下划线）"
        );
        // line = None（EUNKNOWN 无 --> 行）→ 不渲染可点击行号
        let fb_none = fail_fb(vec![card("EUNKNOWN", None, "编译错误", "", None)]);
        let mut crab_quest_ui2 = GameUi::new();
        let out2 = draw_panel(&ctx, &mut crab_quest_ui2, &fb_none);
        assert!(
            !shapes_contain_text(&out2.shapes, "第 "),
            "line=None 不渲染行号"
        );
    }

    #[test]
    fn error_line_click_sets_focus_line() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        app.handle(Input::Enter).unwrap(); // 进入编辑器（last_level 就位）
        let mut crab_quest_ui = GameUi::new();
        let fb = fail_fb(vec![card("E0308", Some(2), "类型不匹配", "", None)]);
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
        let raw = |events: Vec<egui::Event>| egui::RawInput {
            screen_rect: Some(screen_rect),
            events,
            ..Default::default()
        };
        // 帧 1：定位「第 2 行」文本位置（与后续帧同一 screen_rect，布局一致）
        let mut line_pos: Option<egui::Pos2> = None;
        ctx.run(raw(vec![]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crab_quest_ui.draw_feedback_panel(ui, &mut app, &fb, true);
            });
        })
        .shapes
        .iter()
        .for_each(|c| {
            if let egui::Shape::Text(t) = &c.shape {
                if t.galley.text() == "第 2 行" {
                    // 点击标签中心
                    line_pos =
                        Some(t.pos + egui::vec2(t.galley.size().x / 2.0, t.galley.size().y / 2.0));
                }
            }
        });
        let pos = line_pos.expect("应绘制出「第 2 行」");
        let button = |pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        // 帧 2：按下（PointerMoved + PointerButton 更新指针状态与 press_origin）
        let _ = ctx.run(
            raw(vec![egui::Event::PointerMoved(pos), button(true)]),
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    crab_quest_ui.draw_feedback_panel(ui, &mut app, &fb, true);
                });
            },
        );
        // 帧 3：释放（click 在 release 触发）
        let _ = ctx.run(
            raw(vec![egui::Event::PointerMoved(pos), button(false)]),
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    crab_quest_ui.draw_feedback_panel(ui, &mut app, &fb, true);
                });
            },
        );
        assert_eq!(
            app.focus_line,
            Some(1),
            "点击「第 2 行」→ 跳转目标 0-based 1"
        );
        assert!(
            matches!(app.screen(), Screen::Level(_)),
            "点击行号应回到编辑器屏"
        );
    }

    #[test]
    fn focus_jump_applies_cursor_at_line_start() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut crab_quest_ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        // 首帧：同步 code_buf
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        let code = "fn main() {\n    let x = 1;\n    println!(\"hi\");\n}";
        crab_quest_ui.code_buf = code.into();
        crab_quest_ui.sync_code(&mut app);
        app.focus_line = Some(2); // 第 3 行（0-based）
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        assert_eq!(app.focus_line, None, "跳转应用后清除（一次性）");
        // 读取稳定编辑器 id 的持久化状态：光标应等于第 3 行行首。
        let id = egui::Id::new(EDITOR_ID_SALT);
        let st = egui::TextEdit::load_state(&ctx, id).expect("编辑器应有持久化光标状态");
        let cc = st.cursor.char_range().expect("应有光标范围");
        let expected = code
            .split('\n')
            .take(2)
            .map(|l| l.chars().count() + 1)
            .sum::<usize>();
        assert_eq!(cc.primary.index, expected, "光标应落在第 3 行行首");
    }

    #[test]
    fn focus_jump_out_of_range_clamps_to_end() {
        // 纯函数：越界行 → 文件末尾字符索引
        let code = "fn main() {\n    let x = 1;\n}";
        let cc = line_start_ccursor(code, 99);
        assert_eq!(cc.index, code.chars().count(), "越界行钳制到文件末尾");
        assert_eq!(line_start_ccursor(code, 0).index, 0);
        assert_eq!(line_start_ccursor("", 0).index, 0, "空文件恒 0");
        // 端到端：focus_line 越界 → 应用后光标在末尾
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut crab_quest_ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        crab_quest_ui.code_buf = code.into();
        crab_quest_ui.sync_code(&mut app);
        app.focus_line = Some(99);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        let id = egui::Id::new(EDITOR_ID_SALT);
        let st = egui::TextEdit::load_state(&ctx, id).expect("编辑器应有持久化光标状态");
        let cc = st.cursor.char_range().unwrap();
        assert_eq!(
            cc.primary.index,
            code.chars().count(),
            "越界跳转光标在文件末尾"
        );
    }

    #[test]
    fn focus_jump_switches_between_lines() {
        // 连续两次跳转：第二次覆盖第一次（不同错误 → 光标正确切换）
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut crab_quest_ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        let code = "a\nbb\nccc\ndddd";
        crab_quest_ui.code_buf = code.into();
        crab_quest_ui.sync_code(&mut app);
        app.focus_line = Some(1); // 第 2 行
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        app.focus_line = Some(3); // 第 4 行
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        let id = egui::Id::new(EDITOR_ID_SALT);
        let st = egui::TextEdit::load_state(&ctx, id).unwrap();
        let cc = st.cursor.char_range().unwrap();
        let expected = code
            .split('\n')
            .take(3)
            .map(|l| l.chars().count() + 1)
            .sum::<usize>();
        assert_eq!(
            cc.primary.index, expected,
            "第二次跳转覆盖第一次（光标在第 4 行行首）"
        );
    }

    // ===== P2-11：hint 失败联动 UI（headless）=====

    fn unlock_hint_test_app() -> GameApp {
        let levels = parse_levels(
            "[[level]]\nid = \"h\"\ntitle = \"h\"\ntier = \"l1\"\ndescription = \"d\"\nhints = [\"概念\", \"定位\", \"解法\"]\nhint_unlock = [1, 3, 5]\nstarter_code = \"fn main() { println!(1); }\"\nexpect_output = \"1\"\nsource = \"x\"\n",
        )
        .unwrap();
        let engine = Engine::new(
            LevelSet { levels },
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        GameApp::new(engine)
    }

    fn set_fail_count(app: &mut GameApp, level_id: &str, fail_count: u32) {
        app.engine
            .save
            .level_states
            .entry(level_id.into())
            .or_default()
            .fail_count = fail_count;
    }

    #[test]
    fn reference_button_visible_only_at_four_fails() {
        let ctx = egui::Context::default();
        // fc=1：无参考答案按钮
        let mut app1 = unlock_hint_test_app();
        app1.handle(Input::Enter).unwrap();
        set_fail_count(&mut app1, "h", 1);
        let mut ui1 = GameUi::new();
        let out1 = ctx.run(egui::RawInput::default(), |ctx| {
            ui1.draw(ctx, &mut app1);
        });
        assert!(
            !shapes_contain_text(&out1.shapes, "查看参考答案"),
            "fc=1 不应出现参考答案按钮"
        );
        // fc=4：出现
        let mut app4 = unlock_hint_test_app();
        app4.handle(Input::Enter).unwrap();
        set_fail_count(&mut app4, "h", 4);
        let mut ui4 = GameUi::new();
        let out4 = ctx.run(egui::RawInput::default(), |ctx| {
            ui4.draw(ctx, &mut app4);
        });
        assert!(
            shapes_contain_text(&out4.shapes, "查看参考答案"),
            "fc=4 应出现参考答案按钮"
        );
        assert!(
            !shapes_contain_text(&out4.shapes, "先自己试试"),
            "未点击不弹确认框"
        );
    }

    #[test]
    fn reference_dialog_confirm_reveals_reject_hides() {
        let ctx = egui::Context::default();
        // —— 拒绝路径：弹窗 → 再想想 → 不展示任何代码
        let mut app = unlock_hint_test_app();
        app.handle(Input::Enter).unwrap();
        set_fail_count(&mut app, "h", 4);
        let mut crab_quest_ui = GameUi::new();
        crab_quest_ui.ref_dialog = true;
        let out_open = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out_open.shapes, "先自己试试？"),
            "确认弹窗文案"
        );
        assert!(shapes_contain_text(&out_open.shapes, "再想想"), "拒绝按钮");
        assert!(
            shapes_contain_text(&out_open.shapes, "查看答案"),
            "确认按钮"
        );
        assert!(
            !shapes_contain_text(&out_open.shapes, "📖 参考答案"),
            "确认前不展示答案块"
        );
        // 拒绝
        crab_quest_ui.answer_reference(&mut app, false);
        assert!(!crab_quest_ui.ref_dialog, "拒绝后弹窗关闭");
        let out_rej = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        assert!(
            !shapes_contain_text(&out_rej.shapes, "先自己试试？"),
            "拒绝后弹窗消失"
        );
        assert!(
            !shapes_contain_text(&out_rej.shapes, "📖 参考答案"),
            "拒绝后不展示代码"
        );
        assert_eq!(
            app.engine
                .save
                .level_states
                .get("h")
                .map(|p| p.hints_used.clone())
                .unwrap_or_default(),
            Vec::<u32>::new(),
            "拒绝不记录查看"
        );
        match app.screen() {
            Screen::Level(d) => assert!(!d.reference_revealed, "拒绝不标记展示"),
            other => panic!("expected Level, got {other:?}"),
        }
        // —— 确认路径：查看答案 → 展示参考答案（最后一条 hint）+ 记录查看
        crab_quest_ui.ref_dialog = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        crab_quest_ui.answer_reference(&mut app, true);
        assert_eq!(
            app.engine.save.level_states.get("h").unwrap().hints_used,
            vec![2],
            "确认后记录最后一条 hint 查看"
        );
        let out_conf = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out_conf.shapes, "📖 参考答案"),
            "确认后展示答案块"
        );
        assert!(
            shapes_contain_text(&out_conf.shapes, "解法"),
            "答案块含最后一条 hint 内容"
        );
    }

    #[test]
    fn hint_panel_lists_unlocked_hints_with_expanded_highlight() {
        let ctx = egui::Context::default();
        let mut app = unlock_hint_test_app();
        app.handle(Input::Enter).unwrap();
        set_fail_count(&mut app, "h", 3); // 全部解锁，自动推进到 hint[1]
        app.handle(Input::Hint).unwrap(); // 打开面板（联动模式）
        let mut crab_quest_ui = GameUi::new();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            crab_quest_ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out.shapes, "💡 提示 1/3: 概念"),
            "hint1 列出"
        );
        assert!(
            shapes_contain_text(&out.shapes, "💡 提示 2/3: 定位"),
            "hint2 列出"
        );
        assert!(
            shapes_contain_text(&out.shapes, "💡 提示 3/3: 解法"),
            "hint3 列出（fc=3 全解锁）"
        );
        // fc=3 无参考答案按钮
        assert!(!shapes_contain_text(&out.shapes, "查看参考答案"));
    }

    // ===== P4-26：自定义关卡章节（地图渲染 + 游戏内错误提示）=====

    fn custom_map_app() -> GameApp {
        let levels = parse_levels(
            "[[level]]\nid = \"b1\"\ntitle = \"内置一\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(1); }\"\nexpect_output = \"1\"\nsource = \"x\"\n",
        )
        .unwrap();
        let custom = parse_levels(
            "[[level]]\nid = \"c1\"\ntitle = \"自定义关一\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(2); }\"\nexpect_output = \"2\"\nsource = \"community\"\n",
        )
        .unwrap();
        let engine = Engine::with_custom_levels(
            LevelSet { levels },
            custom,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        GameApp::new(engine)
    }

    #[test]
    fn map_shows_custom_section_when_custom_levels_exist() {
        let ctx = egui::Context::default();
        let mut app = custom_map_app();
        let mut ui = GameUi::new();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out.shapes, "自定义关卡"),
            "存在自定义关卡时应显示章节标题"
        );
        assert!(
            shapes_contain_text(&out.shapes, "自定义关一"),
            "自定义关卡条目应渲染"
        );
        assert!(
            shapes_contain_text(&out.shapes, "内置一"),
            "内置关卡条目应渲染"
        );
        // 章节内序号从 1 开始：自定义关一 → 「1. 自定义关一」
        assert!(
            shapes_contain_text(&out.shapes, "1. 自定义关一"),
            "自定义章节内序号应从 1 开始"
        );
    }

    #[test]
    fn map_hides_custom_section_without_custom_levels() {
        let ctx = egui::Context::default();
        let mut app = test_app(); // 无自定义关卡
        let mut ui = GameUi::new();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            !shapes_contain_text(&out.shapes, "自定义关卡"),
            "无自定义关卡时隐藏章节"
        );
    }

    #[test]
    fn map_surfaces_custom_load_errors_in_game() {
        let ctx = egui::Context::default();
        let levels = parse_levels(
            "[[level]]\nid = \"b1\"\ntitle = \"内置一\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(1); }\"\nexpect_output = \"1\"\nsource = \"x\"\n",
        )
        .unwrap();
        let engine = Engine::new(
            LevelSet { levels },
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        let errs = vec!["自定义关卡 bad.toml 加载失败：TOML 解析失败：xxx".to_string()];
        let mut app = GameApp::with_custom_load_errors(engine, errs);
        let mut ui = GameUi::new();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(
            shapes_contain_text(&out.shapes, "自定义关卡加载失败"),
            "地图页应显示加载失败警示"
        );
        assert!(
            shapes_contain_text(&out.shapes, "bad.toml"),
            "警示内容含文件名"
        );
    }
}
