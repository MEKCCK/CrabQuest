# 实测记录：P2-14 内容扩展第二批（100-exercises + rust-quiz）

日期：2026-08-16
环境：rustc 1.97.0 (2d8144b78 2026-07-07)，`--edition 2021`（与游戏沙盒一致）
流水线：S1-S8 全流程；验证文件位于 /tmp/t4verify/（本机临时目录）

## 通用验证命令

```bash
rustc --edition 2021 -o out <file>           # 编译；FAIL 时 stderr 取首条 error[E…]
./out                                         # 运行，捕获 stdout
rustc --edition 2021 <file> 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1   # 错误码确认
```

## 一、100-exercises 关卡（结构借鉴 + 代码重写，CC BY-NC 4.0）

### 1. 04-l0-integers（02_basic_calculator/01_integers）

broken（starter，u8/u32 混乘）：
```bash
$ rustc --edition 2021 -o out broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1
error[E0308]
```
fixed（`let factor: u32 = 4;`）：
```bash
$ ./out
1 + 2 * 4 = 9
```
expect_output = `1 + 2 * 4 = 9`（逐字节一致）

### 2. 20-l1-ownership-ticket（03_ticket_v1/06_ownership）

broken（starter，访问器按值接收 self）：
```bash
$ rustc --edition 2021 -o out broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1
error[E0382]   # 第二次调用 ticket.description() 使用已移动的 ticket
```
fixed（`&self` + clone 返回）：
```bash
$ ./out
周末演唱会
A 区前排
已出票
```
expect_output 三行中文与 fixed 输出逐字节一致（UTF-8，无 BOM）。

### 3. 27-l2-saturating（02_basic_calculator/09_saturating）

broken（starter，`acc *= step`）编译通过，运行期溢出：
```bash
$ ./out
factorial(5) = 120
thread 'main' panicked: attempt to multiply with overflow   # factorial(20) 处 panic
```
fixed（`acc = acc.saturating_mul(step);`）：
```bash
$ ./out
factorial(5) = 120
factorial(20) = 4294967295
```
expect_output 两行与 fixed 输出逐字节一致。

## 二、rust-quiz 关卡（题代码可搬，解释自写中文，CC BY-SA 4.0）

### 4. 47-l4-lazy-map（questions/026-iterator-lazy-map.rs）

broken（starter，`.collect()` 立即驱动 map）：
```bash
$ ./out
123101        # 闭包先全部打印 123，再打印奇偶 101
```
fixed（去掉 `.collect()` 与 Vec 标注，惰性迭代）：
```bash
$ ./out
112031
```
expect_output = `112031`（与 rust-quiz 官方 Answer 一致）。

### 5. 48-l4-fnptr（questions/011-function-pointer-comparison.rs，allow_compile_fail）

starter（可编译，输出 0；带 fn 指针比较 warning，符合预期）：
```bash
$ rustc --edition 2021 -o out starter.rs && ./out
0
```
玩家制造版（`f::<'static>`）：
```bash
$ rustc --edition 2021 player-made.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1
error[E0794]   # cannot specify lifetime arguments explicitly if late bound lifetime parameters are present
```
expect_error_code = E0794（本机实测，非抄旧表）。

### 6. 50-l4-drop-underscore（questions/019-dropped-by-underscore.rs）

broken（starter，`drop(s)` 立即释放）：
```bash
$ ./out
12
```
fixed（`let _ = s;`，不移动，s 存活到 main 结束）：
```bash
$ ./out
21
```
expect_output = `21`（与官方 Answer 一致）。

### 7. 51-l4-lifetime-ext（questions/037-lifetime-extension.rs）

broken（starter，两个块都用赋值语句 `_ = &Drop0;`）：
```bash
$ ./out
0101
```
fixed（第一个块改为 `let _ = &Drop0;`，触发生命周期延长）：
```bash
$ ./out
1001
```
expect_output = `1001`（与官方 Answer 一致）。

### 8. 52-l4-fnmut-copy（questions/036-fnmut-copy.rs）

broken（starter，参数无 Copy 约束，call(f) 移动 f）：
```bash
$ rustc --edition 2021 -o out broken.rs 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1
error[E0382]   # 第二次 f()/call(f) 使用已移动的 f
```
fixed（`impl FnMut() + Copy`）：
```bash
$ ./out
1223
```
expect_output = `1223`（与官方 Answer 一致）。

## 三、Q1-Q10 质量检查小结

- Q1 starter 编译状态符合题型：04/20/52 broken 编译失败（E0308/E0382/E0382）；27/47/50/51 broken 编译通过但输出/运行期不符；48 为 allow_compile_fail（starter 可编译，玩家制造 E0794）。
- Q2 错误码全部本机实测：E0308/E0382/E0382（编译）+ 运行期 overflow panic；48 实测 E0794。
- Q3 expect_output 与 fixed 版逐字节一致（trim+CRLF 归一化后；47/50/51/52 用 `./out | xxd` 核对）。
- Q4 无 \r、无行尾空格、行序与 fixed 一致。
- Q5 hints[0]/[1] 不含修复代码与期望值；hints[2] 为唯一代码位置。
- Q6 starter 无答案痕迹：期望值只出现在 expect_output；100ex 三关无 helpers 依赖（grep 无 `ticket_fields`/`common::`）、无原文复制（结构借鉴、标识符与文本均重写，§6.4 逐条过）。
- Q7 source 精确：100ex 标 `CC BY-NC 4.0，结构借鉴/代码重写`；rust-quiz 标 `CC BY-SA 4.0，解释自写`。
- Q8 edition 2021 + 仅 std，无外部 crate。
- Q9 输出确定性：全部单线程、无时间戳、行序固定。
- Q10 非 quiz 型；description/hints 全中文自写（对照原题 .md 无 ≥3 连词雷同）。
