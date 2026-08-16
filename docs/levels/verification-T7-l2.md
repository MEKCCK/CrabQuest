# T7（Wave 3a）实测记录：L2 层补齐 4 关（P4-25）

日期：2026-08-16
环境：`rustc 1.97.0`，`--edition 2021`（与游戏沙盒一致）
任务：P4-25 53 关全量生产 Wave 3a —— L2 层补齐 30/31/32/33 四关。
方法：每关按 v3 §2 流水线 S1-S8；broken 版实测首条错误码（不抄旧表），fixed 版输出与 expect_output 在 trim+CRLF 归一化后逐字节一致。
验证文件：`/tmp/t7-l2-verify/`（`NN-broken.rs` / `NN-fixed.rs`，starter 与 TOML 内逐字节一致）。

## 错误码实测表（broken 版）

| 关 | id | 素材文件 | 实测错误码 | 与 v3 §4.1 表对比 |
|---|---|---|---|---|
| 30-l2-hashmap | l2-hashmap | rustlings/exercises/11_hashmaps/hashmaps1.rs | `E0425`（cannot find value `basket` in this scope，同码 2 处） | 一致 |
| 31-l2-strings2 | l2-strings2 | rustlings/exercises/09_strings/strings2.rs | `E0308`（mismatched types: expected `&str`, found `String`） | 一致 |
| 32-l2-match | l2-match | 自编（match 穷尽少分支） | `E0004`（non-exhaustive patterns: `West` not covered） | 一致 |
| 33-l2-boss | l2-boss | 自编（Boss：Option+Result+match） | `E0308`（mismatched types: expected `u32`, found `&str`，仅此 1 码） | 一致 |

实测命令：`rustc --edition 2021 <starter> 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1`

Boss 33 错误码唯一性核验：`rustc --edition 2021 33-broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | sort | uniq -c` → `1 error[E0308]`（符合 §4.2「broken 版只允许一个预期错误码」）。

## fixed 版输出表（与 expect_output 逐字节一致）

| 关 | fixed 输出（stdout 原文，trim 后） | 修复点 |
|---|---|---|
| 30-l2-hashmap | `3 7` | 声明 `let mut basket = HashMap::new();` 并补 apple:2 / mango:3（banana:2 已给） |
| 31-l2-strings2 | `green 是颜色词` | 调用处 `is_color_word(word)` → `is_color_word(&word)` |
| 32-l2-match | `西` | match 补 `Direction::West => "西",` 分支 |
| 33-l2-boss | `95` | match 的 `None => "无"` → `None => 0`（88+7） |

实测命令：`rustc --edition 2021 -o out <fixed> && ./out`；全部 fixed 版 0 warning。

## 改编要点（W1-W10 / §6.1-6.2）

- **30-l2-hashmap**：rustlings hashmaps1 的 bug 藏在 `#[cfg(test)]` 内（裸 rustc 直编通过且无输出）→ W1 测试体搬 main：`basket.len()` 与 `values().sum()` 两处断言改 println；声明与补果留白（W7）。输出 `len sum` 与 HashMap 迭代序无关（sum 可交换、len 无顺序），输出确定性达标（Q9）。
- **31-l2-strings2**：main 可见 bug（String 传 &str 形参），直用骨架；W6 中文输出：if 分支输出中文化为「green 是颜色词」，UTF-8 逐字节比对；修复点是唯一的 `&` 借用（§4.2 单点修复风格，非 Boss）。
- **32-l2-match**：自编；枚举 4 变体、match 只写 3 分支制造 E0004；`#[allow(dead_code)]` 消除 fixed 版 warning（Q8）；输出「西」为单字中文。
- **33-l2-boss**：is_boss=true（L2 层末关，v3 §4.2）；覆盖知识点 ≥3：Result（`parse_score` 返回 Result + `unwrap` 取成功值）、Option（`find_score` 返回 Option + `get().copied()`）、match（Some/None 分支处理）、哈希 map（成绩表）；修复点只有一处（match None 分支类型），broken 仅 1 个 E0308。描述/提示用基准术语（Option/Result/match 保留），成绩数据避免出现期望值（小明 100 而非 95，Q6）。

## Q1-Q10 自查

1. starter 编译状态符合题型：4 关均编译失败，首码即题干 bug。✓
2. 预期错误码 = 实测首条 E 码（上表），E0425/E0308/E0004/E0308 均在 assets/errors.toml 活跃码内。✓
3. expect_output 与 fixed 版 trim+CRLF 归一化后逐字节相等。✓
4. expect_output 无 `\r`、无行尾空格、行序与 fixed 一致、末尾无刻意 `\n`。✓
5. hints 三级规范：hints[0] ≤40 字无代码无 API（概念名）；hints[1] ≤60 字（链接不计）+ rustwiki 白名单（ch08-03/ch08-02/ch06-02/ch09-02，book-cn 章节）；hints[2] 唯一修复代码位 ≤3 行 + ≤40 字说明；整关 hints 总量与现有关卡（24/25 关 288-297 字）同量级。✓
6. starter 无答案痕迹：grep 不到 expect_output 值（30 的 3/7、31 的 green 是颜色词、32 的西、33 的 95）与修复写法；TODO 已中文化。✓
7. source 标注精确：30/31 为 `rustlings (<路径>, v6.5.0, MIT，改编说明)`；32/33 为 `自编 (…)`。✓
8. edition 2021 兼容 + 仅 std：fixed 全部编译通过（0 warning）、无外部 crate。✓
9. 输出确定性：无时间戳/HashMap 迭代序/多线程；HashMap 关只输出 len 与 sum。✓
10. quiz 型不涉及（全部 code 关）。✓
