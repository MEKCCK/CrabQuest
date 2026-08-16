//! Rank 系统（v3 §7.3）：按完成关卡数判定的 10 级中文称号。
//!
//! - 纯函数、无存档字段：由 `SaveData::completed_count()`（state==Passed 的关卡数）推导；
//! - rank 只解锁元内容（XP 进度条、错误码图鉴、统计页、自由模式），**不解锁关卡**，
//!   关卡保持线性解锁链（engine.submit 上一关 Passed → 下一关 Unlocked）；
//! - 称号与 v3 §6.1 术语表一致。

/// 单级 rank：`level` 为 1..=10（R1..R10），`title` 为 v3 §7.3 定稿中文称号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rank {
    pub level: u8,
    pub title: &'static str,
}

/// (等级, 所需完成关卡数, 中文称号)；阈值升序，完成关卡数 >= 阈值即达到该级。
/// 里程碑：1→R2(00-l0-hello)、4→R3(L0 全)、5→R4(04-l1-move)、8→R5(L1 全)、
/// 9→R6(08-l2-vec)、11→R7(L2 全)、12→R8(11-l3-lifetime)、13→R9(L3 全)、15→R10(全 15 关)。
const RANKS: [(u8, usize, &str); 10] = [
    (1, 0, "见习学徒"),
    (2, 1, "输出新手"),
    (3, 4, "语法学徒"),
    (4, 5, "所有权新兵"),
    (5, 8, "借用骑士"),
    (6, 9, "集合行者"),
    (7, 11, "错误猎人"),
    (8, 12, "特质学徒"),
    (9, 13, "生命周期贤者"),
    (10, 15, "铁锈冠军"),
];

/// 按完成关卡数判定 rank（v3 §7.3）：R1 开局，R10 全部 15 关；超过 15 封顶 R10。
pub fn rank_for(completed_count: usize) -> Rank {
    let &(level, _, title) = RANKS
        .iter()
        .rev()
        .find(|(_, threshold, _)| completed_count >= *threshold)
        .unwrap_or(&RANKS[0]);
    Rank { level, title }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_milestones_boundaries() {
        // v3 §7.3 / §11.4 里程碑边界：1→R2、4→R3、8→R5、11→R7、15→R10（全 5 项）
        assert_eq!(rank_for(0), Rank { level: 1, title: "见习学徒" });
        assert_eq!(rank_for(1), Rank { level: 2, title: "输出新手" });
        assert_eq!(rank_for(4), Rank { level: 3, title: "语法学徒" });
        assert_eq!(rank_for(8), Rank { level: 5, title: "借用骑士" });
        assert_eq!(rank_for(11), Rank { level: 7, title: "错误猎人" });
        assert_eq!(rank_for(15), Rank { level: 10, title: "铁锈冠军" });
    }

    #[test]
    fn rank_intermediate_thresholds() {
        // 逐级核对全表：5→R4、9→R6、12→R8、13→R9；超过全通关封顶 R10
        assert_eq!(rank_for(5), Rank { level: 4, title: "所有权新兵" });
        assert_eq!(rank_for(9), Rank { level: 6, title: "集合行者" });
        assert_eq!(rank_for(12), Rank { level: 8, title: "特质学徒" });
        assert_eq!(rank_for(13), Rank { level: 9, title: "生命周期贤者" });
        assert_eq!(rank_for(15), rank_for(16));
        assert_eq!(rank_for(15), rank_for(100));
    }

    #[test]
    fn rank_titles_sequence() {
        // 称号序列与 v3 §7.3 完全一致（按首次出现顺序）
        let mut seen: Vec<&str> = Vec::new();
        for n in 0..=15 {
            let t = rank_for(n).title;
            if !seen.contains(&t) {
                seen.push(t);
            }
        }
        assert_eq!(
            seen,
            vec![
                "见习学徒", "输出新手", "语法学徒", "所有权新兵", "借用骑士", "集合行者",
                "错误猎人", "特质学徒", "生命周期贤者", "铁锈冠军",
            ]
        );
    }
}
