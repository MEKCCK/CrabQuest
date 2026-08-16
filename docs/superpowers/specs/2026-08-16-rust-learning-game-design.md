# Rust 学习游戏 — 设计文档

日期：2026-08-16
状态：已确认（用户逐节审批通过）

## 1. 概述

一个通过「写代码过关」来学习 Rust 的终端游戏。玩家在 TUI 界面中完成 rustlings 风格的改错/填空练习，游戏调用编译器校验代码，把编译器报错翻译成游戏化的中文反馈。线性关卡 + 基础养成（XP、连击、生命值、BOSS 关）。

核心目标：**让编译器成为游戏的一部分**——编译错误不再是挫败，而是游戏反馈。

### 设计决策摘要（与用户确认的结果）

| 决策点 | 选择 |
|---|---|
| 交付流程 | 设计文档先行 → 用户确认 → 实施计划 → 实现 |
| 游戏形态 | TUI 先行，UI 层抽象（trait），未来可换 2D |
| 界面语言 | 中英双语（题面英文原文 + 中文提示/反馈） |
| 关卡体系 | 线性 + 基础养成（XP/连击/HP/BOSS） |
| 代码校验 | 混合：syn 静态检查（填空类）+ rustc 子进程编译（语义类/BOSS） |
| 项目结构 | Cargo workspace：game-core / game-tui / game-data |
| 素材来源 | rustlings（普通关）、rust-quiz（BOSS 关）、Book/course.rs（提示文案） |

## 2. 总体架构

### 2.1 项目结构

```
rust-learning-game/
├── Cargo.toml            # workspace
├── crates/
│   ├── game-core/        # 纯逻辑，零 UI 依赖
│   │   ├── engine/       # 关卡引擎（关卡加载、状态机、XP/连击/HP 结算）
│   │   ├── validate/     # 校验层（syn 快速检查 + rustc 编译校验，统一 trait）
│   │   ├── save/         # 存档读写（serde + TOML）
│   │   └── error/        # 错误类型
│   ├── game-tui/         # ratatui 前端（实现 core 的 UiBackend trait）
│   └── game-data/        # 关卡资源（TOML + 错误映射表）
└── assets/levels/        # 关卡 TOML（随 game-data 分发）
```

### 2.2 核心接口（UI 可替换的关键）

```rust
// game-core 定义，game-tui 实现；未来 2D 前端再实现一份
pub trait UiBackend {
    fn render(&mut self, screen: &Screen) -> Result<()>;
    fn poll_input(&mut self) -> Result<Option<Input>>;
}

// Screen 是核心对 UI 的唯一数据出口（纯数据枚举，无终端概念）
pub enum Screen {
    Menu,
    ChapterMap,
    LevelView { /* 题面、代码缓冲、状态栏数据 */ },
    Feedback { /* 编译结果 */ },
    BossView,
    GameOver,
}
```

### 2.3 依赖

- **game-core**：serde、toml、thiserror、syn（+ quote，仅 syn 模式用）。rustc 校验用 `std::process::Command`，不引入额外依赖
- **game-tui**：ratatui、crossterm、unicode-width
- 均无 async 依赖（第一版纯同步，rustc 编译阻塞不超过 10s 超时）

## 3. 关卡数据模型

每关一个 TOML 文件：`levels/<chapter>/<id>.toml`（如 `levels/02-ownership/02-01-borrow.toml`）。

```toml
[meta]
id = "variables-01"
title = "Hello, Variables"
topic = "variables"            # 主题（决定章节/解锁顺序）
difficulty = 1                 # 1-3，影响 XP 倍率
kind = "fix"                   # fix(改错) | fill(填空) | quiz(普通选择题) | boss(BOSS选择题，quiz 规则+特殊机制)
source = "rustlings"           # 素材出处（许可标注）

[content]
en = "Fill in the blank so this compiles: ..."
zh = "补全代码使其通过编译：..."
hints = ["Hint: a variable needs `mut`", "提示：..."]   # 3 级，按失败次数解锁

[verify]
mode = "rustc"                 # rustc | syn
test_code = "..."              # rustc 模式：隐藏测试（rustlings 思路：玩家代码 + 隐藏 main/tests）
check = "contains"             # syn 模式：静态检查规则
pattern = ["let mut x"]        # syn 模式的匹配目标

[quiz]                         # kind = quiz / boss 时使用
question = "..."
options = ["A", "B", "C", "D"]
answer_index = 2
explanation_zh = "完整中文解析"   # 答错后展示
```

- 素材使用规则：题目文本来自 rustlings / rust-quiz（MIT / Apache-2.0 双许可），`source` 字段保留出处，README 写明许可与致谢
- BOSS 关 = rust-quiz 陷阱题，`kind = "boss"`，使用 [quiz] 表，套用 4.5 的 BOSS 特殊规则
- `kind = "quiz"` 为普通选择题（章节内的概念检查题），不套 BOSS 规则；第一版可不使用，但数据模型支持

## 4. 玩法机制

### 4.1 关卡结构

- 按主题分章，每章 3~4 普通关 + 1 BOSS 关
- 第一版 5 个主题：variables / ownership / lifetimes / traits / async
- 约 15 普通关 + 5 BOSS 关；扩展 = 新增 TOML 文件，引擎自动加载

### 4.2 XP 与等级

- 过关 XP = 难度基础值 ×（1 + 生命值加成 + 连击加成）
- 等级 1~10，只影响称号显示（如「初级拥有者」→「借用检查大师」），不影响数值

### 4.3 生命值（HP）

- 初始 3 颗心，每次「关卡内首次编译失败」扣 1 心，重复失败不重复扣
- HP 归零 → 本关失败，重试（保留进度）

### 4.4 连击（Combo）

- 连续「一次通过」计数，连击 ≥ 2 时额外 +10% XP
- 编译失败清零

### 4.5 BOSS 关特殊规则

- rust-quiz 选择题，无 HP 惩罚
- 答错直接失败重来，展示完整解析（dtolnay 题解改写为中文）

### 4.6 提示系统

- 3 级提示（题面自带 hint → 中文提示 → 答案片段）
- 第 1、2 级免费；第 3 级扣 1 心

## 5. 校验引擎（混合）

```rust
// game-core/validate/ —— 统一入口，按 verify.mode 分发
pub trait Verifier {
    fn verify(&self, player_code: &str, level: &Level) -> Result<Verification>;
}
pub enum Verification {
    Pass,
    Fail { feedback: Vec<String> },   // 已解析的游戏化提示
}
```

### 5.1 rustc 模式

1. player_code + test_code 拼成临时 crate（玩家代码 + 隐藏测试函数，rustlings 思路）
2. `cargo/rustc` 编译，超时 10s
3. 解析 stderr → 错误定位（行号 / 错误码 E0xxx / 摘要）
4. 错误码 → 中文映射表翻译
5. 返回 Pass / Fail{feedback}

### 5.2 syn 模式

1. `syn::parse_file` 失败 → 语法错误提示
2. 按 `check` 规则做 AST 检查（contains / type_matches / function_exists）

### 5.3 错误映射表

`game-data/errors.toml`：常见错误码 → 中文解释 + 学习链接（对应 Book 章节）。
第一版覆盖 rustlings 高频错误码约 20 个：E0308、E0502、E0505、E0596、E0382、E0106、E0277、E0599、E0204、E0433 等。

## 6. 存档

- 路径：`~/.local/share/rust-learning-game/save.toml`（serde + toml）
- 内容：解锁状态、每关状态（locked/unlocked/passed/star）、XP、等级、连击、统计
- 过关自动保存；写临时文件后 rename（防崩溃损坏）

## 7. TUI 界面（ratatui + crossterm）

| 界面 | 内容 |
|---|---|
| 主菜单 | 继续 / 新游戏 / 选关（已解锁）/ 退出 |
| 章节地图 | 主题列表 + 状态图标（✅/🔒/⭐），BOSS 特殊标记 |
| 关卡视图 | 上：题面（英文 + 中文折叠）；中：代码编辑器（行号、纯文本编辑 + 光标）；下：状态栏（HP、XP、Combo、提示剩余） |
| 反馈视图 | Pass：庆祝 + XP 获得；Fail：错误列表（错误码 + 中文解释 + 行号），Tab 切换错误 |
| BOSS 视图 | 四选一 + 答错显示完整解释 |
| 结算/Game Over | 本局统计（过关数、总 XP、错误次数） |

- 代码编辑器：`Vec<Vec<char>>` 文本缓冲，方向键/退格/粘贴，够用即可
- 中文渲染：`unicode-width` 校正 CJK 宽度保证对齐

## 8. 测试策略

- **game-core 单测**：关卡状态机（过关/失败/扣心/连击）、校验分发（mock Verifier）、存档读写（临时目录）
- **校验引擎集成测试**：真实调用 rustc 编译「正确答案」与「典型错误答案」样本，断言反馈含预期错误码 —— 核心质量保障
- **syn 模式测试**：AST 检查规则单元测试
- **game-tui**：轻量冒烟测试（启动 → 渲染一帧 → 退出）

## 9. 错误处理

- 校验失败是正常流程（`Verification::Fail`），不是错误
- 真错误：关卡 TOML 解析失败（启动时校验）、rustc 超时（重试一次后报环境错误）、存档损坏（备份后重建）
- 统一 `GameError` 枚举，Display 为中文，UI 直接展示

## 10. 第一版范围

### 做

- 上述全部功能
- 资源采集脚本：克隆 rustlings / rust-quiz → 抽取题目 → 生成第一版关卡 TOML

### 不做（Non-goals）

- 2D 前端（UiBackend 接口留好）
- 联机 / 排行榜 / 技能树
- 自定义关卡编辑器
- vim 模式编辑器
- 音频

## 11. 参考仓库

- rustlings：https://github.com/rust-lang/rustlings （普通关素材，MIT/Apache-2.0）
- rust-quiz：https://github.com/dtolnay/rust-quiz （BOSS 关素材）
- course.rs：https://course.rs/ （提示文案参考）
- The Book：https://doc.rust-lang.org/book/ （错误码学习链接）
