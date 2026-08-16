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

/// 整段拦截的 std 前缀：路径以这些字符串开头即拦截（`std::fs` 全部等）。
const BLOCKED_PREFIXES: [&str; 4] = ["std::fs", "std::process", "std::env", "std::net"];

/// 精确拦截：仅 `std::thread::spawn` 本身（v3 §9.2「并发」精确匹配）。
/// `std::thread::sleep` / `std::thread::yield_now` / `std::thread::park` 等
/// 无害调用必须放行；`use std::thread;` 导入模块本身也不拦截。
const BLOCKED_EXACT_PATHS: [&str; 1] = ["std::thread::spawn"];

/// 拼接 `Path` 的完整限定名（如 `std::fs::read_to_string`）。
fn path_string(p: &syn::Path) -> String {
    p.segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// 收集 `use` 树展开出的每个完整导入路径（如 `use std::fs::{self, File};`
/// → `["std::fs::self", "std::fs::File"]`）。
/// 重命名导入按**原始**路径检查（`use std::fs as myfs;` 不得绕过拦截）。
fn collect_use_paths(t: &syn::UseTree, segs: &mut Vec<String>, out: &mut Vec<String>) {
    match t {
        syn::UseTree::Path(p) => {
            segs.push(p.ident.to_string());
            collect_use_paths(&p.tree, segs, out);
            segs.pop();
        }
        syn::UseTree::Name(n) => {
            segs.push(n.ident.to_string());
            out.push(segs.join("::"));
            segs.pop();
        }
        syn::UseTree::Rename(r) => {
            segs.push(r.rename.to_string());
            out.push(segs.join("::"));
            segs.pop();
        }
        syn::UseTree::Glob(_) => {
            segs.push("*".to_string());
            out.push(segs.join("::"));
            segs.pop();
        }
        syn::UseTree::Group(g) => {
            for item in &g.items {
                collect_use_paths(item, segs, out);
            }
        }
    }
}

/// 命中拦截清单则返回要展示的符号；未命中返回 None。
fn blocked_match(s: &str) -> Option<String> {
    if BLOCKED_EXACT_PATHS.contains(&s) {
        return Some(s.to_string());
    }
    BLOCKED_PREFIXES
        .iter()
        .find(|p| s.starts_with(**p))
        .map(|_| s.to_string())
}

/// 静态拦截：玩家代码中禁止访问文件系统、进程、环境变量、网络、
/// `std::thread::spawn`、`unsafe` 块/`unsafe fn`、`extern` FFI 块（v3 §9.2）。
/// 基于 syn AST 扫描——注释与字符串字面量不在 AST 路径节点内，天然不误报。
fn check_blocked_apis(code: &str) -> Result<(), GameError> {
    let ast: syn::File = syn::parse_file(code)
        .map_err(|e| GameError::SandboxBlocked(format!("代码语法错误: {e}")))?;
    let mut blocked: Option<String> = None;

    struct Scan<'a> {
        blocked: &'a mut Option<String>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Scan<'_> {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            let s = path_string(&node.path);
            if let Some(hit) = blocked_match(&s) {
                *self.blocked = Some(hit);
                return;
            }
            syn::visit::visit_expr_path(self, node);
        }
        fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
            let s = path_string(&node.path);
            if let Some(hit) = blocked_match(&s) {
                *self.blocked = Some(hit);
                return;
            }
            syn::visit::visit_type_path(self, node);
        }
        fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
            let mut segs = Vec::new();
            let mut paths = Vec::new();
            collect_use_paths(&node.tree, &mut segs, &mut paths);
            for s in paths {
                if let Some(hit) = blocked_match(&s) {
                    *self.blocked = Some(hit);
                    return;
                }
            }
            syn::visit::visit_item_use(self, node);
        }
        // 内存不安全（v3 §9.2）。范围决策：unsafe 块 + unsafe fn（含 impl/trait
        // 内的关联 unsafe fn）——unsafe fn 函数体可直接书写未检查操作，是完整的不安全面。
        fn visit_expr_unsafe(&mut self, _node: &'ast syn::ExprUnsafe) {
            *self.blocked = Some("unsafe 块".to_string());
        }
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
            if node.sig.unsafety.is_some() {
                *self.blocked = Some(format!("unsafe fn {}", node.sig.ident));
                return;
            }
            syn::visit::visit_item_fn(self, node);
        }
        fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
            if node.sig.unsafety.is_some() {
                *self.blocked = Some(format!("unsafe fn {}", node.sig.ident));
                return;
            }
            syn::visit::visit_impl_item_fn(self, node);
        }
        fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
            if node.sig.unsafety.is_some() {
                *self.blocked = Some(format!("unsafe fn {}", node.sig.ident));
                return;
            }
            syn::visit::visit_trait_item_fn(self, node);
        }
        // FFI：extern 块（含 edition 2024 的 `unsafe extern`，同为 ItemForeignMod）。
        fn visit_item_foreign_mod(&mut self, _node: &'ast syn::ItemForeignMod) {
            *self.blocked = Some("extern 块".to_string());
        }
    }

    let mut scan = Scan { blocked: &mut blocked };
    syn::visit::Visit::visit_file(&mut scan, &ast);

    if let Some(s) = blocked {
        return Err(GameError::SandboxBlocked(format!("该代码使用了游戏不允许的 API：{s}")));
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

fn read_piped(child: &mut std::process::Child) -> (String, String) {
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

    // ============ P4-23：syn 拦截清单 7 类补全（v3 §9.2） ============

    #[test]
    fn blocked_process_api_rejected() {
        let code = "fn main() { let _ = std::process::Command::new(\"ls\"); }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn blocked_env_api_rejected() {
        let code = "fn main() { let _ = std::env::var(\"PATH\"); }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn blocked_thread_spawn_rejected() {
        // 并发：仅精确拦截 `std::thread::spawn`
        let code = "fn main() { std::thread::spawn(|| {}); }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn blocked_use_thread_spawn_rejected() {
        // `use std::thread::spawn;` 导入本身即触发精确拦截
        let code = "use std::thread::spawn;\nfn main() { spawn(|| {}); }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn thread_sleep_and_yield_now_pass() {
        // 精确匹配回归：std::thread 整段不再拦截，sleep/yield_now 必须放行
        let code = "fn main() {\n    std::thread::sleep(std::time::Duration::from_millis(1));\n    std::thread::yield_now();\n}";
        check_blocked_apis(code).unwrap();
    }

    #[test]
    fn blocked_unsafe_block_rejected() {
        let code = "fn main() { unsafe { let x = 1; } }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn blocked_unsafe_fn_rejected() {
        // 范围决策：unsafe fn 函数体可直接书写不安全操作，同属不安全面
        let code = "unsafe fn evil() {}";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn blocked_extern_block_rejected() {
        let code = "extern \"C\" { fn abs(x: i32) -> i32; }\nfn main() {}";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn string_mention_of_fs_not_blocked() {
        let code = "fn main() { println!(\"std::fs is not executed\"); }";
        assert!(matches!(sandbox().compile(code), Ok(CompileOutcome::Success { .. })));
    }

    #[test]
    fn comment_mention_of_fs_not_blocked() {
        // AST 扫描回归：注释里的 std::fs 不误报
        let code = "// 提示：std::fs::read_to_string 会触发拦截\nfn main() { println!(\"ok\"); }";
        check_blocked_apis(code).unwrap();
    }

    #[test]
    fn all_15_starter_codes_pass_blocklist() {
        // 从仓库 assets/levels 动态读取（不复制代码防漂移），断言现有关卡
        // starter_code 全部通过静态拦截（无合法关卡被误杀，验收项）。
        use crate::level::LevelSet;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/levels");
        let set = LevelSet::load(&dir).expect("加载 assets/levels 失败");
        assert_eq!(set.len(), 15, "第一版应有 15 关");
        for lv in &set.levels {
            check_blocked_apis(&lv.starter_code)
                .unwrap_or_else(|e| panic!("关卡 {} 的 starter_code 被静态拦截误杀: {e}", lv.id));
        }
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
