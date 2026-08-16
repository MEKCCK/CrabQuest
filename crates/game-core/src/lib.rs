pub fn placeholder() {}

pub mod editor;
pub mod engine;
pub mod error;
pub mod level;
pub mod sandbox;
pub mod save;
pub mod validate;

pub use editor::{tokenize, TokenKind, TokenSpan};
pub use engine::{Engine, XP_PER_PASS};
pub use error::GameError;
pub use level::{Level, LevelSet, LevelTier};
pub use sandbox::{CompileOutcome, DevSandbox, RunOutcome, Sandbox};
pub use save::{load as load_save, save as save_game, LevelProgress, LevelState, SaveData};

pub use validate::error_parser::{parse_rustc_stderr, CompileError};
pub use validate::mapper::{ErrorInfo, ErrorMapper};
pub use validate::{validate, Validation};
