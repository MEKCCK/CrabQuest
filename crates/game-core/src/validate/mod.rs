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

/// 剥 panic 文本中的临时目录路径段：`/tmp/rlg-XXXX/` 或 `rlg-XXXX/`
/// （rlg- 后到下一个 `/` 的目录名仅含 [A-Za-z0-9_]），保留其余内容。
/// 防止沙盒临时目录的随机路径干扰 expect_panic 子串匹配。
fn strip_temp_dir(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let Some(i) = rest.find("rlg-") else {
            out.push_str(rest);
            return out;
        };
        let after = &rest[i + 4..];
        let dir_len = after.find('/').unwrap_or(0);
        let is_dir_name = dir_len > 0
            && after[..dir_len]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_');
        // 前导必须是行首、路径分隔符或 /tmp/
        let prefix_ok = i == 0 || rest.as_bytes()[i - 1] == b'/';
        let tmp_prefix_ok = i >= 5 && &rest[i - 5..i] == "/tmp/";
        if !is_dir_name || !(prefix_ok || tmp_prefix_ok) {
            out.push_str(rest);
            return out;
        }
        let start = if tmp_prefix_ok { i - 5 } else { i };
        out.push_str(&rest[..start]);
        rest = &after[dir_len + 1..];
    }
}

/// panic 消息净化（v3 §5.3 第 6 条）：
/// 1. 每行 strip 行首空白；2. 剥临时目录路径（/tmp/rlg-XXXX/）；
/// 3. 剥 `thread 'main' (线程id) panicked at` 头；4. 删 note: 行与空行。
/// 保留 `main.rs:N:M:` 定位行。保证路径与线程 id 不干扰 expect_panic 子串匹配。
pub fn sanitize_panic_message(message: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for raw in message.lines() {
        let mut line = strip_temp_dir(raw.trim_start());
        if let Some(idx) = line.find("panicked at ") {
            if line[..idx].starts_with("thread '") {
                line = line[idx + "panicked at ".len()..].to_string();
            }
        }
        if line.trim_start().starts_with("note:") || line.trim().is_empty() {
            continue;
        }
        lines.push(line.trim_end().to_string());
    }
    lines.join("\n")
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
                    if !level.expect_panic.is_empty() {
                        // expect_panic 非空时优先于 expect_output（互斥校验保证不同时非空）
                        return Ok(Validation::Fail {
                            feedback: vec![format!(
                                "期望 panic 但未触发：程序编译通过并正常运行结束。\n期望 panic 消息包含：{}",
                                level.expect_panic
                            )],
                        });
                    }
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
                RunOutcome::Panic { message } => {
                    if !level.expect_panic.is_empty() {
                        let clean = sanitize_panic_message(&message);
                        if clean.contains(&level.expect_panic) {
                            Ok(Validation::Pass)
                        } else {
                            Ok(Validation::Fail {
                                feedback: vec![format!(
                                    "panic 消息不匹配：期望包含「{}」，实际为：\n{}",
                                    level.expect_panic, clean
                                )],
                            })
                        }
                    } else {
                        Ok(Validation::Fail {
                            feedback: vec![format!("程序运行时出错（panic）：\n{}", message)],
                        })
                    }
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

    // ---- expect_panic（P3-22）：子串匹配判定 ----

    fn panic_level(id: &str, expect_panic: &str) -> Level {
        Level {
            id: id.into(),
            title: "t".into(),
            tier: LevelTier::L2,
            description: "d".into(),
            hint: String::new(),
            hints: Vec::new(),
            starter_code: String::new(),
            expect_output: String::new(),
            allow_compile_fail: false,
            expect_error_code: String::new(),
            source: "test".into(),
            kind: "code".into(),
            expect_panic: expect_panic.into(),
            hint_unlock: Vec::new(),
            is_boss: false,
            trim_lines: false,
            options: Vec::new(),
            answer_index: None,
            link: String::new(),
        }
    }

    #[test]
    fn expect_panic_substring_match_passes() {
        // 净化后消息包含 expect_panic 子串（大小写敏感）→ 通关
        let lv = panic_level("p1", "index out of bounds");
        let code = "fn main() { let v = vec![1, 2, 3]; println!(\"{}\", v[3]); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn expect_panic_not_contained_fails() {
        // 触发了 panic 但消息不包含子串 → 失败，反馈区分「消息不匹配」
        let lv = panic_level("p2", "index out of bounds");
        let code = "fn main() { panic!(\"boom\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("panic 消息不匹配"), "feedback: {feedback:?}");
                assert!(feedback[0].contains("index out of bounds"), "feedback: {feedback:?}");
                assert!(feedback[0].contains("boom"), "feedback: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn expect_panic_case_sensitive() {
        // 大小写敏感：期望大写子串 vs 实际小写消息 → 失败；精确大小写 → 通过
        let upper = panic_level("p3a", "PANIC");
        let code = "fn main() { panic!(\"panic!\"); }";
        match validate(&upper, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("panic 消息不匹配"), "feedback: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
        let lower = panic_level("p3b", "panic");
        assert_eq!(validate(&lower, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn expect_panic_not_triggered_reports() {
        // 编译成功但未 panic → 失败，反馈区分「未触发」
        let lv = panic_level("p4", "index out of bounds");
        let code = "fn main() { println!(\"hello\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("期望 panic 但未触发"), "feedback: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn expect_panic_matches_with_temp_dir_path() {
        // 净化生效：真实沙盒路径 /tmp/rlg-XXXX/main.rs 被剥掉，线程 id 不干扰；
        // 完整消息行（路径无关）仍可匹配 → 通关
        let lv = panic_level("p5", "index out of bounds: the len is 3 but the index is 3");
        let code = "fn main() { let v = vec![1, 2, 3]; println!(\"{}\", v[3]); }";
        for _ in 0..3 {
            // 每次编译都是新临时目录，路径随机后缀不同，均应匹配
            assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
        }
    }

    #[test]
    fn expect_panic_priority_over_output() {
        // 优先级：expect_panic 非空时优先于 expect_output 比对（绕过加载校验直接构造）
        let mut lv = panic_level("p6", "kaboom");
        lv.expect_output = "kaboom".into();
        let code = "fn main() { panic!(\"kaboom\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn sanitize_panic_message_strips_noise() {
        // 直接验证净化：行首空白、临时目录路径、thread 头与线程 id、note 行全部剥除；
        // 保留 main.rs:N:M 定位行与消息体
        let raw = "\nthread 'main' (535665) panicked at /tmp/rlg-Ab12Cd34/main.rs:3:24:\nindex out of bounds: the len is 3 but the index is 3\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n";
        let clean = sanitize_panic_message(raw);
        assert!(clean.contains("main.rs:3:24:"), "clean: {clean:?}");
        assert!(clean.contains("index out of bounds: the len is 3 but the index is 3"), "clean: {clean:?}");
        assert!(!clean.contains("/tmp/"), "clean: {clean:?}");
        assert!(!clean.contains("rlg-"), "clean: {clean:?}");
        assert!(!clean.contains("thread 'main'"), "clean: {clean:?}");
        assert!(!clean.contains("535665"), "clean: {clean:?}");
        assert!(!clean.contains("note:"), "clean: {clean:?}");
    }
}
