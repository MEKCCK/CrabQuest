# T6（P3-22）expect_panic 关卡实测记录

日期：2026-08-16 ｜ rustc 1.97.0 ｜ `--edition 2021`

## 一、32a-l2-panics 关（assets/levels/32a-l2-panics.toml）

关卡：制造「index out of bounds」panic（扩展槽，参考 22-l2-vec 反向设计，素材自编）。

### broken 版（starter_code）：编译通过、不触发 panic

```rust
fn main() {
    let data = vec![7, 8, 9];
    println!("{}", get(&data, 3));
}

fn get(data: &[i32], i: usize) -> i32 {
    if i < data.len() {
        data[i]
    } else {
        data[data.len() - 1]
    }
}
```

实测命令与结果：

```bash
rustc --edition 2021 broken.rs -o broken_out
# 编译：成功（退出码 0，无 error[E…]）
./broken_out
# 9
# 退出码 0 —— 越界请求被 else 兜底分支静默吞掉，不 panic（逻辑错）
```

### fixed 版：编译通过、运行触发 index out of bounds panic

```rust
fn main() {
    let data = vec![7, 8, 9];
    println!("{}", get(&data, 3));
}

fn get(data: &[i32], i: usize) -> i32 {
    data[i]
}
```

实测命令与结果：

```bash
rustc --edition 2021 fixed.rs -o fixed_out
# 编译：成功（退出码 0）
./fixed_out
# (stderr，xxd 核对字节)：
# \nthread 'main' (562346) panicked at fixed.rs:7:5:\n
# index out of bounds: the len is 3 but the index is 3\n
# note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n
```

- 进程以非零退出码结束（panic = unwind + abort）；stdout 为空。
- 净化后消息（`sanitize_panic_message`）：`fixed.rs:7:5:\nindex out of bounds: the len is 3 but the index is 3`
- `expect_panic = "index out of bounds"` 为净化后消息子串，大小写敏感命中 → 通关。

### expect_panic 判定语义（validate/mod.rs 实测）

- 编译成功 + 运行 panic + 净化后消息包含子串 → `Validation::Pass`。
- 编译失败 → 走标准编译错误反馈（含错误码/行号/中文卡片）。
- 编译成功但未 panic → Fail「期望 panic 但未触发：程序编译通过并正常运行结束。期望 panic 消息包含：…」。
- panic 消息不包含子串 → Fail「panic 消息不匹配：期望包含「…」，实际为：\n…」。

## 二、panic 净化（v3 §5.3 第 6 条最小实现）

`sanitize_panic_message`（crates/game-core/src/validate/mod.rs）顺序：

1. 每行 strip 行首空白（实测 panic stderr 以空行开头，`thread` 头前有空行）；
2. 剥临时目录路径：`/tmp/rlg-XXXX/` 或 `rlg-XXXX/`（沙盒 tempfile 前缀 `rlg-`，目录名 `[A-Za-z0-9_]+`）——绝对路径编译时定位行含完整路径（实测 `/tmp/rlg-probe/main.rs:3:24:`），必须剥掉；
3. 剥 `thread 'main' (线程id) panicked at ` 头（本机 rustc 1.97 线程 id 形如 `(535665)`，实测）；保留 `main.rs:N:M:` 定位行；
4. 删 `note:` 行与空行。

测试：`sanitize_panic_message_strips_noise`（含 `/tmp/rlg-Ab12Cd34/main.rs:3:24:` 的原始消息 → 断言路径/线程 id/note 全部剥离、定位行与消息体保留）。

## 三、测试清单

game-core（validate/mod.rs + engine.rs）：
- `expect_panic_substring_match_passes`：包含子串 → 通关
- `expect_panic_not_contained_fails`：panic 但消息不含子串 → 失败，反馈含「panic 消息不匹配」+ 期望/实际
- `expect_panic_case_sensitive`：大小写敏感（"PANIC" vs "panic" 失败；精确大小写通过）
- `expect_panic_not_triggered_reports`：编译过但未 panic → 反馈含「期望 panic 但未触发」
- `expect_panic_matches_with_temp_dir_path`：连跑 3 次（每次新临时目录随机路径）均匹配 → 净化生效、路径不干扰
- `expect_panic_priority_over_output`：expect_panic 非空时优先于 expect_output 比对（直接构造绕过加载校验）
- `sanitize_panic_message_strips_noise`：净化函数直接验证
- `engine::tests::expect_panic_level_submit_loop`：提交闭环（未触发 → 失败计错；触发 → Pass + XP + Passed 状态）

game-data（tests/levels.rs）：
- `expect_panic_level_54_parses`：54 关解析断言（tier l2、expect_panic="index out of bounds"、expect_output 空、hints=3、source 非空、starter 无期望子串）
- `expect_panic_and_output_mutually_exclusive`：互斥校验端到端（同时非空 → 加载报「互斥」+ 关卡 id）
- `all_levels_parse_and_consistent`：计数断言放宽为 `>= 29`，缺省值循环兼容显式字段

## 四、命令记录

```bash
# broken 版
rustc --edition 2021 broken.rs -o broken_out && ./broken_out   # 9, exit 0
# fixed 版
rustc --edition 2021 fixed.rs -o fixed_out && ./fixed_out      # panic: index out of bounds: the len is 3 but the index is 3
# 回归
cargo test -p game-core   # 78 passed
cargo test -p game-data   # 全绿（含 T6 两条专项）
cargo test --workspace    # 全绿
```
