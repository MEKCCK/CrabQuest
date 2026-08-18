use serde::Deserialize;
use std::path::Path;
use std::collections::HashSet;

use crate::error::GameError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LevelTier {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl LevelTier {
    pub fn order(&self) -> u8 {
        match self {
            LevelTier::L0 => 0,
            LevelTier::L1 => 1,
            LevelTier::L2 => 2,
            LevelTier::L3 => 3,
            LevelTier::L4 => 4,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Level {
    pub id: String,
    pub title: String,
    pub tier: LevelTier,
    pub description: String,
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub hints: Vec<String>,
    #[serde(default)]
    pub starter_code: String,
    #[serde(default)]
    pub expect_output: String,
    #[serde(default)]
    pub allow_compile_fail: bool,
    #[serde(default)]
    pub expect_error_code: String,
    #[serde(default)]
    pub source: String,
    /// 关卡类型："code"（编译/输出关，缺省）| "quiz"（选择题）
    #[serde(default = "default_kind")]
    pub kind: String,
    /// panic 消息子串匹配（与 expect_output 互斥，P3-22 使用）
    #[serde(default)]
    pub expect_panic: String,
    /// 失败次数阈值，与 hints 等长；缺省 = 手动逐级揭示
    #[serde(default)]
    pub hint_unlock: Vec<u32>,
    /// Boss 关显式标注（P3-17）
    #[serde(default)]
    pub is_boss: bool,
    /// true 时比对前每行先去尾随空白
    #[serde(default)]
    pub trim_lines: bool,
    /// 选择题选项（kind="quiz" 时必填，2-6 项且无重复）
    #[serde(default)]
    pub options: Vec<String>,
    /// 正确答案 0-based 下标（kind="quiz" 时必填）
    #[serde(default)]
    pub answer_index: Option<u32>,
    /// 可选外部链接（rust-course 素材必填）
    #[serde(default)]
    pub link: String,
}

#[derive(Debug, Deserialize)]
struct LevelFile {
    level: Vec<Level>,
}

fn default_kind() -> String {
    "code".to_string()
}

/// 加载期数据校验（schema v2）：返回第一个不合法原因；经 GameError::LevelDataInvalid 呈现
pub fn validate_level_data(level: &Level) -> Result<(), String> {
    match level.kind.as_str() {
        "code" => {}
        "quiz" => {
            let n = level.options.len();
            if !(2..=6).contains(&n) {
                return Err(format!(
                    "选择题（kind=\"quiz\"）的 options 必须为 2-6 项，实际 {n} 项"
                ));
            }
            let mut seen = HashSet::new();
            for (i, opt) in level.options.iter().enumerate() {
                if !seen.insert(opt) {
                    return Err(format!(
                        "选择题（kind=\"quiz\"）的 options 第 {} 项与其他项重复",
                        i + 1
                    ));
                }
            }
            match level.answer_index {
                Some(idx) if (idx as usize) < n => {}
                Some(idx) => {
                    return Err(format!(
                        "选择题（kind=\"quiz\"）的 answer_index（{idx}）越界：options 共 {n} 项（0-based）"
                    ));
                }
                None => {
                    return Err("选择题（kind=\"quiz\"）缺少 answer_index 字段".to_string());
                }
            }
        }
        other => {
            return Err(format!(
                "kind 取值必须为 \"code\" 或 \"quiz\"，实际 \"{other}\""
            ));
        }
    }
    if !level.hint_unlock.is_empty() && level.hint_unlock.len() != level.hints.len() {
        return Err(format!(
            "hint_unlock 长度（{}）必须与 hints 长度（{}）一致",
            level.hint_unlock.len(),
            level.hints.len()
        ));
    }
    if level.expect_output.contains('\r') {
        return Err("expect_output 不得包含回车符（\\r），请使用 LF 换行".to_string());
    }
    if !level.expect_panic.is_empty() && !level.expect_output.is_empty() {
        return Err("expect_panic 与 expect_output 不能同时填写（二者互斥）".to_string());
    }
    if level.source.trim().is_empty() {
        return Err("source 字段不能为空".to_string());
    }
    Ok(())
}

/// 解析一份 TOML 内容（可能含多个 [[level]]），供 load 与测试复用
pub fn parse_levels(content: &str) -> Result<Vec<Level>, GameError> {
    let file: LevelFile = toml::from_str(content)
        .map_err(|e| GameError::TomlParse("关卡内容".into(), e.to_string()))?;
    for lvl in &file.level {
        if let Err(reason) = validate_level_data(lvl) {
            return Err(GameError::LevelDataInvalid(lvl.id.clone(), reason));
        }
    }
    Ok(file.level)
}

#[derive(Debug, Clone, Default)]
pub struct LevelSet {
    pub levels: Vec<Level>,
}

impl LevelSet {
    /// 从目录加载全部关卡 TOML，按文件名排序形成线性关卡顺序
    pub fn load(dir: &Path) -> Result<Self, GameError> {
        if !dir.exists() {
            return Err(GameError::LevelDirNotFound(dir.display().to_string()));
        }
        let mut files: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |x| x == "toml"))
            .collect();
        files.sort();
        let mut levels = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for f in files {
            let content = std::fs::read_to_string(&f)?;
            let parsed = parse_levels(&content)
                .map_err(|e| match e {
                    GameError::TomlParse(_, msg) => {
                        GameError::TomlParse(f.display().to_string(), msg)
                    }
                    other => other,
                })?;
            for lvl in parsed {
                if !seen.insert(lvl.id.clone()) {
                    return Err(GameError::DuplicateLevelId(lvl.id.clone()));
                }
                levels.push(lvl);
            }
        }
        if levels.is_empty() {
            return Err(GameError::LevelDirNotFound(dir.display().to_string()));
        }
        Ok(Self { levels })
    }

    pub fn get(&self, id: &str) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const LEVEL_TOML: &str = r#"
[[level]]
id = "l0-hello"
title = "你好，变量"
tier = "l0"
description = "修复代码，使其编译并输出预期结果"
hint = "变量需要 let 声明"
starter_code = "fn main() { x = 5; println!(\"x has the value {}\", x); }"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l1-move"
title = "所有权转移"
tier = "l1"
description = "理解 move 语义"
starter_code = "fn main() { let s = String::from(\"hi\"); take(s); println!(\"{}\", s); } fn take(x: String) {}"
expect_output = ""
allow_compile_fail = true
expect_error_code = "E0382"
source = "rustlings"
"#;

    #[test]
    fn parse_levels_ok() {
        let levels = parse_levels(LEVEL_TOML).unwrap();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].id, "l0-hello");
        assert_eq!(levels[0].tier, LevelTier::L0);
        assert_eq!(levels[0].expect_output, "x has the value 5");
        assert!(!levels[0].allow_compile_fail);
        assert_eq!(levels[1].tier, LevelTier::L1);
        assert!(levels[1].allow_compile_fail);
        assert_eq!(levels[1].expect_error_code, "E0382");
        // schema v2 新增字段全部 serde(default)：旧文件缺省值
        for lvl in &levels {
            assert_eq!(lvl.kind, "code", "{} kind 缺省应为 code", lvl.id);
            assert!(lvl.expect_panic.is_empty(), "{} expect_panic 缺省为空", lvl.id);
            assert!(lvl.hint_unlock.is_empty(), "{} hint_unlock 缺省为空", lvl.id);
            assert!(!lvl.is_boss, "{} is_boss 缺省为 false", lvl.id);
            assert!(!lvl.trim_lines, "{} trim_lines 缺省为 false", lvl.id);
            assert!(lvl.options.is_empty(), "{} options 缺省为空", lvl.id);
            assert_eq!(lvl.answer_index, None, "{} answer_index 缺省为 None", lvl.id);
            assert!(lvl.link.is_empty(), "{} link 缺省为空", lvl.id);
        }
    }

    #[test]
    fn parse_levels_v2_fields_roundtrip() {
        let v2 = r#"
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

[[level]]
id = "l4-panic"
title = "制造 panic"
tier = "l4"
description = "描述"
starter_code = "fn main() {}"
expect_panic = "The journey took no time"
source = "100-exercises"
"#;
        let levels = parse_levels(v2).unwrap();
        let quiz = &levels[0];
        assert_eq!(quiz.kind, "quiz");
        assert_eq!(quiz.hints, vec!["概念", "定位", "解法"]);
        assert_eq!(quiz.hint_unlock, vec![1, 3, 5]);
        assert_eq!(quiz.options, vec!["0", "1", "编译错误", "不确定"]);
        assert_eq!(quiz.answer_index, Some(1));
        assert!(quiz.is_boss);
        assert!(quiz.trim_lines);
        assert_eq!(quiz.link, "https://rustwiki.org/zh-CN/rust-by-example/");
        let panic_lvl = &levels[1];
        assert_eq!(panic_lvl.kind, "code");
        assert_eq!(panic_lvl.expect_panic, "The journey took no time");
        assert!(!panic_lvl.is_boss);
    }

    #[test]
    fn parse_levels_validation_errors_are_chinese() {
        let base = "[[level]]\nid = \"t1\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nsource = \"s\"\n";
        let cases: &[(&str, &str)] = &[
            // (追加行, 期望中文子串)
            ("kind = \"quiz\"\noptions = [\"a\"]", "options 必须为 2-6 项"),
            (
                "kind = \"quiz\"\noptions = [\"a\", \"b\", \"c\"]\nanswer_index = 5",
                "越界",
            ),
            ("kind = \"quiz\"\noptions = [\"a\", \"b\"]", "缺少 answer_index"),
            (
                "hints = [\"h1\", \"h2\"]\nhint_unlock = [1]",
                "hint_unlock 长度",
            ),
            ("hint_unlock = [1]", "hint_unlock 长度"),
            (
                "expect_output = \"x\"\nexpect_panic = \"boom\"",
                "互斥",
            ),
            ("expect_output = \"a\\r\\nb\"", "回车符"),
            ("kind = \"quzi\"", "kind 取值"),
        ];
        for (extra, expect_zh) in cases {
            let toml = format!("{base}{extra}\n");
            match parse_levels(&toml) {
                Err(GameError::LevelDataInvalid(id, reason)) => {
                    assert!(reason.contains(expect_zh), "case {extra:?}: 错误「{reason}」不含「{expect_zh}」");
                    assert_eq!(id, "t1");
                }
                other => panic!("case {extra:?}: 期望 LevelDataInvalid，实际 {other:?}"),
            }
        }
        // source 为空：基础块本身就不写 source 字段
        let no_source = "[[level]]\nid = \"t1\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\n";
        match parse_levels(no_source) {
            Err(GameError::LevelDataInvalid(id, reason)) => {
                assert!(reason.contains("source 字段不能为空"), "错误「{reason}」不含预期文案");
                assert_eq!(id, "t1");
            }
            other => panic!("期望 LevelDataInvalid，实际 {other:?}"),
        }
    }

    #[test]
    fn quiz_with_valid_options_and_answer_index_passes() {
        let toml = "[[level]]\nid = \"q\"\ntitle = \"t\"\ntier = \"l4\"\ndescription = \"d\"\nkind = \"quiz\"\noptions = [\"a\", \"b\", \"c\", \"d\"]\nanswer_index = 2\nstarter_code = \"fn main() {}\"\nsource = \"rust-quiz\"\n";
        let levels = parse_levels(toml).unwrap();
        assert_eq!(levels[0].kind, "quiz");
        assert_eq!(levels[0].answer_index, Some(2));
    }

    #[test]
    fn parse_levels_invalid_tier_fails() {
        let bad = LEVEL_TOML.replace("tier = \"l0\"", "tier = \"l9\"");
        assert!(parse_levels(&bad).is_err());
    }

    #[test]
    fn parse_levels_malformed_fails() {
        assert!(parse_levels("not toml at all [[[[").is_err());
    }

    #[test]
    fn tier_order() {
        assert!(LevelTier::L0.order() < LevelTier::L1.order());
        assert!(LevelTier::L4.order() == 4);
    }

    #[test]
    fn level_set_load_sorted_and_duplicate_detected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("02-second.toml"), LEVEL_TOML).unwrap();
        std::fs::write(
            dir.path().join("01-first.toml"),
            "[[level]]\nid = \"first\"\ntitle = \"t\"\ntier = \"l2\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nsource = \"x\"\n",
        )
        .unwrap();
        let set = LevelSet::load(dir.path()).unwrap();
        assert_eq!(set.len(), 3);
        assert_eq!(set.levels[0].id, "first"); // 按文件名排序
        assert!(set.get("first").is_some());
        assert!(set.get("nope").is_none());
    }

    #[test]
    fn level_set_duplicate_id_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.toml"), LEVEL_TOML).unwrap();
        std::fs::write(dir.path().join("b.toml"), LEVEL_TOML).unwrap();
        assert!(matches!(LevelSet::load(dir.path()), Err(GameError::DuplicateLevelId(_))));
    }

    #[test]
    fn level_set_missing_dir_fails() {
        assert!(matches!(
            LevelSet::load(&PathBuf::from("/nonexistent/rlg-levels")),
            Err(GameError::LevelDirNotFound(_))
        ));
    }
}
