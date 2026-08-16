/// 错误分类：有 E 码的编译错误 / 无 E 码编译错误（rustc 未给码，如 format 参数错误）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    CompileCode,
    NoCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub code: String,
    pub line: Option<u32>,
    pub col: Option<u32>,
    pub kind: IssueKind,
    pub message: String,
}

/// 解析 rustc stderr。只依赖错误码 `error[E0xxx]` / `error:` 与 `--> path:line:col` 定位行，
/// 不匹配任何具体报错文本（rustc 版本可能微调措辞，错误码稳定）。
///
/// 边界规则（P1-01）：
/// - 无 E 码错误（trim 后以 `error:` 开头且不含 `[E`，排除 `aborting due to` 汇总行）→ `EUNKNOWN`（NoCode）；
/// - warning 行不生成错误条目，且是错误块边界：其后的 `-->` 不得附加到 pending error；
/// - 每条错误只取第一个 `-->`（首使用点；E0621 指向返回表达式行、E0601 指向文件末尾行是实测事实，不得"修正"）；
/// - 保持 rustc 输出顺序，不按行号重排。
pub fn parse_rustc_stderr(stderr: &str) -> Vec<CompileError> {
    let mut errors: Vec<CompileError> = Vec::new();
    // 最近一个尚未补行号的错误的下标；出现新的 error/warning 行或已补行号后失效
    let mut pending: Option<usize> = None;
    for line in stderr.lines() {
        let t = line.trim();
        if t.starts_with("error[E") {
            // "error[E0308]: mismatched types" -> code = 5 chars after "error[E"
            let code = t[6..(6 + 5).min(t.len())].to_string();
            let message = t[(6 + 5).min(t.len())..]
                .trim_start_matches(|c| c == ']' || c == ':')
                .trim()
                .to_string();
            errors.push(CompileError { code, line: None, col: None, kind: IssueKind::CompileCode, message });
            pending = Some(errors.len() - 1);
        } else if t.starts_with("error:") {
            // 无 E 码错误 → EUNKNOWN（`-D warnings` 提升的 error 也走此路径）；
            // 排除 "error: aborting due to N previous errors" 汇总行
            if !t.contains("aborting due to") {
                let message = t["error:".len()..].trim().to_string();
                errors.push(CompileError {
                    code: "EUNKNOWN".to_string(),
                    line: None,
                    col: None,
                    kind: IssueKind::NoCode,
                    message,
                });
                pending = Some(errors.len() - 1);
            }
        } else if let Some(idx) = t.find("--> ") {
            if let Some(i) = pending {
                // "--> /path/main.rs:3:5" -> 取最后两段 : 分隔（容忍行尾多余冒号，如 panic 定位行）
                let loc = t[idx + 4..].trim().trim_end_matches(':');
                let mut parts = loc.rsplitn(3, ':');
                let col = parts.next().and_then(|c| c.parse::<u32>().ok());
                if let Some(line) = parts.next().and_then(|l| l.parse::<u32>().ok()) {
                    errors[i].line = Some(line);
                    errors[i].col = col;
                    pending = None;
                }
            }
        } else if t.starts_with("warning:") {
            // warning 是错误块边界：阻断后续 --> 附加到 pending error
            pending = None;
        }
    }
    errors
}

/// panic 分类：8 类关键词 + 通用兜底（中文提示见 `PanicClass::zh`）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanicClass {
    ArrayIndexOob,
    UnwrapOptionNone,
    UnwrapResultErr,
    ParseFailure,
    IntegerOverflow,
    DivideByZero,
    ExplicitPanic,
    AllocFailure,
    Generic,
}

impl PanicClass {
    /// 分类关键词匹配（顺序即优先级，见 P1-01 需求 2 与 L3-B2 1.4 表）
    pub fn classify(message: &str) -> PanicClass {
        if message.contains("index out of bounds") {
            PanicClass::ArrayIndexOob
        } else if message.contains("called `Option::unwrap()` on a `None` value") {
            PanicClass::UnwrapOptionNone
        } else if message.contains("called `Result::unwrap()` on an `Err` value") {
            PanicClass::UnwrapResultErr
        } else if message.contains("ParseIntError") {
            PanicClass::ParseFailure
        } else if message.contains("overflow") {
            PanicClass::IntegerOverflow
        } else if message.contains("divide by zero") {
            PanicClass::DivideByZero
        } else if message.contains("explicit panic")
            || message.contains("not yet implemented")
            || message.contains("not implemented")
            || message.contains("assertion failed")
        {
            PanicClass::ExplicitPanic
        } else if message.contains("out of memory")
            || message.contains("capacity overflow")
            || message.contains("allocation of")
        {
            PanicClass::AllocFailure
        } else if message.trim().is_empty() {
            PanicClass::Generic
        } else {
            // 自定义文本：最可能是显式 panic!/assert! 的 payload
            PanicClass::ExplicitPanic
        }
    }

    pub fn zh(self) -> &'static str {
        match self {
            PanicClass::ArrayIndexOob => {
                "索引越界：访问位置超出集合长度。检查 `v.len()`，改用合法索引，或先用 `v.get(i)` 判断"
            }
            PanicClass::UnwrapOptionNone => {
                "对空值 unwrap：`Option` 为 `None` 时 `unwrap()` 会崩溃。改用 `match`/`if let` 或 `unwrap_or(默认值)`"
            }
            PanicClass::UnwrapResultErr => {
                "对错误结果 unwrap：`Result` 为 `Err` 时 `unwrap()` 会崩溃（如 `parse` 失败）。改用 `match` 或 `?`"
            }
            PanicClass::ParseFailure => {
                "字符串解析失败：`\"abc\".parse::<i32>()` 格式不对。检查输入是否真是数字"
            }
            PanicClass::IntegerOverflow => {
                "数值溢出：结果超出类型范围（如 `u8` 最大 255）。改用 `saturating_add`/`checked_add` 或更大类型"
            }
            PanicClass::DivideByZero => {
                "除数为零：`x / 0` 崩溃。先判断除数是否为 0 再运算"
            }
            PanicClass::ExplicitPanic => {
                "代码主动调用了 `panic!`/`todo!`/`unimplemented!`/`assert!`。检查触发分支是否真\"不可能发生\""
            }
            PanicClass::AllocFailure => {
                "内存分配失败（容量溢出）：试图构造超出内存容量的集合，检查容量计算"
            }
            PanicClass::Generic => {
                "程序运行出错：请检查可能越界、unwrap、除零的位置"
            }
        }
    }
}

/// panic stderr 净化结果：定位行号/列号 + 净化后全文（含 `main.rs:N:M` 定位行）+ 分类
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedPanic {
    pub line: Option<u32>,
    pub col: Option<u32>,
    pub message: String,
    pub class: PanicClass,
}

/// 净化 panic stderr（三步，P1-01 需求 2）：
/// 0) 整体 strip（实测 rustc 1.97 panic stderr 以空行开头，必须先 strip）
/// 1) 剥临时目录路径：/tmp/rlg-XXXX/main.rs → main.rs
/// 2) 剥 `thread 'main' (线程id) panicked at` 头（兼容无 id 旧格式），保留 main.rs:N:M 定位行
/// 3) 剥 `note:` 行
pub fn sanitize_panic(stderr: &str) -> SanitizedPanic {
    let no_temp = strip_temp_path(stderr.trim());
    let mut loc: Option<(u32, u32)> = None;
    let mut loc_line: Option<String> = None;
    let mut body: Vec<String> = Vec::new();
    for raw in no_temp.lines() {
        let line = raw.trim_start().trim_end();
        if line.starts_with("note:") {
            continue;
        }
        if line.starts_with("thread '") {
            if let Some(at) = line.find("panicked at") {
                let rest = line[at + "panicked at".len()..].trim();
                if !rest.is_empty() {
                    // 定位行形如 main.rs:3:21:（行尾冒号需容忍）；解析不出行号则按正文处理
                    let loc_str = rest.trim_end_matches(':');
                    let mut parts = loc_str.rsplitn(3, ':');
                    let col = parts.next().and_then(|c| c.parse::<u32>().ok());
                    let ln = parts.next().and_then(|l| l.parse::<u32>().ok());
                    if ln.is_some() {
                        if loc.is_none() {
                            loc = Some((ln.unwrap(), col.unwrap_or(0)));
                            loc_line = Some(rest.to_string());
                        }
                        continue;
                    }
                } else {
                    // "thread ... panicked at" 后无内容：整行丢弃
                    continue;
                }
            }
        }
        if !line.is_empty() {
            body.push(line.to_string());
        }
    }
    let message = match loc_line {
        Some(l) => {
            let mut m = l;
            if !body.is_empty() {
                m.push('\n');
                m.push_str(&body.join("\n"));
            }
            m
        }
        None => body.join("\n"),
    };
    SanitizedPanic {
        line: loc.map(|(l, _)| l),
        col: loc.map(|(_, c)| c),
        class: PanicClass::classify(&body.join("\n")),
        message,
    }
}

/// 剥临时目录路径：`/tmp/rlg-XXXX/main.rs` → `main.rs`（容错任意 `rlg-` 前缀目录名）。
/// 直接编译本地文件（无 rlg- 临时目录）时原样保留。
fn strip_temp_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find("rlg-") {
        let before = &rest[..pos];
        let after = &rest[pos + 4..];
        // 临时目录名 = [A-Za-z0-9_]+，其后必须紧跟 '/'
        let name_len = after
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
            .count();
        let name_end = after.char_indices().nth(name_len).map(|(i, _)| i).unwrap_or(after.len());
        if after[name_end..].starts_with('/') {
            // 剥掉临时目录（含可选 /tmp/ 前缀）
            let cut = before
                .strip_suffix("/tmp/")
                .map(|p| p.len())
                .unwrap_or(before.len());
            out.push_str(&before[..cut]);
            rest = &after[name_end + 1..];
        } else {
            // 不是目录路径（如消息正文恰好含 rlg-），原样保留
            out.push_str(before);
            out.push_str("rlg-");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_error_with_line() {
        let stderr = "\
error[E0308]: mismatched types
  --> src/main.rs:3:9
   |
3  | let x: i32 = \"a\";
   |              ^^^ expected `i32`, found `&str`
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0308");
        assert_eq!(errors[0].line, Some(3));
        assert_eq!(errors[0].col, Some(9));
        assert_eq!(errors[0].kind, IssueKind::CompileCode);
        assert!(errors[0].message.contains("mismatched types"));
    }

    #[test]
    fn multiple_errors() {
        let stderr = "\
error[E0502]: cannot borrow `s` as mutable because it is also borrowed as immutable
  --> src/main.rs:5:10
error[E0382]: use of moved value: `s`
  --> src/main.rs:9:20
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].code, "E0502");
        assert_eq!(errors[1].code, "E0382");
    }

    #[test]
    fn no_error_when_clean() {
        assert!(parse_rustc_stderr("warning: unused variable\n").is_empty());
    }

    #[test]
    fn missing_line_ok() {
        let stderr = "error[E0106]: missing lifetime specifier\n  --> /tmp/rlg-x/main.rs:2:1\n";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line, Some(2));
    }

    // ===== P1-01 新增：无 E 码捕获 / warning 边界 / 15 fixture =====

    #[test]
    fn eunknown_format_args_count() {
        // F12：01-l0-print 的 format 参数数量错误（实测无 E 码，现网空白反馈的元凶）
        let stderr = "\
error: 3 positional arguments in format string, but there are 2 arguments
 --> src/main.rs:2:15
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1, "format 参数错误必须被捕获，不能返回空列表: {errors:?}");
        assert_eq!(errors[0].code, "EUNKNOWN");
        assert_eq!(errors[0].line, Some(2));
        assert_eq!(errors[0].col, Some(15));
        assert_eq!(errors[0].kind, IssueKind::NoCode);
        assert!(errors[0].message.contains("positional arguments in format string"));
    }

    #[test]
    fn eunknown_let_chains_edition() {
        // F13：let chains 版本错误（无 E 码）
        let stderr = "\
error: let chains are only allowed in Rust 2024 or later
 --> src/main.rs:3:8
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "EUNKNOWN");
        assert_eq!(errors[0].line, Some(3));
        assert_eq!(errors[0].kind, IssueKind::NoCode);
        assert!(errors[0].message.contains("let chains"));
    }

    #[test]
    fn eunknown_d_warnings_upgraded_error() {
        // P5：-D warnings 把 warning 提升为无 E 码 error → EUNKNOWN
        let stderr = "error: unused variable: `unused`\n --> src/main.rs:2:9\n";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "EUNKNOWN");
        assert_eq!(errors[0].line, Some(2));
        assert_eq!(errors[0].kind, IssueKind::NoCode);
    }

    #[test]
    fn warning_blocks_arrow_attachment() {
        // 1.3：warning 行是错误块边界——其 --> 不得附加到待补行号的 error
        let stderr = "\
error[E0382]: borrow of moved value: `s`
warning: unused variable: `w`
 --> src/main.rs:9:9
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0382");
        assert_eq!(errors[0].line, None, "warning 的 --> 不应附加到错误");
    }

    // ---- 15 fixture 矩阵（L3-B2-parser.md 实测 line 值，Tier 1 静态 stderr）----

    fn assert_compile(e: &CompileError, code: &str, line: u32, kind: IssueKind) {
        assert_eq!(e.code, code);
        assert_eq!(e.line, Some(line));
        assert_eq!(e.kind, kind);
    }

    #[test]
    fn fixture_e0425_undefined_var() {
        let stderr = "\
error[E0425]: cannot find value `x` in this scope
 --> src/main.rs:2:9
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0425", 2, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0596_param_not_mut() {
        let stderr = "\
error[E0596]: cannot borrow `vec` as mutable, as it is not declared as mutable
 --> src/main.rs:7:9
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0596", 7, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0382_moved_value() {
        let stderr = "\
error[E0382]: borrow of moved value: `s`
 --> src/main.rs:4:20
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0382", 4, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0106_missing_lifetime() {
        let stderr = "\
error[E0106]: missing lifetime specifier
 --> src/main.rs:1:48
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0106", 1, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0599_method_not_found() {
        let stderr = "\
error[E0599]: no method named `area` found for struct `Rectangle` in the current scope
 --> src/main.rs:12:5
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0599", 12, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0597_borrow_dangling() {
        let stderr = "\
error[E0597]: `t` does not live long enough
 --> src/main.rs:5:5
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0597", 5, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0282_type_annotation() {
        let stderr = "\
error[E0282]: type annotations needed
 --> src/main.rs:2:9
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0282", 2, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0384_reassign_immut() {
        let stderr = "\
error[E0384]: cannot assign twice to immutable variable `x`
 --> src/main.rs:3:5
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0384", 3, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0594_assign_behind_ref() {
        let stderr = "\
error[E0594]: cannot assign to `*x`, which is behind a `&` reference
 --> src/main.rs:8:9
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0594", 8, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0621_explicit_lifetime() {
        // 实测：E0621 的 --> 指向返回表达式行（5），非签名行——不得"修正"
        let stderr = "\
error[E0621]: explicit lifetime required in the type of `y`
 --> src/main.rs:5:9
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0621", 5, IssueKind::CompileCode);
    }

    #[test]
    fn fixture_e0601_no_main() {
        // 实测：E0601 的 --> 指向文件末尾行（3），不得"修正"
        let stderr = "\
error[E0601]: `main` function not found in crate `p11`
 --> src/main.rs:3:2
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_compile(&errors[0], "E0601", 3, IssueKind::CompileCode);
    }

    // ---- 边界探针（P1/P2/P3 + 汇总行）----

    #[test]
    fn same_code_two_arrows_takes_first() {
        // P2：同码双 --> 只取第一个（首使用点）
        let stderr = "\
error[E0382]: borrow of moved value: `s`
 --> src/main.rs:4:20
 --> src/main.rs:3:9
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0382");
        assert_eq!(errors[0].line, Some(4));
    }

    #[test]
    fn multicode_keeps_rustc_order() {
        // P1：多错误码保持 rustc 输出顺序（E0382 → E0596），不按行号重排
        let stderr = "\
error[E0382]: borrow of moved value: `s`
 --> src/main.rs:4:20
 --> src/main.rs:7:17
error[E0596]: cannot borrow `s` as mutable, as it is not declared as mutable
 --> src/main.rs:8:5
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].code, "E0382");
        assert_eq!(errors[0].line, Some(4));
        assert_eq!(errors[1].code, "E0596");
        assert_eq!(errors[1].line, Some(8));
    }

    #[test]
    fn aborting_due_to_summary_line_ignored() {
        // 汇总行 "error: aborting due to..." 不得生成 EUNKNOWN
        let stderr = "\
error[E0425]: cannot find value `x` in this scope
 --> src/main.rs:2:9
error: aborting due to 1 previous error
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0425");
    }

    #[test]
    fn empty_stderr_no_errors() {
        assert!(parse_rustc_stderr("").is_empty());
    }

    #[test]
    fn warning_interleaved_produces_no_issue() {
        // P3：warning 不生成错误卡片；warning 的 --> 不误附
        let stderr = "\
warning: unused variable: `u`
 --> src/main.rs:1:9
error[E0382]: borrow of moved value: `s`
 --> src/main.rs:4:20
warning: unused variable: `u2`
 --> src/main.rs:5:9
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0382");
        assert_eq!(errors[0].line, Some(4));
    }

    #[test]
    fn eunknown_extra_no_code_cases() {
        // 其它实测无码错误同样走 EUNKNOWN
        for (msg, line) in [
            ("error: argument never used", 2),
            ("error: this arithmetic operation will overflow", 3),
        ] {
            let stderr = format!("{msg}\n --> src/main.rs:{line}:1\n");
            let errors = parse_rustc_stderr(&stderr);
            assert_eq!(errors.len(), 1, "{msg}");
            assert_eq!(errors[0].code, "EUNKNOWN", "{msg}");
            assert_eq!(errors[0].line, Some(line), "{msg}");
        }
    }

    // ===== panic 净化与分类（F14/F15 + 8 类关键词）=====

    #[test]
    fn panic_index_out_of_bounds() {
        // F14：实测 rustc 1.97 stderr 以空行开头，含线程 id 与临时目录路径
        let stderr = "
thread 'main' (288404) panicked at /tmp/rlg-a1b2c3/main.rs:3:21:
index out of bounds: the len is 3 but the index is 3
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
";
        let sp = sanitize_panic(stderr);
        assert_eq!(sp.line, Some(3));
        assert_eq!(sp.col, Some(21));
        assert_eq!(sp.class, PanicClass::ArrayIndexOob);
        assert!(sp.message.contains("index out of bounds"));
        assert!(sp.message.contains("main.rs:3:21"), "定位行必须保留: {}", sp.message);
        assert!(!sp.message.contains("/tmp/"), "临时目录路径必须剥掉: {}", sp.message);
        assert!(!sp.message.contains("thread 'main'"), "线程头必须剥掉: {}", sp.message);
        assert!(!sp.message.contains("288404"), "线程 id 必须剥掉: {}", sp.message);
        assert!(!sp.message.contains("RUST_BACKTRACE"), "note 行必须剥掉: {}", sp.message);
    }

    #[test]
    fn panic_unwrap_option_none() {
        // F15：unwrap None
        let stderr = "thread 'main' (243224) panicked at /tmp/rlg-x9y8z7/main.rs:3:22:\ncalled `Option::unwrap()` on a `None` value\nnote: run with `RUST_BACKTRACE=1`\n";
        let sp = sanitize_panic(stderr);
        assert_eq!(sp.line, Some(3));
        assert_eq!(sp.class, PanicClass::UnwrapOptionNone);
        assert!(sp.message.contains("called `Option::unwrap()` on a `None` value"));
    }

    #[test]
    fn panic_unwrap_err_parse_int_error() {
        // unwrap Err 带 ParseIntError → 优先 UnwrapResultErr（顺序 3 先于 4）
        let stderr = "thread 'main' panicked at main.rs:4:18:\ncalled `Result::unwrap()` on an `Err` value: ParseIntError { kind: InvalidDigit }\n";
        let sp = sanitize_panic(stderr);
        assert_eq!(sp.class, PanicClass::UnwrapResultErr);
        assert_eq!(sp.line, Some(4));
    }

    #[test]
    fn panic_old_format_without_thread_id() {
        // 旧格式（无线程 id）同样可净化
        let stderr = "thread 'main' panicked at /tmp/rlg-abc/main.rs:2:5:\nboom\n";
        let sp = sanitize_panic(stderr);
        assert_eq!(sp.line, Some(2));
        assert_eq!(sp.class, PanicClass::ExplicitPanic, "自定义文本 → 显式 panic");
        assert!(sp.message.contains("boom"));
        assert!(!sp.message.contains("rlg-abc"));
    }

    #[test]
    fn panic_overflow_and_divide_by_zero() {
        let sp = sanitize_panic("thread 'main' panicked at main.rs:5:9:\nattempt to add with overflow\n");
        assert_eq!(sp.class, PanicClass::IntegerOverflow);
        let sp2 = sanitize_panic("thread 'main' panicked at main.rs:6:9:\nattempt to divide by zero\n");
        assert_eq!(sp2.class, PanicClass::DivideByZero);
    }

    #[test]
    fn panic_explicit_macros() {
        for msg in ["explicit panic", "not yet implemented", "not implemented", "assertion failed: `left == right`"] {
            let stderr = format!("thread 'main' panicked at main.rs:1:1:\n{msg}\n");
            assert_eq!(sanitize_panic(&stderr).class, PanicClass::ExplicitPanic, "{msg}");
        }
    }

    #[test]
    fn panic_allocation_failure() {
        let sp = sanitize_panic("thread 'main' panicked at main.rs:9:9:\nmemory allocation of 4 bytes failed\n");
        assert_eq!(sp.class, PanicClass::AllocFailure);
    }

    #[test]
    fn panic_without_loc_line() {
        // 无定位行（异常场景）也不崩溃，走通用/兜底
        let sp = sanitize_panic("some weird panic text");
        assert_eq!(sp.line, None);
        assert_eq!(sp.class, PanicClass::ExplicitPanic);
        assert_eq!(sp.message, "some weird panic text");
    }

    #[test]
    fn strip_temp_path_keeps_local_file() {
        // 直接编译本地文件（无 rlg- 临时目录）时路径原样保留
        assert_eq!(strip_temp_path("panicked at src/main.rs:3:5:"), "panicked at src/main.rs:3:5:");
        assert_eq!(strip_temp_path("panicked at /tmp/rlg-x9/main.rs:3:5:"), "panicked at main.rs:3:5:");
        assert_eq!(strip_temp_path("panicked at /tmp/rlg-abc_def/out/main.rs:3:5:"), "panicked at out/main.rs:3:5:");
    }
}
