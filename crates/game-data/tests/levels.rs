#[test]
fn all_levels_parse_and_consistent() {
    let set = game_core::LevelSet::load(&game_data::levels_dir()).expect("关卡目录加载失败");
    assert!(set.len() >= 29, "关卡总数应至少 29（15 存量 + 各批新增），实际 {}", set.len());
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
    // schema v2 新增字段全部 serde(default)：未显式设置的关卡保持缺省值；
    // 显式设置的（expect_panic 关 / quiz 关）由各自专项断言覆盖
    for l in &set.levels {
        assert!(l.kind == "code" || l.kind == "quiz", "{} kind 非法: {}", l.id, l.kind);
        assert!(
            l.expect_panic.is_empty() || l.expect_output.is_empty(),
            "{} expect_panic 与 expect_output 互斥",
            l.id
        );
        assert!(
            l.hint_unlock.is_empty() || l.hint_unlock.len() == l.hints.len(),
            "{} hint_unlock 长度必须与 hints 一致",
            l.id
        );
        if l.kind == "code" && l.expect_panic.is_empty() {
            assert!(l.options.is_empty(), "{} code 关不应有 options", l.id);
            assert_eq!(l.answer_index, None, "{} code 关不应有 answer_index", l.id);
        }
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
            "kind = \"quiz\"\noptions = [\"a\", \"a\", \"b\"]\nanswer_index = 0",
            "重复",
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

/// T3（P2-13）：rustlings 三章 +8 关解析断言
#[test]
fn t3_rustlings_levels_parse_ok() {
    let set = game_core::LevelSet::load(&game_data::levels_dir()).expect("关卡目录加载失败");
    let t3_ids = [
        "l1-move2",
        "l1-move3",
        "l2-errors3",
        "l2-errors2",
        "l2-errors4",
        "l2-vecs2",
        "l3-lifetime3",
        "l3-lifetime1",
    ];
    let mut seen = std::collections::HashSet::new();
    for id in t3_ids {
        let l = set.get(id).unwrap_or_else(|| panic!("T3 关卡 {id} 缺失"));
        assert!(!l.starter_code.trim().is_empty(), "{id} starter_code 非空");
        assert!(!l.source.is_empty(), "{id} source 非空");
        assert_eq!(l.hints.len(), 3, "{id} hints 应为三级");
        assert!(seen.insert(id), "{id} id 重复");
        assert!(!l.allow_compile_fail, "{id} 为普通 code 关");
    }
    assert_eq!(seen.len(), t3_ids.len(), "T3 关卡 id 应全局唯一");
}

/// P2-14 第二批新增关（100-exercises 3 关 + rust-quiz 5 关）解析一致性
#[test]
fn batch2_100ex_quiz_levels_parse_and_consistent() {
    let set = game_core::LevelSet::load(&game_data::levels_dir()).expect("关卡目录加载失败");
    let ids = [
        "l0-integers",          // 100ex 02/01_integers，E0308
        "l1-ownership-ticket",  // 100ex 03_ticket_v1/06_ownership，E0382
        "l2-saturating",        // 100ex 02/09_saturating，运行期溢出
        "l4-lazy-map",          // rust-quiz 026，输出 112031
        "l4-fnptr",             // rust-quiz 011，allow_compile_fail E0794
        "l4-drop-underscore",   // rust-quiz 019，输出 21
        "l4-lifetime-ext",      // rust-quiz 037，输出 1001
        "l4-fnmut-copy",        // rust-quiz 036，输出 1223
    ];
    let expect_outputs = [
        "1 + 2 * 4 = 9",
        "周末演唱会\nA 区前排\n已出票",
        "factorial(5) = 120\nfactorial(20) = 4294967295",
        "112031",
        "0",
        "21",
        "1001",
        "1223",
    ];
    for (id, expect) in ids.iter().zip(expect_outputs.iter()) {
        let l = set.get(id).unwrap_or_else(|| panic!("缺少关卡 {id}"));
        assert!(!l.source.is_empty(), "{id} 缺少 source");
        assert_eq!(l.hints.len(), 3, "{id} 应有 3 级 hints");
        assert_eq!(l.kind, "code", "{id} 应为 code 型");
        assert_eq!(l.expect_output, *expect, "{id} expect_output 不一致");
        if *id == "l4-fnptr" {
            assert!(l.allow_compile_fail, "l4-fnptr 应为 allow_compile_fail 关");
            assert_eq!(l.expect_error_code, "E0794", "l4-fnptr 预期错误码 E0794");
            assert_eq!(l.link, "https://dtolnay.github.io/rust-quiz/011");
        } else {
            assert!(!l.allow_compile_fail, "{id} 不应为 allow_compile_fail 关");
        }
    }
    // rust-quiz 关应带官方链接（除 fnptr 已断言外）
    for id in ["l4-lazy-map", "l4-drop-underscore", "l4-lifetime-ext", "l4-fnmut-copy"] {
        let l = set.get(id).unwrap();
        assert!(!l.link.is_empty(), "{id} 缺少 link");
        assert!(l.source.starts_with("rust-quiz"), "{id} source 应标 rust-quiz");
    }
    // 100-exercises 关：source 标注 CC BY-NC 4.0 且 starter 无 helpers 依赖
    for id in ["l0-integers", "l1-ownership-ticket", "l2-saturating"] {
        let l = set.get(id).unwrap();
        assert!(l.source.starts_with("100-exercises-to-learn-rust"), "{id} source 应标 100-exercises");
        assert!(l.source.contains("CC BY-NC 4.0"), "{id} source 应含 CC BY-NC 4.0");
        assert!(!l.starter_code.contains("ticket_fields"), "{id} 不得依赖 helpers");
        assert!(!l.starter_code.contains("common::"), "{id} 不得依赖 helpers");
        assert!(l.starter_code.contains("fn main"), "{id} 必须补 main");
    }
}

/// P3-22 扩展槽关 54-l2-panics（expect_panic 型）解析断言
#[test]
fn expect_panic_level_54_parses() {
    let set = game_core::LevelSet::load(&game_data::levels_dir()).expect("关卡目录加载失败");
    let l = set.get("l2-panics").expect("缺少 54-l2-panics 关");
    assert_eq!(l.tier, game_core::LevelTier::L2);
    assert_eq!(l.kind, "code");
    assert_eq!(l.expect_panic, "index out of bounds");
    assert!(l.expect_output.is_empty(), "expect_panic 关不得填写 expect_output");
    assert_eq!(l.hints.len(), 3, "应有 3 级 hints");
    assert!(!l.source.is_empty(), "缺少 source");
    // starter 不得包含期望子串（W7：无答案痕迹）
    assert!(!l.starter_code.contains("index out of bounds"), "starter 泄露期望子串");
    assert!(l.starter_code.contains("fn main"), "starter 必须含 main");
}

/// 互斥校验端到端：expect_panic 与 expect_output 同时非空 → 加载报错（T2 落地，本测试回归锁定）
#[test]
fn expect_panic_and_output_mutually_exclusive() {
    let toml = "[[level]]\nid = \"t-both\"\ntitle = \"t\"\ntier = \"l2\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nsource = \"s\"\nexpect_output = \"x\"\nexpect_panic = \"boom\"\n";
    match load_single_level(toml) {
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("互斥"), "错误「{msg}」应含「互斥」");
            assert!(msg.contains("t-both"), "错误应带关卡 id: {msg}");
        }
        other => panic!("期望加载失败，实际 {other:?}"),
    }
}

/// P3-21 首个选择题关 49-l4-mutable-zst（kind=quiz）解析断言
#[test]
fn quiz_level_49_parses() {
    let set = game_core::LevelSet::load(&game_data::levels_dir()).expect("关卡目录加载失败");
    let l = set.get("l4-mutable-zst").expect("缺少 49-l4-mutable-zst 关");
    assert_eq!(l.kind, "quiz", "49 关应为 quiz 型");
    assert_eq!(l.tier, game_core::LevelTier::L4);
    assert_eq!(l.options, vec!["0", "1", "编译错误", "不确定"]);
    assert_eq!(l.answer_index, Some(1), "答案应为选项 1");
    assert!(!l.starter_code.trim().is_empty(), "quiz 展示代码不能为空");
    assert!(!l.description.trim().is_empty(), "description 不能为空");
    assert_eq!(l.hints.len(), 3, "应有 3 级 hints");
    assert!(l.source.starts_with("rust-quiz"), "source 应标 rust-quiz");
    assert_eq!(l.link, "https://dtolnay.github.io/rust-quiz/013");
    assert!(l.expect_output.is_empty(), "quiz 关不应有 expect_output");
    assert!(!l.allow_compile_fail, "quiz 关不应为 allow_compile_fail");
}
