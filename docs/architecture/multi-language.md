# 架构扩展性：多语言学习支持路径

> 状态：设计确认（2026-08-16）。**未实现**——本文件只固化「如何扩展」的路径，
> 保证未来加 Python / Go / C 等语言时不需要推翻现有架构。
> 一句话结论：**现有架构已具备多语言接缝，新语言 = 新增一个 crate/模块 + 数据目录，不改引擎。**

## 现状：哪些已经是语言无关的（无需改动）

| 层 | 现状 | 语言耦合点 |
|---|---|---|
| `Sandbox` trait（sandbox.rs:10） | `compile(&str) -> CompileOutcome` / `run(&Path) -> RunOutcome`——接口本身不含任何 rustc 概念 | 实现是 rustc 专用（`DevSandbox`/`BwrapSandbox`） |
| `Validation` / `ErrorCard` / `OutputDiff` / `PanicInfo`（validate/mod.rs） | 纯数据，与语言无关 | 卡片内容来自 Rust 错误码映射 |
| 校验编排 `validate()`（validate/mod.rs:133） | compile → run → compare 的流程本身语言无关；`allow_compile_fail`/quiz/panic 分支通用 | 内部调用了 Rust 的 `ErrorMapper` 与错误解析 |
| `Level` 模型（level.rs） | `starter_code / expect_output / allow_compile_fail / expect_error_code / kind / options / answer_index / link`——对任何语言都成立 | 无 |
| 游戏状态机 `GameApp` / `Engine` | 只消费 `Validation`，不感知具体语言 | 无 |
| UI 壳层（game-ui，eframe/winit） | 只渲染 `Screen` 数据 + 调 `tokenize` 着色 | 着色函数当前是 rustc_lexer |

## 新语言接入步骤（以 Python 为例，未来照此执行）

### 1. 执行层：新增 `PythonSandbox`（实现现有 `Sandbox` trait）

```rust
// crates/crab-quest-core/src/sandbox/python.rs
pub struct PythonSandbox { pub run_timeout_secs: u64 }

impl Sandbox for PythonSandbox {
    fn compile(&self, code: &str) -> Result<CompileOutcome, GameError> {
        // python3 -m py_compile（语法检查）→ 语法错误转 CompileError
    }
    fn run(&self, binary: &Path) -> Result<RunOutcome, GameError> {
        // python3 code.py（解释执行，timeout + 输出捕获；异常 → Panic）
    }
}
```
- 超时/管道/kill 机制与 `DevSandbox` 共用（可抽公共 `spawn_with_timeout` 辅助）。
- `CompileOutcome::Success { binary }` 对解释型语言退化为「语法检查通过」，binary 路径指向源码文件。

### 2. 错误解析：Python 版 `parse_*`（SyntaxError / Traceback）

- 新增 `validate/error_parser_python.rs`：解析 `SyntaxError`（行号 + 消息）与运行时 Traceback（异常类型 + 行号）。
- 产出仍是 `Vec<CompileError>` / `PanicInfo`——下游 `validate()` 无需感知差异。

### 3. 错误映射：Python 版 `errors.toml`（`assets-<lang>/errors.toml`）

- 结构沿用 `code = { zh, link, fix, example }`；key 从 `E0xxx` 换成 `SyntaxError` / `IndentationError` / `TypeError` 等。
- `ErrorMapper` 加载路径按语言切换，其余逻辑复用。

### 4. 着色：Python 版 tokenizer（`editor/python.rs`）

- 实现 `fn tokenize(code: &str) -> Vec<TokenSpan>`（关键字/字符串/注释/数字），签名与 Rust 版一致。
- 可先用轻量实现（几十行），后续可换 pygments 等成熟库。

### 5. 数据：关卡目录 `assets/<lang>/levels/`

- `Level` 增加 `language: String` 字段（`#[serde(default = "rust")]`）——**现有 56 关零改动**。
- `LevelSet::load` 按 `language` 过滤，或按目录加载（`assets/levels/` 保持 Rust 主线不动）。

### 6. 分发：语言注册表

```rust
// crates/crab-quest-core/src/language/mod.rs（未来）
pub trait Language {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn sandbox(&self) -> Box<dyn Sandbox>;
    fn tokenize(&self, code: &str) -> Vec<TokenSpan>;
    fn error_mapper_path(&self) -> PathBuf;
    fn level_dir(&self) -> PathBuf;
}
pub fn by_id(id: &str) -> Option<Box<dyn Language>>; // "rust" | "python" | "go" | "c"
```
- UI 主菜单加语言选择（切换 `Language` 后重载 LevelSet 即可，状态机不变）。
- 存档按语言隔离（`save-<lang>.toml` 或 level_states 加命名空间）。

## 跨平台（Windows / macOS）注意点

| 项 | 现状 | 需要做什么 |
|---|---|---|
| 存档路径 | `$HOME/.local/share/...`（main.rs:354，Linux 专用） | 换 `dirs` crate：Windows `%APPDATA%` / macOS `~/Library/Application Support` |
| 沙盒 | `BwrapSandbox` 依赖 Linux `bwrap` 二进制 | `#[cfg(target_os = "linux")]` 门控；Win/mac 用仅超时+静态拦截的降级沙盒（README 标注限制） |
| 编译器发现 | rustc 从 PATH 找 | 已跨平台（`std::process`）；可加 `rustup which rustc` 兜底 |
| 字体/渲染 | 内嵌 TTF（egui 跨平台） | 无需改动 |
| 编辑器 IME | egui-miniquad 无 IME 通道 | Windows/macOS 后端差异留待 P3-20 调研 |

## 风险与约束

- 每次新增语言 = 独立小任务（sandbox + parser + mapper + tokenizer + 关卡），可并行可独立验收；
- `allow_compile_fail`（制造指定错误）对解释型语言语义不同（SyntaxError vs 运行时异常），按语言实现时单独定义；
- 玩家代码沙盒策略按语言评估（Python 的 `import` 面比 rustc 大，bwrap 在 Linux 上仍是主力方案）。
