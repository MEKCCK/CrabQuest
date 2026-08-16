# 需求 P3-20：双字体与 IME 方案

- 优先级：P3（体验/优化）｜ 前置：P1-06 ｜ 来源：v3 §7.8；docs-review/L3-C2-save-ui.md §3

## 目标

双字体方案（Proportional = Noto Sans SC + maple 兜底）、字体子集化、IME 缺失的交互方案决策。

## 背景（实测事实）

- 内嵌 JetBrains Maple Mono 覆盖 CJK，但为等宽字体，正文渲染偏技术感；emoji 全缺回退单色。
- **IME 不可用**（egui-miniquad 源码 `// no IME`，miniquad 无 IME 事件通道）：编辑器内 fcitx/ibus 中文输入不可用（可粘贴）。
- 字体体积：Maple Mono 当前内嵌体积大（含 33,331 字形），子集化可降至 3-5MB。

## 需求范围

1. **双字体**：Proportional 家族 = Noto Sans SC（OFL）+ maple 兜底（egui 字体栈回退）；Monospace 保持 maple；标题/描述用 Proportional，代码保持 Monospace。
2. **IME 方案调研与决策**（本需求的交付物之一是决策记录）：
   - 候选：① 维持现状（粘贴制）+ 编辑器弱提示「中文请复制粘贴」；② 换窗口后端（macroquad → 支持 IME 的如 wgpu/winit 直连）；③ 「插入片段」按钮（预置中文片段，绕过输入法）。
   - 约束：不引入网络/AI 依赖；不破坏沙盒禁网哲学；最小改动优先。
   - 输出：方案对比表 + 决策 + 影响范围。
3. **字体子集化**：只打包游戏文案用字 + 关卡代码可能出现的 ASCII/CJK 子集（需用字覆盖冒烟测试，fontTools 脚本验证）。
4. **emoji 处理**：减少 UI 对 emoji 依赖或接受单色回退（现有 ✅🔓🔒🔥💡 等）。

## 验收标准

- [ ] 双字体生效：标题/描述用 Noto Sans SC 渲染，代码区保持等宽 maple（截图冒烟）。
- [ ] 字体子集化后产物 ≤5MB，且覆盖全部关卡文案与错误码卡片用字（脚本验证无缺字）。
- [ ] IME 决策记录：方案对比 + 选定方案 + 理由（写入 docs/ 或本任务附录）。
- [ ] 编辑器中文输入约束不变：关卡不要求玩家输入 CJK（P1-06 验收持续有效）。
- [ ] 全量 `cargo test --workspace` 不受影响。

## 参考素材

- v3 §7.8（P3 优化方向）、§10.1（字体许可行）
- docs-review/L3-C2-save-ui.md §3（Noto Sans SC 草案、IME 证据链）
