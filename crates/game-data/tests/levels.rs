#[test]
fn all_levels_parse_and_consistent() {
    let set = game_core::LevelSet::load(&game_data::levels_dir()).expect("关卡目录加载失败");
    assert_eq!(set.len(), 15, "第一版应有 15 关");
    let mut tiers = std::collections::BTreeSet::new();
    for l in &set.levels {
        tiers.insert(l.tier.order());
        assert!(!l.starter_code.trim().is_empty(), "{} 缺少 starter_code", l.id);
        assert!(!l.source.is_empty(), "{} 缺少 source", l.id);
        if l.allow_compile_fail {
            assert!(!l.expect_error_code.is_empty(), "{} 需 expect_error_code", l.id);
        }
    }
    assert_eq!(tiers.len(), 5, "应覆盖 L0-L4 全部难度层");
}

#[test]
fn errors_toml_has_required_codes() {
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    for code in ["E0308", "E0382", "E0502", "E0596", "E0106"] {
        assert!(mapper.lookup(code).is_some(), "缺少错误码 {code}");
    }
}
