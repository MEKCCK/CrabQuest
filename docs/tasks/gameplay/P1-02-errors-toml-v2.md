# 需求 P1-02：errors.toml schema v2（错误码数据层升级）

- 优先级：P1（MVP 必修）｜ 前置：无 ｜ 依赖方：P1-03、P2-15
- 来源：v3 §5.1-5.2；docs-review/L3-B1-errorcodes.md、L2-error.md

## 目标

错误码映射从 20 条纯文案升级为带 severity/概念/修复示例的结构化数据，含兜底段与死码登记段，且**旧文件零改动兼容**。

## 背景（实测事实）

- 现网 bug 关联：无 E 码错误没有兜底文案（见 P1-01）。
- E0412、E0504 在 rustc 1.97 已不再发射（--explain 明示 no longer emitted），留着会误导关卡作者设计无法通过的死码关。
- E0277 与 E0369 需共存（i32+f64 报 E0277，仅 &str+&str 报 E0369）。

## 需求范围

1. **条目字段扩展**（全部 `#[serde(default)]`）：zh（必填 ≤60 字）、link（必填，官方 error_codes URL）、link_zh（可选，rustwiki 中文页）、severity（P0/P1/P2，默认 P1）、concept（12 类枚举）、fix（可选 ≤40 字）、example（可选 ≤8 行复现代码）。
2. **新增 [fallback] 段**：EUNKNOWN 与未收录码的兜底文案（覆盖 format 参数错误等无 E 码场景）。
3. **新增 [deprecated] 段**：登记 E0412、E0504，注明替代码。
4. **活跃码全集**：现有 6 码补 severity/concept；新增 P0 5 码（E0282/E0384/E0594/E0621/E0601，全部实测可发射）+ P1（E0506）+ rustlings 缺口码（E0046/E0283/E0381/E0603 等），各带中文解释草案与双链接。
5. **default_fallback()**：assets 缺失时最小可用集合扩到全部 P0 码 + EUNKNOWN。

## 验收标准

- [ ] 现有 errors.toml 不加字段也能被 v2 解析器正常读取（兼容测试）。
- [ ] 新增 12+ 个条目全部含 zh/link/severity/concept，zh ≤60 字、fix ≤40 字（脚本校验）。
- [ ] 新增错误码的官方链接与 rustwiki 中文页全部实测 200（curl 验证，记录命令）。
- [ ] `[fallback]`、`[deprecated]` 独立结构解析，旧文件缺省不报错。
- [ ] E0412/E0504 从活跃条目标记/移除，不出现在任何关卡设计参考中。

## 依赖 / 风险

- 依赖：P1-01 的 EUNKNOWN 语义先行定义。
- 风险：死码判定随 rustc 版本演化 → deprecated 段维护节奏与 rustc 版本绑定（版本矩阵见 P4-27）。

## 参考素材

- v3 §5.1 错误码全集表（30 活跃 + 2 死码）、§5.2 schema v2 TOML 样板
- docs-review/L3-B1-errorcodes.md（18 条字数预校验、12 官方页 + 15 rustwiki 页实测 200）
