pub fn placeholder() {}

pub mod achievements;
pub mod app;
pub mod editor;
pub mod engine;
pub mod error;
pub mod level;
pub mod rank;
pub mod sandbox;
pub mod save;
pub mod streak;
pub mod validate;

pub use app::{
    ChapterMapData, FeedbackData, GameApp, GameFlow, Input, LevelData, MapEntry, MenuData, Screen,
};
pub use editor::{tokenize, TokenKind, TokenSpan};
pub use engine::{
    boss_hint_lock_remaining, boss_hint_locked, hint_unlock_state, load_custom_levels,
    min_best_time, CustomLevelError, Engine, HintUnlockState, XP_BOSS, XP_BOSS_FALLBACK, XP_COMBO,
    XP_PASS, XP_PERFECT,
};
pub use error::GameError;
pub use level::{Level, LevelSet, LevelTier};
pub use rank::{rank_for, Rank};
pub use sandbox::{BwrapSandbox, CompileOutcome, DevSandbox, RunOutcome, Sandbox};
pub use save::{load as load_save, save as save_game, LevelProgress, LevelState, SaveData};
pub use streak::{
    days_from_civil, is_yesterday, parse_date, previous_day, today_str, touch_streak,
};

pub use achievements::{achievement_name, check_achievements, AchievementCheck, ACHIEVEMENTS};


pub use validate::error_parser::{parse_rustc_stderr, CompileError};
pub use validate::mapper::{ErrorInfo, ErrorMapper};
pub use validate::{validate, Validation};
