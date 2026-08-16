# 现有关卡实测记录（P1-07 T1 收尾）

- 实测环境：rustc 1.97.0 (2d8144b78 2026-07-07)，`--edition 2021`，与游戏沙盒一致
- 方法：每关提取 `starter_code` 写入 /tmp（不污染仓库）→ 编译取首条 `error[E…]`；再按 hints[2]/题面重建 fixed 版，编译运行捕获 stdout，与 `expect_output` 经 trim + CRLF（`\r\n`→`\n`）归一化后逐字节比对
- 结论：15 关 fixed 输出与 expect_output 全部一致 ✓；broken 行为均与题面/hints 描述吻合，**无需修改任何 TOML 内容**

## 汇总表

| 关号 | id | broken 实测 | v3 §4.1 表预期 | 一致 | fixed 输出 vs expect_output |
|---|---|---|---|---|---|
| 00 | l0-hello | E0425 | E0425 | ✓ | ✓ 一致 |
| 01 | l0-print | 无 E 码 | —（无 E 码 → EUNKNOWN 兜底） | ✓ | ✓ 一致 |
| 02 | l0-function | E0425 | E0425 | ✓ | ✓ 一致 |
| 03 | l0-loop | 编译通过，运行不符 | — | ✓ | ✓ 一致 |
| 10 | l1-move | E0596 | E0596 | ✓ | ✓ 一致 |
| 11 | l1-borrow | E0382 | E0308 | ✗（记录差异） | ✓ 一致 |
| 12 | l1-mut-borrow | E0382 | E0596 | ✗（记录差异） | ✓ 一致 |
| 13 | l1-clone | E0382 | — | ✗（记录差异） | ✓ 一致 |
| 22 | l2-vec | 编译通过，运行不符 | —（运行期 panic 越界） | ✓ | ✓ 一致 |
| 23 | l2-option | 编译通过，运行不符 | — | ✓ | ✓ 一致 |
| 24 | l2-result | 编译通过，运行不符 | —（运行期 panic unwrap） | ✓ | ✓ 一致 |
| 34 | l3-lifetime | E0106 | E0106 | ✓ | ✓ 一致 |
| 35 | l3-trait | E0599 | — | ✗（记录差异） | ✓ 一致 |
| 45 | l4-drop-order | 编译通过，运行不符 | — | ✓ | ✓ 一致 |
| 46 | l4-lifetime-trap | E0597 | — | ✗（记录差异） | ✓ 一致 |

> 注：5 处与 v3 §4.1 表不符，均为设计文档表格对存量关的旧预期（11/12 错误码不同、13/35/46 表记「编译通过」而实测编译失败）；实际 broken 行为与**各关 hints[1] 所述错误码一致**（hints 无错，无需改 TOML）。差异详情见各关条目。

---

## l0-hello（00-l0-hello.toml）
- broken 错误码：E0425
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/00-l0-hello-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  x has the value 5
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'x has the value 5'`）

## l0-print（01-l0-print.toml）
- broken 错误码：无 E 码（格式串占位符与参数数量不匹配，非 error[E…]，属 EUNKNOWN 兜底类）
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/01-l0-print-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  1 + 2 = 3
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'1 + 2 = 3'`）

## l0-function（02-l0-function.toml）
- broken 错误码：E0425
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/02-l0-function-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  Call me
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'Call me'`）

## l0-loop（03-l0-loop.toml）
- broken 错误码：编译通过（输出 10 ≠ 目标 15，运行期不符）
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/03-l0-loop-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  15
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'15'`）

## l1-move（10-l1-move.toml）
- broken 错误码：E0596
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/10-l1-move-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  vec1 has length 3 content [22, 44, 66]
  vec1 has length 4 content [22, 44, 66, 88]
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'vec1 has length 3 content [22, 44, 66]\nvec1 has length 4 content [22, 44, 66, 88]'`）

## l1-borrow（11-l1-borrow.toml）
- broken 错误码：E0382
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/11-l1-borrow-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- 差异说明：v3 §4.1 表预期 E0308，实测 E0382；hints[1] 所述错误码与实测一致，TOML 无需修正。
- fixed 输出：
  ```
  The length of 'hello' is 5.
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`"The length of 'hello' is 5."`）

## l1-mut-borrow（12-l1-mut-borrow.toml）
- broken 错误码：E0382（首条；同次编译亦报 E0596，与 hints[1]「E0382/E0596」一致）
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/12-l1-mut-borrow-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- 差异说明：v3 §4.1 表预期 E0596，实测 E0382；hints[1] 所述错误码与实测一致，TOML 无需修正。
- fixed 输出：
  ```
  hello world
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'hello world'`）

## l1-clone（13-l1-clone.toml）
- broken 错误码：E0382
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/13-l1-clone-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- 差异说明：v3 §4.1 表记 —，实测 broken 编译失败；hints[1] 所述错误码与实测一致，TOML 无需修正。
- fixed 输出：
  ```
  hello hello
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'hello hello'`）

## l2-vec（22-l2-vec.toml）
- broken 错误码：编译通过（运行期 panic：index out of bounds，len 3 index 3）
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/22-l2-vec-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  3
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'3'`）

## l2-option（23-l2-option.toml）
- broken 错误码：编译通过（输出 none ≠ 目标 3，运行期不符）
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/23-l2-option-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  3
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'3'`）

## l2-result（24-l2-result.toml）
- broken 错误码：编译通过（运行期 panic：Result::unwrap on Err）
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/24-l2-result-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  解析失败
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'解析失败'`）

## l3-lifetime（34-l3-lifetime.toml）
- broken 错误码：E0106
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/34-l3-lifetime-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  The longest string is 'long string is long'
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`"The longest string is 'long string is long'"`）

## l3-trait（35-l3-trait.toml）
- broken 错误码：E0599
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/35-l3-trait-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- 差异说明：v3 §4.1 表记 —，实测 broken 编译失败；hints[1] 所述错误码与实测一致，TOML 无需修正。
- fixed 输出：
  ```
  area: 20
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'area: 20'`）

## l4-drop-order（45-l4-drop-order.toml）
- broken 错误码：编译通过（输出 drop 1/drop 2/end，行序不符）
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/45-l4-drop-order-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- fixed 输出：
  ```
  end
  drop 2
  drop 1
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'end\ndrop 2\ndrop 1'`）

## l4-lifetime-trap（46-l4-lifetime-trap.toml）
- broken 错误码：E0597
- 实测命令：`rustc --edition 2021 /tmp/rlg-verify/46-l4-lifetime-trap-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`
- 差异说明：v3 §4.1 表记 —，实测 broken 编译失败；hints[1] 所述错误码与实测一致，TOML 无需修正。
- fixed 输出：
  ```
  hi
  ```
- 与 expect_output 一致 ✓（trim + CRLF 归一化后逐字节比对：`'hi'`）

---
## 验收断言结果

| # | 断言 | 结果 |
|---|---|---|
| A1 | 15 关 hints 数组长度 = 3 | 15/15 ✓ |
| A2 | hints[1] URL ∈ 白名单（rustwiki book/rust-by-example） | 15/15 ✓（11 个唯一 URL 实测 HTTP 200） |
| A3 | description+hints[0..1] 无 `clone(` / `&mut` / `let mut` / `match` / `unwrap` | 15/15 ✓ |
| A4 | source 字段格式 `<仓库> (<相对路径>, <版本/主题>, <许可后缀>，[改编说明])` | 15/15 ✓（自编关用「主题+无原题」变体；34 混合来源用 `A + B` 并列并注明改编） |
| A5 | 24-l2-result / 13-l1-clone description 无「用 match 处理」「使用 s1.clone()」类修复表述 | ✓ |
| A6 | cargo test --workspace | 72 passed（game-core 65 + game-data 4 + game-ui 3），0 failed |

### source 字段清单

- 00-l0-hello: `rustlings (exercises/01_variables/variables1.rs, v6.5.0, MIT)`
- 01-l0-print: `自编 (格式化输出占位符练习，无原题)`
- 02-l0-function: `rustlings (exercises/02_functions/functions1.rs, v6.5.0, MIT)`
- 03-l0-loop: `自编 (循环范围练习，无原题)`
- 10-l1-move: `rustlings (exercises/06_move_semantics/move_semantics1.rs, v6.5.0, MIT，push 改 22/44/66 并补 main 打印)`
- 11-l1-borrow: `The Rust Book (ch04-02, 引用与借用, MIT/Apache-2.0)`
- 12-l1-mut-borrow: `自编 (可变借用练习，无原题)`
- 13-l1-clone: `rustlings (exercises/06_move_semantics/move_semantics5.rs, v6.5.0, MIT，主题改编)`
- 22-l2-vec: `自编 (Vec 下标越界练习，无原题)`
- 23-l2-option: `自编 (Option 与 get 练习，无原题)`
- 24-l2-result: `自编 (Result 错误处理练习，无原题)`
- 34-l3-lifetime: `rustlings (exercises/16_lifetimes/lifetimes1.rs, v6.5.0, MIT) + The Rust Book (ch10-03, 生命周期示例, MIT/Apache-2.0，混合改编)`
- 35-l3-trait: `自编 (trait 实现练习，无原题)`
- 45-l4-drop-order: `rust-quiz (questions/012-binding-drop-behavior.rs, CC BY-SA 4.0，主题改编，解释自写)`
- 46-l4-lifetime-trap: `rust-quiz (questions/037-lifetime-extension.rs, CC BY-SA 4.0，主题改编，解释自写)`
