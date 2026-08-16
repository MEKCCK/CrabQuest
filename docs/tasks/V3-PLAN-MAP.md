# v3 迭代计划（P1-P4）→ 三工作区任务映射

> 用途：确认设计文档 v3 §12 的 P1-P4 迭代计划**全部**被三个工作区任务覆盖，无遗漏。
> 追溯路径：v3 §12 → 本表 → 各工作区 INDEX.md / 任务文档。
> 分支基线：`docs/tasks`（共享参考）；三个工作区分支 `fix/error-feedback` / `feat/levels` / `feat/gameplay`。

## P1（MVP 必修）覆盖核对

| v3 §12.2 计划项 | 工作区 | 任务 | 需求文档 |
|---|---|---|---|
| ① EUNKNOWN 无码捕获 + fallback 兜底 | bugfix | T1 | gameplay/P1-01-错误反馈闭环.md |
| ② panic 净化 + 8 类分类 | bugfix | T1 | gameplay/P1-01-错误反馈闭环.md |
| ③ errors.toml schema v2 | features | T1 | gameplay/P1-02-errors-toml-v2.md |
| ④ 反馈面板结构化 | features | T3 | gameplay/P1-03-反馈面板结构化.md |
| ⑤ XP 定价替换 | features | T2 | gameplay/P1-05-XP与Rank.md |
| ⑥ rank 模块 | features | T2 | gameplay/P1-05-XP与Rank.md |
| ⑦ 存档 version + 迁移 + .bak | bugfix | T2 | gameplay/P1-04-存档版本迁移.md |
| ⑧ layouter 修复 | bugfix | T3 | gameplay/P1-06-字体与CJK渲染.md |
| ⑨ OFL.txt 补发 | bugfix | T3 | gameplay/P1-06-字体与CJK渲染.md |
| ⑩ 现有关卡修订（去剧透/补 hints/source） | levels | T1 | levels/P1-07-现有关卡修订.md |
| ⑪ 解析器单测回归锁 | bugfix | T4 | gameplay/P2-16-测试矩阵与clippy决策.md |

**P1 全 11 项覆盖 ✓**

## P2（机制与内容扩展）覆盖核对

| v3 §12.3 计划项 | 工作区 | 任务 | 需求文档 |
|---|---|---|---|
| hearts 3-5 + 0 心禁提交 + 复习回血 | features | T4 | gameplay/P2-08-hearts与复习回血.md |
| streak（chrono Unix day） | features | T5 | gameplay/P2-09-streak.md |
| 成就表（10 个） | features | T6 | gameplay/P2-10-成就系统.md |
| hint 与失败次数联动 | features | T7 | gameplay/P2-11-hint失败联动.md |
| 错误卡片行号跳转 | features | T9 | gameplay/P3-19-行号跳转编辑器.md |
| 新增 P1 码（E0506 等） | features | T8 | gameplay/P2-15-错误码卡片库.md |
| 11/15 关补全三级 hints | levels | T1 | levels/P1-07-现有关卡修订.md |
| 内容扩展第一批（rustlings 三章） | levels | T3 | levels/P2-13-内容扩展-rustlings三章.md |
| fixture 矩阵 Tier2 全量 | bugfix | T4 | gameplay/P2-16-测试矩阵与clippy决策.md |
| clippy 类关卡决策 | bugfix | T4 | gameplay/P2-16-测试矩阵与clippy决策.md |

**P2 全 10 项覆盖 ✓**

## P3（玩法与体验增强）覆盖核对

| v3 §12.4 计划项 | 工作区 | 任务 | 需求文档 |
|---|---|---|---|
| Boss 关机制（is_boss + 配额 + 不扣心 + 禁提示） | features T10 + levels T2 | T10 | gameplay/P3-17-Boss关机制.md（字段在 levels/P2-12） |
| 通关庆典动画（victory_celebrated） | features | T11 | gameplay/P3-18-通关庆典与自由模式.md |
| 自由模式（practice_unlock_all，R10） | features | T11 | gameplay/P3-18-通关庆典与自由模式.md |
| 双字体方案 + 字体子集化 | features | T12 | gameplay/P3-20-双字体与IME方案.md |
| IME 调研 | features | T12 | gameplay/P3-20-双字体与IME方案.md |
| quiz 关卡类型（复议） | levels | T5 | levels/P3-21-quiz关卡类型.md |
| expect_panic 字段启用 | levels | T6 | levels/P3-22-expect-panic关卡.md |

**P3 全 7 项覆盖 ✓**

## P4（安全与发布）覆盖核对

| v3 §12.5 计划项 | 工作区 | 任务 | 需求文档 |
|---|---|---|---|
| bwrap 真隔离沙盒 | features | T14 | gameplay/P4-24-bwrap沙盒.md |
| 自定义关卡导入 | features | T15 | gameplay/P4-26-自定义关卡导入.md |
| 53 关全量 + 7 扩展槽 | levels | T7 | levels/P4-25-53关全量生产.md |
| 发布合规审查（许可/README/CI） | features | T16 | gameplay/P4-27-发布合规.md |
| syn 拦截清单补全 | features | T13 | gameplay/P4-23-syn拦截补全.md |

**P4 全 5 项覆盖 ✓**

## 汇总

| v3 批次 | 计划项数 | 覆盖 | 跨工作区协作点 |
|---|---|---|---|
| P1 | 11 | ✓ 全 | ⑦ 存档在 bugfix → features T2/T4/T6 依赖其字段 |
| P2 | 10 | ✓ 全 | 内容扩展在 levels，卡片库在 features |
| P3 | 7 | ✓ 全 | Boss 字段 is_boss 在 levels，机制在 features |
| P4 | 5 | ✓ 全 | 无 |
| **合计** | **33** | **✓ 全** | 见下 |

## 跨工作区字段依赖（三个分支需对齐口径）

1. **存档字段**（bugfix T2 = P1-04）：features 的 T2（XP completed_steps）/ T4（hearts）/ T5（streak）/ T6（成就）都写这些字段 → 以 v3 §8.2 struct 为准。
2. **错误解析**（bugfix T1 = P1-01）：features T3（ErrorCard）/ T8（卡片库）消费其输出结构 → 以 v3 §5.4/§7.7 为准。
3. **关卡字段**（levels T2 = P2-12）：features T10（Boss 需 is_boss）→ 以 v3 §3.2 为准。

> 三个分支从同一 `docs/tasks` 基线分叉，各自独立 PR；合并到主仓库时按此映射核对无遗漏即可。
