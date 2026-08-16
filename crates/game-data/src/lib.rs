use std::path::PathBuf;

/// 资源目录（workspace 根下 assets/）
pub fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub fn levels_dir() -> PathBuf {
    assets_dir().join("levels")
}

pub fn errors_path() -> PathBuf {
    assets_dir().join("errors.toml")
}
