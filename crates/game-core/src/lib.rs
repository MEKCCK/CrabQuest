pub fn placeholder() {}

pub mod app;
pub mod editor;
pub mod engine;
pub mod error;
pub mod level;
pub mod rank;
pub mod sandbox;
pub mod save;
pub mod ui;
pub mod validate;

pub use app::{ChapterMapData, FeedbackData, GameApp, GameFlow, Input, LevelData, MapEntry, MenuData, Screen};
pub use editor::{tokenize, TokenKind, TokenSpan};
pub use engine::{Engine, XP_BOSS, XP_BOSS_FALLBACK, XP_COMBO, XP_PASS, XP_PERFECT};
pub use error::GameError;
pub use level::{Level, LevelSet, LevelTier};
pub use rank::{rank_for, Rank};
pub use sandbox::{CompileOutcome, DevSandbox, RunOutcome, Sandbox};
pub use save::{load as load_save, save as save_game, LevelProgress, LevelState, SaveData};

pub use ui::UiBackend;

pub use validate::error_parser::{parse_rustc_stderr, CompileError};
pub use validate::mapper::{ErrorInfo, ErrorMapper};
pub use validate::{validate, Validation};
