# 需求 P1-05：XP 一次制分档 + Rank 10 级

- 优先级：P1（MVP 必修）｜ 前置：无 ｜ 依赖方：P2-10 成就（挂 submit）
- 来源：v3 §7.1-7.3；docs-review/L3-C1-gamification.md §1-§2

## 目标

XP 从「固定 20 可重复刷」改为「一次制分档」，并引入按完成关卡数判定的 10 级 rank（与 XP 解耦）。

## 背景（实测事实）

- 当前 `XP_PER_PASS=20`，重复通关可无限刷 XP，失去进度意义。
- rust-quest 经验：rank 按完成关卡数判定比按 XP 更直观、不可刷。

## 需求范围

1. **XP 定价表**（替换 XP_PER_PASS=20）：

   | 事件 | XP | 条件 |
   |---|---|---|
   | 首次通关（普通关） | +25 | completed_steps 无 `"{id}:pass"` |
   | 重复通关 | +0 | 已有记录（combo 仍更新） |
   | 完美通关（首次提交即过） | +10 | fail_count == 0 |
   | 连击加成 | +5 | 首通且 combo ≥ 3 |
   | Boss 首通 ≤4 次 | +50 | 见 P3-17 |
   | Boss 首通 >4 次 | +30 | 见 P3-17 |

   单关上限：普通 40、Boss 65。实现为 `award_xp()` 四步累加 + completed_steps 写入。
2. **completed_steps 去重**：`HashSet<String>`，`"{level_id}:pass"` 标记首通（配合 P1-04 存档）。
3. **rank 模块**（game-core/src/rank.rs 纯模块，不新增存档字段）：
   - 10 级中文称号：见习学徒 → 输出新手 → 语法学徒 → 所有权新兵 → 借用骑士 → 集合行者 → 错误猎人 → 特质学徒 → 生命周期贤者 → 铁锈冠军。
   - 判定 = 完成关卡数（R2 首关 / R5 L1 全 / R7 L2 全 / R9 L3 全 / R10 全 15 关）。
   - rank 只解锁元内容（XP 进度条、错误码图鉴、统计页、自由模式），**不解锁关卡**。

## 验收标准

- [ ] 首次通关 +25、重复通关 +0（completed_steps 有记录时）。
- [ ] 完美通关 +10、连击 ≥3 再 +5（可叠加，单关上限 40 验证）。
- [ ] rank 里程碑边界单测：完成 1/4/8/11/15 关时 rank 正确（§11.4 用例）。
- [ ] rank 不解锁关卡：关卡线性解锁链不受 rank 影响。
- [ ] 现有测试中 XP_PER_PASS 引用全部迁移，`cargo test --workspace` 全绿。

## 依赖 / 风险

- 依赖：P1-04（completed_steps 字段落存档）。
- 风险：现有存档无 completed_steps → 老玩家重玩不重复得 XP 的语义由 P1-04 迁移补全（state==Passed → 写入）。

## 参考素材

- v3 §7.2 XP 定价表、§7.3 Rank 表（10 级全文）、§12.2-5
- docs-review/L3-C1-gamification.md §1（rust-quest XP 定价对比）、§2
