//! 玩家流程烟雾回归：每一份内置素材都要能从地图进入，并经 GameApp 走到反馈屏。
//!
//! 关卡 starter code 故意大多不能通过；本测试不要求通关，而是锁定 UI/应用层的
//! 接线：代码关调用编译验证、选择题提交选项且不经过代码编辑器、所有失败都产生
//! 可展示的反馈，之后可返回地图继续选择下一关。

use crab_quest_core::app::{GameApp, Input, Screen};
use crab_quest_core::engine::Engine;
use crab_quest_core::level::LevelSet;
use crab_quest_core::sandbox::DevSandbox;
use crab_quest_core::save::{LevelProgress, LevelState, SaveData};
use crab_quest_core::validate::mapper::ErrorMapper;

fn assets_levels_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/levels")
}

fn app_with_all_builtin_levels_selectable() -> GameApp {
    let levels = LevelSet::load(&assets_levels_dir()).expect("加载内置关卡素材");
    let mut save = SaveData::default();
    // 这是回归测试的地图导航前置条件，不代表正常游戏中的解锁规则。
    for level in &levels.levels {
        save.level_states.insert(
            level.id.clone(),
            LevelProgress {
                state: LevelState::Unlocked,
                ..Default::default()
            },
        );
    }
    GameApp::new(Engine::new(
        levels,
        save,
        ErrorMapper::default_fallback(),
        Box::new(DevSandbox::new()),
    ))
}

#[test]
fn every_builtin_level_reaches_feedback_through_player_flow() {
    let level_count = LevelSet::load(&assets_levels_dir())
        .expect("加载内置关卡素材")
        .len();
    assert_eq!(level_count, 56, "新增内置关卡时须覆盖本玩家流程回归");

    for index in 0..level_count {
        // 独立 app 防止失败扣心、XP 和解锁状态影响另一个关卡的结果。
        let mut app = app_with_all_builtin_levels_selectable();
        for _ in 0..index {
            app.handle(Input::Down).expect("地图向下选择");
        }
        app.handle(Input::Enter).expect("从地图进入关卡");

        let (id, kind, answer) = match app.screen() {
            Screen::Level(data) => (
                data.level.id.clone(),
                data.level.kind.clone(),
                data.level.answer_index,
            ),
            other => panic!("关卡索引 {index} 未进入 Level 屏：{other:?}"),
        };
        if kind == "quiz" {
            // 选择题必须由答案选择输入驱动，不能落入代码提交路径。
            let answer = answer.expect("合法选择题必须有 answer_index");
            app.handle(Input::SelectQuizAnswer(answer))
                .expect("选择正确选项");
        }
        app.handle(Input::Submit).expect("提交关卡");

        match app.screen() {
            Screen::Feedback(feedback) => {
                assert_eq!(feedback.level_id, id, "反馈不得串到其他关卡");
                if kind == "quiz" {
                    assert!(feedback.passed, "选择题 {id} 的正确选项应能通关");
                }
                if !feedback.passed {
                    // 失败反馈三分支必须恰好有一个载荷，确保玩家不会看到空白面板。
                    let branches = (!feedback.errors.is_empty()) as u8
                        + feedback.expectation.is_some() as u8
                        + feedback.panic.is_some() as u8;
                    assert_eq!(branches, 1, "{id} 的失败反馈必须恰有一个展示分支");
                }
            }
            other => panic!("{id} 提交后未进入 Feedback 屏：{other:?}"),
        }

        // 任何反馈都可按 Esc 返回地图，不滞留在无法退出的屏幕。
        app.handle(Input::Esc).expect("从反馈返回地图");
        assert!(matches!(app.screen(), Screen::ChapterMap(_)), "{id} 反馈无法返回地图");
    }
}
