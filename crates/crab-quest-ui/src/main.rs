use crab_quest_core::app::GameApp;
use crab_quest_core::engine::Engine;
use crab_quest_core::level::LevelSet;
use crab_quest_core::sandbox::BwrapSandbox;
use crab_quest_core::validate::mapper::ErrorMapper;
use crab_quest_ui::{CrabQuestApp, GameUi};
use std::collections::HashSet;

fn save_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".local/share/crab-quest/save.toml")
}

fn main() {
    let level_set = match LevelSet::load(&crab_quest_data::levels_dir()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("关卡加载失败: {e}");
            std::process::exit(1);
        }
    };
    // P4-26：自定义关卡目录——`--levels <dir>` 覆盖默认用户目录
    // `~/.local/share/crab-quest/levels/`；目录不存在时无自定义章节（行为与现状一致）。
    let custom_dir = crab_quest_data::custom_levels_dir_from_args(std::env::args().skip(1));
    let builtin_ids: HashSet<String> = level_set.levels.iter().map(|l| l.id.clone()).collect();
    let (custom_levels, custom_errors) =
        crab_quest_core::load_custom_levels(&custom_dir, &builtin_ids);
    for err in &custom_errors {
        eprintln!("{}", err.message());
    }
    let save_data = crab_quest_core::save::load(&save_path()).unwrap_or_default();
    let mapper = match ErrorMapper::load(&crab_quest_data::errors_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("错误码映射加载失败（使用兜底表）: {e}");
            ErrorMapper::default_fallback()
        }
    };
    // P4-24：bwrap 真隔离沙盒。启动时探测一次完整隔离调用；bwrap 缺失或
    // 内核不允许用户命名空间 → 显式中文错误并退出，绝不静默降级到无隔离模式。
    let sandbox = match BwrapSandbox::try_new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let engine = Engine::with_custom_levels(level_set, custom_levels, save_data, mapper, Box::new(sandbox));
    let app = GameApp::with_custom_load_errors(
        engine,
        custom_errors.iter().map(|e| e.message()).collect(),
    );
    let mut ui = GameUi::new();
    ui.set_save_path(save_path().display().to_string());

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("CrabQuest"),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "CrabQuest",
        options,
        Box::new(move |_cc| Ok(Box::new(CrabQuestApp::new(app, ui)))),
    ) {
        eprintln!("CrabQuest 启动失败: {e}");
        std::process::exit(1);
    }
}
