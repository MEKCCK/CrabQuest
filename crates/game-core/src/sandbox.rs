use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::GameError;
use crate::validate::error_parser::{parse_rustc_stderr, CompileError};

pub trait Sandbox {
    fn compile(&self, code: &str) -> Result<CompileOutcome, GameError>;
    fn run(&self, binary: &Path) -> Result<RunOutcome, GameError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileOutcome {
    Success { binary: PathBuf },
    Failed { errors: Vec<CompileError> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    Ok { stdout: String },
    Panic { message: String },
    Timeout,
}

/// 开发期沙盒：临时目录 + 超时 + syn 静态拦截。
/// 真隔离（bwrap）在计划②实现，实现新的 Sandbox 类型即可替换。
pub struct DevSandbox {
    pub compile_timeout_secs: u64,
    pub run_timeout_secs: u64,
}

impl DevSandbox {
    pub fn new() -> Self {
        Self { compile_timeout_secs: 10, run_timeout_secs: 2 }
    }
}

impl Default for DevSandbox {
    fn default() -> Self {
        Self::new()
    }
}

const BLOCKED_PREFIXES: [&str; 5] = ["std::fs", "std::net", "std::process", "std::env", "std::thread"];

fn use_tree_str(t: &syn::UseTree) -> String {
    match t {
        syn::UseTree::Path(p) => format!("{}::{}", p.ident, use_tree_str(&p.tree)),
        syn::UseTree::Name(n) => n.ident.to_string(),
        syn::UseTree::Rename(r) => r.ident.to_string(),
        syn::UseTree::Glob(_) => "*".to_string(),
        syn::UseTree::Group(g) => g.items.iter().map(use_tree_str).collect::<Vec<_>>().join(","),
    }
}

/// 粗略静态拦截：玩家代码中禁止访问文件系统、网络、进程、环境变量、线程。
/// 只扫描 AST 路径（注释/字符串不会误报）。
fn check_blocked_apis(code: &str) -> Result<(), GameError> {
    let ast: syn::File = syn::parse_file(code)
        .map_err(|e| GameError::SandboxBlocked(format!("代码语法错误: {e}")))?;
    let mut blocked: Option<String> = None;

    struct Scan<'a> {
        blocked: &'a mut Option<String>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Scan<'_> {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            let s = node
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            for p in BLOCKED_PREFIXES {
                if s.starts_with(p) {
                    *self.blocked = Some(s);
                    return;
                }
            }
            syn::visit::visit_expr_path(self, node);
        }
        fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
            let s = node
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            for p in BLOCKED_PREFIXES {
                if s.starts_with(p) {
                    *self.blocked = Some(s);
                    return;
                }
            }
            syn::visit::visit_type_path(self, node);
        }
        fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
            let s = use_tree_str(&node.tree);
            for p in BLOCKED_PREFIXES {
                if s.starts_with(p) {
                    *self.blocked = Some(s);
                    return;
                }
            }
            syn::visit::visit_item_use(self, node);
        }
    }

    let mut scan = Scan { blocked: &mut blocked };
    syn::visit::Visit::visit_file(&mut scan, &ast);

    if let Some(s) = blocked {
        return Err(GameError::SandboxBlocked(format!("检测到被禁用的 API：{s}")));
    }
    Ok(())
}

fn wait_with_timeout(child: &mut std::process::Child, secs: u64) -> Result<std::process::ExitStatus, GameError> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(GameError::CompileTimeout(secs));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read_piped(mut child: &mut std::process::Child) -> (String, String) {
    use std::io::Read;
    let mut out = String::new();
    let mut err = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    (out, err)
}

impl Sandbox for DevSandbox {
    fn compile(&self, code: &str) -> Result<CompileOutcome, GameError> {
        check_blocked_apis(code)?;

        let dir = tempfile::Builder::new()
            .prefix("rlg-")
            .tempdir()
            .map_err(|e| GameError::CompileEnv(e.to_string()))?;
        // 编译产物必须活到 run() 之后：泄漏 TempDir 避免目录被自动删除。
        // 开发期沙盒可接受 /tmp 累积；计划②真隔离（bwrap）时整体替换。
        let dir = Box::leak(Box::new(dir));
        let src = dir.path().join("main.rs");
        let out = dir.path().join("main");
        std::fs::write(&src, code)?;

        let mut child = Command::new("rustc")
            .arg("--edition")
            .arg("2021")
            .arg(&src)
            .arg("-o")
            .arg(&out)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GameError::CompileEnv(format!("无法启动 rustc: {e}")))?;

        let status = wait_with_timeout(&mut child, self.compile_timeout_secs)?;

        if status.success() {
            Ok(CompileOutcome::Success { binary: out })
        } else {
            let (_, stderr) = read_piped(&mut child);
            Ok(CompileOutcome::Failed { errors: parse_rustc_stderr(&stderr) })
        }
    }

    fn run(&self, binary: &Path) -> Result<RunOutcome, GameError> {
        let mut child = Command::new(binary)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GameError::RunEnv(format!("无法启动玩家程序: {e}")))?;

        let deadline = Instant::now() + Duration::from_secs(self.run_timeout_secs);
        let status = loop {
            if let Some(st) = child.try_wait()? {
                break st;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(RunOutcome::Timeout);
            }
            std::thread::sleep(Duration::from_millis(50));
        };

        let (stdout, stderr) = read_piped(&mut child);
        if status.success() {
            Ok(RunOutcome::Ok { stdout })
        } else {
            let msg = if stderr.trim().is_empty() {
                format!("程序以非零退出码退出（code {:?}）", status.code())
            } else {
                stderr.trim().to_string()
            };
            Ok(RunOutcome::Panic { message: msg })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::GameError;

    fn sandbox() -> DevSandbox {
        DevSandbox::new()
    }

    #[test]
    fn compile_success_then_run_ok() {
        let code = "fn main() { println!(\"hello {}\", 42); }";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Success { binary } => match sandbox().run(&binary).unwrap() {
                RunOutcome::Ok { stdout } => assert_eq!(stdout.trim(), "hello 42"),
                other => panic!("expected Ok, got {:?}", other),
            },
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn compile_failed_parses_error_code() {
        // E0502: 同时存在不可变与可变借用
        let code = "fn main() {\n    let mut s = String::from(\"hi\");\n    let r1 = &s;\n    let r2 = &mut s;\n    println!(\"{} {}\", r1, r2);\n}";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Failed { errors } => {
                assert!(!errors.is_empty());
                assert!(errors.iter().any(|e| e.code == "E0502"));
            }
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn run_panic_captures_message() {
        let code = "fn main() { panic!(\"boom {}\", 1); }";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Success { binary } => match sandbox().run(&binary).unwrap() {
                RunOutcome::Panic { message } => assert!(message.contains("boom"), "msg: {message}"),
                other => panic!("expected Panic, got {:?}", other),
            },
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn run_timeout_kills_infinite_loop() {
        let code = "fn main() { loop {} }";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Success { binary } => {
                let mut sb = sandbox();
                sb.run_timeout_secs = 1; // 测试用短超时
                assert!(matches!(sb.run(&binary).unwrap(), RunOutcome::Timeout));
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn blocked_fs_api_rejected() {
        let code = "fn main() { let _ = std::fs::read_to_string(\"/etc/passwd\"); }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn blocked_use_statement_rejected() {
        let code = "use std::net::TcpStream;\nfn main() { let _ = TcpStream::connect(\"x\"); }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn string_mention_of_fs_not_blocked() {
        let code = "fn main() { println!(\"std::fs is not executed\"); }";
        assert!(matches!(sandbox().compile(code), Ok(CompileOutcome::Success { .. })));
    }

    #[test]
    fn output_mismatch_is_ok_with_expected_out() {
        // 运行输出由调用方比对；这里验证 stdout 原样返回
        let code = "fn main() { println!(\"a\\nb\"); }";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Success { binary } => match sandbox().run(&binary).unwrap() {
                RunOutcome::Ok { stdout } => assert_eq!(stdout, "a\nb\n"),
                other => panic!("expected Ok, got {:?}", other),
            },
            other => panic!("expected Success, got {:?}", other),
        }
    }
}
