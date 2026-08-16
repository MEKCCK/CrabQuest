use game_core::app::GameApp;
use game_core::engine::Engine;
use game_core::level::LevelSet;
use game_core::sandbox::DevSandbox;
use game_core::save;
use game_core::ui::UiBackend;
use game_core::validate::mapper::ErrorMapper;
use game_ui::GameUi;

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
    let save_data = save::load(&save_path()).unwrap_or_default();
    let mapper = match ErrorMapper::load(&game_data::errors_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("错误码映射加载失败（使用兜底表）: {e}");
            ErrorMapper::default_fallback()
        }
    };
    let engine = Engine::new(level_set, save_data, mapper, Box::new(DevSandbox::new()));
    let mut app = GameApp::new(engine);
    let mut ui = GameUi::new();
    if let Err(e) = ui.run(&mut app).await {
        eprintln!("运行错误: {e}");
    }
    if let Err(e) = save::save(app.save_ref(), &save_path()) {
        eprintln!("存档失败: {e}");
    }
}
