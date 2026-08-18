# T7-L1 实测记录：L1 层 5 关补齐（P4-25 Wave 3a）

日期：2026-08-16
环境：`rustc 1.97.0 (2d8144b78 2026-07-07)`，`--edition 2021`（与游戏沙盒一致）
任务：T7（P4-25）Wave 3a 按层并行生产 v3 §4.1 缺失关卡，本层负责 L1 层 5 关（16/17/18/19/21）。
方法：每关按 L3-A2 流水线 S1-S8；broken 版实测首条错误码（不抄旧表），fixed 版输出与 expect_output 在 trim+CRLF 归一化后逐字节一致。
边界：未触碰 crates/game-core/tests/levels_regression.rs、assets/errors.toml、其他层关卡文件；crates/game-data/tests/levels.rs 仅追加本层断言。

## 错误码实测表（broken 版）

| 关 | id | 素材文件 | 实测错误码（首条） | 与 v3 §4.1 表对比 |
|---|---|---|---|---|
| 16-l1-strings | l1-strings | rustlings/exercises/09_strings/strings1.rs | `E0308`（mismatched types：函数体返回 `&str`，签名要求 `String`） | 一致 |
| 17-l1-structs | l1-structs | rustlings/exercises/07_structs/structs1.rs（R1 改写，按 L3-A1 §5 草案4） | `E0063`（missing field `blue` in initializer of `ColorClassicStruct`） | 一致 |
| 18-l1-options1 | l1-options1 | rustlings/exercises/12_options/options1.rs（W1 测试搬 main） | `E0308`（mismatched types：函数体空，实际返回 `()`，要求 `Option<u16>`） | 一致 |
| 19-l1-enums | l1-enums | rustlings/exercises/08_enums/enums1.rs | `E0599`（no variant … named `Resize` found for enum `Message`） | 一致 |
| 21-l1-boss | l1-boss | 自编（Boss：move+borrow+clone 综合） | `E0596`（cannot borrow `note` as mutable, as it is not declared as mutable） | 一致 |

实测命令：`rustc --edition 2021 <starter> 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
21-l1-boss broken 版错误码全集：仅 `E0596` 一条（`uniq -c` 计数 = 1），符合 Boss「只允许一个预期错误码」验收。

## fixed 版输出表（与 expect_output 逐字节一致，trim+CRLF 归一化）

| 关 | fixed 输出（stdout 原文） |
|---|---|
| 16-l1-strings | `My current favorite color is blue` |
| 17-l1-structs | `(0, 255, 0)` |
| 18-l1-options1 | `Some(5)` + 换行 + `Some(0)` + 换行 + `None` |
| 19-l1-enums | `Resize` / `Move` / `Echo` / `ChangeColor` / `Quit`（五行） |
| 21-l1-boss | `标题：今日计划` + 换行 + `正文：写代码，然后实测` |

实测命令：`rustc --edition 2021 -o out <fixed> && ./out`；全部 fixed 版 0 warning、仅 std、edition 2021。

## 改编要点（W1-W10）

- 16/19：main 可见 bug，直用（注释中文化）；修复位 = 函数体返回类型 / 枚举变体定义。
- 17：按 L3-A1 §5 草案4 落地（E0063 缺 blue 字段，实例化搬 main）；hints 按 v3 §3.4 新规范重写（草案旧链接 course.rs 已按 §6.3 白名单替换为 rustwiki book-cn）。
- 18：原题 bug 藏在 `#[cfg(test)]`（函数体空，裸 rustc 直编即 E0308，bug 本就可见）；测试体搬 main 选 12/22/24 三点覆盖 5 勺/0 勺/None 三分支（W1/W3：Option 用 `{:?}`）。
- 21-l1-boss：自编综合关，覆盖 L1 层 ≥3 知识点——移动（move）（`new` 按值接收 String 移入结构体）、借用（`body(&self) -> &str`）、Clone（`title(&self)` 返回需 clone）、可变借用（`append(&mut self)`）；修复点唯一（`let note` → `let mut note`）；`is_boss = true`。

## 三级 hints 达标（v3 §6.2）

- description ≤80 字：16=79 / 17=78 / 18=79 / 19=68 / 21=76，均含「症状+目标输出+概念名」且无修复动作。
- hints[0] ≤40 字无代码无 API（max 33）；hints[1] ≤60 字（链接不计）+ rustwiki 白名单（ch08-02 / ch05-01 / ch06-01 / ch04-02，book-cn）；hints[2] ≤3 行代码 + ≤40 字说明（全关卡唯一修复代码位）；整关 hints ≤200 字（链接不计，max 192）。
- 术语按 §6.1：无「特征/特质/生存期/悬挂」；move→移动（move）、trait/Clone/Option 保留。
- 未用 hint_unlock（缺省手动揭示，与现有关卡一致）；source 均精确标注（rustlings=MIT only；21 标注自编）。

## Q1-Q10 自查

1. starter 编译状态：5 关均编译失败且首码即题干 bug。✓
2. 预期错误码 = 实测首条 E 码，与 v3 §4.1 表逐一核对一致。✓
3. expect_output 与 fixed 版 trim+CRLF 归一化后逐字节相等（上表实测）。✓
4. expect_output 无 `\r`、无行尾空格、行序与 fixed 一致、末尾无刻意 `\n`。✓
5. hints 第 1/2 级不含修复代码与期望值；修复代码只出现在 hints[2]。✓
6. starter 无答案痕迹：期望值（如 `blue: 0`、`Some(5)`、变体名列表）不写进 starter；TODO 已中文化。✓
7. source 精确：`rustlings (exercises/<dir>/<file>.rs, v6.5.0, MIT，[改编说明])`；21 自编标注。✓
8. edition 2021 + 仅 std：fixed 全部编译通过、无外部 crate、0 warning。✓
9. 输出确定性：全部确定（无时间戳/HashMap 序/多线程）。✓
10. quiz 型不涉及（全部 code 关）。✓

## 验证文件

全部验证代码对位于 `/tmp/t7-l1/verify/`（`NN-broken.rs` / `NN-fixed.rs`；broken 与 TOML starter_code 逐字节一致）。
