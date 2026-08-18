# T3 实测记录：rustlings 三章 +8 关（P2-13）

日期：2026-08-16
环境：`rustc 1.97.0`，`--edition 2021`（与游戏沙盒一致）
任务：P2-13 内容扩展第一批（rustlings 06_move_semantics / 13_error_handling / 16_lifetimes / 05_vecs），产出 8 关。
方法：每关按 v3 §2 流水线 S1-S8；broken 版实测首条错误码（不抄旧表），fixed 版输出与 expect_output 在 trim+CRLF 归一化后逐字节一致。

## 错误码实测表（broken 版）

| 关 | id | 素材文件 | 实测错误码 | 与 v3 §4.1 表对比 |
|---|---|---|---|---|
| 14-l1-move2 | l1-move2 | rustlings/exercises/06_move_semantics/move_semantics2.rs | `E0382`（use of moved value: vec0） | 一致 |
| 15-l1-move3 | l1-move3 | rustlings/exercises/06_move_semantics/move_semantics3.rs | `E0596`（cannot borrow `vec` as mutable） | 一致 |
| 25-l2-errors3 | l2-errors3 | rustlings/exercises/13_error_handling/errors3.rs | `E0277`（main 内 `?` 但 main 返回 `()`） | 一致 |
| 26-l2-errors2 | l2-errors2 | rustlings/exercises/13_error_handling/errors2.rs | `E0369`（Result 不能与整数相乘） | 一致 |
| 28-l2-errors4 | l2-errors4 | rustlings/exercises/13_error_handling/errors4.rs | 无 E 码（broken 编译通过） | 记录实况：逻辑/运行期修复关 |
| 29-l2-vecs2 | l2-vecs2 | rustlings/exercises/05_vecs/vecs2.rs | 无 E 码（broken 编译通过） | 记录实况：逻辑/运行期修复关 |
| 36-l3-lifetime3 | l3-lifetime3 | rustlings/exercises/16_lifetimes/lifetimes3.rs | `E0106`（missing lifetime specifier ×2） | 一致 |
| 37-l3-lifetime1 | l3-lifetime1 | rustlings/exercises/16_lifetimes/lifetimes1.rs | `E0106`（missing lifetime specifier） | 一致 |

实测命令：`rustc --edition 2021 <starter> 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`

## fixed 版输出表（与 expect_output 逐字节一致）

| 关 | fixed 输出（stdout 原文，trim 后） |
|---|---|
| 14-l1-move2 | `vec0 has length 3 content [22, 44, 66]` + 换行 + `vec1 has length 4 content [22, 44, 66, 88]` |
| 15-l1-move3 | `[22, 44, 66, 88]` |
| 25-l2-errors3 | `You now have 59 tokens.` |
| 26-l2-errors2 | `171` |
| 28-l2-errors4 | `Ok(PositiveNonzeroInteger(10))` + 换行 + `Err(Negative)` + 换行 + `Err(Zero)` |
| 29-l2-vecs2 | `[4, 8, 12, 16, 20]` |
| 36-l3-lifetime3 | `1984 by George Orwell` |
| 37-l3-lifetime1 | `abcd` + 换行 + `1234` |

实测命令：`rustc --edition 2021 -o out <fixed> && ./out`

## 28/29 无 E 码关实况

- **28-l2-errors4**：broken 编译通过（仅 dead_code warning：`Negative`/`Zero` 从未构造），运行输出
  `Ok(PositiveNonzeroInteger(10))` / `Ok(PositiveNonzeroInteger(18446744073709551606))` / `Ok(PositiveNonzeroInteger(0))` —— 负数被 `as u64` 强转成巨大值、0 也被放行，输出错误。fixed（`value.cmp(&0)` 三分支）输出 `Ok(...)` / `Err(Negative)` / `Err(Zero)`。按「逻辑/运行期修复关」验收：编译成功 + 输出比对失败 → 玩家修 new 的函数体。
- **29-l2-vecs2**：broken 编译通过（unused_mut + unused_variables warning），运行输出 `[]` —— 循环体为空。fixed（`output.push(2 * element);`）输出 `[4, 8, 12, 16, 20]`。同样走输出比对验收。

## 改编要点（W1-W10）

- 14/15/26/28/29/37：原题 bug 藏在 `#[cfg(test)]` 内，裸 rustc 直编不可见 → W1 测试体搬 main、W2 assert→println、W3 集合用 `{:?}`。
- 25/36：main 可见 bug，直用（25 注释中文化、36 直用）。
- 全部 fixed 版 0 warning；edition 2021、仅 std、无外部 crate；输出确定（无时间戳/HashMap 序/多线程）。
- 三级 hints 达标：hints[0] 概念（≤40 字无代码）、hints[1] rustwiki book-cn 白名单链接（ch03-01/ch04-01/ch08-01/ch09-02/ch10-03，全部实测 200）、hints[2] 唯一修复代码位（≤3 行）；整关 hints ≤200 字。

## Q1-Q10 自查

1. starter 编译状态符合题型：6 关编译失败且首码即题干 bug；2 关（28/29）编译通过但输出错误（逻辑修复型）。✓
2. 预期错误码 = 实测首条 E 码；无 E 码题（28/29）为逻辑修复关，已在表内记录实况。✓
3. expect_output 与 fixed 版 trim+CRLF 归一化后逐字节相等（上表）。✓
4. expect_output 无 `\r`、无行尾空格、行序与 fixed 一致、末尾无刻意 `\n`。✓
5. hints 第 1/2 级不含修复代码与期望值；未用 hint_unlock（缺省手动揭示，与现有关卡一致）。✓
6. starter 无答案痕迹：grep 不到期望值/修复写法；TODO 已中文化。✓
7. source 精确：`rustlings (exercises/<dir>/<file>.rs, v6.5.0, MIT，[改编说明])`，代码直搬自 rustlings（MIT，README 已有署名）。✓
8. edition 2021 兼容 + 仅 std：fixed 全部编译通过、无外部 crate。✓
9. 输出确定性：全部确定。✓
10. quiz 型不涉及（全部 code 关）。✓

## 验证文件

全部验证脚本/代码对位于 `/tmp/t3-verify/`（`NN-id-broken.rs` / `NN-id-fixed.rs` / `NN-id-starter.rs`，starter 与 TOML 内逐字节一致）。
