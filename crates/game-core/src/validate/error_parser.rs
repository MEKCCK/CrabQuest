#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub code: String,
    pub line: Option<u32>,
    pub message: String,
}

/// 解析 rustc stderr。只依赖错误码 `error[E0xxx]` 与 `--> path:line:col` 定位行，
/// 不匹配任何具体报错文本（rustc 版本可能微调措辞，错误码稳定）。
pub fn parse_rustc_stderr(stderr: &str) -> Vec<CompileError> {
    let mut errors: Vec<CompileError> = Vec::new();
    for line in stderr.lines() {
        let t = line.trim();
        if let Some(pos) = t.find("error[E") {
            // "error[E0308]: ..." -> code = 6 chars after "error[E"
            let code_end = (pos + 6 + 5).min(t.len());
            let code = t[pos + 6..code_end].to_string();
            let message = t[code_end.min(t.len())..]
                .trim_start_matches(':')
                .trim()
                .to_string();
            errors.push(CompileError { code, line: None, message });
        } else if let Some(idx) = t.find("--> ") {
            if let Some(last) = errors.last_mut() {
                if last.line.is_none() {
                    // "--> /path/main.rs:3:5" -> 取最后两段 :分隔
                    let loc = &t[idx + 5..];
                    let mut parts = loc.rsplitn(3, ':');
                    let _col = parts.next();
                    if let Some(line) = parts.next().and_then(|s| s.parse::<u32>().ok()) {
                        last.line = Some(line);
                    }
                }
            }
        }
    }
    errors
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
}
