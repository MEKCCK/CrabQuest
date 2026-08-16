//! Tier 3：关卡回归（P2-16 §3）——15 关 starter 编译断言 + 01-l0-print 反馈非空锁。
//!
//! 从 `assets/levels/*.toml` **动态读取** starter_code（不复制代码进测试，防素材
//! 漂移；T3 中文字面量修订也不影响），用 DevSandbox 真实编译，断言每关 starter 的
//! 期望画像：首条错误码（rustc 输出顺序，L3-B2 §1.1）/ 编译成功 / 编译成功+运行
//! panic 分类。期望表源自 docs-review/L3-B2-parser.md §3 的 15 fixture 实测
//! （rustc 1.97）；**不锁行号**（素材微调会漂），只锁错误码与运行行为。
//!
//! 01-l0-print 单独经 `validate()` 全流程断言反馈非空（锁死空反馈 bug，P1-01 /
//! L3-B2 §1.5：format 参数错误无 E 码，解析器必须产出 EUNKNOWN）。
//!
//! 本文件为默认运行的回归测试（非 #[ignore]）；成本 ≈ 15 次编译 + 2 次运行 ≈ 7s。

mod common;
use common::*;

use game_core::level::LevelSet;
use game_core::sandbox::{CompileOutcome, DevSandbox, Sandbox};
use game_core::validate::error_parser::IssueKind;
use game_core::validate::mapper::ErrorMapper;
use game_core::validate::{validate, Validation};

/// Tier 3 主断言：15 关 starter 编译画像（错误码 / 编译成功 / panic 分类）。
#[test]
fn tier3_levels_starter_error_codes() {
    let set = LevelSet::load(&assets_levels_dir()).expect("加载 assets/levels 失败");
    assert_eq!(
        set.len(),
        15,
        "关卡数应为 15（新增关卡必须同步更新 expect_for 期望表）"
    );
    let sb = DevSandbox::new();
    let mut failures: Vec<String> = Vec::new();
    for lv in &set.levels {
        if let Err(e) = check_level(lv, &sb) {
            failures.push(format!("{}: {e}", lv.id));
        }
    }
    assert!(
        failures.is_empty(),
        "Tier 3 失败 {} / {} 关：\n{}",
        failures.len(),
        set.len(),
        failures.join("\n")
    );
}

/// 锁死空反馈 bug（P1-01 / L3-B2 §1.5）：
/// 01-l0-print starter 的 format 参数错误**无 E 码**，解析器必须产出 EUNKNOWN，
/// validate() 反馈必须非空（历史 bug：空白反馈面板，现网级回归）。
#[test]
fn tier3_l0_print_feedback_non_empty() {
    let set = LevelSet::load(&assets_levels_dir()).expect("加载 assets/levels 失败");
    let lv = set.get("l0-print").expect("缺少 l0-print 关卡");
    let sb = DevSandbox::new();

    // 1) 真实编译：首条错误必须是无码错误 EUNKNOWN（kind=NoCode）
    match sb.compile(&lv.starter_code).unwrap() {
        CompileOutcome::Failed { errors } => {
            let first = errors
                .first()
                .expect("编译失败但 errors 为空：解析器漏检（将触发 FALLBACK）");
            assert_eq!(first.code, "EUNKNOWN", "应解析出 EUNKNOWN: {errors:?}");
            assert_eq!(
                first.kind,
                IssueKind::NoCode,
                "kind 应为 NoCode: {errors:?}"
            );
        }
        other => panic!("01-l0-print starter 应编译失败，实际 {other:?}"),
    }

    // 2) 全流程 validate：反馈非空且走「无码编译错误」分支（EUNKNOWN 或 fallback 均可，
    //    但绝不能空白）
    match validate(lv, &lv.starter_code, &ErrorMapper::default_fallback(), &sb).unwrap() {
        Validation::Fail { errors, .. } => {
            assert!(!errors.is_empty(), "禁止空反馈（锁死空白反馈面板 bug）");
            assert!(!errors[0].zh.trim().is_empty(), "卡片 zh 不能是空白串");
            assert!(
                errors[0].zh.contains("编译错误") || errors[0].zh.contains("无法解析"),
                "应展示无码错误文案或硬兜底文案: {errors:?}"
            );
        }
        Validation::Pass { .. } => panic!("broken starter 不应 Pass"),
    }
}
