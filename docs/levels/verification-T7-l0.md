# T7 实测记录：L0 层 +5 关（P4-25 Wave 3a）

日期：2026-08-16
环境：`rustc 1.97.0 (2d8144b78 2026-07-07)`，`--edition 2021`（与游戏沙盒一致）
任务：P4-25 按层补齐 v3 §4.1 表缺失的 L0 层 5 关（05/06/07/08/09）。
方法：每关按 v3 §2 流水线 S1-S8；S5 错误码本机实测（不抄旧表）；S4 fixed 版输出与 expect_output 在 trim+CRLF 归一化后逐字节一致（xxd 核对无 `\r`、无行尾空格）。

## S2 三查结论

| 关 | 素材文件 | fn main | cfg(test) | 外部依赖 | 改写策略 |
|---|---|---|---|---|---|
| 05 | rustlings/exercises/01_variables/variables2.rs | 1 | 0 | 无 | 直用（TODO 中文化，W7） |
| 06 | rustlings/exercises/03_if/if1.rs | 1（空体） | 1 | 无 | W1：测试体搬 main（3 断言 → 2 println） |
| 07 | rustlings/exercises/04_primitive_types/primitive_types1.rs | 1 | 0 | 无 | 直用（TODO 中文化，W7） |
| 08 | rustlings/exercises/02_functions/functions3.rs | 1 | 0 | 无 | 直用（TODO 中文化，W7） |
| 09 | 自编（综合 L0：变量+函数+分支） | 1 | 0 | 无 | 自编（未定义变量名制造 E0425） |

## 错误码实测表（broken 版，S5）

实测命令：`rustc --edition 2021 <starter> 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`；全部单错误码（`grep -c 'error\['` = 1）。

| 关 | id | 实测错误码 | 与 v3 §4.1 表对比 |
|---|---|---|---|
| 05-l0-variables2 | l0-variables2 | `E0283`（type annotations needed，let x; 无法推断） | 一致 |
| 06-l0-if | l0-if | `E0308`（mismatched types：函数体为空返回 `()`，期望 `i32`） | 一致 |
| 07-l0-primitives | l0-primitives | `E0425`（cannot find value `is_evening` in this scope） | 一致 |
| 08-l0-functions2 | l0-functions2 | `E0061`（this function takes 1 argument but 0 arguments were supplied） | 一致 |
| 09-l0-boss | l0-boss | `E0425`（cannot find value `total` in this scope） | 一致 |

## fixed 版输出表（S4，与 expect_output 逐字节一致）

实测命令：`rustc --edition 2021 -o out <fixed> && ./out`；全部 0 warning。

| 关 | fixed 输出（stdout 原文，trim 后） | expect_output |
|---|---|---|
| 05-l0-variables2 | `x is ten!` | `x is ten!` |
| 06-l0-if | `10` + 换行 + `42` | `10\n42` |
| 07-l0-primitives | `Good morning!` | `Good morning!` |
| 08-l0-functions2 | `Ring! Call number 1` + 换行 + `Ring! Call number 2` + 换行 + `Ring! Call number 3` | `Ring! Call number 1\nRing! Call number 2\nRing! Call number 3` |
| 09-l0-boss | `总和：15` | `总和：15` |

说明：07 的 fixed 输出只有 `Good morning!` 一行——按 rustlings 原题语义 `is_evening = !is_morning = false`，第二个 if 分支不打印（与官方题行为一致，直用不改语义）。

## 改编要点（W1-W10）

- 06：原题 bug 藏在 `#[cfg(test)]` 内（裸 rustc 直编不可见）→ W1 测试体搬 main，3 条断言（10>8 / 42>32 / 相等）收敛为 2 条 println（10 与 42，覆盖 a>b 与 a<b 两分支）；相等分支语义在 hints 说明。
- 05/07/08：main 可见 bug，直用；TODO 注释中文化（W7）。
- 09：自编，broken 只含一个 E 码（E0425），满足 §4.2「Boss broken 版只允许一个预期错误码」；is_boss=false（§4.2 裁决：L0 不设 Boss，09 按普通综合关处理）。
- 全部 fixed 版 0 warning；edition 2021、仅 std、无外部 crate、无 unsafe/thread；输出确定（无时间戳/HashMap 序/多线程）。
- 三级 hints 达标（§6.2 逐字数核验）：hints[0] 概念 ≤40 字无代码无 API、hints[1] 定位 ≤60 字（链接不计）+ rustwiki book-cn 白名单链接（ch03-01/ch03-02/ch03-03/ch03-05，全部实测 HTTP 200）、hints[2] 唯一修复代码位（≤3 行 + 说明 ≤40 字）、整关 hints ≤200 字。

## Q1-Q10 自查

1. starter 编译状态符合题型：5 关全部编译失败且首码即题干 bug（E0283/E0308/E0425/E0061/E0425）。✓
2. 预期错误码 = 实测首条 E 码（上表，全部单码）。✓
3. expect_output 与 fixed 版 trim+CRLF 归一化后逐字节相等（xxd 核对）。✓
4. expect_output 无 `\r`、无行尾空格、行序与 fixed 一致（06/08 多行逐行核对）、末尾无刻意 `\n`。✓
5. hints 第 1/2 级不含修复代码与期望值（hints[1] 用「add 的返回值存在哪个变量？」式提问，不点名 sum）；未用 hint_unlock（缺省手动揭示，与存量关卡一致）。✓
6. starter 无答案痕迹：TOML 内 starter_code 与 broken 实测版逐字节一致；grep 不到修复写法（`let x = 10;`/`if a > b`/`!is_morning`/`call_me(3)`/`sum`）；打印消息本身（x is ten!/Good morning!）为题目骨架，非答案泄漏；TODO 已中文化。✓
7. source 精确：`rustlings (exercises/<dir>/<file>.rs, v6.5.0, MIT，[改编说明])`；09 自编标注「自编（…，无原题）」。✓
8. edition 2021 兼容 + 仅 std：fixed 全部编译通过 0 warning、无外部 crate。✓
9. 输出确定性：全部确定（单线程、无集合迭代序依赖）。✓
10. quiz 型不涉及（全部 code 关）。✓

## 验证文件

全部验证代码对位于 `/tmp/t7-l0-verify/`（`NN-id-broken.rs` / `NN-id-fixed.rs`，starter 与 TOML 内 starter_code 逐字节一致）。
