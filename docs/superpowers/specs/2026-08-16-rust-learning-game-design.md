# Rust 学习游戏 — 设计文档 v2（修订版）

日期：2026-08-16（修订）
状态：用户逐项确认（UI/编辑器/沙盒/关卡结构/校验流程/计划切分）

## 1. 概述

闯关式学习游戏：玩家编写 Rust 代码，系统校验（编译 → 运行 → 输出比对），推进关卡。
面向从零到初级、被所有权/借用/生命周期困扰的学习者。核心目标：**把编译错误、借用检查报错包装成游戏反馈，而不是冷冰冰的报错文本**。

## 2. 技术选型

| 模块 | 选择 | 说明 |
|---|---|---|
| 图形 UI | **macroquad 0.4 + egui-macroquad 0.17（egui 0.31）** | 2D 游戏层（通关动画）+ 立即模式 UI |
| 代码编辑器 | **自研轻量**：egui TextEdit + 自定义 layouter + syn 分词着色（行号、关键字/注释/字符串着色） | egui_code_editor 0.3.8 需 egui ^0.36，与 egui-macroquad 锁定的 ^0.31 冲突，弃用 |
| 校验 | rustc 子进程：编译 → 运行 → stdout 比对；错误解析**基于错误码 E0xxx**，不匹配字符串 | 抗 rustc 版本差异 |
| 数据 | serde + toml（关卡文件、存档） | |
| 沙盒 | **bwrap（bubblewrap）真隔离**（已安装，无需 root）；开发期兜底 = timeout + 临时目录 + syn 静态拦截 | firejail 未安装且无 apt |
| 备选 | ratatui 终端版 | UiBackend 抽象保留，未来可新增实现 |

## 3. 总体架构

```
rust-learning-game/
├── Cargo.toml            # workspace
├── crates/
│   ├── game-core/        # 纯逻辑，零 UI 依赖
│   │   ├── engine/       # 关卡状态机、解锁、得分
│   │   ├── validate/     # 校验层（compile→run→compare、错误解析、错误码映射）
│   │   ├── sandbox/      # 执行环境抽象（开发期兜底实现；计划②加 bwrap 实现）
│   │   ├── editor/       # syn 分词 → 着色片段（纯逻辑，可测）
│   │   ├── save/         # 存档读写
│   │   └── error/        # 错误类型
│   ├── game-ui/          # macroquad + egui 前端（实现 UiBackend）
│   └── game-data/        # 关卡 TOML 资源 + errors.toml
└── assets/levels/        # 关卡 TOML（随 game-data 分发）
```

核心接口（UI 可替换的关键）：

```rust
pub trait UiBackend {
    fn run(&mut self, app: &mut GameApp) -> Result<(), GameError>; // 事件循环由后端驱动
}
```

- `Screen` / `Input` 纯数据枚举在 game-core 定义（无终端/窗口概念）
- `GameApp`（core）持有引擎 + 界面状态，处理 `Input` 产出 `Screen`
- 未来 ratatui 版 = 新增一个实现 `UiBackend` 的 crate

## 4. 关卡数据模型

TOML，L0-L4 难度分层。字段（用户规格 + 必要补充）：

```toml
[[level]]
id = "ownership_01"
title = "所有权转移"
tier = "l1"                    # l0..l4
description = "修复代码，使程序正常编译运行，理解 move 语义"
hint = "当把 String 传给函数，所有权会发生转移"
hints = ["提示1", "提示2（章节链接）", "提示3（片段代码）"]  # 多级提示，计划②启用
starter_code = '''
fn main() {
    let s = String::from("hello");
    take(s);
    println!("{}", s);
}
fn take(x: String) {}
'''
expect_output = ""
allow_compile_fail = false
expect_error_code = ""         # allow_compile_fail=true 时必填
source = "rustlings"           # 素材出处（许可标注）
```

字段语义：
- `starter_code`：玩家打开关卡时默认填充的代码（通常有错）
- `expect_output`：运行 stdout 预期输出；`""` = 只要求编译通过
- `allow_compile_fail=false`：必须编译成功；`=true`：任务就是写出指定编译错误（`expect_error_code`），用于专门理解借用检查报错

## 5. 关卡分层（L0-L4）

| 层 | 内容 | 素材 |
|---|---|---|
| L0 入门 | 变量、基础类型、函数、控制流，语法改错填空 | rustlings 基础 exercise |
| L1 所有权核心 | move 语义、拷贝类型、借用 & 可变借用（**游戏重点**，大量改错） | rustlings move_semantics/ownership |
| L2 集合与错误处理 | 集合、Option、Result、模式匹配 | rustlings vecs/option/result/errors |
| L3 难点 | 生命周期、trait、泛型（可做 BOSS 关） | rustlings lifetimes/traits/generics |
| L4 挑战 | rust-quiz 陷阱题（选择题 → 挑战关） | rust-quiz |

- 关卡按 L0→L4 顺序解锁（线性推进）
- 第一版内容量：L0~L2 各 3~4 关 + L3 2 关 + L4 2 关（挑战），共约 15 关

## 6. 校验流程（compile → run → compare）

1. 玩家代码写入临时目录
2. 编译：`rustc --edition 2021`（仅 std，无外部 crate）；开发期 timeout 10s 兜底
3. 编译失败分支：
   - `allow_compile_fail=true`：提取错误码 → `== expect_error_code` 则通关，否则失败
   - 否则：解析错误码/行号/摘要 → 错误码映射表 → 游戏化中文反馈
4. 编译成功 → 运行二进制（timeout 2s）→ 比对 stdout 与 `expect_output`
5. 全匹配 → 通关（经验 + 解锁下一关 + 自动存档）

## 7. 错误解析器

- 输入 rustc stderr → 结构化输出：是否成功 / 错误码 E0xxx / 行号 / 摘要
- 映射表 `errors.toml`：错误码 → 面向新手的通俗解释 + Book/course.rs 章节链接
- **只依赖错误码匹配，不匹配报错字符串**（rustc 版本会微调文本，错误码稳定）

## 8. 界面（macroquad + egui）

1. 主菜单：章节选择、显示已解锁关卡、玩家进度/得分
2. 关卡界面布局：
   - 左上：关卡标题、任务描述、提示按钮
   - 主体：代码编辑区（行号 + 语法着色）
   - 下方按钮：【提交运行】【显示提示】【重置代码】
   - 底部：反馈面板
3. 提交结果分支：
   - ✅ 通关：简短动画、增加经验、解锁下一关、保存存档
   - ❌ 编译错误：不直接抛 rustc 原文，解析改写为通俗人话（错误码 + 解释 + 行号 + 章节链接）
   - ❗ 运行时 panic：捕获 panic 信息并提示
   - ⚠️ 编译成功但输出不符：提示编译通过、输出不符合要求

## 9. 存档

- serde + toml，用户配置目录：`~/.local/share/rust-learning-game/save.toml`
- 内容：已通关 id、经验、解锁状态、关卡完成时间
- 通关自动保存；写临时文件后 rename（防崩溃损坏）

## 10. 沙盒（计划②，bwrap 真隔离）

- `bwrap` 参数要点：`--unshare-all`（用户命名空间）、系统目录只读挂载（--ro-bind）、tmpfs 工作区、最小化 /proc /dev、禁网络、禁写主目录
- 叠加：`timeout`（编译 10s / 运行 2s）+ 内存限制（ulimit -v）
- 开发期兜底（计划①）：timeout + 临时目录 + syn 静态扫描拦截 `std::fs` / 网络相关 API
- **风险：用户命名空间需内核支持，实现时先验证 `bwrap --unshare-all true` 能否运行**

## 11. 迭代路线（两个实施计划）

- **计划①（P1+P2，MVP → 完整关卡系统）**：
  - macroquad+egui 窗口与界面（菜单/关卡/反馈）
  - 轻量代码编辑器（TextEdit + 行号 + syn 着色）
  - TOML 关卡加载系统（L0-L4 分层）
  - compile → run → compare 校验闭环 + 错误码解析映射
  - 存档系统
  - 开发期安全兜底（timeout + 临时目录 + syn 拦截）
  - 产出：完整可玩的 15 关游戏
- **计划②（P3+P4，安全与玩法增强）**：
  - bwrap 真隔离沙盒
  - 多级提示系统
  - 「制造指定编译错误」特色关卡（数据模型已支持，计划②补充关卡内容）
  - 计分、经验、关卡完成时间展示
  - 自定义关卡导入（外部 TOML 关卡目录）

## 12. 潜在坑点（已确认的约束）

1. rustc 版本差异：错误解析只依赖错误码 E0xxx，不匹配字符串
2. 玩家代码仅 std，禁止外部 crate（裸 rustc，无 cargo 依赖解析）
3. 每次编译有耗时（约 1-2s）：界面需 loading 提示
4. 沙盒是硬门槛：不做 bwrap 隔离前，游戏不得公开分发（开发期本地可跑）

## 13. 素材与许可

- rustlings（MIT / Apache-2.0）：普通关素材，改写任务描述、保留初始错误代码
- rust-quiz：选择题转化为 L4 挑战关
- The Book / course.rs：提示文本来源，**改写精简，不大段复制**
- 关卡 TOML 保留 `source` 字段标注出处；README 写明许可与致谢

## 14. 测试策略

- **game-core 单测**：错误解析器（fixture stderr 样本）、TOML 关卡解析（合法/非法）、存档读写（临时目录）、关卡状态机（解锁/通关）、syn 拦截规则、着色 tokenizer
- **校验集成测试**：真实 rustc 编译「正确代码」与「典型错误代码」样本，断言错误码提取正确；运行比对 stdout
- **game-ui**：轻量冒烟测试（窗口初始化 → 渲染一帧 → 退出）
