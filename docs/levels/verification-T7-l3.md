# T7-L3 实测记录：L3 层补齐 7 关（P4-25 Wave 3a）

日期：2026-08-16
环境：`rustc 1.97.0`，`--edition 2021`（与游戏沙盒一致）
任务：P4-25 按层并行补齐 v3 §4.1 表 L3 层缺失关（38-44），产出 7 关。
方法：每关按 v3 §2 流水线 S1-S8；broken 版实测首条错误码（不抄旧表），fixed 版输出与 expect_output 在 trim+CRLF 归一化后逐字节一致；40 关沿用 docs-review/L3-A1-levels.md §5 草案 8（iterators 阶乘）。

## 错误码实测表（broken 版）

| 关 | id | 素材文件 | 实测错误码 | 与 v3 §4.1 表对比 |
|---|---|---|---|---|
| 38-l3-generics | l3-generics | rustlings/exercises/14_generics/generics1.rs | `E0282`（type annotations needed，Vec<T> 的 T 无法推断） | 一致 |
| 39-l3-traits1 | l3-traits1 | rustlings/exercises/15_traits/traits1.rs | `E0046`（missing trait item，impl 缺 append_bar） | 一致 |
| 40-l3-iterators | l3-iterators | rustlings/exercises/18_iterators/iterators4.rs | `E0308`（空函数体返回 () 与 u64 不符） | 一致 |
| 41-l3-iterators2 | l3-iterators2 | rustlings/exercises/18_iterators/iterators2.rs | `E0308`（返回 Map 迭代器与 Vec<String> 不符） | 一致 |
| 42-l3-conversions | l3-conversions | rustlings/exercises/23_conversions/conversions1.rs | `E0277`（f64 与 usize 不能相除） | 一致 |
| 43-l3-enums3 | l3-enums3 | rustlings/exercises/08_enums/enums3.rs | 无 E 码（broken 编译通过） | 记录实况：逻辑修复关（§4.1 表为输出比对） |
| 44-l3-boss | l3-boss | 自编（lifetime+generic+trait 综合） | `E0106`（Item<T> 的 name 字段缺生命周期标注；**broken 全量仅此一个 E 码**） | 一致 |

实测命令：`rustc --edition 2021 <starter> 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`

## fixed 版输出表（与 expect_output 逐字节一致）

| 关 | fixed 输出（stdout 原文，trim 后） |
|---|---|
| 38-l3-generics | `[42, -1]` |
| 39-l3-traits1 | `s: FooBar` + 换行 + `s: BarBar` |
| 40-l3-iterators | `0! = 1` + 换行 + `5! = 120` + 换行 + `10! = 3628800` |
| 41-l3-iterators2 | `Hello` + 换行 + `["Hello", "World"]` |
| 42-l3-conversions | `7.125` |
| 43-l3-enums3 | `Move to (1, 2)` + 换行 + `Echo: Hello world!` + 换行 + `Color: (255, 0, 255)` + 换行 + `Quit: true` |
| 44-l3-boss | `1984: 42` |

实测命令：`rustc --edition 2021 -o out <fixed> && ./out`

## 43 无 E 码关实况

- **43-l3-enums3**：broken 编译通过（unused_variables/dead_code warning），运行输出全部初始状态
  `Move to (0, 0)` / `Echo: ` / `Color: (0, 0, 0)` / `Quit: false` —— process 方法体为空。fixed（match 四分支更新状态）输出上表四行。按「逻辑/运行期修复关」验收：编译成功 + 输出比对失败 → 玩家补 process 的 match。

## 改编要点（W1-W10）

- 39/40/41/43：原题 bug 藏在 `#[cfg(test)]` 内（39 测试体含两条断言）→ W1 测试体搬 main、W2 assert→println；43 裁去 Resize 变体与辅助方法，断言改为 main 打印状态。
- 38/42：main 可见 bug，直用（注释中文化）。
- 40：按 L3-A1 §5 草案 8 落地（草案 course.rs 链接已按 §6.3 白名单换成 rustwiki ch13-02）。
- 41：capitalize_first 预先补全为题干（演示 chars/next 模式），保留 capitalize_words_vector 缺 `.collect()` 的函数体 bug，broken 首码 E0308。
- 44（Boss）：`is_boss = true`；覆盖生命周期（bug）+ 泛型（Item<T>）+ trait（Summarize 及 impl/方法调用）三知识点，修复点只有一处（结构体补 `'a`），broken 全量仅 1 个 E0106；trait impl 不依赖结构体生命周期，修结构体后无连锁编译错误。
- 全部 fixed 版 0 error；edition 2021、仅 std、无外部 crate；输出确定（无时间戳/HashMap 序/多线程）。
- 三级 hints 达标：hints[0] 概念（≤40 字无代码无 API）、hints[1] rustwiki 白名单链接（ch10-01/ch10-02/ch13-02/ch06-01/ch10-03/types-cast，全部实测 200）、hints[2] 唯一修复代码位（≤3 行 + ≤40 字说明）；description ≤80 字。

## Q1-Q10 自查

1. starter 编译状态符合题型：6 关编译失败且首码即题干 bug；1 关（43）编译通过但输出错误（逻辑修复型）。✓
2. 预期错误码 = 实测首条 E 码；无 E 码题（43）为逻辑修复关，已在表内记录实况。✓
3. expect_output 与 fixed 版 trim+CRLF 归一化后逐字节相等（上表）。✓
4. expect_output 无 `\r`、无行尾空格、行序与 fixed 一致、末尾无刻意 `\n`。✓
5. hints 第 1/2 级不含修复代码与期望值；未用 hint_unlock（缺省手动揭示，与现有关卡一致）。✓
6. starter 无答案痕迹：grep 不到期望输出行；TODO 已中文化。✓
7. source 精确：rustlings 六关标 `rustlings (exercises/<dir>/<file>.rs, v6.5.0, MIT，[改编说明])`，Boss 44 标 `自编`。✓
8. edition 2021 兼容 + 仅 std：fixed 全部编译通过、无外部 crate。✓
9. 输出确定性：全部确定。✓
10. quiz 型不涉及（全部 code 关）。✓

## 验证文件

broken/fixed 代码对在 `/tmp/t7l3/`（`g38_*` 至 `g44_*`），starter 与 TOML 内逐字节一致（脚本从 TOML 提取 starter_code 编译复测）。
