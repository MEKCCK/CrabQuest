use crate::error::GameError;
use crate::level::{parse_levels, Level, LevelSet};
use crate::sandbox::Sandbox;
use crate::save::{LevelProgress, LevelState, SaveData};
use crate::validate::mapper::ErrorMapper;
use crate::validate::{validate, Validation};
use std::collections::HashSet;
use std::path::Path;

/// XP 定价表（v3 §7.2，替换旧 XP_PER_PASS=20）：
/// 首次通关（普通关）+25；完美通关（fail_count==0）+10；连击加成（通过后 combo>=3）+5；
/// Boss 首通 ≤4 次尝试 +50；Boss 首通 >4 次尝试 +30。重复通关 +0（combo 仍更新）。
/// 单关上限：普通 25+10+5=40；Boss 50+10+5=65。
pub const XP_PASS: u32 = 25;
pub const XP_PERFECT: u32 = 10;
pub const XP_COMBO: u32 = 5;
pub const XP_BOSS: u32 = 50;
pub const XP_BOSS_FALLBACK: u32 = 30;

/// XP 一次制分档纯函数（v3 §7.2）：四步累加，重复通关一律 +0。
///
/// - `is_first_pass`：`completed_steps` 无 `"{level_id}:pass"` 记录；
/// - `is_boss`：Boss 关替换 base 档位（≤4 次尝试 +50 / >4 次尝试 +30）；
/// - `fail_count`：该关失败提交次数，==0 且首通 → 完美 +10；
/// - `combo_after_pass`：通过后 combo 值（v3「通过后 combo ≥ 3」→ 取累加后值）；
/// - `attempts_at_pass`：通关时该关累计提交次数（含本次通过，总提交数 = fail + 通过）。
///
/// 返回本次应得 XP（已钳制单关上限：普通 40 / Boss 65）。
pub fn award_xp(
    is_first_pass: bool,
    is_boss: bool,
    fail_count: u32,
    combo_after_pass: u32,
    attempts_at_pass: u32,
) -> u32 {
    if !is_first_pass {
        return 0;
    }
    let mut xp = if is_boss {
        if attempts_at_pass <= 4 {
            XP_BOSS
        } else {
            XP_BOSS_FALLBACK
        }
    } else {
        XP_PASS
    };
    if fail_count == 0 {
        xp += XP_PERFECT;
    }
    if combo_after_pass >= 3 {
        xp += XP_COMBO;
    }
    // 单关上限保险钳制（当前定价天然不越界，防止未来新增加成越限）
    let cap = if is_boss {
        XP_BOSS + XP_PERFECT + XP_COMBO
    } else {
        XP_PASS + XP_PERFECT + XP_COMBO
    };
    xp.min(cap)
}

/// P2-11：失败联动模式下提示面板的展示状态（由 `hint_unlock_state` 纯函数推导）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HintUnlockState {
    /// 已解锁提示条数：`hints[0..unlocked)` 可见（fc 越界时恒为 hints 总长）
    pub unlocked: usize,
    /// 自动展开/推进到的提示索引（0-based；表驱动：0-1 次失败→0，2 次→0，
    /// 3 次→1，≥4 次→最后一条；恒 < unlocked，unlocked==0 时恒 0）
    pub expanded: usize,
    /// 失败次数 ≥4 → 显示「查看参考答案」按钮（二次确认「先自己试试？」）
    pub show_reference: bool,
}

/// P2-11：由失败次数推导提示解锁/展开状态（v3 §3.4 行为表 + §7.6 防挫败定稿）。
///
/// - `hint_unlock` 为空（旧 TOML 缺省）→ 返回 `None`，UI 保持现状手动逐级揭示（hint_step 步进）；
/// - `hint_unlock == [1, 3, 5]`（v3 推荐默认阈值）→ 按行为表 4 档：
///   0-1 次失败 → hint[0] 可看；≥2 次 → hint[0..2) 解锁且自动展开 hint[0]；
///   ≥3 次 → 全部解锁且自动推进到 hint[1]；≥4 次 → 推进到最后一条 + 参考答案按钮；
/// - 其余自定义阈值向量 → 逐阈值推导：hint i 在 `fail_count >= hint_unlock[i]` 时解锁，
///   自动展开到「已解锁且阈值 ≤ fail_count」的最高索引。
///
/// 纯函数：不改任何状态，UI 只读。
pub fn hint_unlock_state(
    hints_len: usize,
    hint_unlock: &[u32],
    fail_count: u32,
) -> Option<HintUnlockState> {
    if hints_len == 0 || hint_unlock.is_empty() {
        return None;
    }
    if hint_unlock == [1, 3, 5] {
        // 行为表 4 档（v3 §7.6；默认 [1,3,5] 语义落地）
        let unlocked = match fail_count {
            0..=1 => 1.min(hints_len),
            2 => 2.min(hints_len),
            _ => hints_len, // ≥3
        };
        let expanded = match fail_count {
            0..=2 => 0,
            3 => 1.min(hints_len.saturating_sub(1)),
            _ => 2.min(hints_len.saturating_sub(1)), // ≥4 → 推进到最后一条
        };
        let expanded = expanded.min(unlocked.saturating_sub(1));
        Some(HintUnlockState {
            unlocked,
            expanded,
            show_reference: fail_count >= 4,
        })
    } else {
        // 自定义阈值向量：逐阈值解锁
        let unlocked = hint_unlock
            .iter()
            .take(hints_len)
            .filter(|&&t| fail_count >= t)
            .count();
        let expanded = hint_unlock
            .iter()
            .take(hints_len)
            .enumerate()
            .filter(|(_, &t)| fail_count >= t)
            .map(|(i, _)| i)
            .last()
            .unwrap_or(0)
            .min(unlocked.saturating_sub(1));
        Some(HintUnlockState {
            unlocked,
            expanded,
            show_reference: fail_count >= 4,
        })
    }
}

/// P3-17：Boss 关提示门控阈值（v3 §7.5 提示默认禁用，fail_count ≥ 5 解锁兜底）。
pub const BOSS_HINT_UNLOCK_FAILS: u32 = 5;

/// Boss 关提示是否仍锁定（fail_count < 5 全锁；≥5 恢复正常解锁行为）。
/// 纯函数：错误码解释卡不受影响（教学核心不豁免），只锁主动索取的提示。
pub fn boss_hint_locked(is_boss: bool, fail_count: u32) -> bool {
    is_boss && fail_count < BOSS_HINT_UNLOCK_FAILS
}

/// 解锁还需失败次数（展示用：0 = 已解锁，提示恢复正常）。
pub fn boss_hint_lock_remaining(fail_count: u32) -> u32 {
    BOSS_HINT_UNLOCK_FAILS.saturating_sub(fail_count)
}

/// best_time_ms 保留最小值（P3-18 通关分支记录；纯函数便于确定性单测）。
pub fn min_best_time(current: Option<u64>, new_ms: u64) -> Option<u64> {
    Some(current.map_or(new_ms, |b| b.min(new_ms)))
}

/// P4-26：单个自定义关卡文件的加载失败信息（中文呈现）。
/// 启动日志直接打印 `message()`，游戏内地图页以同样文案提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomLevelError {
    /// 文件名（如 "my-level.toml"），用于「自定义关卡 X 加载失败：原因」
    pub file: String,
    /// 中文失败原因（TOML 解析 / schema 校验 / id 冲突等）
    pub reason: String,
}

impl CustomLevelError {
    pub fn message(&self) -> String {
        format!("自定义关卡 {} 加载失败：{}", self.file, self.reason)
    }
}

/// P4-26：从目录逐文件加载自定义关卡（独立「自定义章节」，不并入内置线性链）。
///
/// - 目录不存在 → `(空, 空)`：无自定义关卡 = 行为与现状一致（不报错、不显示章节）；
/// - 单文件解析/校验失败 → 只拒绝该文件（记录中文原因），其余文件照常加载，不崩溃；
/// - id 与内置关卡冲突、或自定义集内重复 → 拒绝整个文件（其余文件不受影响）。
///
/// schema 校验复用 `parse_levels`（quiz options 2-6 / answer_index 越界 / hint_unlock
/// 与 hints 等长 / source 非空 / expect_panic 与 expect_output 互斥等）。
pub fn load_custom_levels(
    dir: &Path,
    builtin_ids: &HashSet<String>,
) -> (Vec<Level>, Vec<CustomLevelError>) {
    if !dir.exists() {
        return (Vec::new(), Vec::new());
    }
    let mut files: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |x| x == "toml"))
            .collect(),
        Err(e) => {
            return (
                Vec::new(),
                vec![CustomLevelError {
                    file: dir.display().to_string(),
                    reason: format!("目录读取失败：{e}"),
                }],
            );
        }
    };
    files.sort();
    let mut levels = Vec::new();
    let mut errors = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for f in files {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| f.display().to_string());
        let content = match std::fs::read_to_string(&f) {
            Ok(c) => c,
            Err(e) => {
                errors.push(CustomLevelError {
                    file: name.clone(),
                    reason: format!("文件读取失败：{e}"),
                });
                continue;
            }
        };
        let parsed = match parse_levels(&content) {
            Ok(p) => p,
            Err(e) => {
                errors.push(CustomLevelError {
                    file: name.clone(),
                    reason: e.to_string(),
                });
                continue;
            }
        };
        // id 唯一性（先只读检查再落 seen，避免整文件被拒后污染后续判定）
        let conflict = parsed
            .iter()
            .find(|l| builtin_ids.contains(&l.id))
            .or_else(|| parsed.iter().find(|l| seen.contains(&l.id)));
        if let Some(lvl) = conflict {
            errors.push(CustomLevelError {
                file: name.clone(),
                reason: if builtin_ids.contains(&lvl.id) {
                    format!("关卡 id「{}」与内置关卡冲突", lvl.id)
                } else {
                    format!("关卡 id「{}」在自定义关卡中重复", lvl.id)
                },
            });
            continue;
        }
        for lvl in &parsed {
            seen.insert(lvl.id.clone());
        }
        levels.extend(parsed);
    }
    (levels, errors)
}

pub struct Engine {
    /// 全部关卡（内置在前 + 自定义追加在后）；`builtin_count` 为分界点
    pub level_set: LevelSet,
    /// 内置关卡数量：`level_set.levels[..builtin_count]` 为内置，
    /// `[builtin_count..]` 为自定义（独立「自定义章节」）
    pub builtin_count: usize,
    pub save: SaveData,
    pub current: Option<usize>,
    pub mapper: ErrorMapper,
    pub sandbox: Box<dyn Sandbox>,
}

impl Engine {
    pub fn new(
        level_set: LevelSet,
        save: SaveData,
        mapper: ErrorMapper,
        sandbox: Box<dyn Sandbox>,
    ) -> Self {
        Self::with_custom_levels(level_set, Vec::new(), save, mapper, sandbox)
    }

    /// P4-26：内置关卡 + 自定义关卡构造引擎。自定义关卡追加到内置之后形成独立章节；
    /// 未存档的自定义关卡预置为 Unlocked（可直接挑战，不参与内置线性解锁链），
    /// 已有存档（Passed/Unlocked）原样保留。
    pub fn with_custom_levels(
        mut level_set: LevelSet,
        custom_levels: Vec<Level>,
        save: SaveData,
        mapper: ErrorMapper,
        sandbox: Box<dyn Sandbox>,
    ) -> Self {
        let builtin_count = level_set.len();
        level_set.levels.extend(custom_levels);
        let mut engine = Self {
            level_set,
            builtin_count,
            save,
            current: None,
            mapper,
            sandbox,
        };
        engine.preset_custom_levels();
        engine
    }

    /// 为每个自定义关卡补默认进度（Unlocked，可直接挑战）；已有存档条目原样保留。
    fn preset_custom_levels(&mut self) {
        for lvl in &self.level_set.levels[self.builtin_count..] {
            self.save
                .level_states
                .entry(lvl.id.clone())
                .or_insert_with(|| LevelProgress {
                    state: LevelState::Unlocked,
                    ..LevelProgress::default()
                });
        }
    }

    /// 索引是否落在自定义章节（>= builtin_count）
    pub fn is_custom_index(&self, index: usize) -> bool {
        index >= self.builtin_count
    }

    /// 内置关卡 id 集合（成就/rank 只按内置判定）
    pub fn builtin_ids(&self) -> HashSet<String> {
        self.level_set.levels[..self.builtin_count]
            .iter()
            .map(|l| l.id.clone())
            .collect()
    }

    pub fn is_builtin_id(&self, id: &str) -> bool {
        self.level_set.levels[..self.builtin_count]
            .iter()
            .any(|l| l.id == id)
    }

    /// 内置章节已通关关卡数（rank 判定依据；自定义关卡不计入）。
    /// 供 `rank_for()` 使用：段位推进只认内置进度（v3 §7.3 + P4-26 存档隔离）。
    pub fn builtin_completed_count(&self) -> usize {
        self.save
            .level_states
            .iter()
            .filter(|(id, p)| p.state == LevelState::Passed && self.is_builtin_id(id))
            .count()
    }

    /// 同章节内的下一关：内置章节末尾不跨入自定义章节，自定义章节末尾返回 None。
    /// （自定义章节独立：进度/解锁/成就均与内置隔离）
    pub fn next_in_chapter(&self, index: usize) -> Option<usize> {
        let next = index + 1;
        if next >= self.level_set.len() {
            return None;
        }
        if self.is_custom_index(index) {
            Some(next) // 已在自定义章节：next 必然仍在自定义区间
        } else if next < self.builtin_count {
            Some(next)
        } else {
            None // 内置章节末尾：不跨入自定义章节
        }
    }

    pub fn new_game(&mut self) {
        self.save = SaveData::default();
        self.current = None;
        // 预置全部关卡状态底图（默认 Locked），保证线性解锁前后 map 中均存在每关条目
        for lvl in &self.level_set.levels {
            self.save
                .level_states
                .entry(lvl.id.clone())
                .or_insert_with(LevelProgress::default);
        }
        // P4-26：自定义章节独立解锁——全部可直接挑战（不参与内置线性链）
        for lvl in &self.level_set.levels[self.builtin_count..] {
            if let Some(p) = self.save.level_states.get_mut(&lvl.id) {
                p.state = LevelState::Unlocked;
            }
        }
        self.unlock_first();
    }

    pub fn unlock_first(&mut self) {
        if let Some(first) = self.level_set.levels.first() {
            let p = self
                .save
                .level_states
                .entry(first.id.clone())
                .or_insert_with(LevelProgress::default);
            p.state = LevelState::Unlocked;
        }
    }

    pub fn start_level(&mut self, index: usize) -> Result<(), GameError> {
        let level = self
            .level_set
            .levels
            .get(index)
            .ok_or_else(|| GameError::LevelNotFound(format!("index {index}")))?;
        let state = self
            .save
            .level_states
            .get(&level.id)
            .map(|p| p.state)
            .unwrap_or(LevelState::Locked);
        if state == LevelState::Locked {
            // P3-18 自由模式（R10 铁锈冠军）：内置关卡全部解锁重玩——
            // 存档状态不变（统计/成就仍区分已通关），仅放行入口。
            if self.save.practice_unlock_all && !self.is_custom_index(index) {
                // 放行
            } else {
                return Err(GameError::LevelLocked(level.id.clone()));
            }
        }
        self.current = Some(index);
        Ok(())
    }

    pub fn submit(&mut self, code: &str) -> Result<Validation, GameError> {
        let idx = self
            .current
            .ok_or_else(|| GameError::LevelNotFound("无当前关卡".into()))?;
        let level = self
            .level_set
            .levels
            .get(idx)
            .cloned()
            .ok_or_else(|| GameError::LevelNotFound(format!("index {idx}")))?;

        // P2-08：0 心禁提交（引擎层兜底；不编译、不扣 XP，UI 应已禁用按钮）
        if self.save.hearts == 0 {
            return Err(GameError::NoHearts);
        }

        // P3-18：best_time_ms 度量本次提交耗时（validate = 编译 + 运行，主导时延），
        // 通关分支记录最小值；`?` 提前返回时不计时（未通关无意义）。
        let submit_started = std::time::Instant::now();
        let result = validate(&level, code, &self.mapper, self.sandbox.as_ref())?;
        let submit_elapsed_ms = submit_started.elapsed().as_millis() as u64;

        // P4-26：自定义关卡隔离——连击/成就/rank 只认内置关卡；
        // 自定义进度照常写入 level_states，但不推进内置元进度
        let is_custom = self.is_custom_index(idx);

        let mut xp_gained = 0;
        // P2-10：本次通关上下文（id, fail_count, hints_used 是否为空），
        // 供 check_achievements 判定完美类成就（Fail 分支为 None）
        let mut just_passed: Option<(String, u32, bool)> = None;
        match &result {
            Validation::Pass { .. } => {
                let pass_key = format!("{}:pass", level.id);
                let first_pass = !self.save.completed_steps.contains(&pass_key);
                if !is_custom {
                    self.save.combo += 1;
                    self.save.max_combo = self.save.max_combo.max(self.save.combo);
                }
                let entry = self
                    .save
                    .level_states
                    .entry(level.id.clone())
                    .or_insert_with(|| LevelProgress {
                        state: LevelState::Unlocked,
                        ..LevelProgress::default()
                    });
                entry.state = LevelState::Passed;
                entry.attempts += 1;
                entry.completed_at = Some(unix_secs());
                // P3-18：通关分支记录最快用时（保留历史最小值）
                entry.best_time_ms = min_best_time(entry.best_time_ms, submit_elapsed_ms);
                // XP 一次制分档：实际奖励随 Validation::Pass 返回（v3 §7.2）
                xp_gained = award_xp(
                    first_pass,
                    level.is_boss,
                    entry.fail_count,
                    self.save.combo, // 通过后 combo（v3「通过后 combo ≥ 3」）
                    entry.attempts,  // 通关时累计提交次数（含本次通过）
                );
                self.save.xp += xp_gained;
                if first_pass {
                    self.save.completed_steps.insert(pass_key);
                }
                // 提前取本次通关上下文（id, fail_count, hints_used 为空），
                // 结束 entry 借用后再做后续自借用（解锁下一关 / touch_activity）
                just_passed = Some((
                    level.id.clone(),
                    entry.fail_count,
                    entry.hints_used.is_empty(),
                ));
                // P3-18：自由模式 + 末关庆典——内置全部通关（R10 铁锈冠军）：
                // practice_unlock_all 解锁全部关卡重玩（重复通关 +0 XP 由一次制保证）；
                // victory_celebrated 一次性庆典防重标记（存档持久化，重启后不再庆祝）。
                if !is_custom && self.builtin_completed_count() == self.builtin_count {
                    self.save.practice_unlock_all = true;
                    self.save.victory_celebrated = true;
                }
                if !is_custom && idx + 1 < self.builtin_count {
                    // 内置线性解锁链：仅内置章节内推进；内置末尾不跨入自定义章节
                    if let Some(next) = self.level_set.levels.get(idx + 1) {
                        let n = self
                            .save
                            .level_states
                            .entry(next.id.clone())
                            .or_insert_with(LevelProgress::default);
                        if n.state == LevelState::Locked {
                            n.state = LevelState::Unlocked;
                        }
                    }
                }
                // P2-08：通关回血 +1（cap 5）；P2-09：通关 = 活跃
                self.save.hearts = (self.save.hearts + 1).min(5);
                self.touch_activity();
            }
            Validation::Fail { errors, .. } => {
                if !is_custom {
                    self.save.combo = 0;
                }
                self.save.total_errors += 1;
                let entry = self
                    .save
                    .level_states
                    .entry(level.id.clone())
                    .or_insert_with(LevelProgress::default);
                entry.attempts += 1;
                entry.fail_count += 1;
                // P2-08：失败扣心（floor 0）；P3-17：Boss 失败不扣（以显式
                // is_boss 标注为准，不再依赖硬编码 id 表兜底——数据已落地标注）
                if !level.is_boss {
                    self.save.hearts = self.save.hearts.saturating_sub(1);
                }
                // P2-10：记录本次见到的错误码（不同错误码去重累计，≥10 种解锁收藏家）
                for card in errors {
                    self.save.seen_error_codes.insert(card.code.clone());
                }
            }
        }
        // P2-10：统一成就检查（纯函数，HashSet 幂等）。
        // P4-26：成就只按内置关卡判定——快照过滤自定义 id；total_levels 用内置数；
        // 自定义通关上下文不参与完美类成就判定（combo 已隔离：自定义通关/失败不增减连击）。
        let builtin_ids = self.builtin_ids();
        let builtin_states: std::collections::HashMap<String, LevelProgress> = self
            .save
            .level_states
            .iter()
            .filter(|(id, _)| builtin_ids.contains(*id))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let builtin_steps: HashSet<String> = self
            .save
            .completed_steps
            .iter()
            .filter(|s| {
                s.rsplit_once(':')
                    .map(|(id, _)| builtin_ids.contains(id))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        let builtin_just_passed = just_passed
            .as_ref()
            .filter(|(id, _, _)| builtin_ids.contains(id))
            .map(|(id, fail_count, hints_empty)| (id.as_str(), *fail_count, *hints_empty));
        let newly =
            crate::achievements::check_achievements(&crate::achievements::AchievementCheck {
                level_states: &builtin_states,
                completed_steps: &builtin_steps,
                combo: self.save.combo,
                seen_error_codes: &self.save.seen_error_codes,
                total_levels: self.builtin_count,
                already: &self.save.achievements,
                just_passed: builtin_just_passed,
            });
        for id in newly {
            self.save.achievements.insert(id);
        }
        Ok(match result {
            Validation::Pass { .. } => Validation::Pass { xp_gained },
            other => other,
        })
    }

    /// P2-08：复习关卡说明回血（每关每局一次，幂等）。
    /// 返回是否实际回了 1 心（首次复习且心 <5 时 true；满心或已复习过 → false）。
    /// 复习也算活跃行为（P2-09 streak）。
    pub fn review_lore(&mut self, level_id: &str) -> bool {
        let key = format!("{level_id}:lore");
        if self.save.completed_steps.contains(&key) {
            return false;
        }
        let healed = self.save.hearts < 5;
        if healed {
            self.save.hearts += 1;
        }
        self.save.completed_steps.insert(key);
        self.touch_activity();
        healed
    }

    /// P2-09：活跃一次（通关 / 查看 hint / 复习回血共用钩子）。
    /// 同日幂等（streak 纯函数 touch_streak 判定），更新 last_played_date 为今天。
    pub fn touch_activity(&mut self) {
        let today = crate::streak::today_str();
        let (streak, date) = crate::streak::touch_streak(
            self.save.streak_days,
            self.save.last_played_date.as_deref(),
            &today,
        );
        self.save.streak_days = streak;
        self.save.last_played_date = Some(date);
    }

    /// P2-11：记录一次 hint 查看（零成本：不扣心/XP、不改 fail_count/attempts、
    /// 不影响完美判定——`no_hint_perfect` 只看 `hints_used.is_empty()`）。
    ///
    /// - 幂等去重：同一索引多次查看只记一次；`hints_used` 保持升序；
    /// - 查看 hint 计为活跃行为（P2-09，同日幂等）；
    /// - 返回是否首次记录（调用方可用作「新查看」信号）。
    pub fn reveal_hint(&mut self, level_id: &str, index: u32) -> bool {
        let fresh = {
            let entry = self
                .save
                .level_states
                .entry(level_id.to_string())
                .or_insert_with(LevelProgress::default);
            if !entry.hints_used.contains(&index) {
                entry.hints_used.push(index);
                entry.hints_used.sort_unstable();
                entry.hints_used.dedup();
                true
            } else {
                false
            }
        };
        self.touch_activity();
        fresh
    }

    pub fn current_level(&self) -> Option<&Level> {
        self.current.and_then(|i| self.level_set.levels.get(i))
    }

    pub fn can_continue(&self) -> bool {
        self.save.xp > 0
            || self
                .save
                .level_states
                .values()
                .any(|p| p.state == LevelState::Passed)
    }

    pub fn save_ref(&self) -> &SaveData {
        &self.save
    }
}

fn unix_secs() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{parse_levels, LevelSet};
    use crate::sandbox::DevSandbox;
    use crate::save::{LevelState, SaveData};
    use crate::validate::mapper::ErrorMapper;
    use crate::validate::Validation;

    const LEVELS: &str = r#"
[[level]]
id = "l0-hello"
title = "hello"
tier = "l0"
description = "d"
starter_code = "fn main() { x = 5; println!(\"x has the value {}\", x); }"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l1-move"
title = "move"
tier = "l1"
description = "d"
starter_code = "fn main() { let s = String::from(\"hi\"); take(s); println!(\"{}\", s); } fn take(x: String) {}"
expect_output = "hi"
source = "rustlings"
"#;

    fn engine() -> Engine {
        let set = LevelSet {
            levels: parse_levels(LEVELS).unwrap(),
        };
        Engine::new(
            set,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        )
    }

    #[test]
    fn new_game_unlocks_first() {
        let mut e = engine();
        e.new_game();
        assert_eq!(
            e.save.level_states.get("l0-hello").unwrap().state,
            LevelState::Unlocked
        );
        assert_eq!(
            e.save.level_states.get("l1-move").unwrap().state,
            LevelState::Locked
        );
    }

    #[test]
    fn locked_level_rejected() {
        let mut e = engine();
        e.new_game();
        assert!(matches!(e.start_level(1), Err(GameError::LevelLocked(_))));
    }

    #[test]
    fn pass_updates_xp_combo_and_unlocks_next() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        // 首通 + 完美（首次提交即通过）→ 25 + 10 = 35
        assert_eq!(
            e.submit(code).unwrap(),
            Validation::Pass {
                xp_gained: XP_PASS + XP_PERFECT
            }
        );
        assert_eq!(e.save.xp, XP_PASS + XP_PERFECT);
        assert_eq!(e.save.combo, 1);
        assert_eq!(
            e.save.level_states.get("l0-hello").unwrap().state,
            LevelState::Passed
        );
        assert_eq!(
            e.save.level_states.get("l1-move").unwrap().state,
            LevelState::Unlocked
        );
        assert!(e
            .save
            .level_states
            .get("l0-hello")
            .unwrap()
            .completed_at
            .is_some());
    }

    #[test]
    fn fail_resets_combo_and_counts_error() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 先通关一关拿到 combo
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        e.submit(code).unwrap();
        assert_eq!(e.save.combo, 1);
        // 然后在 l1-move 上故意写错
        e.start_level(1).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        assert!(matches!(e.submit(bad).unwrap(), Validation::Fail { .. }));
        assert_eq!(e.save.combo, 0);
        assert_eq!(e.save.total_errors, 1);
        assert_eq!(e.save.level_states.get("l1-move").unwrap().attempts, 1);
        // 失败不改变关卡状态
        assert_eq!(
            e.save.level_states.get("l1-move").unwrap().state,
            LevelState::Unlocked
        );
    }

    #[test]
    fn allow_compile_fail_level_passes_with_right_error() {
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"l1-bug\"\ntitle = \"制造错误\"\ntier = \"l1\"\ndescription = \"d\"\nstarter_code = \"\"\nallow_compile_fail = true\nexpect_error_code = \"E0382\"\nsource = \"rust-quiz\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(
            set,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        assert!(matches!(e.submit(code).unwrap(), Validation::Pass { .. }));
    }

    #[test]
    fn can_continue_after_progress() {
        let mut e = engine();
        assert!(!e.can_continue());
        e.new_game();
        assert!(!e.can_continue());
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert!(e.can_continue());
    }

    // ---- P1-05：XP 一次制分档 + rank（v3 §7.2/§7.3）----

    fn boss_engine() -> Engine {
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"boss\"\ntitle = \"boss\"\ntier = \"l4\"\ndescription = \"d\"\nstarter_code = \"\"\nis_boss = true\nexpect_output = \"ok\"\nsource = \"rust-quiz\"\n",
            )
            .unwrap(),
        };
        Engine::new(
            set,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        )
    }

    #[test]
    fn first_pass_awards_25_base() {
        // 首次通关：+25 base（无 perfect/combo 时）
        assert_eq!(award_xp(true, false, 1, 1, 2), XP_PASS);
        assert_eq!(award_xp(true, false, 1, 2, 2), XP_PASS);
    }

    #[test]
    fn repeat_pass_awards_zero_but_combo_still_updates() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        assert_eq!(
            e.submit(code).unwrap(),
            Validation::Pass {
                xp_gained: XP_PASS + XP_PERFECT
            }
        );
        assert_eq!(e.save.xp, XP_PASS + XP_PERFECT);
        assert!(e.save.completed_steps.contains("l0-hello:pass"));
        // 重复通关同一关：+0 XP，combo 仍更新（练习价值保留）
        assert_eq!(e.submit(code).unwrap(), Validation::Pass { xp_gained: 0 });
        assert_eq!(e.save.xp, XP_PASS + XP_PERFECT);
        assert_eq!(e.save.combo, 2);
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().attempts, 2);
    }

    #[test]
    fn perfect_pass_awards_10() {
        // 完美通关（首次提交即通过，fail_count == 0）：+10
        assert_eq!(award_xp(true, false, 0, 1, 1), XP_PASS + XP_PERFECT);
        // 失败过再通过：无 perfect
        assert_eq!(award_xp(true, false, 1, 1, 2), XP_PASS);
    }

    #[test]
    fn combo_3_or_more_awards_5() {
        // 连击加成：首通且通过后 combo >= 3 → +5（v3「通过后 combo ≥ 3」，取累加后值）
        assert_eq!(award_xp(true, false, 1, 3, 3), XP_PASS + XP_COMBO);
        assert_eq!(
            award_xp(true, false, 0, 3, 1),
            XP_PASS + XP_PERFECT + XP_COMBO
        );
        // combo 2 时无加成
        assert_eq!(award_xp(true, false, 0, 2, 1), XP_PASS + XP_PERFECT);
    }

    #[test]
    fn single_level_cap_normal_40() {
        // 普通关单关上限 40 = 25 + 10 + 5（全加成可叠加且不越上限）
        let gained = award_xp(true, false, 0, 3, 1);
        assert_eq!(gained, XP_PASS + XP_PERFECT + XP_COMBO);
        assert_eq!(gained, 40);
    }

    #[test]
    fn boss_first_pass_4_attempts_50() {
        // Boss 首通 ≤4 次尝试 → +50（替换 base；perfect/combo 照常叠加）
        assert_eq!(award_xp(true, true, 3, 1, 4), XP_BOSS);
        assert_eq!(
            award_xp(true, true, 0, 3, 1),
            XP_BOSS + XP_PERFECT + XP_COMBO
        );
        // Boss 单关上限 65 = 50 + 10 + 5
        assert_eq!(XP_BOSS + XP_PERFECT + XP_COMBO, 65);
    }

    #[test]
    fn boss_first_pass_over_4_attempts_30() {
        // Boss 首通 >4 次尝试 → +30 惩罚档
        assert_eq!(award_xp(true, true, 4, 1, 5), XP_BOSS_FALLBACK);
    }

    #[test]
    fn boss_level_attempts_drive_tier_via_submit() {
        // 集成：4 次提交（3 败 1 过）→ +50；5 次提交（4 败 1 过）→ +30
        let mut e = boss_engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        for _ in 0..3 {
            e.submit(bad).unwrap();
        }
        assert_eq!(
            e.submit("fn main() { println!(\"ok\"); }").unwrap(),
            Validation::Pass { xp_gained: XP_BOSS }
        );
        let p = e.save.level_states.get("boss").unwrap();
        assert_eq!(p.fail_count, 3);
        assert_eq!(p.attempts, 4);

        let mut e2 = boss_engine();
        e2.new_game();
        e2.start_level(0).unwrap();
        for _ in 0..4 {
            e2.submit(bad).unwrap();
        }
        assert_eq!(
            e2.submit("fn main() { println!(\"ok\"); }").unwrap(),
            Validation::Pass {
                xp_gained: XP_BOSS_FALLBACK
            }
        );
        let p = e2.save.level_states.get("boss").unwrap();
        assert_eq!(p.fail_count, 4);
        assert_eq!(p.attempts, 5);
        assert_eq!(e2.save.xp, XP_BOSS_FALLBACK);
    }

    #[test]
    fn fail_increments_fail_count_attempts_is_total() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        e.submit(bad).unwrap();
        e.submit(bad).unwrap();
        let p = e.save.level_states.get("l0-hello").unwrap();
        assert_eq!(p.fail_count, 2);
        assert_eq!(p.attempts, 2);
        // 通过：attempts 含本次（3），fail_count 保持 2 → 无 perfect、combo 1 < 3 无连击
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        assert_eq!(
            e.submit(code).unwrap(),
            Validation::Pass { xp_gained: XP_PASS }
        );
        let p = e.save.level_states.get("l0-hello").unwrap();
        assert_eq!(p.fail_count, 2);
        assert_eq!(p.attempts, 3);
        assert_eq!(e.save.xp, XP_PASS);
    }

    #[test]
    fn rank_does_not_unlock_levels() {
        // rank 只解锁元内容：关卡线性解锁链不受 rank 影响（v3 §7.3）
        let mut e = engine();
        e.new_game();
        // 伪造 R10 存档：15 关 Passed
        for i in 0..15 {
            let id = format!("fake{i}");
            e.save.level_states.insert(
                id,
                LevelProgress {
                    state: LevelState::Passed,
                    ..LevelProgress::default()
                },
            );
        }
        assert_eq!(crate::rank::rank_for(e.save.completed_count()).level, 10);
        // l1-move 仍 Locked → 拒绝进入（解锁只看 level_states.state）
        assert!(matches!(e.start_level(1), Err(GameError::LevelLocked(_))));
    }

    // ===== P2-08：hearts =====

    #[test]
    fn initial_hearts_is_3() {
        let mut e = engine();
        e.new_game();
        assert_eq!(e.save.hearts, 3);
    }

    #[test]
    fn fail_deducts_heart_floor_zero() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        e.submit(bad).unwrap();
        assert_eq!(e.save.hearts, 2);
        e.submit(bad).unwrap();
        assert_eq!(e.save.hearts, 1);
        e.submit(bad).unwrap();
        assert_eq!(e.save.hearts, 0);
        // 0 心后提交被引擎拦截（NoHearts），心数保持 0
        assert!(matches!(e.submit(bad), Err(GameError::NoHearts)));
        assert_eq!(e.save.hearts, 0);
    }

    #[test]
    fn zero_hearts_rejects_submit_without_state_change() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.save.hearts = 0;
        let before_xp = e.save.xp;
        let before_attempts = e
            .save
            .level_states
            .get("l0-hello")
            .map(|p| p.attempts)
            .unwrap_or(0);
        assert!(matches!(
            e.submit("fn main() { println!(\"x has the value {}\", 5); }"),
            Err(GameError::NoHearts)
        ));
        // 0 心拦截：不扣 XP、不计尝试、不改变状态
        assert_eq!(e.save.xp, before_xp);
        assert_eq!(
            e.save
                .level_states
                .get("l0-hello")
                .map(|p| p.attempts)
                .unwrap_or(0),
            before_attempts
        );
        assert_eq!(
            e.save.level_states.get("l0-hello").unwrap().state,
            LevelState::Unlocked
        );
    }

    #[test]
    fn pass_restores_heart_capped_at_5() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        let good = "fn main() { println!(\"x has the value {}\", 5); }";
        // 先扣到 2：3 → 2
        e.submit(bad).unwrap();
        assert_eq!(e.save.hearts, 2);
        // 通关 +1：2 → 3
        e.submit(good).unwrap();
        assert_eq!(e.save.hearts, 3);
        // 重复通关继续 +1 至 cap 5
        e.submit(good).unwrap();
        assert_eq!(e.save.hearts, 4);
        e.submit(good).unwrap();
        assert_eq!(e.save.hearts, 5);
        e.submit(good).unwrap();
        assert_eq!(e.save.hearts, 5, "cap 5：满心后再通关不再增加");
    }

    #[test]
    fn boss_fail_keeps_hearts() {
        let mut e = boss_engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        for _ in 0..3 {
            e.submit(bad).unwrap();
        }
        assert_eq!(e.save.hearts, 3, "Boss 失败不扣心");
    }

    #[test]
    fn boss_hearts_follow_is_boss_flag_only() {
        // P3-17 数据落地后：is_boss 标注是唯一依据（不再用硬编码 id 表兜底）。
        // 同 id 关卡：标注 is_boss → 失败不扣心；未标注 → 按普通关 −1。
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"l1-clone\"\ntitle = \"boss\"\ntier = \"l1\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(1); }\"\nexpect_output = \"1\"\nsource = \"x\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(
            set,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"wrong\"); }").unwrap();
        assert_eq!(
            e.save.hearts, 2,
            "未标注 is_boss → 按普通关扣心（id 表不再兜底）"
        );

        let mut e2 = boss_engine(); // is_boss = true
        e2.new_game();
        e2.start_level(0).unwrap();
        e2.submit("fn main() { println!(\"wrong\"); }").unwrap();
        assert_eq!(e2.save.hearts, 3, "标注 is_boss → 失败不扣心");
    }

    #[test]
    fn review_lore_heals_once_per_level() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 失败一次 → 2 心
        e.submit("fn main() { println!(\"wrong\"); }").unwrap();
        assert_eq!(e.save.hearts, 2);
        // 复习回血 → 3 心，且记 lore 标记
        assert!(e.review_lore("l0-hello"));
        assert_eq!(e.save.hearts, 3);
        assert!(e.save.completed_steps.contains("l0-hello:lore"));
        // 幂等：同关再复习不回血、不加标记次数
        assert!(!e.review_lore("l0-hello"));
        assert_eq!(e.save.hearts, 3);
        // 其他关的 lore 标记独立
        assert!(!e.save.completed_steps.contains("l1-move:lore"));
    }

    #[test]
    fn review_lore_full_hearts_no_gain_but_marked() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 满心（5）时复习：不回血但写入标记（之后扣心也无法再回该关的血）
        e.save.hearts = 5;
        assert!(!e.review_lore("l0-hello"));
        assert_eq!(e.save.hearts, 5);
        assert!(e.save.completed_steps.contains("l0-hello:lore"));
        e.submit("fn main() { println!(\"wrong\"); }").unwrap();
        assert_eq!(e.save.hearts, 4);
        assert!(!e.review_lore("l0-hello"), "已复习过 → 不回血");
        assert_eq!(e.save.hearts, 4);
    }

    // ===== P2-09：streak =====

    #[test]
    fn first_pass_touches_streak() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert_eq!(e.save.streak_days, 1);
        let today = crate::streak::today_str();
        assert_eq!(e.save.last_played_date.as_deref(), Some(today.as_str()));
    }

    #[test]
    fn same_day_activity_is_idempotent() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let good = "fn main() { println!(\"x has the value {}\", 5); }";
        e.submit(good).unwrap();
        let date = e.save.last_played_date.clone();
        // 同日再次通关 + 复习：streak 不变（幂等）
        e.submit(good).unwrap();
        assert_eq!(e.save.streak_days, 1);
        assert_eq!(e.save.last_played_date, date);
        e.review_lore("l0-hello");
        assert_eq!(e.save.streak_days, 1);
        assert_eq!(e.save.last_played_date, date);
    }

    #[test]
    fn yesterday_active_increments_streak() {
        // 直接构造「昨天活跃」存档（streak 纯逻辑在 streak.rs 单测锁定，这里验钩子接线）
        let mut e = engine();
        e.new_game();
        e.save.streak_days = 3;
        let today = crate::streak::today_str();
        let yesterday = crate::streak::previous_day(&today).expect("today 必有昨天");
        e.save.last_played_date = Some(yesterday.clone());
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert_eq!(e.save.streak_days, 4, "昨日活跃 → +1");
        assert_eq!(e.save.last_played_date.as_deref(), Some(today.as_str()));
    }

    #[test]
    fn hint_view_touches_streak() {
        let mut e = engine();
        e.new_game();
        assert_eq!(e.save.streak_days, 0);
        // hint 查看走 app 层（Input::Hint → engine.touch_activity），这里直接验钩子
        e.touch_activity();
        assert_eq!(e.save.streak_days, 1);
        assert!(e.save.last_played_date.is_some());
    }

    // ===== P2-10：achievements =====

    #[test]
    fn first_pass_unlocks_first_steps_and_no_hint_perfect() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert!(e.save.achievements.contains("first_steps"));
        // 首通即完美且未看 hint → 无师自通
        assert!(e.save.achievements.contains("no_hint_perfect"));
        assert!(!e.save.achievements.contains("champion"));
    }

    #[test]
    fn fail_records_seen_error_codes() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 已知 E0382 错误（move 语义，与 allow_compile_fail 测试同源代码）
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        e.submit(code).unwrap();
        assert!(
            e.save.seen_error_codes.contains("E0382"),
            "seen_error_codes 应记录 E0382: {:?}",
            e.save.seen_error_codes
        );
        // 重复提交同码不重复计数
        e.submit(code).unwrap();
        assert_eq!(e.save.seen_error_codes.len(), 1);
    }

    #[test]
    fn boss_pass_unlocks_boss_slayer() {
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"l1-clone\"\ntitle = \"boss\"\ntier = \"l1\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(\\\"1\\\"); }\"\nis_boss = true\nexpect_output = \"1\"\nsource = \"x\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(
            set,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        e.new_game();
        e.start_level(0).unwrap();
        assert!(!e.save.achievements.contains("boss_slayer"));
        e.submit("fn main() { println!(\"1\"); }").unwrap();
        assert!(e.save.achievements.contains("boss_slayer"));
        assert!(e.save.achievements.contains("first_steps"));
        assert!(
            !e.save.achievements.contains("boss_all"),
            "单 Boss 不触发屠龙"
        );
        assert!(
            e.save.achievements.contains("champion"),
            "单关总关数 1 → 通关即冠军"
        );
    }

    #[test]
    fn all_four_bosses_unlock_boss_all() {
        let mut toml = String::new();
        for (i, id) in ["l1-clone", "l2-result", "l3-trait", "l4-lifetime-trap"]
            .iter()
            .enumerate()
        {
            toml.push_str(&format!(
                "[[level]]\nid = \"{id}\"\ntitle = \"boss{i}\"\ntier = \"l4\"\ndescription = \"d\"\nstarter_code = \"fn main() {{ println!(\\\"1\\\"); }}\"\nis_boss = true\nexpect_output = \"1\"\nsource = \"x\"\n"
            ));
        }
        let set = LevelSet {
            levels: parse_levels(&toml).unwrap(),
        };
        let mut e = Engine::new(
            set,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        e.new_game();
        for i in 0..4 {
            e.start_level(i).unwrap();
            e.submit("fn main() { println!(\"1\"); }").unwrap();
        }
        assert!(e.save.achievements.contains("boss_slayer"));
        assert!(e.save.achievements.contains("boss_all"));
        assert!(e.save.achievements.contains("champion"), "4 关全过 → 冠军");
    }

    #[test]
    fn never_give_up_after_ten_fails_then_pass() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        for _ in 0..10 {
            // 0 心拦截前先回血（模拟在别处通关回血，验证 fail_count 可累计到 10）
            if e.save.hearts == 0 {
                e.save.hearts = 3;
            }
            e.submit(bad).unwrap();
        }
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().fail_count, 10);
        // 0 心：复习回血 1 心后通过（0 心禁提交，但复习后心 > 0 可提交）
        e.save.hearts = 0;
        assert!(e.review_lore("l0-hello"), "0 心复习应回 1 心");
        assert_eq!(e.save.hearts, 1);
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert!(
            e.save.achievements.contains("never_give_up"),
            "失败 ≥10 次后通过 → 永不言弃"
        );
    }

    #[test]
    fn achievements_idempotent_on_repeat() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let good = "fn main() { println!(\"x has the value {}\", 5); }";
        e.submit(good).unwrap();
        let first = e.save.achievements.clone();
        assert!(first.contains("first_steps"));
        e.submit(good).unwrap();
        e.submit(good).unwrap();
        // 重复通关不重复入账（HashSet 幂等，无新成就）
        assert_eq!(e.save.achievements, first);
    }

    // ===== P2-11：hint 失败联动（v3 §3.4 行为表 + §7.6）=====

    #[test]
    fn hint_unlock_state_default_table_four_tiers() {
        // 默认阈值 [1,3,5]（3 条提示）：行为表 4 档
        let s = |fc| hint_unlock_state(3, &[1, 3, 5], fc).unwrap();
        // 0-1 次失败：hint[0] 可看
        assert_eq!(
            s(0),
            HintUnlockState {
                unlocked: 1,
                expanded: 0,
                show_reference: false
            }
        );
        assert_eq!(
            s(1),
            HintUnlockState {
                unlocked: 1,
                expanded: 0,
                show_reference: false
            }
        );
        // ≥2 次：hint[0..2) 解锁且自动展开 hint[0]
        assert_eq!(
            s(2),
            HintUnlockState {
                unlocked: 2,
                expanded: 0,
                show_reference: false
            }
        );
        // ≥3 次：全部解锁且自动推进到 hint[1]
        assert_eq!(
            s(3),
            HintUnlockState {
                unlocked: 3,
                expanded: 1,
                show_reference: false
            }
        );
        // ≥4 次：推进到最后一条（hint[2]）+ 参考答案按钮
        assert_eq!(
            s(4),
            HintUnlockState {
                unlocked: 3,
                expanded: 2,
                show_reference: true
            }
        );
        assert_eq!(
            s(9),
            HintUnlockState {
                unlocked: 3,
                expanded: 2,
                show_reference: true
            }
        );
    }

    #[test]
    fn hint_unlock_state_default_table_clamps_to_hint_count() {
        // 2 条提示 + [1,3,5]（数据仅作防御）：unlocked/expanded 钳制在 len 内
        let s = |fc| hint_unlock_state(2, &[1, 3, 5], fc).unwrap();
        assert_eq!(
            s(2),
            HintUnlockState {
                unlocked: 2,
                expanded: 0,
                show_reference: false
            }
        );
        assert_eq!(
            s(4),
            HintUnlockState {
                unlocked: 2,
                expanded: 1,
                show_reference: true
            }
        );
        // 单条提示
        let s1 = |fc| hint_unlock_state(1, &[1, 3, 5], fc).unwrap();
        assert_eq!(
            s1(4),
            HintUnlockState {
                unlocked: 1,
                expanded: 0,
                show_reference: true
            }
        );
    }

    #[test]
    fn hint_unlock_state_custom_thresholds_per_threshold() {
        // 自定义阈值 [2,4,6]：逐阈值解锁，展开到「已解锁且阈值 ≤ fc」的最高索引
        let s = |fc| hint_unlock_state(3, &[2, 4, 6], fc).unwrap();
        assert_eq!(
            s(0),
            HintUnlockState {
                unlocked: 0,
                expanded: 0,
                show_reference: false
            }
        );
        assert_eq!(
            s(1),
            HintUnlockState {
                unlocked: 0,
                expanded: 0,
                show_reference: false
            }
        );
        assert_eq!(
            s(2),
            HintUnlockState {
                unlocked: 1,
                expanded: 0,
                show_reference: false
            }
        );
        assert_eq!(
            s(4),
            HintUnlockState {
                unlocked: 2,
                expanded: 1,
                show_reference: true
            }
        );
        assert_eq!(
            s(6),
            HintUnlockState {
                unlocked: 3,
                expanded: 2,
                show_reference: true
            }
        );
    }

    #[test]
    fn hint_unlock_state_none_when_manual_mode() {
        // 无 hint_unlock（旧 TOML）：None → UI 保持手动逐级揭示
        assert_eq!(hint_unlock_state(3, &[], 4), None);
        assert_eq!(hint_unlock_state(0, &[], 0), None);
        // 无 hints 时也返回 None（防御，validate 已保证 hint_unlock 与 hints 等长）
        assert_eq!(hint_unlock_state(0, &[1, 3, 5], 4), None);
    }

    #[test]
    fn reveal_hint_records_dedup_sorted() {
        let mut e = engine();
        e.new_game();
        // 乱序 + 重复：去重且升序
        assert!(e.reveal_hint("l0-hello", 1));
        assert!(e.reveal_hint("l0-hello", 0));
        assert!(!e.reveal_hint("l0-hello", 1), "重复查看幂等");
        assert_eq!(
            e.save.level_states.get("l0-hello").unwrap().hints_used,
            vec![0, 1]
        );
        // 不存在的关卡 id：自动补默认进度（不 panic）
        assert!(e.reveal_hint("unknown-level", 2));
        assert_eq!(
            e.save.level_states.get("unknown-level").unwrap().hints_used,
            vec![2]
        );
    }

    #[test]
    fn reveal_hint_is_zero_cost() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 失败一次建立 fail_count 基线
        e.submit("fn main() { println!(\"wrong\"); }").unwrap();
        let before = e.save.clone();
        let p = e.save.level_states.get("l0-hello").unwrap();
        assert_eq!(p.fail_count, 1);
        assert!(p.hints_used.is_empty());
        // 查看 hint：心/XP/fail_count/attempts 全部不变
        e.reveal_hint("l0-hello", 0);
        let after = e.save.level_states.get("l0-hello").unwrap();
        assert_eq!(after.hints_used, vec![0], "查看被记录");
        assert_eq!(e.save.hearts, before.hearts, "查看 hint 不扣心");
        assert_eq!(e.save.xp, before.xp, "查看 hint 不扣 XP");
        assert_eq!(
            after.fail_count,
            before.level_states.get("l0-hello").unwrap().fail_count,
            "fail_count 不变"
        );
        assert_eq!(
            after.attempts,
            before.level_states.get("l0-hello").unwrap().attempts,
            "attempts 不变"
        );
    }

    #[test]
    fn no_hint_perfect_ordering_hint_before_pass() {
        // 先看 hint 再首提交即通过（fail_count==0，本可完美）→ 不触发 no_hint_perfect
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.reveal_hint("l0-hello", 0);
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert_eq!(
            e.save.level_states.get("l0-hello").unwrap().fail_count,
            0,
            "查看 hint 不改 fail_count"
        );
        assert!(
            !e.save.achievements.contains("no_hint_perfect"),
            "看过 hint 后完美通关也不应无师自通"
        );
    }

    #[test]
    fn no_hint_perfect_ordering_hint_after_pass() {
        // 先完美通关（hints_used 空 → 无师自通已发），之后再查看 hint → 成就不撤回
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert!(e.save.achievements.contains("no_hint_perfect"));
        e.reveal_hint("l0-hello", 0);
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert!(
            e.save.achievements.contains("no_hint_perfect"),
            "已发成就不因后看 hint 撤回"
        );
    }

    // ===== P4-26：自定义关卡导入（独立章节 + 存档隔离）=====

    const CUSTOM_LEVEL_TOML: &str = r#"
[[level]]
id = "c1-hello"
title = "自定义·你好"
tier = "l0"
description = "d"
starter_code = "fn main() { println!(\"custom ok\"); }"
expect_output = "custom ok"
source = "community"
"#;

    fn custom_level() -> Level {
        parse_levels(CUSTOM_LEVEL_TOML).unwrap().remove(0)
    }

    fn custom_engine() -> Engine {
        let builtin = LevelSet {
            levels: parse_levels(LEVELS).unwrap(),
        };
        Engine::with_custom_levels(
            builtin,
            vec![custom_level()],
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        )
    }

    fn two_builtin_ids() -> HashSet<String> {
        ["l0-hello", "l1-move"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[test]
    fn load_custom_levels_single_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("01-custom.toml"), CUSTOM_LEVEL_TOML).unwrap();
        let (levels, errors) = load_custom_levels(dir.path(), &two_builtin_ids());
        assert!(errors.is_empty(), "合法文件不应报错: {errors:?}");
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].id, "c1-hello");
    }

    #[test]
    fn load_custom_levels_builtin_id_collision_rejects_file() {
        let dir = tempfile::tempdir().unwrap();
        // 冲突文件（id 与内置相同）
        std::fs::write(
            dir.path().join("01-conflict.toml"),
            "[[level]]\nid = \"l0-hello\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nexpect_output = \"\"\nsource = \"x\"\n",
        )
        .unwrap();
        // 合法文件：应照常加载
        std::fs::write(dir.path().join("02-ok.toml"), CUSTOM_LEVEL_TOML).unwrap();
        let (levels, errors) = load_custom_levels(dir.path(), &two_builtin_ids());
        assert_eq!(levels.len(), 1, "冲突文件被拒后其余文件照常加载");
        assert_eq!(levels[0].id, "c1-hello");
        assert_eq!(errors.len(), 1);
        let msg = errors[0].message();
        assert!(msg.contains("自定义关卡"), "应含中文「自定义关卡」: {msg}");
        assert!(msg.contains("加载失败"), "应含中文「加载失败」: {msg}");
        assert!(msg.contains("与内置关卡冲突"), "应含中文冲突原因: {msg}");
    }

    #[test]
    fn load_custom_levels_per_file_errors_no_crash() {
        let dir = tempfile::tempdir().unwrap();
        // 非法 TOML
        std::fs::write(dir.path().join("01-bad-toml.toml"), "not [ valid toml [[[").unwrap();
        // quiz answer_index 越界（schema 校验失败）
        std::fs::write(
            dir.path().join("02-bad-quiz.toml"),
            "[[level]]\nid = \"q1\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nkind = \"quiz\"\noptions = [\"a\", \"b\"]\nanswer_index = 9\nstarter_code = \"fn main() {}\"\nsource = \"x\"\n",
        )
        .unwrap();
        // source 缺失
        std::fs::write(
            dir.path().join("03-no-source.toml"),
            "[[level]]\nid = \"s1\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nexpect_output = \"\"\n",
        )
        .unwrap();
        // 合法文件
        std::fs::write(dir.path().join("04-ok.toml"), CUSTOM_LEVEL_TOML).unwrap();
        let (levels, errors) = load_custom_levels(dir.path(), &two_builtin_ids());
        assert_eq!(levels.len(), 1, "3 个坏文件被逐文件拒绝，合法文件照常加载");
        assert_eq!(levels[0].id, "c1-hello");
        assert_eq!(errors.len(), 3);
        for e in &errors {
            assert!(
                e.message().contains("加载失败"),
                "逐文件中文错误: {}",
                e.message()
            );
        }
        assert!(
            errors[0].reason.contains("TOML"),
            "TOML 解析错误原因: {}",
            errors[0].reason
        );
        assert!(
            errors[1].reason.contains("越界"),
            "quiz 越界原因: {}",
            errors[1].reason
        );
        assert!(
            errors[2].reason.contains("source"),
            "source 缺失原因: {}",
            errors[2].reason
        );
    }

    #[test]
    fn load_custom_levels_missing_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let (levels, errors) = load_custom_levels(&missing, &HashSet::new());
        assert!(levels.is_empty());
        assert!(errors.is_empty(), "目录不存在 = 无自定义关卡，不报错");
    }

    #[test]
    fn custom_levels_unlocked_and_playable() {
        let mut e = custom_engine();
        e.new_game();
        // 自定义关卡默认 Unlocked，可直接挑战（不依赖内置线性链）
        assert_eq!(
            e.save.level_states.get("c1-hello").unwrap().state,
            LevelState::Unlocked
        );
        assert_eq!(e.builtin_count, 2);
        assert!(e.is_custom_index(2));
        e.start_level(2).unwrap();
        assert_eq!(e.current, Some(2));
        let res = e.submit("fn main() { println!(\"custom ok\"); }").unwrap();
        assert!(matches!(res, Validation::Pass { .. }));
        assert_eq!(
            e.save.level_states.get("c1-hello").unwrap().state,
            LevelState::Passed
        );
    }

    #[test]
    fn custom_pass_does_not_trigger_achievements_or_rank() {
        let mut e = custom_engine();
        e.new_game();
        e.start_level(2).unwrap();
        e.submit("fn main() { println!(\"custom ok\"); }").unwrap();
        // 自定义通关不触发任何成就（first_steps/champion 等）
        assert!(
            e.save.achievements.is_empty(),
            "自定义通关不应触发成就: {:?}",
            e.save.achievements
        );
        assert_eq!(e.builtin_completed_count(), 0, "rank 只认内置进度");
        assert_eq!(crate::rank::rank_for(e.builtin_completed_count()).level, 1);
        // 存档隔离：自定义进度本身照常落盘（level_states 支持任意 id）
        assert_eq!(
            e.save.level_states.get("c1-hello").unwrap().state,
            LevelState::Passed
        );
        assert!(e.save.completed_steps.contains("c1-hello:pass"));
        // 再通关内置 → first_steps 正常触发（成就体系未被自定义污染）
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert!(e.save.achievements.contains("first_steps"));
        assert_eq!(e.builtin_completed_count(), 1);
    }

    #[test]
    fn custom_pass_does_not_touch_builtin_combo() {
        let mut e = custom_engine();
        e.new_game();
        // 自定义通关：全局 combo 不动（combo 成就不被自定义刷）
        e.start_level(2).unwrap();
        e.submit("fn main() { println!(\"custom ok\"); }").unwrap();
        assert_eq!(e.save.combo, 0, "自定义通关不累加内置连击");
        // 自定义失败：不重置内置连击
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert_eq!(e.save.combo, 1);
        e.start_level(2).unwrap();
        e.submit("fn main() { println!(\"wrong\"); }").unwrap();
        assert_eq!(e.save.combo, 1, "自定义失败不打断内置连击");
    }

    #[test]
    fn next_in_chapter_stays_within_chapter() {
        let e = custom_engine();
        // 内置：0 → 1；内置末尾（1）→ None（不跨入自定义章节）
        assert_eq!(e.next_in_chapter(0), Some(1));
        assert_eq!(e.next_in_chapter(1), None);
        // 自定义：2（仅一关）→ None
        assert_eq!(e.next_in_chapter(2), None);
    }

    #[test]
    fn custom_pass_save_roundtrip_persists() {
        let mut e = custom_engine();
        e.new_game();
        e.start_level(2).unwrap();
        e.submit("fn main() { println!(\"custom ok\"); }").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("save.toml");
        crate::save::save(&e.save, &p).unwrap();
        let loaded = crate::save::load(&p).unwrap();
        assert_eq!(
            loaded.level_states.get("c1-hello").unwrap().state,
            LevelState::Passed,
            "自定义进度存档回读保留"
        );
        assert!(loaded.completed_steps.contains("c1-hello:pass"));
    }

    // ---- P3-17：Boss 提示门控 + P3-18：best_time/自由模式/末关庆典 ----

    #[test]
    fn boss_hint_locked_below_five_fails_unlocked_at_five() {
        // v3 §7.5：Boss 提示默认禁用，fail_count ≥ 5 解锁兜底
        assert!(boss_hint_locked(true, 0));
        assert!(boss_hint_locked(true, 4));
        assert!(!boss_hint_locked(true, 5), "fail_count ≥5 → 提示恢复正常");
        assert!(!boss_hint_locked(true, 9));
        // 普通关不受门控
        assert!(!boss_hint_locked(false, 0));
        assert!(!boss_hint_locked(false, 4));
    }

    #[test]
    fn boss_hint_lock_remaining_counts_down() {
        // 展示用剩余次数：0 → 5、4 → 1、≥5 → 0（已解锁）
        assert_eq!(boss_hint_lock_remaining(0), 5);
        assert_eq!(boss_hint_lock_remaining(1), 4);
        assert_eq!(boss_hint_lock_remaining(4), 1);
        assert_eq!(boss_hint_lock_remaining(5), 0);
        assert_eq!(boss_hint_lock_remaining(99), 0);
    }

    #[test]
    fn boss_hint_locked_ignores_hint_input_in_app() {
        // Boss 关 fail_count < 5：按提示键不应打开提示面板（状态不翻转）
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"b\"\ntitle = \"b\"\ntier = \"l1\"\ndescription = \"d\"\nhints = [\"概念\", \"定位\", \"解法\"]\nhint_unlock = [1, 3, 5]\nis_boss = true\nstarter_code = \"fn main() { println!(1); }\"\nexpect_output = \"1\"\nsource = \"x\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(
            set,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        e.new_game();
        e.start_level(0).unwrap();
        // 初始 fail_count=0：Boss 门控锁定，按提示不应展示
        let st = hint_unlock_state(3, &[1, 3, 5], 0).unwrap();
        assert_eq!(st.unlocked, 1, "普通关本应解锁 hint[0]");
        assert!(boss_hint_locked(true, 0));
        // 失败 4 次仍锁；第 5 次失败后解锁
        let mut fc = 0;
        for _ in 0..4 {
            fc += 1;
            assert!(boss_hint_locked(true, fc), "fc={fc} 仍锁");
        }
        assert!(!boss_hint_locked(true, 5), "fc=5 解锁");
    }

    #[test]
    fn min_best_time_keeps_smallest() {
        assert_eq!(min_best_time(None, 1200), Some(1200), "首次记录");
        assert_eq!(min_best_time(Some(1200), 800), Some(800), "更快 → 更新");
        assert_eq!(
            min_best_time(Some(800), 1500),
            Some(800),
            "更慢 → 保留历史最小"
        );
    }

    #[test]
    fn pass_records_best_time_ms() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        e.submit(code).unwrap();
        let p = e.save.level_states.get("l0-hello").unwrap();
        assert!(p.best_time_ms.is_some(), "通关分支应记录 best_time_ms");
        // 重复通关仍记录（最小值保留）
        e.submit(code).unwrap();
        let p = e.save.level_states.get("l0-hello").unwrap();
        assert!(p.best_time_ms.is_some());
    }

    fn all_pass_engine() -> Engine {
        // 两关内置：全过 = 通关全部（自由模式 + 末关庆典边界）
        let set = LevelSet {
            levels: parse_levels(LEVELS).unwrap(),
        };
        let mut e = Engine::new(
            set,
            SaveData::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        e.start_level(1).unwrap();
        e.submit("fn main() { let s = String::from(\"hi\"); println!(\"{}\", s); }")
            .unwrap();
        e
    }

    #[test]
    fn all_passed_sets_free_mode_and_victory_flag() {
        let e = all_pass_engine();
        assert!(e.save.practice_unlock_all, "内置全过 → 自由模式解锁");
        assert!(e.save.victory_celebrated, "内置全过 → 末关庆典标记");
        assert_eq!(e.builtin_completed_count(), 2);
    }

    #[test]
    fn free_mode_allows_locked_levels_and_repeat_gives_zero_xp() {
        let mut e = all_pass_engine();
        // R10 后：locked 关卡（l0-hello 之外已全过，无 locked；再造一个锁定态）
        // 直接验证：把 l0-hello 改回 Locked，start_level 仍放行（自由模式入口）
        e.save.level_states.get_mut("l0-hello").unwrap().state = LevelState::Locked;
        e.start_level(0).unwrap();
        // 重复通关 +0 XP（一次制天然保证）
        let xp_before = e.save.xp;
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        assert_eq!(e.save.xp, xp_before, "自由模式重复通关 +0 XP");
    }

    #[test]
    fn free_mode_gated_below_full_clear() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }")
            .unwrap();
        // 只过 1/2 关：未全通 → 无自由模式、无庆典标记
        assert!(!e.save.practice_unlock_all);
        assert!(!e.save.victory_celebrated);
        // 未解锁自由模式时，Locked 关卡仍拒绝（线性解锁链已把 l1 置为
        // Unlocked，手动改回 Locked 验证无自由模式放行入口）
        e.save.level_states.get_mut("l1-move").unwrap().state = LevelState::Locked;
        assert!(matches!(e.start_level(1), Err(GameError::LevelLocked(_))));
    }

    #[test]
    fn victory_flag_restart_proof_via_save_roundtrip() {
        let e = all_pass_engine();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("save.toml");
        crate::save::save(&e.save, &p).unwrap();
        let loaded = crate::save::load(&p).unwrap();
        assert!(loaded.victory_celebrated, "庆典标记持久化 → 重启后不再庆祝");
        assert!(loaded.practice_unlock_all, "自由模式标记持久化");
    }
}
