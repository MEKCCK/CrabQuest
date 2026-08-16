pub mod error_parser;
pub mod mapper;

use crate::error::GameError;
use crate::level::Level;
use crate::sandbox::{CompileOutcome, RunOutcome, Sandbox};
use crate::validate::mapper::ErrorMapper;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Pass,
    Fail { feedback: Vec<String> },
}

/// expect_output 规范化（v3 §3.3）：CRLF 归一化（先于 trim）→ 两端 trim；
/// trim_lines=true 时每行再去尾随空白。行序敏感、内部空行参与比对。
pub fn normalize_output(text: &str, trim_lines: bool) -> String {
    let t = text.replace("\r\n", "\n");
    let t = t.trim();
    if trim_lines {
        t.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
    } else {
        t.to_string()
    }
}

/// 核心校验：编译 → （失败按 allow_compile_fail 分支；成功）→ 运行 → 比对 stdout
pub fn validate(
    level: &Level,
    code: &str,
    mapper: &ErrorMapper,
    sandbox: &dyn Sandbox,
) -> Result<Validation, GameError> {
    let compile = sandbox.compile(code)?;
    match compile {
        CompileOutcome::Failed { errors } => {
            if level.allow_compile_fail {
                let got = errors.first().map(|e| e.code.clone()).unwrap_or_default();
                if !level.expect_error_code.is_empty() && got == level.expect_error_code {
                    return Ok(Validation::Pass);
                }
                let shown = if got.is_empty() { "无错误".to_string() } else { got };
                return Ok(Validation::Fail {
                    feedback: vec![format!(
                        "需要制造编译错误 {}，实际得到 {}\n（先看第一条错误，再调整代码）",
                        level.expect_error_code, shown
                    )],
                });
            }
            let feedback = errors
                .iter()
                .map(|e| {
                    let loc = e.line.map(|l| format!("（第 {l} 行）")).unwrap_or_default();
                    let zh = mapper
                        .lookup(&e.code)
                        .map(|i| format!("  💡 {}（{}）", i.zh, i.link))
                        .unwrap_or_default();
                    format!("{}{} {}: {}", e.code, loc, e.message, zh)
                })
                .collect();
            Ok(Validation::Fail { feedback })
        }
        CompileOutcome::Success { binary } => {
            if level.allow_compile_fail {
                return Ok(Validation::Fail {
                    feedback: vec!["该关卡要求制造编译错误，但代码编译成功了".to_string()],
                });
            }
            match sandbox.run(&binary)? {
                RunOutcome::Ok { stdout } => {
                    let expect = normalize_output(&level.expect_output, level.trim_lines);
                    let got = normalize_output(&stdout, level.trim_lines);
                    if expect.is_empty() || got == expect {
                        Ok(Validation::Pass)
                    } else {
                        Ok(Validation::Fail {
                            feedback: vec![format!(
                                "编译通过，但输出不符合要求。\n期望输出：{}\n实际输出：{}",
                                expect, got
                            )],
                        })
                    }
                }
                RunOutcome::Panic { message } => Ok(Validation::Fail {
                    feedback: vec![format!("程序运行时出错（panic）：\n{}", message)],
                }),
                RunOutcome::Timeout => Err(GameError::RunTimeout(2)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Level, LevelTier};
    use crate::sandbox::DevSandbox;
    use crate::validate::mapper::ErrorMapper;

    fn level(id: &str, expect_output: &str, allow_fail: bool, expect_code: &str) -> Level {
        Level {
            id: id.into(),
            title: "t".into(),
            tier: LevelTier::L1,
            description: "d".into(),
            hint: String::new(),
            hints: Vec::new(),
            starter_code: String::new(),
            expect_output: expect_output.into(),
            allow_compile_fail: allow_fail,
            expect_error_code: expect_code.into(),
            source: "test".into(),
            kind: "code".into(),
            expect_panic: String::new(),
            hint_unlock: Vec::new(),
            is_boss: false,
            trim_lines: false,
            options: Vec::new(),
            answer_index: None,
            link: String::new(),
        }
    }

    fn sb() -> DevSandbox {
        DevSandbox::new()
    }

    #[test]
    fn pass_when_output_matches() {
        let lv = level("t1", "hello 42", false, "");
        let code = "fn main() { println!(\"hello {}\", 42); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn pass_when_no_output_required() {
        let lv = level("t2", "", false, "");
        let code = "fn main() { println!(\"anything\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn fail_when_output_mismatch_shows_expectation() {
        let lv = level("t3", "wanted", false, "");
        let code = "fn main() { println!(\"got\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("wanted"), "feedback: {feedback:?}");
                assert!(feedback[0].contains("got"));
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn fail_compile_error_mapped_to_chinese() {
        let lv = level("t4", "", false, "");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("E0382"), "feedback: {feedback:?}");
                assert!(feedback[0].contains("所有权"), "中文映射缺失: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn allow_compile_fail_matches_code() {
        let lv = level("t5", "", true, "E0382");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn allow_compile_fail_wrong_code_fails() {
        let lv = level("t6", "", true, "E0502");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("E0382"), "feedback: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn allow_compile_fail_but_success_fails() {
        let lv = level("t7", "", true, "E0308");
        let code = "fn main() { println!(\"ok\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => assert!(feedback[0].contains("编译成功")),
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn crlf_normalized_before_compare() {
        // expect 含 \r\n 时先归一化为 \n 再 trim，与运行输出逐字节相等
        let lv = level("t9", "a\r\nb\r\n", false, "");
        let code = "fn main() { println!(\"a\"); println!(\"b\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn trailing_space_fails_by_default() {
        // 行尾空格敏感：内部行的行尾空格参与比对（不整行 trim）
        let lv = level("t10", "a \nb", false, "");
        let code = "fn main() { println!(\"a\"); println!(\"b\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("a \nb"), "feedback: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn trim_lines_relaxes_trailing_space() {
        let mut lv = level("t11", "a \nb", false, "");
        lv.trim_lines = true;
        let code = "fn main() { println!(\"a\"); println!(\"b\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn internal_blank_lines_participate() {
        // 内部空行参与比对：输出中的空行必须在 expect 中出现
        let lv = level("t12", "a\n\nb", false, "");
        let code = "fn main() { println!(\"a\"); println!(); println!(\"b\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn panic_reported() {
        let lv = level("t8", "", false, "");
        let code = "fn main() { panic!(\"kaboom\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("panic"), "feedback: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }
}
