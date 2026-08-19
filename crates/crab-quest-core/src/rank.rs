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
/// 当前主线为 55 关：R4/R6/R8 对应 L0/L1/L2 完成，R9 在 L3 后段开放统计，
/// R10 仅在全主线通关时取得并开启自由模式。
const RANKS: [(u8, usize, &str); 10] = [
    (1, 0, "见习学徒"),
    (2, 1, "输出新手"),
    (3, 4, "语法学徒"),
    (4, 10, "所有权新兵"),
    (5, 16, "借用骑士"),
    (6, 22, "集合行者"),
    (7, 29, "错误猎人"),
    (8, 36, "特质学徒"),
    (9, 43, "生命周期贤者"),
    (10, 55, "铁锈冠军"),
];

/// 按完成关卡数判定 rank：R1 开局，R10 全部 55 关；超过 55 封顶 R10。
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
        // 55 关主线的关键边界：首关、L0/L1/L2 完成、统计页、全通关。
        assert_eq!(rank_for(0), Rank { level: 1, title: "见习学徒" });
        assert_eq!(rank_for(1), Rank { level: 2, title: "输出新手" });
        assert_eq!(rank_for(4), Rank { level: 3, title: "语法学徒" });
        assert_eq!(rank_for(10), Rank { level: 4, title: "所有权新兵" });
        assert_eq!(rank_for(22), Rank { level: 6, title: "集合行者" });
        assert_eq!(rank_for(36), Rank { level: 8, title: "特质学徒" });
        assert_eq!(rank_for(43), Rank { level: 9, title: "生命周期贤者" });
        assert_eq!(rank_for(55), Rank { level: 10, title: "铁锈冠军" });
    }

    #[test]
    fn rank_intermediate_thresholds() {
        // 逐级核对全表，并确认超过全通关后封顶 R10。
        assert_eq!(rank_for(9), Rank { level: 3, title: "语法学徒" });
        assert_eq!(rank_for(16), Rank { level: 5, title: "借用骑士" });
        assert_eq!(rank_for(29), Rank { level: 7, title: "错误猎人" });
        assert_eq!(rank_for(55), rank_for(56));
        assert_eq!(rank_for(55), rank_for(100));
    }

    #[test]
    fn rank_titles_sequence() {
        // 称号序列与 v3 §7.3 完全一致（按首次出现顺序）
        let mut seen: Vec<&str> = Vec::new();
        for n in 0..=55 {
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
