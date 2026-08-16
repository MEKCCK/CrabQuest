//! P4-24：BwrapSandbox 真隔离集成测试（与 DevSandbox 单测分离，验收项）。
//!
//! - bwrap 缺失或用户命名空间受限时整体 SKIP（打印原因不失败）；本机已实测
//!   bwrap 0.11.2 + unprivileged_userns_clone=1，全部用例真实执行。
//! - 攻击用例刻意**绕过静态拦截**：用主机侧裸 rustc 编译恶意二进制再经
//!   `run()` 注入沙盒（run 不查 blocklist）——这正是 bwrap 的兜底价值：
//!   即使静态拦截被绕过，真隔离仍必须拦住网络 / 写主目录 / 资源耗尽。
//! - 15 关回归与 Tier 3 共用同一期望画像（tests/common/mod.rs 的 check_level，
//!   沙盒实现无关），断言 bwrap 隔离环境下编译/运行结果与 DevSandbox 一致。

mod common;
use common::*;

use game_core::level::LevelSet;
use game_core::sandbox::{BwrapSandbox, CompileOutcome, RunOutcome, Sandbox};
use std::path::PathBuf;
use std::process::Command;

/// bwrap 可用则返回沙盒，否则打印原因并返回 None（SKIP）。
fn sb_or_skip() -> Option<BwrapSandbox> {
    match BwrapSandbox::try_new() {
        Ok(sb) => Some(sb),
        Err(e) => {
            eprintln!("SKIP: bwrap 不可用（{e}）");
            None
        }
    }
}

/// 主机侧裸 rustc 编译（不经静态拦截）：返回（临时目录, 二进制路径）。
fn host_compile(code: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let src = dir.path().join("main.rs");
    let out = dir.path().join("main");
    std::fs::write(&src, code).expect("写入 main.rs 失败");
    let st = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(&src)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("启动 rustc 失败");
    assert!(
        st.status.success(),
        "夹具编译失败: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    (dir, out)
}

/// 主机上仍在运行的沙盒压力测试子进程数。
///
/// 子进程最多只休眠 2 秒：即使进程树清理回归，测试也只会短暂留下至多
/// 64 个进程，不会像无限 fork 炸弹那样耗尽主机资源。
fn host_stress_procs() -> usize {
    Command::new("pgrep")
        .arg("-fc")
        .arg("^/bin/sleep 2$")
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

// ============ 成功路径（与 DevSandbox 行为一致） ============

#[test]
fn bwrap_compile_run_roundtrip_ok() {
    let Some(sb) = sb_or_skip() else { return };
    match sb
        .compile("fn main() { println!(\"hello {}\", 42); }")
        .unwrap()
    {
        CompileOutcome::Success { binary } => match sb.run(&binary).unwrap() {
            RunOutcome::Ok { stdout } => assert_eq!(stdout.trim(), "hello 42"),
            other => panic!("expected Ok, got {other:?}"),
        },
        other => panic!("expected Success, got {other:?}"),
    }
}

#[test]
fn bwrap_compile_error_parses_ecode() {
    let Some(sb) = sb_or_skip() else { return };
    // E0502：同时存在不可变与可变借用
    let code = "fn main() {\n    let mut s = String::from(\"hi\");\n    let r1 = &s;\n    let r2 = &mut s;\n    println!(\"{} {}\", r1, r2);\n}";
    match sb.compile(code).unwrap() {
        CompileOutcome::Failed { errors } => {
            assert!(!errors.is_empty());
            assert!(errors.iter().any(|e| e.code == "E0502"));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn bwrap_panic_captured() {
    let Some(sb) = sb_or_skip() else { return };
    match sb.compile("fn main() { panic!(\"boom {}\", 1); }").unwrap() {
        CompileOutcome::Success { binary } => match sb.run(&binary).unwrap() {
            RunOutcome::Panic { message } => assert!(message.contains("boom"), "msg: {message}"),
            other => panic!("expected Panic, got {other:?}"),
        },
        other => panic!("expected Success, got {other:?}"),
    }
}

#[test]
fn bwrap_infinite_loop_timeout() {
    let Some(mut sb) = sb_or_skip() else { return };
    sb.run_timeout_secs = 1; // 测试用短超时
    match sb.compile("fn main() { loop {} }").unwrap() {
        CompileOutcome::Success { binary } => {
            assert!(matches!(sb.run(&binary).unwrap(), RunOutcome::Timeout));
        }
        other => panic!("expected Success, got {other:?}"),
    }
}

// ============ 攻击用例（注入绕过静态拦截的二进制） ============

/// 攻击①：网络访问被禁（--unshare-net）。TcpStream::connect 必须失败，
/// 不得建立任何连接。
#[test]
fn bwrap_network_connect_blocked() {
    let Some(sb) = sb_or_skip() else { return };
    let (_dir, bin) = host_compile(
        r#"fn main() {
    match std::net::TcpStream::connect("1.1.1.1:80") {
        Ok(_) => println!("CONNECTED"),
        Err(e) => println!("FAILED: {e}"),
    }
}"#,
    );
    match sb.run(&bin).unwrap() {
        RunOutcome::Ok { stdout } => {
            assert!(stdout.contains("FAILED"), "网络未被禁：{stdout}");
            assert!(!stdout.contains("CONNECTED"), "网络未被禁：{stdout}");
        }
        // 连接失败引发的 panic 同样证明网络被禁
        RunOutcome::Panic { .. } => {}
        RunOutcome::Timeout => panic!("连接挂起（应立刻网络不可达）"),
    }
}

/// 攻击②：写主目录被禁（根文件系统只读）。std::fs::write 到 $HOME 必须失败，
/// 且主机不得留下文件。
#[test]
fn bwrap_home_write_blocked() {
    let Some(sb) = sb_or_skip() else { return };
    let marker = format!(".rlg_bwrap_pwn_{}", std::process::id());
    let (_dir, bin) = host_compile(
        &r#"fn main() {
    let home = std::env::var("HOME").unwrap_or_default();
    let p = format!("{home}/__MARKER__");
    match std::fs::write(&p, "pwned") {
        Ok(_) => println!("WROTE {p}"),
        Err(e) => println!("WRITE_FAILED: {e}"),
    }
}"#
        .replace("__MARKER__", &marker),
    );
    match sb.run(&bin).unwrap() {
        RunOutcome::Ok { stdout } => {
            assert!(stdout.contains("WRITE_FAILED"), "写主目录未被禁：{stdout}");
            assert!(!stdout.contains("WROTE"), "写主目录未被禁：{stdout}");
        }
        other => panic!("expected Ok(WRITE_FAILED), got {other:?}"),
    }
    let home = std::env::var("HOME").expect("HOME 未设置");
    assert!(
        !std::path::Path::new(&home).join(&marker).exists(),
        "主机被写入文件！"
    );
}

/// 攻击③：有界进程树被 timeout 终止，且子进程不逃逸到主机。
/// `--unshare-pid` 把子进程限制在沙盒命名空间内；kill bwrap（命名空间内
/// pid 1）即整树终止——沙盒外不得残留任何子进程。
#[test]
fn bwrap_bounded_process_tree_timeout_cleans_tree() {
    let Some(mut sb) = sb_or_skip() else { return };
    sb.run_timeout_secs = 1;
    let before = host_stress_procs();
    let (_dir, bin) = host_compile(
        r#"fn main() {
    for _ in 0..64 {
        let _ = std::process::Command::new("/bin/sleep").arg("2").spawn();
    }
    loop {
        std::hint::spin_loop();
    }
}"#,
    );
    assert!(
        matches!(sb.run(&bin).unwrap(), RunOutcome::Timeout),
        "有界进程树未被超时终止"
    );
    // 给内核回收命名空间一点时间，然后确认主机无残留
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = host_stress_procs();
    assert!(
        after <= before,
        "沙盒外残留 {after} 个压力测试子进程（预期 ≤ {before}）"
    );
}

/// 攻击④：内存炸弹被 ulimit -v 512 MiB 拒绝（1 GiB 分配必须失败，
/// 不得成功返回）。
#[test]
fn bwrap_memory_bomb_rejected() {
    let Some(sb) = sb_or_skip() else { return };
    let (_dir, bin) = host_compile(
        r#"fn main() {
    let buf = vec![0u8; 1 << 30];
    println!("ALLOCATED {}", buf.len());
}"#,
    );
    match sb.run(&bin).unwrap() {
        RunOutcome::Ok { stdout } => panic!("1 GiB 分配应被内存限制拒绝，实际成功: {stdout}"),
        // 分配失败 → abort/panic（可能带 "memory allocation failed" 或非零退出兜底文案）
        RunOutcome::Panic { .. } => {}
        RunOutcome::Timeout => panic!("内存炸弹不应超时"),
    }
}

// ============ 降级策略 ============

/// bwrap 缺失 → 显式中文错误（「沙盒初始化失败」），
/// 绝不静默回退到 DevSandbox（无隔离模式）。
#[test]
fn bwrap_missing_is_explicit_error_not_fallback() {
    let sb = BwrapSandbox {
        bwrap: PathBuf::from("/nonexistent/bwrap"),
        ..Default::default()
    };
    // 提交路径（compile）：bwrap 缺失 → 显式中文初始化错误
    let msg = sb.compile("fn main() {}").unwrap_err().to_string();
    assert!(
        msg.contains("沙盒初始化失败"),
        "应给出中文初始化错误: {msg}"
    );
    assert!(msg.contains("bwrap"), "应点名 bwrap: {msg}");

    // 运行路径（run）：同样显式报错，不静默回退
    let err = sb
        .run(std::path::Path::new("/tmp/nonexistent-main"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("沙盒"), "run 应显式报错: {err}");
    assert!(err.contains("bwrap"), "run 应点名 bwrap: {err}");

    // 启动探测路径（try_new）：假 bwrap 路径 → 显式初始化失败
    let err2 = BwrapSandbox::with_bwrap(PathBuf::from("/nonexistent/bwrap"))
        .unwrap_err()
        .to_string();
    assert!(err2.contains("沙盒初始化失败"), "try_new 应失败: {err2}");
}

// ============ 全量关卡回归（与 DevSandbox 同一期望画像） ============

/// 全部 starter 经 BwrapSandbox 编译/运行画像与 DevSandbox 完全一致
/// （check_level 沙盒实现无关，单一事实源在 tests/common/mod.rs）。
#[test]
fn bwrap_all_levels_starter_regression() {
    let Some(sb) = sb_or_skip() else { return };
    let set = LevelSet::load(&assets_levels_dir()).expect("加载 assets/levels 失败");
    assert_eq!(
        set.len(),
        55,
        "当前关卡集应为 55 关"
    );
    let mut failures: Vec<String> = Vec::new();
    for lv in &set.levels {
        if let Err(e) = check_level(lv, &sb) {
            failures.push(format!("{}: {e}", lv.id));
        }
    }
    assert!(
        failures.is_empty(),
        "bwrap 回归失败 {} / {} 关：\n{}",
        failures.len(),
        set.len(),
        failures.join("\n")
    );
}
