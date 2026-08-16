use thiserror::Error;

#[derive(Debug, Error)]
pub enum GameError {
    #[error("TOML 解析失败 {0}: {1}")]
    TomlParse(String, String),
    #[error("关卡数据校验失败 {0}: {1}")]
    LevelDataInvalid(String, String),
    #[error("关卡目录不存在或为空: {0}")]
    LevelDirNotFound(String),
    #[error("关卡 ID 重复: {0}")]
    DuplicateLevelId(String),
    #[error("关卡不存在: {0}")]
    LevelNotFound(String),
    #[error("关卡未解锁: {0}")]
    LevelLocked(String),
    #[error("关卡类型不匹配: {0}")]
    LevelKindMismatch(String),
    #[error("选择题选项越界: 提交 {index}（0-based），选项共 {len} 项")]
    QuizAnswerOutOfRange { index: u32, len: usize },
    #[error("编译超时（超过 {0} 秒）")]
    CompileTimeout(u64),
    #[error("运行超时（超过 {0} 秒）")]
    RunTimeout(u64),
    #[error("编译环境错误: {0}")]
    CompileEnv(String),
    #[error("运行环境错误: {0}")]
    RunEnv(String),
    #[error("存档损坏: {0}")]
    CorruptSave(String),
    #[error("沙盒拦截: {0}")]
    SandboxBlocked(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}
