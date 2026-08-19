use crate::app::GameApp;
use crate::error::GameError;

/// UI 后端抽象：核心产出 Screen/Input 纯数据，后端负责渲染与事件采集。
/// macroquad+egui 是实现之一；未来 ratatui 版再实现一份。
/// async：macroquad 的主循环依赖 async（next_frame().await），trait 方法用原生 async fn（Rust 1.75+）。
pub trait UiBackend {
    async fn run(&mut self, app: &mut GameApp) -> Result<(), GameError>;
}
