pub mod error_parser;
pub mod mapper;

use crate::error::GameError;
use crate::level::Level;
use crate::sandbox::{CompileOutcome, RunOutcome, Sandbox};
use crate::validate::error_parser::{sanitize_panic, CompileError, IssueKind};
use crate::validate::mapper::ErrorMapper;

/// 结构化错误卡片（P1-03 v3 §7.7）：错误码徽章 + 行号 + 中文解释 + 修复方向 + 可折叠原文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorCard {
    /// 错误码（无 E 码错误为 "EUNKNOWN"）
    pub code: String,
    /// rustc 定位行号（首条 `-->`；无定位时为 None）
    pub line: Option<u32>,
    /// rustc 英文摘要（折叠态原文）
    pub summary: String,
    /// 中文解释（是什么→为什么；非空硬约束，禁止空白卡）
    pub zh: String,
    /// 修复方向（怎么改；可为空串）
    pub fix: String,
    /// 最小复现代码（可选）
    pub example: Option<String>,
    /// 概念链接（link_zh 优先，其次官方页；离线时 UI 降级为灰字提示）
    pub link: Option<String>,
    /// 关联的关卡 hint 序号（0-based，「💡 与提示 2/3 相关」；
    /// 概念→hint 映射数据尚未落地，恒为 None，字段为 P3 预留）
    pub hint_index: Option<u32>,
}

/// 输出不符分支：期望 vs 实际（UI 两栏逐行着色）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDiff {
    pub expected: String,
    pub actual: String,
}

/// panic 分支：短分类名 + 净化消息（UI 折叠原文，标题用分类名）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicInfo {
    pub class_zh: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Pass { xp_gained: u32 },
    Fail {
        errors: Vec<ErrorCard>,
        expectation: Option<OutputDiff>,
        panic: Option<PanicInfo>,
    },
}

impl ErrorCard {
    /// 从解析出的 CompileError + mapper 解析结构化卡片（P1-03）。
    /// 兜底优先级（P1-01/P1-02 行为保持）：
    /// - NoCode（EUNKNOWN）：mapper `[fallback]` → lookup("EUNKNOWN") → NO_CODE_ZH；
    /// - CompileCode：lookup(code) → `[fallback]` → NO_CODE_ZH。
    /// zh 恒非空（禁止空白反馈卡）；link 取 link_zh（中文页优先）→ 官方页。
    fn from_compile(e: &CompileError, mapper: &ErrorMapper) -> Self {
        let (zh, link, fix, example) = match e.kind {
            IssueKind::NoCode => {
                if let Some(fb) = mapper.fallback() {
                    (fb.zh.clone(), Some(fb.link.clone()), String::new(), None)
                } else if let Some(info) = mapper.lookup("EUNKNOWN") {
                    (
                        info.zh.clone(),
                        info.link_zh.clone().or_else(|| Some(info.link.clone())),
                        info.fix.clone().unwrap_or_default(),
                        info.example.clone(),
                    )
                } else {
                    (NO_CODE_ZH.to_string(), None, String::new(), None)
                }
            }
            IssueKind::CompileCode => {
                if let Some(info) = mapper.lookup(&e.code) {
                    (
                        info.zh.clone(),
                        info.link_zh.clone().or_else(|| Some(info.link.clone())),
                        info.fix.clone().unwrap_or_default(),
                        info.example.clone(),
                    )
                } else if let Some(fb) = mapper.fallback() {
                    (fb.zh.clone(), Some(fb.link.clone()), String::new(), None)
                } else {
                    (NO_CODE_ZH.to_string(), None, String::new(), None)
                }
            }
        };
        ErrorCard {
            code: e.code.clone(),
            line: e.line,
            summary: e.message.clone(),
            zh,
            fix,
            example,
            link,
            hint_index: None,
        }
    }
}

/// 解析器漏检时的硬兜底文案（禁止空反馈，L1 §9 硬约束）
const FALLBACK_PARSE_MSG: &str =
    "编译失败，但无法解析出错误信息（可能是 rustc 版本差异）。请尝试简化代码，或对照关卡示例逐行检查。";

/// 无 E 码错误在 mapper 缺失时的兜底中文提示
const NO_CODE_ZH: &str =
    "这是一个编译错误（rustc 未提供错误码）。请对照报错原文，检查最近的改动（如 println! 格式参数、语法拼写）";

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
///
/// P1-01 规则（结构化后保持）：
/// - 编译失败且解析不出任何错误 → 强制兜底卡（FALLBACK_PARSE_MSG），禁止空白反馈；
/// - allow_compile_fail 判定只取首条错误码（rustc 输出顺序）；失败时提示「另有 N 条」；
/// - 编译错误全部转 ErrorCard（UI 负责折叠展示，数据不截断）；
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
                // 解析器漏检硬兜底：编译失败但无任何错误条目 → 强制兜底卡（禁止空白反馈）
                return Ok(Validation::Fail {
                    errors: vec![ErrorCard {
                        code: "EUNKNOWN".into(),
                        line: None,
                        summary: String::new(),
                        zh: FALLBACK_PARSE_MSG.to_string(),
                        fix: String::new(),
                        example: None,
                        link: mapper.fallback().map(|fb| fb.link.clone()),
                        hint_index: None,
                    }],
                    expectation: None,
                    panic: None,
                });
            }
            if level.allow_compile_fail {
                // 判定只取首条 E 码（rustc 输出顺序，不按行号重排）
                let got = errors.first().map(|e| e.code.clone()).unwrap_or_default();
                if !level.expect_error_code.is_empty() && got == level.expect_error_code {
                    return Ok(Validation::Pass { xp_gained: 0 });
                }
                let shown = if got.is_empty() { "无错误".to_string() } else { got };
                let extra = if errors.len() > 1 {
                    format!("（另有 {} 条）", errors.len() - 1)
                } else {
                    String::new()
                };
                let first = &errors[0];
                // 指导卡：把「需要制造 X 错误，实际得到 Y」的教法信息做成卡片
                return Ok(Validation::Fail {
                    errors: vec![ErrorCard {
                        code: shown.clone(),
                        line: first.line,
                        summary: first.message.clone(),
                        zh: format!(
                            "需要制造编译错误 {}，实际得到 {}{}",
                            level.expect_error_code, shown, extra
                        ),
                        fix: "先看第一条错误，再调整代码".to_string(),
                        example: None,
                        link: None,
                        hint_index: None,
                    }],
                    expectation: None,
                    panic: None,
                });
            }
            // 编译错误：全部解析出的错误都转结构化卡片（UI 负责折叠展示）
            let cards: Vec<ErrorCard> = errors
                .iter()
                .map(|e| ErrorCard::from_compile(e, mapper))
                .collect();
            Ok(Validation::Fail {
                errors: cards,
                expectation: None,
                panic: None,
            })
        }
        CompileOutcome::Success { binary } => {
            if level.allow_compile_fail {
                return Ok(Validation::Fail {
                    errors: vec![ErrorCard {
                        code: "无错误".into(),
                        line: None,
                        summary: String::new(),
                        zh: "该关卡要求制造编译错误，但代码编译成功了".to_string(),
                        fix: "故意制造一个编译错误（如类型不匹配、使用未定义的名字）再提交".to_string(),
                        example: None,
                        link: None,
                        hint_index: None,
                    }],
                    expectation: None,
                    panic: None,
                });
            }
            match sandbox.run(&binary)? {
                RunOutcome::Ok { stdout } => {
                    let expect = normalize_output(&level.expect_output, level.trim_lines);
                    let got = normalize_output(&stdout, level.trim_lines);
                    if expect.is_empty() || got == expect {
                        Ok(Validation::Pass { xp_gained: 0 })
                    } else {
                        Ok(Validation::Fail {
                            errors: Vec::new(),
                            expectation: Some(OutputDiff {
                                expected: expect,
                                actual: got,
                            }),
                            panic: None,
                        })
                    }
                }
                // 优先级：panic 分支 > 输出比对（运行失败时不会有 stdout 比对机会，结构上保证）
                RunOutcome::Panic { message } => {
                    let sp = sanitize_panic(&message);
                    Ok(Validation::Fail {
                        errors: Vec::new(),
                        expectation: None,
                        panic: Some(PanicInfo {
                            class_zh: sp.class.short_zh().to_string(),
                            message: sp.message,
                        }),
                    })
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
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass { xp_gained: 0 });
    }

    #[test]
    fn pass_when_no_output_required() {
        let lv = level("t2", "", false, "");
        let code = "fn main() { println!(\"anything\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass { xp_gained: 0 });
    }

    #[test]
    fn fail_when_output_mismatch_shows_expectation() {
        let lv = level("t3", "wanted", false, "");
        let code = "fn main() { println!(\"got\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { errors, expectation, panic } => {
                assert!(errors.is_empty());
                assert!(panic.is_none());
                let d = expectation.expect("输出不符应携带 OutputDiff");
                assert!(d.expected.contains("wanted"), "expected: {d:?}");
                assert!(d.actual.contains("got"));
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn fail_compile_error_mapped_to_chinese() {
        let lv = level("t4", "", false, "");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { errors, expectation, panic } => {
                assert_eq!(errors[0].code, "E0382", "errors: {errors:?}");
                assert!(errors[0].zh.contains("所有权"), "中文映射缺失: {errors:?}");
                assert!(errors[0].line.is_some(), "应带行号: {errors:?}");
                assert!(expectation.is_none() && panic.is_none());
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn allow_compile_fail_matches_code() {
        let lv = level("t5", "", true, "E0382");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass { xp_gained: 0 });
    }

    #[test]
    fn allow_compile_fail_wrong_code_fails() {
        let lv = level("t6", "", true, "E0502");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { errors, .. } => {
                assert_eq!(errors[0].code, "E0382", "errors: {errors:?}");
                assert!(errors[0].zh.contains("E0502"), "指导卡应含目标码: {errors:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn allow_compile_fail_but_success_fails() {
        let lv = level("t7", "", true, "E0308");
        let code = "fn main() { println!(\"ok\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { errors, .. } => assert!(errors[0].zh.contains("编译成功")),
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn crlf_normalized_before_compare() {
        // expect 含 \r\n 时先归一化为 \n 再 trim，与运行输出逐字节相等
        let lv = level("t9", "a\r\nb\r\n", false, "");
        let code = "fn main() { println!(\"a\"); println!(\"b\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass { xp_gained: 0 });
    }

    #[test]
    fn trailing_space_fails_by_default() {
        // 行尾空格敏感：内部行的行尾空格参与比对（不整行 trim）
        let lv = level("t10", "a \nb", false, "");
        let code = "fn main() { println!(\"a\"); println!(\"b\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { expectation, .. } => {
                let d = expectation.expect("输出不符应携带 OutputDiff");
                assert_eq!(d.expected, "a \nb", "expected 应保留行尾空格: {d:?}");
                assert_eq!(d.actual, "a\nb", "实际输出无行尾空格: {d:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn trim_lines_relaxes_trailing_space() {
        let mut lv = level("t11", "a \nb", false, "");
        lv.trim_lines = true;
        let code = "fn main() { println!(\"a\"); println!(\"b\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass { xp_gained: 0 });
    }

    #[test]
    fn internal_blank_lines_participate() {
        // 内部空行参与比对：输出中的空行必须在 expect 中出现
        let lv = level("t12", "a\n\nb", false, "");
        let code = "fn main() { println!(\"a\"); println!(); println!(\"b\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass { xp_gained: 0 });
    }

    #[test]
    fn panic_reported() {
        let lv = level("t8", "", false, "");
        let code = "fn main() { panic!(\"kaboom\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { panic, .. } => {
                let p = panic.expect("panic 分支应携带 PanicInfo");
                assert_eq!(p.class_zh, "显式 panic", "class: {p:?}");
                assert!(p.message.contains("kaboom"), "净化消息: {p:?}");
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
            Validation::Fail { errors, .. } => {
                assert!(!errors.is_empty(), "禁止空白反馈");
                assert_eq!(errors[0].code, "EUNKNOWN", "errors: {errors:?}");
                assert!(!errors[0].zh.trim().is_empty());
                assert!(errors[0].zh.contains("编译错误"), "应展示无码错误文案: {errors:?}");
                assert!(errors[0].summary.contains("positional arguments"), "应含报错原文: {errors:?}");
                assert!(errors[0].line.is_some(), "应带行号: {errors:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn compile_fail_empty_errors_forces_fallback() {
        // 解析器漏检硬兜底：编译失败且 errors 为空 → 强制兜底卡
        let lv = level("t10", "", false, "");
        let sb = failed(vec![]);
        match validate(&lv, "fn main() {}", &ErrorMapper::default_fallback(), &sb).unwrap() {
            Validation::Fail { errors, .. } => {
                assert_eq!(errors.len(), 1);
                assert!(errors[0].zh.contains("无法解析"), "兜底文案: {errors:?}");
                assert!(!errors[0].zh.trim().is_empty());
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn eunknown_shows_fallback_zh_even_without_mapper_entry() {
        // 无 E 码错误：mapper 缺失 EUNKNOWN 条目/fallback 段时也要有中文兜底提示（NO_CODE_ZH）
        let lv = level("t11", "", false, "");
        let sb = failed(vec![CompileError {
            code: "EUNKNOWN".into(),
            line: Some(2),
            col: Some(15),
            kind: IssueKind::NoCode,
            message: "3 positional arguments in format string, but there are 2 arguments".into(),
        }]);
        match validate(&lv, "x", &ErrorMapper::default(), &sb).unwrap() {
            Validation::Fail { errors, .. } => {
                assert_eq!(errors[0].code, "EUNKNOWN");
                assert!(errors[0].zh.contains("编译错误"), "兜底中文缺失: {errors:?}");
                assert!(!errors[0].zh.trim().is_empty());
                assert!(errors[0].summary.contains("positional arguments"));
                assert!(errors[0].link.is_none(), "空 mapper 无链接: {errors:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn uncovered_code_uses_fallback() {
        // P1-02：未收录码（CompileCode 查表落空）→ 走 [fallback] 兜底文案，而不是空白
        let lv = level("t15", "", false, "");
        let sb = failed(vec![err("E9999", 3)]);
        match validate(&lv, "x", &ErrorMapper::default_fallback(), &sb).unwrap() {
            Validation::Fail { errors, .. } => {
                assert_eq!(errors[0].code, "E9999", "errors: {errors:?}");
                assert_eq!(errors[0].line, Some(3));
                assert!(errors[0].zh.contains("编译错误"), "fallback 文案: {errors:?}");
                assert!(!errors[0].zh.trim().is_empty());
                assert!(errors[0].link.is_some(), "fallback 应带链接: {errors:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn all_errors_become_cards_for_ui_folding() {
        // P1-03：全部解析出的错误都转结构化卡片（UI 负责「第一条展开、其余折叠」），数据不截断
        let lv = level("t12", "", false, "");
        let sb = failed(vec![
            err("E0425", 2),
            err("E0425", 3),
            err("E0425", 4),
            err("E0425", 5),
            err("E0425", 6),
        ]);
        match validate(&lv, "x", &ErrorMapper::default_fallback(), &sb).unwrap() {
            Validation::Fail { errors, .. } => {
                assert_eq!(errors.len(), 5, "全部错误都应成卡: {errors:?}");
                assert_eq!(errors[0].code, "E0425");
                assert_eq!(errors[0].line, Some(2));
                assert!(errors.iter().all(|c| !c.zh.trim().is_empty()), "卡片 zh 恒非空");
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
            Validation::Pass { xp_gained: 0 }
        );
        let lv2 = level("t14", "", true, "E0502");
        let sb2 = failed(vec![err("E0382", 4), err("E0596", 8)]);
        match validate(&lv2, "x", &ErrorMapper::default_fallback(), &sb2).unwrap() {
            Validation::Fail { errors, .. } => {
                assert_eq!(errors[0].code, "E0382", "首条码: {errors:?}");
                assert!(errors[0].zh.contains("另有 1 条"), "多码计数: {errors:?}");
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
            Validation::Fail { panic, .. } => {
                let p = panic.expect("panic 分支应携带 PanicInfo");
                assert_eq!(p.class_zh, "索引越界", "分类中文提示缺失: {p:?}");
                assert!(p.message.contains("main.rs:3:"), "保留定位行: {p:?}");
                assert!(!p.message.contains("/tmp/"), "临时路径泄漏: {p:?}");
                assert!(!p.message.contains("thread 'main'"), "线程头泄漏: {p:?}");
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
            Validation::Fail { errors, expectation, panic } => {
                assert!(errors.is_empty());
                assert!(expectation.is_none(), "panic 优先于输出比对");
                let p = panic.expect("panic 分支应携带 PanicInfo");
                assert_eq!(p.class_zh, "索引越界");
                assert!(p.message.contains("main.rs:3:21"), "净化后定位行: {p:?}");
                assert!(!p.message.contains("/tmp/"), "临时路径泄漏: {p:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    // ===== P1-03 新增：结构化卡片字段解析 =====

    /// v2 全字段 errors.toml：E0308 带 link_zh/fix/example；[fallback] 段存在
    const V2_FULL: &str = r#"
[E0308]
zh = "类型不匹配：表达式的实际类型与期望类型不一致"
link = "https://doc.rust-lang.org/error_codes/E0308.html"
link_zh = "https://rustwiki.org/zh-CN/book/ch03-02-data-types.html"
fix = "显式转换（n.to_string()），或修改类型标注使两边一致"
example = '''
let n: u32 = 5;
let s = n.to_string();
'''

[fallback]
zh = "这是一个编译错误（rustc 未提供错误码）。请对照报错原文，检查最近的改动（如 println! 格式参数、语法拼写）"
link = "https://doc.rust-lang.org/error_codes/index.html"
"#;

    fn mapper_from(content: &str) -> ErrorMapper {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("errors.toml");
        std::fs::write(&p, content).unwrap();
        ErrorMapper::load(&p).unwrap()
    }

    #[test]
    fn errorcard_resolves_mapper_full_fields() {
        // P1-03：ErrorCard 从 mapper 解析 zh/fix/example/link（link_zh 中文页优先）
        let lv = level("t20", "", false, "");
        let sb = failed(vec![err("E0308", 3)]);
        match validate(&lv, "x", &mapper_from(V2_FULL), &sb).unwrap() {
            Validation::Fail { errors, .. } => {
                assert_eq!(errors[0].code, "E0308");
                assert_eq!(errors[0].line, Some(3));
                assert!(errors[0].zh.contains("类型不匹配"));
                assert!(errors[0].fix.contains("n.to_string()"), "fix 缺失: {errors:?}");
                assert!(errors[0].example.as_deref().unwrap().contains("n.to_string()"));
                assert_eq!(
                    errors[0].link.as_deref(),
                    Some("https://rustwiki.org/zh-CN/book/ch03-02-data-types.html"),
                    "link 应取 link_zh（中文页优先）"
                );
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn errorcard_uncovered_code_gets_fallback_zh_and_link() {
        // P1-03：未收录码（查表落空）→ [fallback] zh + link；fix 为空串（fallback 无修复字段）
        let lv = level("t21", "", false, "");
        let sb = failed(vec![err("E7777", 9)]);
        match validate(&lv, "x", &mapper_from(V2_FULL), &sb).unwrap() {
            Validation::Fail { errors, .. } => {
                assert_eq!(errors[0].code, "E7777");
                assert!(!errors[0].zh.trim().is_empty());
                assert!(errors[0].link.is_some(), "fallback 应带链接: {errors:?}");
                assert!(errors[0].fix.is_empty());
                assert!(errors[0].example.is_none());
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn structured_fail_branches_carry_exactly_one_payload() {
        // P1-03 验收：三种失败分支互斥——编译错误卡 / 输出 diff / panic，各自只填对应字段
        // 分支 1：编译错误 → errors 非空，expectation/panic 均为 None
        let lv = level("t22", "", false, "");
        let sb = failed(vec![err("E0425", 2)]);
        match validate(&lv, "x", &ErrorMapper::default_fallback(), &sb).unwrap() {
            Validation::Fail { errors, expectation, panic } => {
                assert_eq!(errors.len(), 1);
                assert!(expectation.is_none());
                assert!(panic.is_none());
            }
            other => panic!("expected Fail, got {:?}", other),
        }
        // 分支 2：输出不符 → expectation Some，errors/panic 为空
        let lv2 = level("t23", "aaa", false, "");
        let sb2 = MockSandbox {
            compile: CompileOutcome::Success { binary: PathBuf::from("/tmp/rlg-zz/main") },
            run: RunOutcome::Ok { stdout: "bbb".into() },
        };
        match validate(&lv2, "fn main() {}", &ErrorMapper::default_fallback(), &sb2).unwrap() {
            Validation::Fail { errors, expectation, panic } => {
                assert!(errors.is_empty());
                assert!(panic.is_none());
                let d = expectation.unwrap();
                assert_eq!((d.expected.as_str(), d.actual.as_str()), ("aaa", "bbb"));
            }
            other => panic!("expected Fail, got {:?}", other),
        }
        // 分支 3：panic → panic Some，errors/expectation 为空（优先级结构上保证）
        let raw = "\nthread 'main' panicked at main.rs:2:5:\nboom\n";
        let sb3 = MockSandbox {
            compile: CompileOutcome::Success { binary: PathBuf::from("/tmp/rlg-zz/main") },
            run: RunOutcome::Panic { message: raw.into() },
        };
        match validate(&lv, "fn main() {}", &ErrorMapper::default_fallback(), &sb3).unwrap() {
            Validation::Fail { errors, expectation, panic } => {
                assert!(errors.is_empty());
                assert!(expectation.is_none());
                assert!(panic.is_some());
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }
}
