pub fn placeholder() {}

pub mod error;
pub mod level;
pub mod save;

pub use error::GameError;
pub use level::{Level, LevelSet, LevelTier};
pub use save::{load as load_save, save as save_game, LevelProgress, LevelState, SaveData};
