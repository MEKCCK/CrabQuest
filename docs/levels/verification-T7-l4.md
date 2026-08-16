# 实测记录：T7 Wave 3a L4 补齐（53-l4-boss）

日期：2026-08-16
环境：rustc 1.97.0 (2d8144b78 2026-07-07)，`--edition 2021`（与游戏沙盒一致）
流水线：S1-S8 全流程；验证文件位于 /tmp/t7l4/（本机临时目录，broken.rs / fixed.rs / starter_from_toml.rs）

## 通用验证命令

```bash
rustc --edition 2021 -o out <file>           # 编译；FAIL 时 stderr 取首条 error[E…]
./out                                         # 运行，捕获 stdout
rustc --edition 2021 <file> 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1   # 错误码确认
```

## 一、53-l4-boss（自编 Boss：借用+所有权+Option 综合，购物车场景）

素材：自编（综合借用/所有权/Option 知识点；参考 100-exercises-to-learn-rust 的 ticket 项目结构，CC BY-NC 4.0 仅借鉴思路，代码与文案自写）。覆盖知识点：结构体 + impl + `&mut self` + Vec<(String, u32)> + 迭代器 sum + Option + match（≥3 个）；修复点仅一处（`&self`→`&mut self`）。

broken（starter，`add(&self)` 内 push，编译失败）：
```bash
$ rustc --edition 2021 -o out starter_from_toml.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1
error[E0596]   # cannot borrow `self.items` as mutable, as it is behind a `&` reference
# 全量错误仅此 1 个 E 码（另有 1 条 unused_mut warning，符合 Boss 单错误码约束）
```

fixed（`fn add(&mut self, name: &str, qty: u32)`，其余代码不动）：
```bash
$ ./out
总数量：4
药水数量：3
```
expect_output = `总数量：4\n药水数量：3`（两行中文 UTF-8，`xxd` 核对：`e6 80 bb e6 95 b0 e9 87 8f ef bc 9a 34 0a e8 8d af e6 b0 b4 e6 95 b0 e9 87 8f ef bc 9a 33 0a`；全角冒号 `：`，无 \r、无行尾空格、末尾无刻意换行；与 fixed 版 trim+CRLF 归一化后逐字节一致）。

is_boss = true；hint_unlock = [1, 2, 3]（第 1/2/3 次失败依次解锁 hints[0]/[1]/[2]，与 L3-A1 草案 9 注释一致）。

## 二、Q1-Q10 质量检查小结

- Q1 starter 编译失败（E0596），题型符合 code 编译修复关。
- Q2 预期错误码 E0596 为本机实测（非抄旧表）；broken 版仅 1 个 E 码（Boss 约束）。
- Q3 expect_output 与 fixed 版逐字节一致（trim+CRLF 归一化后，`xxd` 核对）。
- Q4 expect_output 无 `\r`、无行尾空格、行序与 fixed 一致、末尾无刻意 `\n`。
- Q5 hints[0] 概念级无代码无 API（≤40 字）；hints[1] 定位级含错误码含义与 rustwiki 白名单链接（≤60 字，链接不计）；hints[2] 为唯一修复代码位（1 行代码 + ≤40 字说明）。
- Q6 starter 无答案痕迹：grep 不到 `总数量：4`/`药水数量：3`/`4`；println 用占位符 `{}`，期望值只出现在 expect_output。
- Q7 source 标注自编 + 100-exercises 结构借鉴注明（CC BY-NC 4.0 仅借鉴思路）。
- Q8 edition 2021 + 仅 std：无外部 crate、无 unsafe/thread/async。
- Q9 输出确定性：单线程、无时间戳、行序固定。
- Q10 非 quiz 型；description/hints 全中文自写，术语符合 §6.1（未用 特征/特质/生存期/悬挂）。

## 三、文案字数核对（§6.2）

| 文本位 | 上限 | 实测（中文字符） |
|---|---|---|
| description | ≤80 | 73 |
| hints[0] | ≤40 | 33 |
| hints[1] | ≤60（链接不计） | 31 |
| hints[2] | ≤3 行代码 + ≤40 字 | 1 行代码 + 21 字 |
| 整关 hints | ≤200 | 81 |

## 四、测试

- `cargo test -p game-data --test levels`：新增 `t7_l4_boss_level_53_consistent` 断言通过（id 唯一 / tier l4 / is_boss / starter 非空 / source 非空 / hints 长度 3 / hint_unlock 等长 / expect_output 一致）。
