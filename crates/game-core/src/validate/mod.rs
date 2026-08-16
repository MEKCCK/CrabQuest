pub mod error_parser;
pub mod mapper;

use crate::error::GameError;
use crate::level::Level;
use crate::sandbox::{CompileOutcome, RunOutcome, Sandbox};
use crate::validate::error_parser::{sanitize_panic, IssueKind};
use crate::validate::mapper::ErrorMapper;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Pass,
    Fail { feedback: Vec<String> },
}

/// 解析器漏检时的硬兜底文案（禁止空反馈，L1 §9 硬约束）
const FALLBACK_PARSE_MSG: &str =
    "编译失败，但无法解析出错误信息（可能是 rustc 版本差异）。请尝试简化代码，或对照关卡示例逐行检查。";

/// 无 E 码错误在 mapper 缺失时的兜底中文提示
const NO_CODE_ZH: &str =
    "这是一个编译错误（rustc 未提供错误码）。请对照报错原文，检查最近的改动（如 println! 格式参数、语法拼写）";

/// 核心校验：编译 → （失败按 allow_compile_fail 分支；成功）→ 运行 → 比对 stdout
///
/// P1-01 规则：
/// - 编译失败且解析不出任何错误 → 强制兜底文案（FALLBACK_PARSE_MSG），禁止空反馈；
/// - allow_compile_fail 判定只取首条错误码（rustc 输出顺序）；失败时提示「另有 N 条」；
/// - 非 allow 分支展示最多 3 条，超出折叠「+N 条」；
/// - panic 分支 > 输出比对 > 错误码（优先级）；panic 消息经 sanitize_panic 净化并分类。
pub fn validate(
    level: &Level,
    code: &str,
    mapper: &ErrorMapper,
    sandbox: &dyn Sandbox,
) -> Result<Validation, GameError> {
    let compile = sandbox.compile(code)?;
    match compile {
        CompileOutcome::Failed { errors } => {
            if errors.is_empty() {
                // 解析器漏检硬兜底：编译失败但无任何错误条目 → 禁止空白反馈
                return Ok(Validation::Fail {
                    feedback: vec![FALLBACK_PARSE_MSG.to_string()],
                });
            }
            if level.allow_compile_fail {
                // 判定只取首条 E 码（rustc 输出顺序，不按行号重排）
                let got = errors.first().map(|e| e.code.clone()).unwrap_or_default();
                if !level.expect_error_code.is_empty() && got == level.expect_error_code {
                    return Ok(Validation::Pass);
                }
                let shown = if got.is_empty() { "无错误".to_string() } else { got };
                let extra = if errors.len() > 1 {
                    format!("（另有 {} 条）", errors.len() - 1)
                } else {
                    String::new()
                };
                return Ok(Validation::Fail {
                    feedback: vec![format!(
                        "需要制造编译错误 {}，实际得到 {}{}\n（先看第一条错误，再调整代码）",
                        level.expect_error_code, shown, extra
                    )],
                });
            }
            // 展示最多 3 条，超出折叠为「+N 条」
            let mut feedback: Vec<String> = errors
                .iter()
                .take(3)
                .map(|e| {
                    let loc = e.line.map(|l| format!("（第 {l} 行）")).unwrap_or_default();
                    let zh = match e.kind {
                        IssueKind::NoCode => mapper
                            .lookup(&e.code)
                            .map(|i| format!("  💡 {}（{}）", i.zh, i.link))
                            .unwrap_or_else(|| format!("  💡 {NO_CODE_ZH}")),
                        IssueKind::CompileCode => mapper
                            .lookup(&e.code)
                            .map(|i| format!("  💡 {}（{}）", i.zh, i.link))
                            .unwrap_or_default(),
                    };
                    let code_label = match e.kind {
                        IssueKind::NoCode => "编译错误（无错误码）".to_string(),
                        IssueKind::CompileCode => e.code.clone(),
                    };
                    format!("{code_label}{loc} {}:{zh}", e.message)
                })
                .collect();
            if errors.len() > 3 {
                feedback.push(format!("…（+{} 条）", errors.len() - 3));
            }
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
                    if level.expect_output.trim().is_empty() || stdout.trim() == level.expect_output.trim() {
                        Ok(Validation::Pass)
                    } else {
                        Ok(Validation::Fail {
                            feedback: vec![format!(
                                "编译通过，但输出不符合要求。\n期望输出：{}\n实际输出：{}",
                                level.expect_output.trim(),
                                stdout.trim()
                            )],
                        })
                    }
                }
                // 优先级：panic 分支 > 输出比对（运行失败时不会有 stdout 比对机会，结构上保证）
                RunOutcome::Panic { message } => {
                    let sp = sanitize_panic(&message);
                    let loc = sp.line.map(|l| format!("（main.rs 第 {l} 行）")).unwrap_or_default();
                    let mut feedback = format!("❗ 运行时 panic{loc}：{}", sp.class.zh());
                    if !sp.message.is_empty() {
                        feedback.push_str("\n\n");
                        feedback.push_str(&sp.message);
                    }
                    Ok(Validation::Fail { feedback: vec![feedback] })
                }
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

    // ===== P1-01 新增：EUNKNOWN 兜底 / panic 净化 / 折叠 / 优先级 =====

    use crate::error::GameError;
    use crate::sandbox::{CompileOutcome, RunOutcome};
    use crate::validate::error_parser::{CompileError, IssueKind};
    use std::path::{Path, PathBuf};

    /// 测试用假沙盒：直接注入编译/运行结果，避免依赖 rustc 的行为分支测试
    struct MockSandbox {
        compile: CompileOutcome,
        run: RunOutcome,
    }

    impl Sandbox for MockSandbox {
        fn compile(&self, _code: &str) -> Result<CompileOutcome, GameError> {
            Ok(self.compile.clone())
        }
        fn run(&self, _binary: &Path) -> Result<RunOutcome, GameError> {
            Ok(self.run.clone())
        }
    }

    fn failed(errors: Vec<CompileError>) -> MockSandbox {
        MockSandbox {
            compile: CompileOutcome::Failed { errors },
            run: RunOutcome::Timeout,
        }
    }

    fn err(code: &str, line: u32) -> CompileError {
        CompileError {
            code: code.into(),
            line: Some(line),
            col: None,
            kind: IssueKind::CompileCode,
            message: format!("msg {code}"),
        }
    }

    #[test]
    fn compile_fail_nocode_never_blank_feedback() {
        // P1-01 回归锁（01-l0-print 场景，真实 rustc）：format 参数错误无 E 码，
        // 必须解析出 EUNKNOWN 且反馈非空（禁止空白反馈面板）
        let lv = level("t9", "1 + 2 = 3", false, "");
        let code = "fn main() {\n    println!(\"{} + {} = {}\", 1, 2);\n}";
        let sb = sb();
        match sb.compile(code).unwrap() {
            CompileOutcome::Failed { errors } => {
                assert!(!errors.is_empty(), "format 参数错误必须解析出错误");
                assert_eq!(errors[0].code, "EUNKNOWN", "errors: {errors:?}");
            }
            other => panic!("预期编译失败，实际 {other:?}"),
        }
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb).unwrap() {
            Validation::Fail { feedback } => {
                assert!(!feedback.is_empty(), "禁止空白反馈");
                assert!(!feedback[0].trim().is_empty());
                assert!(feedback[0].contains("编译错误"), "应展示无码错误文案: {feedback:?}");
                assert!(feedback[0].contains("positional arguments"), "应含报错原文: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn compile_fail_empty_errors_forces_fallback() {
        // 解析器漏检硬兜底：编译失败且 errors 为空 → 强制兜底文案
        let lv = level("t10", "", false, "");
        let sb = failed(vec![]);
        match validate(&lv, "fn main() {}", &ErrorMapper::default_fallback(), &sb).unwrap() {
            Validation::Fail { feedback } => {
                assert_eq!(feedback.len(), 1);
                assert!(feedback[0].contains("无法解析"), "兜底文案: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn eunknown_shows_fallback_zh_even_without_mapper_entry() {
        // 无 E 码错误：mapper 缺失 EUNKNOWN 条目时也要有中文兜底提示
        let lv = level("t11", "", false, "");
        let sb = failed(vec![CompileError {
            code: "EUNKNOWN".into(),
            line: Some(2),
            col: Some(15),
            kind: IssueKind::NoCode,
            message: "3 positional arguments in format string, but there are 2 arguments".into(),
        }]);
        match validate(&lv, "x", &ErrorMapper::default(), &sb).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("编译错误（无错误码）"), "feedback: {feedback:?}");
                assert!(feedback[0].contains("positional arguments"));
                assert!(feedback[0].contains("💡"), "兜底中文提示缺失: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn feedback_folds_after_three_errors() {
        // 展示最多 3 条，超出折叠为「+N 条」
        let lv = level("t12", "", false, "");
        let sb = failed(vec![
            err("E0425", 2),
            err("E0425", 3),
            err("E0425", 4),
            err("E0425", 5),
            err("E0425", 6),
        ]);
        match validate(&lv, "x", &ErrorMapper::default_fallback(), &sb).unwrap() {
            Validation::Fail { feedback } => {
                assert_eq!(feedback.len(), 4, "3 条 + 折叠行: {feedback:?}");
                assert!(feedback[0].contains("E0425（第 2 行）"));
                assert!(feedback[3].contains("+2"), "折叠行: {:?}", feedback[3]);
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn allow_fail_multicode_uses_first_code() {
        // allow_compile_fail 判定只取首条 E 码；不匹配时提示「另有 N 条」
        let lv = level("t13", "", true, "E0382");
        let sb = failed(vec![err("E0382", 4), err("E0596", 8)]);
        assert_eq!(
            validate(&lv, "x", &ErrorMapper::default_fallback(), &sb).unwrap(),
            Validation::Pass
        );
        let lv2 = level("t14", "", true, "E0502");
        let sb2 = failed(vec![err("E0382", 4), err("E0596", 8)]);
        match validate(&lv2, "x", &ErrorMapper::default_fallback(), &sb2).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("E0382"), "首条码: {feedback:?}");
                assert!(feedback[0].contains("另有 1 条"), "多码计数: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn panic_feedback_sanitized() {
        // 真实运行越界 panic：反馈无临时目录路径、无线程 id，保留 main.rs:N:M 行 + 分类中文提示
        let lv = level("t15", "", false, "");
        let code = "fn main() {\n    let v = vec![1, 2, 3];\n    println!(\"{}\", v[3]);\n}";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("运行时 panic"), "feedback: {feedback:?}");
                assert!(feedback[0].contains("索引越界"), "分类中文提示缺失: {feedback:?}");
                assert!(feedback[0].contains("main.rs:3:"), "保留定位行: {feedback:?}");
                assert!(!feedback[0].contains("/tmp/"), "临时路径泄漏: {feedback:?}");
                assert!(!feedback[0].contains("thread 'main'"), "线程头泄漏: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn panic_branch_takes_priority_over_output() {
        // 优先级：panic 分支 > 输出比对（P1-01 需求 3）
        let lv = level("t16", "should-not-match", false, "");
        let raw = "\nthread 'main' (123) panicked at /tmp/rlg-zz/main.rs:3:21:\nindex out of bounds: the len is 3 but the index is 3\nnote: run with `RUST_BACKTRACE=1`\n";
        let sb = MockSandbox {
            compile: CompileOutcome::Success { binary: PathBuf::from("/tmp/rlg-zz/main") },
            run: RunOutcome::Panic { message: raw.into() },
        };
        match validate(&lv, "fn main() {}", &ErrorMapper::default_fallback(), &sb).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("运行时 panic"), "feedback: {feedback:?}");
                assert!(!feedback[0].contains("输出不符合"));
                assert!(feedback[0].contains("main.rs:3:21"), "净化后定位行: {feedback:?}");
                assert!(!feedback[0].contains("/tmp/"), "临时路径泄漏: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }
}
