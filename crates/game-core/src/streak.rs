//! 连续游玩日（P2-09，v3 §7.4）：纯 std 日期算法（Hinnant civil-days 公式），
//! 无 chrono 依赖。引擎只消费 `touch_streak` / `today_str`，跨月/跨年边界
//! 在本模块的纯函数单测中锁定（§11.4：2-28→3-01 与 12-31→1-01 不断链）。

/// Hinnant civil-days 算法：公历日期 (y, m, d) → 自 1970-01-01 起的天数
/// （proleptic Gregorian，等价于 chrono `NaiveDate::num_days_from_ce` 的偏移量）。
/// 纯函数，无 IO；`days_from_civil(1970, 1, 1) == 0`。
///
/// 公式来源：Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms"
/// （`days_from_civil`，即 C++20 `std::chrono::days` 底层的标准实现）。
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // [0, 11]（3 月 = 0）
    let doy = (153 * mp as i64 + 2) / 5 + d as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719_468
}

/// `days_from_civil` 的逆变换：天数 → (年, 月, 日)（Hinnant `civil_from_days`）。
/// 纯函数；用于 `today_str` 把系统时钟天数还原成日期。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { (mp + 3) as u32 } else { (mp - 9) as u32 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// 解析 "yyyy-mm-dd" → (年, 月, 日)；格式非法（缺零、非数字、段数不对、日期越界）→ None。
/// 纯函数；月份天数含闰年校验（2 月 29 仅闰年合法）。
pub fn parse_date(s: &str) -> Option<(i64, u32, u32)> {
    let mut it = s.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let max = match m {
        2 => {
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            if leap { 29 } else { 28 }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if d > max {
        return None;
    }
    Some((y, m, d))
}

/// `last` 是否是 `today` 的昨天（二者均为合法日期时；任一非法 → false）。
/// 跨月/跨年由 days_from_civil 序列号天然处理：2-28→3-01（平年）与 12-31→1-01 相邻。
pub fn is_yesterday(last: &str, today: &str) -> bool {
    match (parse_date(last), parse_date(today)) {
        (Some((y1, m1, d1)), Some((y2, m2, d2))) => {
            days_from_civil(y1, m1, d1) == days_from_civil(y2, m2, d2) - 1
        }
        _ => false,
    }
}

/// 今天日期 "yyyy-mm-dd"（系统时钟 UTC，std 实现；无 chrono）。
/// 纯 std：unix 秒数 ÷ 86400 得纪元天数，再经 civil_from_days 还原。
pub fn today_str() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// 给定日期的「昨天」ISO 串 "yyyy-mm-dd"（非法输入 → None）。
/// 纯函数：civil-days 序列号 −1 再逆变换，天然处理跨月/跨年。
pub fn previous_day(date: &str) -> Option<String> {
    let (y, m, d) = parse_date(date)?;
    let (y2, m2, d2) = civil_from_days(days_from_civil(y, m, d) - 1);
    Some(format!("{y2:04}-{m2:02}-{d2:02}"))
}

/// 活跃一次（通关 / 查看 hint / 复习回血）后的 streak 更新（纯函数，v3 §7.4）：///
/// - `last == 昨天` → +1（跨月/跨年不断链）；
/// - `last == 今天` → 幂等不变（同日多次活跃只算一天；计数与日期失配的 0 修复为 1）；
/// - 更早 / 首次（None）→ 重置 1。
///
/// 返回 `(新 streak_days, 新 last_played_date = today)`。
pub fn touch_streak(streak_days: u32, last: Option<&str>, today: &str) -> (u32, String) {
    let streak = match last {
        Some(last) if is_yesterday(last, today) => streak_days + 1,
        Some(last) if last == today => streak_days.max(1),
        _ => 1,
    };
    (streak, today.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_from_civil_epoch_is_zero() {
        // 1970-01-01 是纪元日（Hinnant 公式基准）
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
    }

    #[test]
    fn feb_non_leap_28_to_mar1_consecutive() {
        // 平年 2 月只有 28 天：2-28 → 3-01 相邻（rust-quest 旧 y*372+m*31+d 公式在此断链）
        assert_eq!(days_from_civil(2025, 3, 1) - days_from_civil(2025, 2, 28), 1);
        assert!(is_yesterday("2025-02-28", "2025-03-01"));
        // 连续链：2-27 → 2-28 → 3-01 每一步 +1
        assert!(is_yesterday("2025-02-27", "2025-02-28"));
    }

    #[test]
    fn leap_year_feb29_bridges_mar1() {
        // 闰年 2-29 存在：2-28 → 3-01 相隔两天，2-29 → 3-01 相邻
        assert_eq!(days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 29), 1);
        assert!(is_yesterday("2024-02-29", "2024-03-01"));
        assert!(!is_yesterday("2024-02-28", "2024-03-01"));
    }

    #[test]
    fn year_boundary_dec31_to_jan1_consecutive() {
        // 跨年：12-31 → 1-01 相邻，链不断
        assert_eq!(days_from_civil(2026, 1, 1) - days_from_civil(2025, 12, 31), 1);
        assert!(is_yesterday("2025-12-31", "2026-01-01"));
    }

    #[test]
    fn civil_roundtrip_days_and_date() {
        // days_from_civil ∘ civil_from_days = id（双向），锁定逆变换正确性
        for (y, m, d) in [
            (1970, 1, 1),
            (1999, 12, 31),
            (2000, 2, 29),
            (2024, 2, 29),
            (2025, 2, 28),
            (2025, 3, 1),
            (2025, 12, 31),
            (2026, 1, 1),
            (2026, 8, 16),
            (2099, 7, 4),
        ] {
            assert_eq!(civil_from_days(days_from_civil(y, m, d)), (y, m, d));
        }
        // 逆方向：任取若干天数序列号，正反一致
        for z in [-719_468_i64, -1, 0, 1, 19_000, 20_000, 21_000, 99_999] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z, "z={z}");
        }
    }

    #[test]
    fn same_day_idempotent() {
        // 同日多次活跃只 +0（幂等）；计数与日期失配（0）修复为 1
        let (s, date) = touch_streak(4, Some("2026-08-16"), "2026-08-16");
        assert_eq!(s, 4);
        assert_eq!(date, "2026-08-16");
        let (s, _) = touch_streak(0, Some("2026-08-16"), "2026-08-16");
        assert_eq!(s, 1);
    }

    #[test]
    fn yesterday_increments_across_month_and_year() {
        // 昨天活跃 → +1；2-28→3-01（平年）与 12-31→1-01 链不断
        let (s, date) = touch_streak(3, Some("2025-02-28"), "2025-03-01");
        assert_eq!(s, 4);
        assert_eq!(date, "2025-03-01");
        let (s, date) = touch_streak(5, Some("2025-12-31"), "2026-01-01");
        assert_eq!(s, 6);
        assert_eq!(date, "2026-01-01");
        // 闰年：2-29 → 3-01 也 +1
        let (s, _) = touch_streak(2, Some("2024-02-29"), "2024-03-01");
        assert_eq!(s, 3);
    }

    #[test]
    fn two_day_gap_resets_to_one() {
        // 断档（隔 2 天）→ 重置 1
        let (s, date) = touch_streak(7, Some("2026-08-14"), "2026-08-16");
        assert_eq!(s, 1);
        assert_eq!(date, "2026-08-16");
    }

    #[test]
    fn first_activity_sets_one() {
        // 首次活跃（无 last_played_date）→ 1
        let (s, date) = touch_streak(0, None, "2026-08-16");
        assert_eq!(s, 1);
        assert_eq!(date, "2026-08-16");
    }

    #[test]
    fn invalid_dates_never_adjacent() {
        // 非法日期（越界/格式错）→ is_yesterday false（不 panic、不断错链）
        assert!(!is_yesterday("2026-02-30", "2026-03-01"));
        assert!(!is_yesterday("2025-02-29", "2025-03-01")); // 平年无 2-29
        assert!(!is_yesterday("2026-13-01", "2026-03-01"));
        assert!(!is_yesterday("2026-8-16", "2026-08-16")); // 缺零格式
        assert!(!is_yesterday("not-a-date", "2026-08-16"));
        assert_eq!(parse_date("2026-02-30"), None);
        assert_eq!(parse_date("2024-02-30"), None);
        assert_eq!(parse_date("2024-02-29"), Some((2024, 2, 29)));
    }

    #[test]
    fn previous_day_crosses_month_and_year() {
        assert_eq!(previous_day("2026-08-16").as_deref(), Some("2026-08-15"));
        assert_eq!(previous_day("2025-03-01").as_deref(), Some("2025-02-28"), "平年 3-01 前一天是 2-28");
        assert_eq!(previous_day("2024-03-01").as_deref(), Some("2024-02-29"), "闰年 3-01 前一天是 2-29");
        assert_eq!(previous_day("2026-01-01").as_deref(), Some("2025-12-31"), "跨年");
        assert_eq!(previous_day("bad-date"), None);
    }

    #[test]
    fn today_str_is_valid_date() {
        // 系统时钟实现的 today_str 必须可解析且合法（算法自洽冒烟）
        let t = today_str();
        assert_eq!(t.len(), 10);
        assert!(parse_date(&t).is_some(), "today_str 输出非法日期: {t}");
    }
}
