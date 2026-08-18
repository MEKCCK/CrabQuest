# Rust 学习游戏：界面图标候选

调研日期：2026-08-17。范围仅包含许可清晰、可用于商业发布的矢量 UI 图标集；本清单不下载或引入任何资源。

## 结论

首选 **[Tabler Icons](https://tabler.io/icons)**：MIT 许可、轮廓与填充风格成对、24×24 网格和可调描边，适合本项目的地图与闯关界面。它的[官方说明](https://tabler.io/icons)明确列出 MIT 许可、个人与商业使用以及 SVG 格式。建议全界面只采用其一个图标集，常规交互使用 Outline，状态/奖励使用 Filled；不要混入其他集合的图标，以免线宽、圆角和视觉重心不一致。

| 用途 | 推荐 Tabler 图标名 | 备用图标名 | 说明 |
| --- | --- | --- | --- |
| 世界地图 / 关卡路线 | `map-2`, `route`, `map-pin` | `road`, `signpost-2` | 地图页标题、当前节点与路线进度。 |
| 关卡节点 / 已完成 | `circle`, `circle-check`, `lock` | `flag-3`, `rosette-discount-check` | 用统一圆形节点表示未解锁、完成与锁定。 |
| 代码编辑 | `code`, `brackets`, `terminal-2` | `brand-rust`, `file-code-2` | `code` 最直观；Rust 标志只宜作专题装饰，不作功能唯一语义。 |
| 运行 / 提交 / 重试 | `player-play`, `send`, `refresh` | `device-floppy`, `rotate` | 运行与提交保留不同图形，避免玩家误以为会立即执行。 |
| 提示 | `bulb`, `bulb-filled` | `help-circle`, `message-circle-question` | 灯泡对应「提示」的熟悉心智模型。 |
| 生命 / 心 | `heart`, `heart-filled` | `heart-broken` | 正常心用填充，损失生命可用轮廓或破碎心。 |
| XP / 升级 | `bolt`, `sparkles`, `trending-up` | `rocket`, `chart-bar` | XP 徽章优先用 `bolt`；升级动画可叠加 `sparkles`。 |
| 连击 / 热度 | `flame`, `flame-filled` | `bolt`, `meteors` | 可随连击由轮廓切换至填充、加亮。 |
| Boss / 高难挑战 | `sword`, `skull`, `crown` | `shield`, `target-arrow` | 推荐 `sword` 作 Boss 节点，避免用过强恐怖元素。 |
| 成就 / 奖杯 | `trophy`, `medal`, `award` | `rosette`, `star` | 奖杯用于大成就，勋章用于普通里程碑。 |
| 设置 / 音量 / 显示 | `settings`, `volume`, `palette` | `adjustments`, `device-desktop-cog` | 设置入口固定使用齿轮；不要另用滑块图标混淆。 |
| 反馈状态 | `circle-check`, `circle-x`, `alert-triangle` | `info-circle`, `loader-2` | 成功、失败、警告/编译中保持颜色之外的形状差异。 |
| 学习内容 / 复习 | `book-2`, `bookmark`, `clipboard-list` | `school`, `brain` | 适合复习入口、知识卡与任务列表。 |

图标目录可以直接在 Tabler 的[官方浏览器](https://tabler.io/icons)中按上述名字检索；该站支持下载 SVG、改色、调整描边，并提供 outline / filled 两种样式。

## 候选图标集

| 优先级 | 图标集 | 许可证与商用判断 | 风格 / 覆盖度 | 适配理由 |
| --- | --- | --- | --- | --- |
| 1 | [Tabler Icons](https://tabler.io/icons) | **MIT**；官方页面明确说明开放源码、个人和商业使用。 | 6,000+，24×24、2px 描边，Outline 与 Filled。 | 地图、路线、火焰、奖杯、代码、设置均齐全；细线但不冷淡，最匹配轻量游戏 UI。 |
| 2 | [Phosphor Icons](https://phosphoricons.com/) | **MIT**；官网明确标注免费开源。 | 9,000+；Thin / Light / Regular / Bold / Fill / Duotone 六种字重。 | 若想让 XP、成就与 Boss 更有游戏感，可用其 `Duotone`；但必须全局固定一个字重。 |
| 3 | [Heroicons](https://heroicons.com/) | **MIT**；官网标注。 | 316 个，高质量 20/24px Outline 与 Solid。 | 总量小、选择成本低，适合极简工具栏；地图/Boss 等游戏语义相对不足。 |
| 4 | [Material Symbols](https://fonts.google.com/icons) | **Apache-2.0**；[官方仓库](https://github.com/google/material-design-icons)标注。 | 三种轮廓风格，填充、字重、光学尺寸等可变轴。 | 组件和无障碍语义成熟；视觉更像 Android/Material 应用，和当前游戏感相比不如 Tabler。 |
| 5 | [Remix Icon](https://github.com/Remix-Design/RemixIcon) | **Remix Icon License v1.0**；仓库说明可在产品中使用并分发，署名非强制。引入前仍应将完整许可证随发行物保留。 | 3,200+，24×24 Outlined / Filled。 | 分类丰富、游戏常用符号完整；自定义许可证不如 MIT / Apache-2.0 省审核成本，故不作为首选。 |

不建议把 Microsoft Fluent System Icons 作为本项目默认候选：虽然其仓库标有 MIT，但 Microsoft 生态中部分字体/品牌资产有单独条款，容易增加发布时的许可证审查负担。若未来选用，限定使用其明确标注为 MIT 的普通 SVG，并再次核验具体发行版本。

## 推荐落地方案

1. 选 **Tabler Icons**，锁定一个版本，将实际使用的约 20–30 个 SVG 放入独立资源目录，而不是导入整包或通过 CDN 加载。
2. 保留上游 `LICENSE` 文本，并在发布时的第三方许可页注明 “Tabler Icons — MIT License”。MIT 不要求界面署名，但保留许可文本是稳妥做法。
3. 规范尺寸：工具栏 20px、普通按钮 24px、关卡节点 28–32px、奖励/Boss 40–48px；描边优先 2px。状态不要只靠颜色，始终搭配 `check` / `x` / `lock` 等形状。
4. 本项目目前使用 `macroquad` 与 `egui-macroquad`，并未配置 SVG 运行时渲染。集成时应在构建阶段把选定 SVG 栅格化为 1× / 2× PNG，或增加经过评估的 SVG 解析依赖；不要在运行时联网获取图标。
5. 先制作一个小型“图标语义表”（本文件首表即可作为初稿），把同一概念固定为一个图标名，避免关卡页、反馈页和成就页各自使用不同隐喻。

## 许可核验链接

- [Tabler Icons 官网：MIT、SVG、个人与商业使用](https://tabler.io/icons)
- [Tabler Icons 开源仓库说明](https://tabler.io/repositories)
- [Phosphor Icons 官网：MIT 与下载入口](https://phosphoricons.com/)
- [Heroicons 官网：MIT](https://heroicons.com/)
- [Google Material Symbols / Icons 官方仓库：Apache-2.0 与 SVG / 可变字体说明](https://github.com/google/material-design-icons)
- [Remix Icon 官方仓库：产品使用、SVG 下载与许可证说明](https://github.com/Remix-Design/RemixIcon)

本清单是工程选型建议，不替代发布前对最终下载版本及其随附 LICENSE / NOTICE 文件的复核。
