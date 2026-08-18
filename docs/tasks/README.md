# 开发任务索引（策划版）

> 依据：[设计文档 v3](../superpowers/specs/2026-08-16-rust-learning-game-design-v3.md)
> 每个待办项一个 md，可独立排期/验收。本文件是唯一索引。
> 分类：**玩法**（gameplay/，机制/系统/UI/反馈/安全）｜ **关卡**（levels/，关卡内容生产与关卡类型定义）

## 优先级与排期

| 批次 | 定位 | 内容 | 前置 |
|---|---|---|---|
| **P1** | MVP 必修（修复与基础） | 错误反馈闭环 / errors.toml v2 / 反馈面板 / 存档迁移 / XP+rank / 字体 / 现有关卡修订 | 无 |
| **P2** | 机制与内容 | hearts / streak / 成就 / hint 联动 / 数据模型 v2 / 内容扩展 ×2 / 卡片库 / 测试矩阵 | P1-01~05 |
| **P3** | 玩法与体验 | Boss 机制 / 庆典+自由模式 / 行号跳转 / 双字体+IME / quiz 类型 / expect_panic | P2-12、P2-16 |
| **P4** | 安全与发布 | syn 补全 / bwrap / 53 关全量 / 自定义关卡导入 / 发布合规 | P3 全量 |

---

## 🎮 玩法（gameplay/，20 项：机制 / 系统 / UI / 反馈 / 安全）

### P1（MVP 必修）

| # | 文档 | 一句话需求 | 验收要点 |
|---|---|---|---|
| 1 | [gameplay/P1-01-错误反馈闭环.md](gameplay/P1-01-错误反馈闭环.md) | 编译失败永不空白反馈：EUNKNOWN 兜底 + panic 净化分类 + 解析器边界规则 | 01-l0-print 反馈非空；15 fixture 解析正确 |
| 2 | [gameplay/P1-02-errors-toml-v2.md](gameplay/P1-02-errors-toml-v2.md) | 错误码数据层升级：新字段 + fallback + deprecated + 新增 P0 码 | 旧文件零改动可解析；字数约束 |
| 3 | [gameplay/P1-03-反馈面板结构化.md](gameplay/P1-03-反馈面板结构化.md) | 错误从一维列表升级为结构化卡片（徽章/行号/折叠/链接降级） | 多错误默认展开第一条；离线隐藏链接 |
| 4 | [gameplay/P1-04-存档版本迁移.md](gameplay/P1-04-存档版本迁移.md) | 存档加 version + v0→v1 迁移 + .bak，杜绝静默丢档 | 迁移矩阵 5 条全过；fail-fast |
| 5 | [gameplay/P1-05-XP与Rank.md](gameplay/P1-05-XP与Rank.md) | XP 一次制分档（25/10/5/50/30）+ 10 级 rank | 重复通关 +0；rank 按关卡数判定 |
| 6 | [gameplay/P1-06-字体与CJK渲染.md](gameplay/P1-06-字体与CJK渲染.md) | 修 layouter 换行 + 字号统一 + OFL.txt 许可补发 | 超宽中文行正确换行；OFL 随包 |
| 7 | [gameplay/P2-16-测试矩阵与clippy决策.md](gameplay/P2-16-测试矩阵与clippy决策.md) | fixture 矩阵 Tier2 全量 + clippy 类关卡改写决策 | 15+ 场景编译矩阵 |

### P2（机制扩展）

| # | 文档 | 一句话需求 | 验收要点 |
|---|---|---|---|
| 8 | [gameplay/P2-08-hearts与复习回血.md](gameplay/P2-08-hearts与复习回血.md) | hearts 3-5、失败−1 通关+1、0 心禁提交、复习回血幂等 | 心增减边界；Boss 失败不扣 |
| 9 | [gameplay/P2-09-streak.md](gameplay/P2-09-streak.md) | 连续游玩日统计（纯展示），日期算法禁跨月 bug | 跨月/跨年单测 |
| 10 | [gameplay/P2-10-成就系统.md](gameplay/P2-10-成就系统.md) | 10 个成就静态表 + HashSet 存档，挂 submit 触发 | 触发条件逐条可测 |
| 11 | [gameplay/P2-11-hint失败联动.md](gameplay/P2-11-hint失败联动.md) | 提示按失败次数逐级解锁（0/2/3/≥4 阈值）+ 参考答案二次确认 | 联动行为表 4 档全过 |
| 12 | [gameplay/P2-15-错误码卡片库.md](gameplay/P2-15-错误码卡片库.md) | 8 张高频卡精修入库 + P0 全码卡片 + E0506 等新码 | 字数上限；rustc 实测 |

### P3（玩法与体验）

| # | 文档 | 一句话需求 | 验收要点 |
|---|---|---|---|
| 13 | [gameplay/P3-17-Boss关机制.md](gameplay/P3-17-Boss关机制.md) | 四段 Boss 机制：尝试配额 XP + 失败不扣心 + 提示禁用 | 5 个 Boss 单错误码约束 |
| 14 | [gameplay/P3-18-通关庆典与自由模式.md](gameplay/P3-18-通关庆典与自由模式.md) | 通关动画最小集 + R10 自由模式 + 全通关庆典一次制 | victory_celebrated 防重 |
| 15 | [gameplay/P3-19-行号跳转编辑器.md](gameplay/P3-19-行号跳转编辑器.md) | 错误卡片「第 N 行」点击跳转编辑器光标 | focus_line 状态联动 |
| 16 | [gameplay/P3-20-双字体与IME方案.md](gameplay/P3-20-双字体与IME方案.md) | Noto Sans SC 回退 + 字体子集化 + IME 调研决策 | 中文输入约束收口 |

### P4（安全与发布）

| # | 文档 | 一句话需求 | 验收要点 |
|---|---|---|---|
| 17 | [gameplay/P4-23-syn拦截补全.md](gameplay/P4-23-syn拦截补全.md) | 拦截清单扩到 7 类（fs/process/env/net/thread/unsafe/extern） | 每类触发用例可测 |
| 18 | [gameplay/P4-24-bwrap沙盒.md](gameplay/P4-24-bwrap沙盒.md) | 真隔离：--unshare-all + 只读系统 + tmpfs + 禁网 + ulimit | 先验证 bwrap 可用性 |
| 19 | [gameplay/P4-26-自定义关卡导入.md](gameplay/P4-26-自定义关卡导入.md) | 外部 TOML 关卡目录加载 | 校验失败有中文报错 |
| 20 | [gameplay/P4-27-发布合规.md](gameplay/P4-27-发布合规.md) | 许可核对 + README 致谢 + source/链接 CI 检查 + 版本矩阵 | 分发门槛通过 |

---

## 🗺 关卡（levels/，7 项：关卡内容生产与关卡类型定义）

| # | 文档 | 一句话需求 | 产出关卡数 |
|---|---|---|---|
| 1 | [levels/P1-07-现有关卡修订.md](levels/P1-07-现有关卡修订.md) | 15 关去剧透 + 补三级 hints + source 复核 | 0（修订 15 关） |
| 2 | [levels/P2-12-关卡数据模型v2.md](levels/P2-12-关卡数据模型v2.md) | schema 扩展字段落地：kind/expect_panic/hint_unlock/is_boss/trim_lines | 0（定义能力） |
| 3 | [levels/P2-13-内容扩展-rustlings三章.md](levels/P2-13-内容扩展-rustlings三章.md) | 06_move/16_lifetime/13_error 三章转关卡（stdout 化流水线） | +8（15→23） |
| 4 | [levels/P2-14-内容扩展-100ex与quiz.md](levels/P2-14-内容扩展-100ex与quiz.md) | 100-exercises 02/03/04 章 + rust-quiz D1 输出比对题转关卡 | +6（23→29） |
| 5 | [levels/P3-21-quiz关卡类型.md](levels/P3-21-quiz关卡类型.md) | 选择题型（kind=quiz）复议与落地（v1 不做，P3 决策） | +1（29→30） |
| 6 | [levels/P3-22-expect-panic关卡.md](levels/P3-22-expect-panic关卡.md) | 「制造指定 panic」关类型启用 | +1（30→31） |
| 7 | [levels/P4-25-53关全量生产.md](levels/P4-25-53关全量生产.md) | 按大纲批量产出关卡（含 Boss 与扩展槽） | 31→53（+22）+ 7 扩展槽 |

**关卡扩展总览**：现有 15 关 → 53 关主线（+38 新关）+ 7 扩展槽（上限 60）；关卡类型 1 种（code）→ 4 种（code / allow_compile_fail / quiz / expect_panic）+ Boss 变体。

---

## 依赖关系图

```
P1-01 ──→ P1-03（面板消费 ErrorCard）
P1-02 ──→ P1-03、P2-15
P1-04 ──→ P2-08/09/10（新字段进存档）
P1-05 ──→ P2-10（成就挂 submit）
P1-07 ──→ P2-13/14（改写规范先行）
P2-12 ──→ P3-17/21/22（新字段被消费）
P2-16 ──→ P3-21（clippy 决策影响 quiz 取舍）
P3 全量 ──→ P4-25（关卡内容完整后发布）
P4-23 ──→ P4-24（拦截清单是 bwrap 前兜底）
```

## 工作方式约定

- 每个任务文档 = 需求规格：目标 / 背景（含实测事实）/ 需求范围 / 验收标准 / 依赖 / 风险与开放问题 / 参考素材（v3 章节 + docs-review 文件路径）
- 需求落地顺序：P1 → P2 → P3 → P4；同批次可并行（文档间无强依赖的）
- 所有验收标准必须**可测试**（命令、断言、fixture），不写「看起来正常」类表述
- 关卡类需求必须引用 §2 改编流水线（S1-S8 + Q1-Q10）与 §6.2 文案字数规范
