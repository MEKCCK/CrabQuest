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
    // schema v2 新增字段全部 serde(default)：存量 15 关未写新字段，缺省值一致
    for l in &set.levels {
        assert_eq!(l.kind, "code", "{} kind 缺省应为 code", l.id);
        assert!(l.expect_panic.is_empty(), "{} expect_panic 缺省为空", l.id);
        assert!(l.hint_unlock.is_empty(), "{} hint_unlock 缺省为空", l.id);
        assert!(!l.is_boss, "{} is_boss 缺省为 false", l.id);
        assert!(!l.trim_lines, "{} trim_lines 缺省为 false", l.id);
        assert!(l.options.is_empty(), "{} options 缺省为空", l.id);
        assert_eq!(l.answer_index, None, "{} answer_index 缺省为 None", l.id);
        assert!(l.link.is_empty(), "{} link 缺省为空", l.id);
    }
}

/// 把单关 TOML 写入临时目录后走 LevelSet::load 加载路径，返回结果
fn load_single_level(toml: &str) -> Result<game_core::LevelSet, game_core::GameError> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rlg-levels-test-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("00-test.toml"), toml).unwrap();
    let result = game_core::LevelSet::load(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn invalid_level_data_rejected_with_chinese_errors() {
    let base = "[[level]]\nid = \"t1\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nsource = \"s\"\n";
    let cases: &[(&str, &str)] = &[
        // (追加行, 期望中文子串)
        ("kind = \"quiz\"\noptions = [\"a\"]", "options 必须为 2-6 项"),
        (
            "kind = \"quiz\"\noptions = [\"a\", \"b\", \"c\"]\nanswer_index = 5",
            "越界",
        ),
        (
            "hints = [\"h1\", \"h2\"]\nhint_unlock = [1]",
            "hint_unlock 长度",
        ),
        ("expect_output = \"x\"\nexpect_panic = \"boom\"", "互斥"),
        ("expect_output = \"a\\r\\nb\"", "回车符"),
    ];
    for (extra, expect_zh) in cases {
        let toml = format!("{base}{extra}\n");
        match load_single_level(&toml) {
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains(expect_zh),
                    "case {extra:?}: 错误「{msg}」不含「{expect_zh}」"
                );
            }
            other => panic!("case {extra:?}: 期望加载失败，实际 {other:?}"),
        }
    }
}

#[test]
fn quiz_level_with_valid_options_loads() {
    let toml = r#"
[[level]]
id = "l4-quiz"
title = "零大小类型"
tier = "l4"
description = "描述"
kind = "quiz"
hints = ["概念", "定位", "解法"]
hint_unlock = [1, 3, 5]
starter_code = "fn main() {}"
options = ["0", "1", "编译错误", "不确定"]
answer_index = 1
source = "rust-quiz (questions/013, CC BY-SA 4.0，解释自写)"
is_boss = true
trim_lines = true
link = "https://rustwiki.org/zh-CN/rust-by-example/"
"#;
    let set = load_single_level(toml).expect("合法 quiz 关应加载成功");
    assert_eq!(set.len(), 1);
    let l = &set.levels[0];
    assert_eq!(l.kind, "quiz");
    assert_eq!(l.options, vec!["0", "1", "编译错误", "不确定"]);
    assert_eq!(l.answer_index, Some(1));
    assert_eq!(l.hint_unlock, vec![1, 3, 5]);
    assert!(l.is_boss);
    assert!(l.trim_lines);
    assert_eq!(l.link, "https://rustwiki.org/zh-CN/rust-by-example/");
}

#[test]
fn errors_toml_has_required_codes() {
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    for code in ["E0308", "E0382", "E0502", "E0596", "E0106"] {
        assert!(mapper.lookup(code).is_some(), "缺少错误码 {code}");
    }
}
