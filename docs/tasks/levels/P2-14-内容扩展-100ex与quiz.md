# 需求 P2-14：内容扩展第二批（100-exercises + rust-quiz）

- 优先级：P2（内容扩展）｜ 前置：P1-07、P2-12 ｜ 来源：v3 §4.1；docs-review/L3-A1-levels.md

## 目标

将 100-exercises 02/03/04 章（结构借鉴 + 代码重写）与 rust-quiz D1 输出比对题（题代码可搬、解释自写）转成关卡，填充 L0-L4。

## 背景（实测事实）

- 100-exercises 为 CC BY-NC 4.0（非商业）：只借鉴结构与教学法，不复制习题文本；06 章依赖 helpers（ticket_fields/common）需内联，07_threads/08_futures 排除。
- 100-exercises 全为 lib + `#[cfg(test)]` 无 main → 裸 rustc 报 E0601，需补 main（W2 规则）。
- rust-quiz 37 题实测：35 输出预测型（D1=21 / D2=11 / D3=3）、2 编译失败型（007→E0170、011→E0794）、0 UB；005/007 为 tombstone 不采用。
- 样例已验证：rust-quiz 026 输出 `112031`、011 编译失败报 E0794（allow_compile_fail 关）、013 为选择题（D1，需 kind=quiz）。

## 需求范围

1. **100-exercises 关卡**（结构借鉴 + 代码重写，non-NC 合规）：L0：04-l0-integers（02/01_integers，E0308）；L1：20-l1-ownership-ticket（03/06_ownership 内联后，E0382）；L2：27-l2-saturating（02/09_saturating，溢出 panic，fixed 输出 120/4294967295）。
2. **rust-quiz 关卡**（题代码可搬 + 全部自写中文解释，规避 CC BY-SA 传染）：L4：47-l4-lazy-map（026，输出 `112031`）、48-l4-fnptr（011，allow_compile_fail → E0794）、49-l4-mutable-zst（013，D1，选择题，需 P3-21 或暂缓）、50-l4-drop-underscore（019，输出 `21`）、51-l4-lifetime-ext（037，输出 `1001`）、52-l4-fnmut-copy（036，输出 `1223`）。
3. **每关走 §2 流水线** S1-S8 + Q1-Q10；rust-quiz 改编关 source 标 `rust-quiz (questions/NNN-*.rs, CC BY-SA 4.0，解释自写)`。
4. **选择题暂缓**：49 关若 quiz 类型未落地（P3-21），先转为输出比对关或挂起。

## 验收标准

- [ ] 每关 broken/fixed 实测（错误码 + expect_output），命令记录在案。
- [ ] rust-quiz 关卡的 description/hints 全部为自写中文（grep 对照原题 .md 无 ≥3 连词雷同）。
- [ ] 100-exercises 关卡无 helpers 依赖残留、无原文复制（自查清单 §6.4）。
- [ ] 关卡数量 23 → 29（6 新增；49 视 quiz 落地与否 ±1）。

## 参考素材

- v3 §4.1 L4 分布表、§3.5 source 规范、§6.4 自查清单
- docs-review/L3-A1-levels.md §5（48 草稿已实测 E0794）、L2-exercises.md A2/A3（37 题分类表）
