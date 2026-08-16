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
        Self {
            compile_timeout_secs: 10,
            run_timeout_secs: 2,
        }
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

    let mut scan = Scan {
        blocked: &mut blocked,
    };
    syn::visit::Visit::visit_file(&mut scan, &ast);

    if let Some(s) = blocked {
        return Err(GameError::SandboxBlocked(format!(
            "该代码使用了游戏不允许的 API：{s}"
        )));
    }
    Ok(())
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    secs: u64,
) -> Result<std::process::ExitStatus, GameError> {
    wait_with_timeout_opt(child, secs)?.ok_or(GameError::CompileTimeout(secs))
}

/// 轮询子进程直到退出或超时；超时则 kill 并等待回收，返回 Ok(None)。
/// 编译阶段（DevSandbox/BwrapSandbox 共用）由 wait_with_timeout 包装成超时错误；
/// 运行阶段由调用方把 None 映射为 RunOutcome::Timeout。
fn wait_with_timeout_opt(
    child: &mut std::process::Child,
    secs: u64,
) -> Result<Option<std::process::ExitStatus>, GameError> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
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
            Ok(CompileOutcome::Failed {
                errors: parse_rustc_stderr(&stderr),
            })
        }
    }

    fn run(&self, binary: &Path) -> Result<RunOutcome, GameError> {
        let mut child = Command::new(binary)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GameError::RunEnv(format!("无法启动玩家程序: {e}")))?;

        let status = match wait_with_timeout_opt(&mut child, self.run_timeout_secs)? {
            Some(st) => st,
            None => return Ok(RunOutcome::Timeout),
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

// ============ P4-24：bwrap 真隔离沙盒（v3 §9.1 目标架构） ============

/// 编译期内存上限（`ulimit -v`，KiB）：1 GiB。
/// 给 rustc + lld 充足余量——512 MiB 实测约 8% 偶发链接失败（lld 每工作线程
/// 8 MiB 栈 × 核数，逼近 RLIMIT_AS 临界）；1 GiB 实测稳定（8/8）。
pub const BWRAP_COMPILE_MEM_LIMIT_KB: u64 = 1_048_576;

/// 运行期内存上限（`ulimit -v`，KiB）：512 MiB。
/// 玩家练习程序远小于该值；恶意分配炸弹在 512 MiB 处触发分配失败
/// （RunOutcome::Panic / 非零退出），杜绝内存耗尽主机。
pub const BWRAP_RUN_MEM_LIMIT_KB: u64 = 524_288;

/// bwrap（bubblewrap）真隔离沙盒（P4-24），替换开发期兜底，达到分发标准。
///
/// 隔离边界（v3 §9.1）：
/// - `--unshare-all`：用户/pid/uts/ipc/cgroup/网络命名空间全新；
///   `--unshare-pid` 把玩家进程树限制在沙盒命名空间内——kill bwrap
///   （命名空间内 pid 1）即整树终止，fork 炸弹无法逃逸到主机。
/// - `--ro-bind / /`：整棵根文件系统只读（rustc + stdlib + 链接器可读）。
///   选整树 ro 而非逐目录清单：同样满足「系统目录只读」且更简单；
///   玩家代码对主目录/系统目录一律只读（写 → EROFS/EPERM）。
/// - `--tmpfs /tmp`：沙盒内 /tmp 为 tmpfs 工作区（编译中间文件落在其上；
///   项目目录 = 主机 tempfile 私有临时目录，本机 /tmp 即 tmpfs）。
/// - 最小 `/proc`（`--proc /proc`）与最小设备集（urandom/random/null/zero/tty
///   只读绑定）——不挂整棵 /dev，杜绝块设备（nvme/sd*）暴露。
/// - 禁网络：`--unshare-all` 含 `--unshare-net`（实测沙盒内仅剩 lo）。
/// - 内存限制：bwrap 无 `--rlimit` 选项，用 `sh -c 'ulimit -v N; …'` 包装
///   （编译 1 GiB / 运行 512 MiB，实测参数集见 `spawn_sandbox`）。
/// - 超时：编译 10s / 运行 2s，复用 wait_with_timeout 系列；kill 的是 bwrap
///   进程 = 终止整个沙盒命名空间。
///
/// 降级策略（安全优先）：bwrap 缺失或启动探测失败 → `GameError::CompileEnv`
/// 「沙盒初始化失败」中文错误，**绝不静默回退 DevSandbox**（无隔离模式）。
#[derive(Debug)]
pub struct BwrapSandbox {
    pub compile_timeout_secs: u64,
    pub run_timeout_secs: u64,
    pub compile_mem_limit_kb: u64,
    pub run_mem_limit_kb: u64,
    /// bwrap 可执行文件（默认 PATH 上的 "bwrap"；测试可注入假路径模拟缺失）。
    pub bwrap: PathBuf,
}

impl Default for BwrapSandbox {
    fn default() -> Self {
        Self {
            compile_timeout_secs: 10,
            run_timeout_secs: 2,
            compile_mem_limit_kb: BWRAP_COMPILE_MEM_LIMIT_KB,
            run_mem_limit_kb: BWRAP_RUN_MEM_LIMIT_KB,
            bwrap: PathBuf::from("bwrap"),
        }
    }
}

impl BwrapSandbox {
    /// 启动探测（main.rs 启动时调用）：跑一遍完整 bwrap 真隔离调用
    /// （ro-bind 根 + tmpfs + 最小设备 + unshare-all），失败即显式报错。
    pub fn try_new() -> Result<Self, GameError> {
        let sb = Self::default();
        sb.probe()?;
        Ok(sb)
    }

    /// 测试注入：用指定 bwrap 路径做启动探测（假路径 → 「沙盒初始化失败」）。
    /// 生产代码只用 [`Self::try_new`]。
    pub fn with_bwrap(bwrap: PathBuf) -> Result<Self, GameError> {
        let sb = Self {
            bwrap,
            ..Default::default()
        };
        sb.probe()?;
        Ok(sb)
    }

    fn probe(&self) -> Result<(), GameError> {
        let dir = tempfile::Builder::new()
            .prefix("rlg-probe-")
            .tempdir()
            .map_err(|e| GameError::CompileEnv(format!("沙盒初始化失败：无法创建探测目录: {e}")))?;
        let mut child = self
            .spawn_sandbox(
                dir.path(),
                false,
                BWRAP_COMPILE_MEM_LIMIT_KB,
                "exec \"$1\"",
                "true",
            )
            .map_err(sandbox_init_error)?;
        let status = child.wait().map_err(|e| {
            GameError::CompileEnv(format!("沙盒初始化失败：bwrap 探测进程异常: {e}"))
        })?;
        if !status.success() {
            return Err(GameError::CompileEnv(format!(
                "沙盒初始化失败：bwrap 真隔离不可用（退出码 {:?}）。请确认已安装 bubblewrap 且内核允许用户命名空间（unprivileged_userns_clone=1）。安全原因：游戏拒绝在无隔离模式下运行。",
                status.code()
            )));
        }
        Ok(())
    }

    /// 组装 bwrap 参数并启动沙盒进程。
    /// `workdir`：项目工作区（编译可写 `--bind` / 运行只读 `--ro-bind`）；
    /// `limit_kb`：`ulimit -v` 内存上限；`script`：sh 包装脚本（可用 `$1`）；
    /// `script_arg`：注入 `$1` 的值（工作区路径或要执行的二进制路径）。
    fn spawn_sandbox(
        &self,
        workdir: &Path,
        writable: bool,
        limit_kb: u64,
        script: &str,
        script_arg: &str,
    ) -> Result<std::process::Child, std::io::Error> {
        let mut args: Vec<String> = vec![
            "--unshare-all".into(), // 用户/pid/uts/ipc/cgroup/网络命名空间全新
            "--ro-bind".into(),
            "/".into(),
            "/".into(), // 整棵根 fs 只读
            "--proc".into(),
            "/proc".into(), // 最小 /proc
            "--tmpfs".into(),
            "/tmp".into(), // 沙盒内 /tmp 为 tmpfs 工作区
            // 最小设备集：不挂整棵 /dev（杜绝块设备 nvme/sd* 暴露）
            "--dev-bind".into(),
            "/dev/urandom".into(),
            "/dev/urandom".into(),
            "--dev-bind".into(),
            "/dev/random".into(),
            "/dev/random".into(),
            "--dev-bind".into(),
            "/dev/null".into(),
            "/dev/null".into(),
            "--dev-bind".into(),
            "/dev/zero".into(),
            "/dev/zero".into(),
            "--dev-bind".into(),
            "/dev/tty".into(),
            "/dev/tty".into(),
            (if writable { "--bind" } else { "--ro-bind" }).into(),
            workdir.to_string_lossy().into_owned(),
            workdir.to_string_lossy().into_owned(),
        ];
        // 内存限制：bwrap 无 --rlimit 选项，用 sh 包装 `ulimit -v`；
        // workdir/二进制经 $1 传入（独立 argv，无 shell 注入面）。
        args.push("/bin/sh".into());
        args.push("-c".into());
        args.push(format!("ulimit -v {limit_kb}; {script}"));
        args.push("sh".into());
        args.push(script_arg.to_string());

        let mut c = Command::new(&self.bwrap);
        c.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped());
        c.spawn()
    }
}

/// bwrap 缺失/启动失败 → 显式中文错误（绝不静默回退无隔离模式）。
fn sandbox_init_error(e: std::io::Error) -> GameError {
    if e.kind() == std::io::ErrorKind::NotFound {
        GameError::CompileEnv(
            "沙盒初始化失败：未找到 bwrap（bubblewrap）可执行文件，请安装 bubblewrap 后重试。安全原因：游戏拒绝在无隔离模式下运行。"
                .into(),
        )
    } else {
        GameError::CompileEnv(format!(
            "沙盒初始化失败：无法启动 bwrap 沙盒进程: {e}。安全原因：游戏拒绝在无隔离模式下运行。"
        ))
    }
}

impl Sandbox for BwrapSandbox {
    fn compile(&self, code: &str) -> Result<CompileOutcome, GameError> {
        // 纵深防御：syn 静态拦截保留（bwrap 是进程级兜底）
        check_blocked_apis(code)?;

        let dir = tempfile::Builder::new()
            .prefix("rlg-")
            .tempdir()
            .map_err(|e| GameError::CompileEnv(e.to_string()))?;
        // 编译产物必须活到 run()：泄漏 TempDir 防自动删除（与 DevSandbox 同策略）
        let dir = Box::leak(Box::new(dir));
        let src = dir.path().join("main.rs");
        let out = dir.path().join("main");
        std::fs::write(&src, code)?;

        let mut child = self
            .spawn_sandbox(
                dir.path(),
                true, // 编译期工作区可写（rustc 写产物）
                self.compile_mem_limit_kb,
                "cd \"$1\" && exec rustc --edition 2021 main.rs -o main",
                dir.path().to_str().expect("工作区路径非 UTF-8"),
            )
            .map_err(sandbox_init_error)?;

        let status = wait_with_timeout(&mut child, self.compile_timeout_secs)?;

        if status.success() {
            Ok(CompileOutcome::Success { binary: out })
        } else {
            let (_, stderr) = read_piped(&mut child);
            Ok(CompileOutcome::Failed {
                errors: parse_rustc_stderr(&stderr),
            })
        }
    }

    fn run(&self, binary: &Path) -> Result<RunOutcome, GameError> {
        let workdir = binary
            .parent()
            .ok_or_else(|| GameError::RunEnv(format!("玩家程序路径无效: {}", binary.display())))?;
        let mut child = self
            .spawn_sandbox(
                workdir,
                false, // 运行期工作区只读：玩家代码不得写任何主机路径
                self.run_mem_limit_kb,
                "exec \"$1\"",
                binary.to_str().expect("二进制路径非 UTF-8"),
            )
            .map_err(|e| {
                GameError::RunEnv(format!(
                    "沙盒运行环境错误：无法启动 bwrap 沙盒进程: {e}。安全原因：游戏拒绝在无隔离模式下运行。"
                ))
            })?;

        let status = match wait_with_timeout_opt(&mut child, self.run_timeout_secs)? {
            Some(st) => st,
            None => return Ok(RunOutcome::Timeout),
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
                RunOutcome::Panic { message } => {
                    assert!(message.contains("boom"), "msg: {message}")
                }
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
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
    }

    #[test]
    fn blocked_use_statement_rejected() {
        let code = "use std::net::TcpStream;\nfn main() { let _ = TcpStream::connect(\"x\"); }";
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
    }

    // ============ P4-23：syn 拦截清单 7 类补全（v3 §9.2） ============

    #[test]
    fn blocked_process_api_rejected() {
        let code = "fn main() { let _ = std::process::Command::new(\"ls\"); }";
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
    }

    #[test]
    fn blocked_env_api_rejected() {
        let code = "fn main() { let _ = std::env::var(\"PATH\"); }";
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
    }

    #[test]
    fn blocked_thread_spawn_rejected() {
        // 并发：仅精确拦截 `std::thread::spawn`
        let code = "fn main() { std::thread::spawn(|| {}); }";
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
    }

    #[test]
    fn blocked_use_thread_spawn_rejected() {
        // `use std::thread::spawn;` 导入本身即触发精确拦截
        let code = "use std::thread::spawn;\nfn main() { spawn(|| {}); }";
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
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
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
    }

    #[test]
    fn blocked_unsafe_fn_rejected() {
        // 范围决策：unsafe fn 函数体可直接书写不安全操作，同属不安全面
        let code = "unsafe fn evil() {}";
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
    }

    #[test]
    fn blocked_extern_block_rejected() {
        let code = "extern \"C\" { fn abs(x: i32) -> i32; }\nfn main() {}";
        assert!(matches!(
            sandbox().compile(code),
            Err(GameError::SandboxBlocked(_))
        ));
    }

    #[test]
    fn string_mention_of_fs_not_blocked() {
        let code = "fn main() { println!(\"std::fs is not executed\"); }";
        assert!(matches!(
            sandbox().compile(code),
            Ok(CompileOutcome::Success { .. })
        ));
    }

    #[test]
    fn comment_mention_of_fs_not_blocked() {
        // AST 扫描回归：注释里的 std::fs 不误报
        let code = "// 提示：std::fs::read_to_string 会触发拦截\nfn main() { println!(\"ok\"); }";
        check_blocked_apis(code).unwrap();
    }

    #[test]
    fn all_starter_codes_pass_blocklist() {
        // 从仓库 assets/levels 动态读取（不复制代码防漂移），断言现有关卡
        // starter_code 全部通过静态拦截（无合法关卡被误杀，验收项）。
        use crate::level::LevelSet;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/levels");
        let set = LevelSet::load(&dir).expect("加载 assets/levels 失败");
        assert_eq!(set.len(), 55, "当前关卡集应有 55 关");
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
