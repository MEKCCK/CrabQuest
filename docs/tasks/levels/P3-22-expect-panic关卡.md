# 需求 P3-22：expect_panic 关卡（制造指定 panic）

- 优先级：P3（玩法）｜ 前置：P2-12（expect_panic 字段）｜ 来源：v3 §3.2、§12.4

## 目标

启用 expect_panic 字段：「制造指定 panic」型关卡——玩家修复代码使其触发指定 panic 消息，子串匹配判定通关。

## 背景（实测事实）

- W9 规则已定义两方案：方案 A = expect_panic 字段（子串匹配 panic 消息）；方案 B = 引擎未支持前改写为修复后正常输出关。
- panic 分支优先级 = panic > 输出比对 > 错误码（P1-01 已落地）；panic 净化 8 类分类已存在，本需求复用分类框架。
- 现有 08-l2-vec（越界 panic）是天然素材；扩展槽 l2-panics 已预留。

## 需求范围

1. **判定**：expect_panic 非空时，编译成功 + 运行 panic + 净化后 panic 消息包含 expect_panic 子串 → 通关；否则失败（编译失败/无 panic/子串不匹配都算失败，反馈区分原因）。
2. **反馈**：失败时告知「期望 panic 但未触发」或「panic 消息不匹配（期望含 X，实际为 Y）」；复用 panic 分类卡。
3. **关卡内容**：首批 2 关——l2-panics（扩展槽，制造 index out of bounds panic）+ 复用 08-l2-vec 反向题（可选）。
4. **与 expect_output 互斥**：expect_panic 与 expect_output 同时出现时 expect_panic 优先（v3 §3.3 语义）；数据校验禁止两者同时非空（简化判定，P2-12 校验补一条）。

## 验收标准

- [ ] 子串匹配判定正确：包含即过、不包含即败（大小写敏感，与净化后消息比对）。
- [ ] 数据校验：expect_panic 与 expect_output 同时非空 → 加载报错。
- [ ] l2-panics 关落地：broken 版不 panic（编译过但输出不符或逻辑错）、fixed 版触发指定 panic 子串。
- [ ] 失败反馈区分「未触发」与「消息不匹配」两种原因。
- [ ] panic 净化规则（P1-01）对 expect_panic 判定同样生效（路径/线程 id 不干扰子串匹配）。

## 参考素材

- v3 §3.2（expect_panic 字段定义）、§3.3（优先级语义）、§4.1 扩展槽（l2-panics）
- docs-review/L3-A2-pipeline.md §2 W9（panic 型改写规则）
