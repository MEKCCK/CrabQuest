# 错误码 fixture 矩阵（Tier 2 / Tier 3）

本目录是 `crates/game-core/tests/fixtures_compile.rs`（Tier 2）与
`tests/levels_regression.rs`（Tier 3）的素材。需求：docs/tasks/gameplay/P2-16-测试矩阵与clippy决策.md；
实测基准：docs-review/L3-B2-parser.md（15 fixture，rustc 1.97 实测）。

## 目录结构与计数（共 18 个 fixture 场景）

| 目录 | 数量 | 说明 |
|---|---|---|
| `errors/` | 12 | 有 E 码编译错误（11 个 P0/现有码 + 1 个方案 A clippy 改写） |
| `nocode/` | 2 | 无 E 码编译错误（format 参数错误、let chains edition）→ EUNKNOWN 兜底 |
| `panic/` | 2 | 运行期 panic 分类（索引越界、unwrap None） |
| `dead_codes/` | 2 | E0412/E0504 负面断言（死码不得误报为活跃码） |

每个 fixture 目录内：

- `broken.rs`：最小触发代码（`rustc --edition 2021` 下必现预期行为）；
- `fixed.rs`：修复版（编译通过 + 运行成功；`dead_codes/` 无 fixed.rs，它们是负面断言）；
- `expected.toml`：断言元数据（见下）；
- `errors/E0308_clippy_approx_constant/level.toml`：方案 A 样例关素材（未挂载 assets/levels/）。

## expected.toml 元数据 schema

```toml
# errors/ 类：断言 errors.first() 的 code 与 line（line 为实测 `-->` 值）
kind = "compile"
code = "E0425"
line = 2

# nocode/ 类：无 E 码 → 断言 code=EUNKNOWN、line、message 稳定子串
kind = "nocode"
code = "EUNKNOWN"
line = 2
message_contains = "positional arguments in format string"

# panic/ 类：编译成功 + 运行 panic，断言 sanitize+classify 结果
kind = "panic"
classification = "array_index_oob"
message_contains = "index out of bounds"
line = 3

# dead_codes/ 类：负面断言——编译 broken.rs 后，此码不得出现在解析结果中
#（errors 为空 或 只出现其它活跃码，如 E0412 历史触发码现在报 E0425）
kind = "deadcode"
code = "E0412"
```

## 生成 / 更新流程

- 基准 rustc 版本：**1.97.0**（`rustc --version` 留档）。line/col 是实测值
  （L3-B2 §1.2：E0621 指向返回表达式行、E0601 指向文件末尾行，不得按直觉"修正"）。
- 更新某个 fixture 后必须在目录内重跑：
  `rustc --edition 2021 broken.rs -o /tmp/out && rustc --edition 2021 fixed.rs -o /tmp/outf && /tmp/outf`
  并同步 expected.toml 的 `line`。
- 新增 fixture：按 `目录/名称/` 三件套（dead_codes 两件套）落盘后在
  `tests/fixtures_compile.rs` 无需改动（自动遍历发现）；改 `expected.toml` 即改断言。

## 测试分层与 CI 策略

- **Tier 1**（解析器单测）：已由 P1-01 在 `src/validate/error_parser.rs` 内落地（静态 stderr 快照，零 rustc）。
- **Tier 2**（真实编译矩阵，本目录）：`tests/fixtures_compile.rs` 全用真实 rustc 编译/运行，
  只比 E 码与行号、不比文本。**全部标 `#[ignore]`**：本地 `cargo test -p game-core` 默认跳过
  （依赖 rustc 可执行文件；L3-B2 预估 18 场景 ≈ 8-10s 串行 / 4 路并行 ≈ 3s，本机实测
  0.9s / 1.7s，仍不宜进日常增量）；CI 用 `cargo test -p game-core -- --ignored` 全跑。
- **Tier 3**（关卡回归）：`tests/levels_regression.rs` 从 `assets/levels/*.toml` 动态读取
  15 关 starter_code 真实编译断言（默认运行，非 ignored）；01-l0-print 额外锁「反馈非空」。
