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

/// P4-26：用户自定义关卡目录（默认位置）：
/// `~/.local/share/crab-quest/levels/`。该目录不存在时游戏行为与现状一致
/// （无自定义章节、不报错）。
pub fn user_levels_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local/share/crab-quest/levels")
}

/// P4-26：从命令行参数解析自定义关卡目录：`--levels <dir>` 优先，未提供回退默认用户目录。
/// 支持同时传 `--levels` 与后续参数（只取第一个 `--levels` 的值）。
pub fn custom_levels_dir_from_args<'a, I: Iterator<Item = String>>(args: I) -> PathBuf {
    let mut iter = args;
    while let Some(a) = iter.next() {
        if a == "--levels" {
            if let Some(dir) = iter.next() {
                return PathBuf::from(dir);
            }
        }
    }
    user_levels_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_levels_dir_flag_overrides_default() {
        // --levels <dir> → 显式目录
        let args = ["--levels".to_string(), "/tmp/my-levels".to_string()];
        assert_eq!(custom_levels_dir_from_args(args.into_iter()), PathBuf::from("/tmp/my-levels"));
        // 参数出现在中间/末尾均可
        let args = ["--foo".to_string(), "--levels".to_string(), "rel/levels".to_string(), "tail".to_string()];
        assert_eq!(custom_levels_dir_from_args(args.into_iter()), PathBuf::from("rel/levels"));
    }

    #[test]
    fn custom_levels_dir_defaults_to_user_dir() {
        // 无 --levels → 回退 ~/.local/share/crab-quest/levels
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let expect = PathBuf::from(home).join(".local/share/crab-quest/levels");
        assert_eq!(custom_levels_dir_from_args(Vec::<String>::new().into_iter()), expect);
        // --levels 缺值 → 同样回退默认
        let args = ["--levels".to_string()];
        assert_eq!(custom_levels_dir_from_args(args.into_iter()), expect);
        assert_eq!(user_levels_dir(), expect);
    }
}
