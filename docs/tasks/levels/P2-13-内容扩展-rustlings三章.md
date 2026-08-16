# 需求 P2-13：内容扩展第一批（rustlings 核心三章）

- 优先级：P2（内容扩展）｜ 前置：P1-07（改写规范先行）｜ 来源：v3 §4.1；docs-review/L3-A1-levels.md

## 目标

将 rustlings 06_move_semantics / 16_lifetimes / 13_error_handling 三章习题经 stdout 化流水线转成游戏关卡，进入 L1-L3 主线。

## 背景（实测事实）

- rustlings 6.5.0 该三章共 15 题（6/3/6），52 个文件含 `#[cfg(test)]`（bug 藏在测试模块内，裸 rustc 直编不可见），必须 W1 改写（测试体搬 main、assert→println）。
- 06_move_semantics：2 题裸编 OK（bug 在测试内），3 题触发 E0596/E0308+E0382；16_lifetimes：3 题全触发 E0106；13_error_handling：3 题裸编 OK，3 题触发 E0277/E0369 等。
- 改编样例已验证：errors3 → "You now have 59 tokens."、move_semantics1 → E0596 + 两行 len/内容输出。

## 需求范围

1. **产出关卡**（对应 v3 §4.1 已编排关号）：L1：14-l1-move2（ms2，E0382）、15-l1-move3（ms3，E0596）；L2：25-l2-errors3（E0277）、26-l2-errors2（E0369）、28-l2-errors4（自定义错误）、29-l2-vecs2；L3：36-l3-lifetime3（E0106）、37-l3-lifetime1（E0106）。
2. **每关走 §2 流水线**：S1-S8（选素材→分析→stdout 化→expect_output 提取→错误码实测→三级 hints→source 标注→Q1-Q10 质量检查）。
3. **许可**：rustlings 为 MIT only，代码可直搬；保留版权声明；source 字段标 `rustlings (06_move_semantics/move_semantics2.rs, MIT)`。
4. **复用现有草案**：docs-review/L3-A1-levels.md §5 已有 04/05/14/17/25/27/36/40/48/53 十个实测草案，本批优先取 14/25/36 落地。

## 验收标准

- [ ] 每关 broken 版实测报预期错误码（S5 重测，不抄旧表）；fixed 版输出与 expect_output 逐字节一致（trim+CRLF 归一化后）。
- [ ] 每关三级 hints 达标（概念→rustwiki 链接→代码片段，hints[2] 才允许修复代码）。
- [ ] Q1-Q10 全过（含 source 精确、无答案痕迹、edition 2021 仅 std）。
- [ ] 新增关卡 id 不冲突、tier 正确、地图顺序正确；`cargo test --workspace` 全绿。
- [ ] 关卡数量从 15 → 23（8 新增）。

## 参考素材

- v3 §4.1 L1/L2/L3 分布表、§2.2-2.4 流水线
- docs-review/L3-A1-levels.md §5（10 个实测草稿）、L3-A2-pipeline.md §2（W1-W10）
