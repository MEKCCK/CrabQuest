pub fn placeholder() {}

pub mod error;
pub mod level;

pub use error::GameError;
pub use level::{Level, LevelSet, LevelTier};
