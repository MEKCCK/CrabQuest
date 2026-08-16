# 49-l4-mutable-zst（quiz 关）实测记录

任务：T5（P3-21 quiz 关卡类型）· 关卡 `assets/levels/49-l4-mutable-zst.toml`

## 展示代码（starter_code）

```rust
struct S;

fn main() {
    let [x, y] = &mut [S, S];
    let eq = x as *mut S == y as *mut S;
    print!("{}", eq as u8);
}
```

来源：rust-quiz questions/013-mutable-zst.rs（CC BY-SA 4.0，解释文案自写中文）。

## 编译实测（rustc 1.97.0，--edition 2021）

```bash
$ printf 'struct S;\n\nfn main() {\n    let [x, y] = &mut [S, S];\n    let eq = x as *mut S == y as *mut S;\n    print!("{}", eq as u8);\n}\n' > /tmp/rlg-013/main.rs
$ rustc --edition 2021 /tmp/rlg-013/main.rs -o /tmp/rlg-013/quiz013
compile_exit=0            # 无警告无错误 → 展示代码可编译（quiz 提交前校验的前提）
$ ./tmp/rlg-013/quiz013 | xxd
00000000: 31                                      1
run_exit=0                # 运行输出恰为 "1"（单字节 0x31，无换行）
```

- 错误码：无（quiz 关无 broken 版；展示代码必须可编译，这是提交前校验的实测依据）。
- fixed 输出：`1` → 与 answer_index=1（选项「1」）一致，确认答案正确。

## 判定流程（engine.submit_quiz）

1. 校验当前关卡 kind=quiz（否则 `LevelKindMismatch`）。
2. 选项索引越界（≥ options.len()）→ `QuizAnswerOutOfRange`，不记账。
3. 沙盒编译展示代码（提交前校验可编译；失败 → `LevelDataInvalid`）。
4. 索引 == answer_index → `Validation::Pass`（XP +20 / combo / 状态 / 解锁下一关，与普通关共用 record_pass）。
5. 否则 → `Validation::Fail`（combo 清零 / total_errors / attempts，共用 record_fail）。

## 命令记录

```bash
cargo test -p game-core --lib engine          # 11 通过（含 5 个 quiz 判定单测）
cargo test -p game-core --lib                 # 78 通过
cargo test -p game-ui --lib                   # 4 通过（含 quiz 渲染单测）
cargo test -p game-data --test levels         # 8 通过（含 quiz_level_49_parses + 重复选项端到端）
cargo test --workspace                        # 全绿
```
