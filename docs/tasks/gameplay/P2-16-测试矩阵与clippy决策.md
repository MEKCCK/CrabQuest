# 需求 P2-16：测试矩阵与 clippy 决策

- 优先级：P2（质量）｜ 前置：P1-01/02 ｜ 来源：v3 §11.1；docs-review/L3-B2-parser.md §2-§3

## 目标

错误码 fixture 矩阵 Tier 2 全量落地（真实编译验证），并决策 clippy 类关卡的收录方式。

## 背景（实测事实）

- L3-B2 已产出 15 个 fixture（13 编译失败带预期码+行号、2 无码、2 panic），全部 rustc 1.97 实测通过。
- clippy lint 在裸 rustc 下完全不触发（22_clippy 零输出退出码 0）→ 与当前管线不兼容。

## 需求范围

1. **fixture 目录结构**（crates/game-core/tests/fixtures/）：`errors/`（每场景 broken.rs + fixed.rs + expected.toml）、`nocode/`（format 参数错误、let chains 版本）、`panic/`（越界、unwrap None）、`dead_codes/`（E0412/E0504 负面断言）。
2. **expected.toml 元数据**：code（断言首条）/ line（实测 --> 行号）/ kind（compile|nocode|panic）/ message_contains（稳定子串）/ classification（panic 分类 id）。
3. **测试分层**：Tier 1 解析器单测（静态 stderr 快照，≈0 成本）→ Tier 2 真实编译矩阵（只比 E 码与行号不比文本，~8-10s 串行 / 4 路并行 ≈3s）→ Tier 3 关卡回归（15 关 starter 断言预期错误码集合 + 01-l0-print 的 EUNKNOWN 非空断言）。
4. **clippy 决策**：方案 A（推荐）= lint 违规改写成等价编译错误（如 `let pi: i32 = 3.14;` → E0308），clippy 类内容以编译错误关形式收录；方案 B lint_mode 关卡类型第二版再议——本需求先落方案 A 的 1 个样例关。

## 验收标准

- [ ] Tier 2 全量 15+ 场景通过（并行 ≈3s，串行 ≤10s）。
- [ ] Tier 3 关卡回归：15 关 starter 编译断言预期错误码；01-l0-print 断言反馈非空（锁死空反馈 bug）。
- [ ] dead_codes/ 断言：E0412/E0504 的 broken 代码解析结果 errors 为空或走 fallback（不误报活跃码）。
- [ ] 方案 A 样例关（clippy→编译错误改写）落地并有对应 fixture。
- [ ] `cargo test --workspace` 全绿，Tier 2 可标记 ignored 供本地跳过（CI 全跑）。

## 参考素材

- v3 §11.1-11.3（fixture 目录、分层成本表、边界用例清单）
- docs-review/L3-B2-parser.md §2-§3（15 fixture 实测结果）
