# 需求 P1-06：中文字体与 CJK 渲染修复

- 优先级：P1（MVP 必修）｜ 前置：无 ｜ 依赖方：P3-20 双字体
- 来源：v3 §7.8；docs-review/L3-C2-save-ui.md §3、L2-gamification.md C4

## 目标

修复 CJK 渲染的三个已知问题（超宽行不换行、字号不一致、OFL 许可缺失），保证中文文案正确显示且许可合规。

## 背景（实测事实）

- 内嵌 JetBrains Maple Mono 33,331 字形覆盖 CJK 区，epaint 0.31 原生支持 CJK 断行——渲染能力具备。
- 三个缺陷：① layouter 忽略 wrap_width（现为 INFINITY），超宽中文行不换行；② 字号 14.0 vs 12.0 不一致；③ OFL 1.1 许可文件未随包分发（许可义务缺失）。
- **IME 不可用**（egui-miniquad 无 IME 通道）：编辑器内中文输入不可用（可粘贴）——本需求只收口约束，方案调研在 P3-20。

## 需求范围

1. **layouter 修复**：`job.wrap.max_width = wrap_width`（一行修复）；超宽中文行正确换行。
2. **字号统一**：编辑器用 `TextStyle::Monospace.resolve(ui.style())`，消除 14/12 混用。
3. **OFL.txt 补发**：`crates/game-ui/assets/OFL.txt`（OFL 1.1 全文 + JetBrains Mono 与 Maple Mono 双版权声明）；README 致谢升级。
4. **关卡设计约束（验收标准的一部分，写进关卡生产文档）**：所有关卡可编辑代码**不要求玩家输入 CJK 字符**；需要中文输出的关，starter_code 直接给出字符串字面量；编辑器首次进入显示一行弱提示「中文请复制粘贴」。

## 验收标准

- [ ] 超宽中文行（> 面板宽度）正确换行，无截断无横向溢出（截图冒烟）。
- [ ] 编辑器行号与代码字号一致。
- [ ] `crates/game-ui/assets/OFL.txt` 存在且含双版权声明；README 提及字体许可。
- [ ] 15 个现有关卡 + 新增关卡的 starter_code 均不要求玩家输入 CJK（脚本扫描：代码中需要中文输出的关，字面量已内嵌）。
- [ ] `cargo test --workspace` 全绿；字体字形覆盖冒烟（可选：fontTools 脚本验证关卡文案用字在内嵌字体中）。

## 依赖 / 风险

- 依赖：无。
- 风险：IME 缺失是结构性限制 → 不尝试在编辑器内做中文输入支持（P3-20 再调研插入片段按钮/换后端）。

## 参考素材

- v3 §7.8（4 条必修 + IME 约束 + P3 优化方向）
- docs-review/L3-C2-save-ui.md §3（双字体草案、IME 证据链 egui-miniquad lib.rs:208）
