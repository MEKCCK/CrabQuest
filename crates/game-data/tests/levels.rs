#[test]
fn all_levels_parse_and_consistent() {
    let set = game_core::LevelSet::load(&game_data::levels_dir()).expect("关卡目录加载失败");
    assert_eq!(set.len(), 55, "当前内置关卡集应有 55 关");
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
    // 主线按文件名线性解锁，因此加载后的 tier 必须单调递增；尤其不能把 L2
    // 扩展关排到 L4 终章之后。
    assert!(
        set.levels.windows(2).all(|pair| pair[0].tier.order() <= pair[1].tier.order()),
        "主线难度顺序倒退: {:?}",
        set.levels
            .windows(2)
            .filter(|pair| pair[0].tier.order() > pair[1].tier.order())
            .map(|pair| format!("{} -> {}", pair[0].id, pair[1].id))
            .collect::<Vec<_>>()
    );
    let panic_index = set
        .levels
        .iter()
        .position(|l| l.id == "l2-panics")
        .expect("缺少 l2-panics");
    let l2_boss_index = set
        .levels
        .iter()
        .position(|l| l.id == "l2-boss")
        .expect("缺少 l2-boss");
    assert!(panic_index < l2_boss_index, "l2-panics 应在 l2-boss 前完成");
    // schema v2：代码关不携带选择题字段；选择题必须带合法选项与答案。
    // 其余 v2 字段（panic / hints / Boss 等）由各关按教学需要使用，不能再假定全部缺省。
    let mut quiz_count = 0;
    for l in &set.levels {
        match l.kind.as_str() {
            "code" => {
                assert!(l.options.is_empty(), "代码关 {} 不应有选择题选项", l.id);
                assert_eq!(l.answer_index, None, "代码关 {} 不应有正确选项", l.id);
            }
            "quiz" => {
                quiz_count += 1;
                assert!((2..=6).contains(&l.options.len()), "选择题 {} 选项数非法", l.id);
                assert!(l.answer_index.is_some(), "选择题 {} 缺少正确选项", l.id);
            }
            other => panic!("关卡 {} kind 非法: {other}", l.id),
        }
    }
    assert_eq!(quiz_count, 1, "当前素材应恰有 1 个选择题关卡");
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

/// P1-02 验收：活跃条目全部含 zh/link/severity/concept；zh ≤60、fix ≤40、example ≤8 行；
/// link_zh 仅允许 rustwiki.org 白名单（v3 §6.3）。
#[test]
fn errors_toml_v2_entries_complete_and_bounded() {
    use game_core::validate::mapper::Severity;
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    assert!(!mapper.is_empty());
    for (code, info) in mapper.iter() {
        assert!(!info.zh.is_empty(), "{code} 缺 zh");
        assert!(
            info.link.starts_with("https://doc.rust-lang.org/error_codes/E"),
            "{code} link 非法: {}",
            info.link
        );
        assert!(
            matches!(info.severity, Severity::P0 | Severity::P1 | Severity::P2),
            "{code} severity 非法"
        );
        assert!(info.concept.is_some(), "{code} 缺 concept");
        assert!(info.zh.chars().count() <= 60, "{code} zh 超 60 字: {}", info.zh);
        if let Some(fix) = &info.fix {
            assert!(fix.chars().count() <= 40, "{code} fix 超 40 字: {fix}");
        }
        if let Some(example) = &info.example {
            assert!(example.lines().count() <= 8, "{code} example 超 8 行");
        }
        if let Some(link_zh) = &info.link_zh {
            assert!(
                link_zh.starts_with("https://rustwiki.org/"),
                "{code} link_zh 非 rustwiki 白名单: {link_zh}"
            );
        }
    }
}

/// P1-02 验收：新增码 12+ 枚全部收录（这里断言 14 枚全在，活跃条目 ≥32）
#[test]
fn errors_toml_has_all_new_entries() {
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    let new_codes = [
        "E0282", "E0384", "E0594", "E0621", "E0601", // P0 5 码
        "E0506",                                        // P1 borrow 家族
        "E0046", "E0283", "E0381", "E0603",             // rustlings 缺口
        "E0072", "E0423",                               // rustlings 缺口 D2
        "E0063", "E0794",                               // 关卡大纲已收录码
    ];
    let active_count = mapper.iter().count();
    assert!(active_count >= 32, "活跃条目应 ≥32，实际 {active_count}");
    for code in new_codes {
        let info = mapper.lookup(code).unwrap_or_else(|| panic!("新增码 {code} 未收录"));
        assert!(!info.zh.is_empty());
        assert!(!info.link.is_empty());
    }
}

/// P1-02 验收：全部 P0 码（A 组 6 + B 组 5）在活跃表中且 severity=P0
#[test]
fn errors_toml_all_p0_codes_present() {
    use game_core::validate::mapper::Severity;
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    for code in [
        "E0425", "E0596", "E0382", "E0106", "E0599", "E0597",
        "E0282", "E0384", "E0594", "E0621", "E0601",
    ] {
        let info = mapper.lookup(code).unwrap_or_else(|| panic!("缺少 P0 码 {code}"));
        assert_eq!(info.severity, Severity::P0, "{code} 应为 P0");
    }
}

/// P1-02 验收：E0412/E0504 不在活跃查找中，且已在 [deprecated] 登记（注明替代码）
#[test]
fn errors_toml_deprecated_codes_absent_and_registered() {
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    assert!(mapper.lookup("E0412").is_none(), "E0412 死码不得出现在活跃映射");
    assert!(mapper.lookup("E0504").is_none(), "E0504 死码不得出现在活跃映射");
    let r1 = mapper.deprecated_reason("E0412").expect("E0412 应在 [deprecated] 登记");
    assert!(r1.contains("E0425"), "E0412 登记原因应注明替代码: {r1}");
    let r2 = mapper.deprecated_reason("E0504").expect("E0504 应在 [deprecated] 登记");
    assert!(r2.contains("E0506"), "E0504 登记原因应注明替代码: {r2}");
}

/// P1-02 验收：[fallback] 段解析成功且指向官方错误码索引
#[test]
fn errors_toml_has_fallback_section() {
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    let fb = mapper.fallback().expect("errors.toml 必须含 [fallback] 段");
    assert!(!fb.zh.is_empty());
    assert_eq!(fb.link, "https://doc.rust-lang.org/error_codes/index.html");
}

/// T8（P2-15）验收：P0 12 张基础卡（A 组 6 + B 组 5 + EUNKNOWN 兜底）zh/fix/example 全填充。
/// 8 张高频卡（L3-D2 精修）人话首段 ≤30 字；全表术语合规（v3 §6.1：无 生存期/特征/悬挂）。
#[test]
fn errors_toml_p0_cards_complete_and_terminology_clean() {
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    let p0 = [
        "E0425", "E0596", "E0382", "E0106", "E0599", "E0597",
        "E0282", "E0384", "E0594", "E0621", "E0601",
    ];
    for code in p0 {
        let info = mapper.lookup(code).unwrap_or_else(|| panic!("缺少 P0 码 {code}"));
        assert!(!info.zh.is_empty(), "{code} 缺 zh");
        assert!(
            info.fix.as_deref().is_some_and(|f| !f.is_empty()),
            "{code} 缺 fix（P0 基础卡要求修复方向全填充）"
        );
        assert!(
            info.example.as_deref().is_some_and(|e| !e.trim().is_empty()),
            "{code} 缺 example（P0 基础卡要求复现代码全填充）"
        );
    }
    // EUNKNOWN 兜底卡：fallback zh 非空（ErrorMapper 侧已断言 link，此处锁内容非空）
    let fb = mapper.fallback().expect("errors.toml 必须含 [fallback] 段");
    assert!(!fb.zh.is_empty());

    // 8 张高频卡：人话（zh 首个「：」之前）≤30 字（v3 §6.2 卡片「一句话人话」上限）
    let refined = [
        "E0382", "E0502", "E0499", "E0596", "E0106", "E0308", "E0277", "E0507",
    ];
    for code in refined {
        let info = mapper.lookup(code).unwrap_or_else(|| panic!("缺少高频卡 {code}"));
        let human = info.zh.split('：').next().unwrap_or("");
        assert!(
            human.chars().count() <= 30,
            "{code} 人话首段超 30 字: {human}（zh={}）",
            info.zh
        );
    }

    // 术语合规：全表 zh/fix 无 v3 §6.1 禁用词（特征= trait 译名禁用；卡片文案全部规避）
    let banned = ["生存期", "特征", "悬挂"];
    for (code, info) in mapper.iter() {
        for w in banned {
            assert!(!info.zh.contains(w), "{code} zh 含禁用词「{w}」: {}", info.zh);
            if let Some(fix) = &info.fix {
                assert!(!fix.contains(w), "{code} fix 含禁用词「{w}」: {fix}");
            }
        }
    }
}
