pub fn placeholder() {}

pub mod error;
pub mod level;
pub mod save;
pub mod validate;

pub use error::GameError;
pub use level::{Level, LevelSet, LevelTier};
pub use save::{load as load_save, save as save_game, LevelProgress, LevelState, SaveData};

pub use validate::error_parser::{parse_rustc_stderr, CompileError};
pub use validate::mapper::{ErrorInfo, ErrorMapper};
