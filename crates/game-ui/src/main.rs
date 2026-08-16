use game_core::app::GameApp;
use game_core::engine::Engine;
use game_core::level::LevelSet;
use game_core::sandbox::DevSandbox;
use game_core::save;
use game_core::ui::UiBackend;
use game_core::validate::mapper::ErrorMapper;
use game_ui::GameUi;
use std::collections::HashSet;

fn save_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".local/share/rust-learning-game/save.toml")
}

#[macroquad::main("Rust 学习游戏")]
async fn main() {
    let level_set = match LevelSet::load(&game_data::levels_dir()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("关卡加载失败: {e}");
            std::process::exit(1);
        }
    };
    // P4-26：自定义关卡目录——`--levels <dir>` 覆盖默认用户目录
    // `~/.local/share/rust-learning-game/levels/`；目录不存在时无自定义章节（行为与现状一致）。
    let custom_dir = game_data::custom_levels_dir_from_args(std::env::args().skip(1));
    let builtin_ids: HashSet<String> =
        level_set.levels.iter().map(|l| l.id.clone()).collect();
    let (custom_levels, custom_errors) =
        game_core::load_custom_levels(&custom_dir, &builtin_ids);
    for err in &custom_errors {
        // 启动日志：逐文件中文报错；其余文件照常加载，游戏不崩溃
        eprintln!("{}", err.message());
    }
    let save_data = save::load(&save_path()).unwrap_or_default();
    let mapper = match ErrorMapper::load(&game_data::errors_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("错误码映射加载失败（使用兜底表）: {e}");
            ErrorMapper::default_fallback()
        }
    };
    let engine = Engine::with_custom_levels(
        level_set,
        custom_levels,
        save_data,
        mapper,
        Box::new(DevSandbox::new()),
    );
    let mut app = GameApp::with_custom_load_errors(
        engine,
        custom_errors.iter().map(|e| e.message()).collect(),
    );
    let mut ui = GameUi::new();
    // P3-18：通关庆祝「已自动保存」阶段首次显示存档路径
    ui.set_save_path(save_path().display().to_string());
    if let Err(e) = ui.run(&mut app).await {
        eprintln!("运行错误: {e}");
    }
    if let Err(e) = save::save(app.save_ref(), &save_path()) {
        eprintln!("存档失败: {e}");
    }
}
