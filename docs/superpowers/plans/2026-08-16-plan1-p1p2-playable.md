# Rust 学习游戏 计划①（P1+P2：完整可玩版本）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现一个完整可玩的 Rust 学习游戏：macroquad+egui 窗口界面、TOML 关卡系统（L0-L4 共 15 关）、compile→run→compare 校验闭环、错误码→中文提示映射、存档系统、开发期安全兜底（timeout+临时目录+syn 静态拦截）。

**Architecture:** Cargo workspace 三 crate。`game-core` 零 UI 依赖：关卡模型、校验（rustc 子进程编译→运行→输出比对）、错误解析器（仅按错误码 E0xxx 匹配）、存档、开发期沙盒、syn 分词着色、引擎状态机、GameApp（Input→Screen 纯状态机）。`game-ui` 实现 `UiBackend`（macroquad+egui，自研轻量代码编辑器）。`game-data` 提供关卡 TOML 与 errors.toml 路径。

**Tech Stack:** Rust 2021（rustc 1.97）、macroquad 0.4、egui-macroquad 0.17（egui 0.31）、serde+toml、thiserror、syn 2、tempfile。无 async 运行时。

## Global Constraints

- Rust edition 2021；rustc 1.97 已装（`/usr/bin/rustc`）
- 玩家代码仅 std：编译用裸 `rustc --edition 2021`，无 cargo 依赖解析
- 错误解析只匹配错误码 `E0xxx`，**绝不匹配报错字符串**（抗 rustc 版本差异）
- 所有错误经 `GameError`，`Display` 为中文
- 关卡与错误映射只存 TOML（`assets/` 下），不硬编码进代码
- 每个关卡 TOML 必须保留 `source` 字段（素材出处）
- TDD：每个任务先写失败测试 → 跑失败 → 最小实现 → 跑通过 → 提交
- 提交信息格式 `feat(scope): 描述` / `test(scope): 描述`
- 不引入 tokio 等 async 运行时；trait 用原生 async fn（1.75+ 稳定）
- 本机无 apt，禁止把 firejail 等系统包安装列为依赖（沙盒真隔离在计划②用已安装的 bwrap）

## 文件结构总览

```
rust-learning-game/
├── Cargo.toml                  # workspace
├── .gitignore
├── crates/
│   ├── game-core/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs
│   │   ├── src/error.rs
│   │   ├── src/level.rs
│   │   ├── src/save.rs
│   │   ├── src/sandbox.rs
│   │   ├── src/editor.rs
│   │   ├── src/engine.rs
│   │   ├── src/app.rs
│   │   ├── src/ui.rs
│   │   ├── src/validate/mod.rs
│   │   ├── src/validate/error_parser.rs
│   │   └── src/validate/mapper.rs
│   ├── game-ui/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs
│   │   ├── src/app_ui.rs
│   │   └── src/main.rs
│   └── game-data/
│       ├── Cargo.toml
│       ├── src/lib.rs
│       └── tests/levels.rs
├── assets/
│   ├── errors.toml
│   └── levels/  (15 个关卡 TOML)
└── README.md
```

---

### Task 1: Workspace 脚手架

**Files:**
- Create: `Cargo.toml`（workspace 根）
- Create: `.gitignore`
- Create: `crates/game-core/Cargo.toml`, `crates/game-core/src/lib.rs`
- Create: `crates/game-ui/Cargo.toml`, `crates/game-ui/src/lib.rs`, `crates/game-ui/src/main.rs`
- Create: `crates/game-data/Cargo.toml`, `crates/game-data/src/lib.rs`

**Interfaces:**
- Produces: 三个可编译的空 crate；后续任务在此结构上填充

- [ ] **Step 1: 写 workspace 根 Cargo.toml 与 .gitignore**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/game-core", "crates/game-ui", "crates/game-data"]

[workspace.package]
version = "0.1.0"
edition = "2021"
```

`.gitignore`:
```
/target
/vendor
Cargo.lock
```

- [ ] **Step 2: 写三个 crate 的清单**

`crates/game-core/Cargo.toml`:
```toml
[package]
name = "game-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
thiserror = "2"
syn = { version = "2", features = ["full"] }
rustc_lexer = "0.1"
tempfile = "3"

[dev-dependencies]
```

`crates/game-ui/Cargo.toml`:
```toml
[package]
name = "game-ui"
version.workspace = true
edition.workspace = true

[dependencies]
game-core = { path = "../game-core" }
game-data = { path = "../game-data" }
macroquad = "0.4"
egui-macroquad = "0.17"
```

`crates/game-data/Cargo.toml`:
```toml
[package]
name = "game-data"
version.workspace = true
edition.workspace = true

[dependencies]

[dev-dependencies]
game-core = { path = "../game-core" }
```

- [ ] **Step 3: 写三个 crate 的最小 lib.rs / main.rs**

`crates/game-core/src/lib.rs`:
```rust
pub fn placeholder() {}
```

`crates/game-data/src/lib.rs`:
```rust
pub fn placeholder() {}
```

`crates/game-ui/src/lib.rs`:
```rust
pub fn placeholder() {}
```

`crates/game-ui/src/main.rs`:
```rust
fn main() {
    println!("rust-learning-game ui placeholder");
}
```

- [ ] **Step 4: 验证编译**

Run: `cd /home/elite/Projects/rust-learning-game && cargo build`
Expected: 编译成功（首次拉取依赖较慢，设 timeout 300s）

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(scaffold): workspace 三 crate 脚手架"
```

---

### Task 2: 错误类型 + 关卡模型 + LevelSet

**Files:**
- Create: `crates/game-core/src/error.rs`
- Create: `crates/game-core/src/level.rs`
- Modify: `crates/game-core/src/lib.rs`
- Test: `crates/game-core/src/level.rs`（内联 `#[cfg(test)]`）

**Interfaces:**
- Consumes: 无（纯新增）
- Produces:
  - `GameError` 枚举（error.rs，Display 中文）
  - `LevelTier::{L0,L1,L2,L3,L4}`，`#[serde(rename_all = "lowercase")]`（TOML 中为 `"l0".."l4"`），附 `fn order(&self) -> u8`
  - `Level` 结构体（字段见下）
  - `LevelSet { pub levels: Vec<Level> }` 与 `LevelSet::load(dir: &Path) -> Result<Self, GameError>`、`get(&self, id) -> Option<&Level>`、`len()`
  - `pub fn parse_levels(content: &str) -> Result<Vec<Level>, GameError>`（供 load 与测试复用）

- [ ] **Step 1: 写失败测试**（level.rs 底部 `#[cfg(test)] mod tests`）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const LEVEL_TOML: &str = r#"
[[level]]
id = "l0-hello"
title = "你好，变量"
tier = "l0"
description = "修复代码，使其编译并输出预期结果"
hint = "变量需要 let 声明"
starter_code = "fn main() { x = 5; println!(\"x has the value {}\", x); }"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l1-move"
title = "所有权转移"
tier = "l1"
description = "理解 move 语义"
starter_code = "fn main() { let s = String::from(\"hi\"); take(s); println!(\"{}\", s); } fn take(x: String) {}"
expect_output = ""
allow_compile_fail = true
expect_error_code = "E0382"
source = "rustlings"
"#;

    #[test]
    fn parse_levels_ok() {
        let levels = parse_levels(LEVEL_TOML).unwrap();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].id, "l0-hello");
        assert_eq!(levels[0].tier, LevelTier::L0);
        assert_eq!(levels[0].expect_output, "x has the value 5");
        assert!(!levels[0].allow_compile_fail);
        assert_eq!(levels[1].tier, LevelTier::L1);
        assert!(levels[1].allow_compile_fail);
        assert_eq!(levels[1].expect_error_code, "E0382");
    }

    #[test]
    fn parse_levels_invalid_tier_fails() {
        let bad = LEVEL_TOML.replace("tier = \"l0\"", "tier = \"l9\"");
        assert!(parse_levels(&bad).is_err());
    }

    #[test]
    fn parse_levels_malformed_fails() {
        assert!(parse_levels("not toml at all [[[[").is_err());
    }

    #[test]
    fn tier_order() {
        assert!(LevelTier::L0.order() < LevelTier::L1.order());
        assert!(LevelTier::L4.order() == 4);
    }

    #[test]
    fn level_set_load_sorted_and_duplicate_detected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("02-second.toml"), LEVEL_TOML).unwrap();
        std::fs::write(
            dir.path().join("01-first.toml"),
            "[[level]]\nid = \"first\"\ntitle = \"t\"\ntier = \"l2\"\ndescription = \"d\"\nstarter_code = \"fn main() {}\"\nsource = \"x\"\n",
        )
        .unwrap();
        let set = LevelSet::load(dir.path()).unwrap();
        assert_eq!(set.len(), 3);
        assert_eq!(set.levels[0].id, "first"); // 按文件名排序
        assert!(set.get("first").is_some());
        assert!(set.get("nope").is_none());
    }

    #[test]
    fn level_set_duplicate_id_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.toml"), LEVEL_TOML).unwrap();
        std::fs::write(dir.path().join("b.toml"), LEVEL_TOML).unwrap();
        assert!(matches!(LevelSet::load(dir.path()), Err(GameError::DuplicateLevelId(_))));
    }

    #[test]
    fn level_set_missing_dir_fails() {
        assert!(matches!(
            LevelSet::load(&PathBuf::from("/nonexistent/rlg-levels")),
            Err(GameError::LevelDirNotFound(_))
        ));
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd /home/elite/Projects/rust-learning-game && cargo test -p game-core level`
Expected: 编译失败（模块不存在）

- [ ] **Step 3: 写实现**

`crates/game-core/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GameError {
    #[error("TOML 解析失败 {0}: {1}")]
    TomlParse(String, String),
    #[error("关卡目录不存在或为空: {0}")]
    LevelDirNotFound(String),
    #[error("关卡 ID 重复: {0}")]
    DuplicateLevelId(String),
    #[error("关卡不存在: {0}")]
    LevelNotFound(String),
    #[error("关卡未解锁: {0}")]
    LevelLocked(String),
    #[error("编译超时（超过 {0} 秒）")]
    CompileTimeout(u64),
    #[error("运行超时（超过 {0} 秒）")]
    RunTimeout(u64),
    #[error("编译环境错误: {0}")]
    CompileEnv(String),
    #[error("运行环境错误: {0}")]
    RunEnv(String),
    #[error("存档损坏: {0}")]
    CorruptSave(String),
    #[error("沙盒拦截: {0}")]
    SandboxBlocked(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}
```

`crates/game-core/src/level.rs`:
```rust
use serde::Deserialize;
use std::path::Path;
use std::collections::HashSet;

use crate::error::GameError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LevelTier {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl LevelTier {
    pub fn order(&self) -> u8 {
        match self {
            LevelTier::L0 => 0,
            LevelTier::L1 => 1,
            LevelTier::L2 => 2,
            LevelTier::L3 => 3,
            LevelTier::L4 => 4,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Level {
    pub id: String,
    pub title: String,
    pub tier: LevelTier,
    pub description: String,
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub hints: Vec<String>,
    #[serde(default)]
    pub starter_code: String,
    #[serde(default)]
    pub expect_output: String,
    #[serde(default)]
    pub allow_compile_fail: bool,
    #[serde(default)]
    pub expect_error_code: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct LevelFile {
    level: Vec<Level>,
}

/// 解析一份 TOML 内容（可能含多个 [[level]]），供 load 与测试复用
pub fn parse_levels(content: &str) -> Result<Vec<Level>, GameError> {
    let file: LevelFile = toml::from_str(content)
        .map_err(|e| GameError::TomlParse("关卡内容".into(), e.to_string()))?;
    Ok(file.level)
}

#[derive(Debug, Clone, Default)]
pub struct LevelSet {
    pub levels: Vec<Level>,
}

impl LevelSet {
    /// 从目录加载全部关卡 TOML，按文件名排序形成线性关卡顺序
    pub fn load(dir: &Path) -> Result<Self, GameError> {
        if !dir.exists() {
            return Err(GameError::LevelDirNotFound(dir.display().to_string()));
        }
        let mut files: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |x| x == "toml"))
            .collect();
        files.sort();
        let mut levels = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for f in files {
            let content = std::fs::read_to_string(&f)?;
            let parsed = parse_levels(&content)
                .map_err(|e| match e {
                    GameError::TomlParse(_, msg) => {
                        GameError::TomlParse(f.display().to_string(), msg)
                    }
                    other => other,
                })?;
            for lvl in parsed {
                if !seen.insert(lvl.id.clone()) {
                    return Err(GameError::DuplicateLevelId(lvl.id.clone()));
                }
                levels.push(lvl);
            }
        }
        if levels.is_empty() {
            return Err(GameError::LevelDirNotFound(dir.display().to_string()));
        }
        Ok(Self { levels })
    }

    pub fn get(&self, id: &str) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}
```

`crates/game-core/src/lib.rs`:
```rust
pub mod error;
pub mod level;

pub use error::GameError;
pub use level::{Level, LevelSet, LevelTier};
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-core`
Expected: 全部 PASS（含 Task 1 的 placeholder 无测试，忽略）

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): 错误类型与关卡模型 LevelSet"
```

---

### Task 3: 存档模块

**Files:**
- Create: `crates/game-core/src/save.rs`
- Modify: `crates/game-core/src/lib.rs`
- Test: `crates/game-core/src/save.rs`（内联 `#[cfg(test)]`）

**Interfaces:**
- Consumes: `GameError`（error.rs）
- Produces:
  - `LevelState::{Locked, Unlocked, Passed}`（lowercase serde，Default=Locked）
  - `LevelProgress { state, attempts, completed_at: Option<String> }`，Default 实现
  - `SaveData { xp: u32, combo: u32, max_combo: u32, level_states: HashMap<String, LevelProgress>, total_errors: u32 }`，全部 `#[serde(default)]`，derive Default
  - `pub fn load(path: &Path) -> Result<SaveData, GameError>`（文件不存在 → 返回 default；损坏 → `CorruptSave`）
  - `pub fn save(data: &SaveData, path: &Path) -> Result<(), GameError>`（写 `.toml.tmp` 后 rename，原子写）

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rlg-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn default_when_missing() {
        let p = temp_path("missing.toml");
        let _ = std::fs::remove_file(&p);
        let data = load(&p).unwrap();
        assert_eq!(data.xp, 0);
        assert!(data.level_states.is_empty());
    }

    #[test]
    fn roundtrip() {
        let p = temp_path("roundtrip.toml");
        let mut data = SaveData::default();
        data.xp = 120;
        data.combo = 3;
        data.level_states.insert(
            "l0-hello".into(),
            LevelProgress { state: LevelState::Passed, attempts: 2, completed_at: Some("1720000000".into()) },
        );
        save(&data, &p).unwrap();
        let loaded = load(&p).unwrap();
        assert_eq!(loaded.xp, 120);
        assert_eq!(loaded.combo, 3);
        let prog = loaded.level_states.get("l0-hello").unwrap();
        assert_eq!(prog.state, LevelState::Passed);
        assert_eq!(prog.attempts, 2);
        // 原子写：临时文件应不存在
        assert!(!p.with_extension("toml.tmp").exists());
    }

    #[test]
    fn corrupt_file_returns_corrupt_save() {
        let p = temp_path("corrupt.toml");
        std::fs::write(&p, "not a toml [[[").unwrap();
        assert!(matches!(load(&p), Err(GameError::CorruptSave(_))));
    }

    #[test]
    fn state_serde_lowercase() {
        let s: LevelState = toml::from_str("\"passed\"").unwrap();
        assert_eq!(s, LevelState::Passed);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p game-core save`
Expected: 编译失败（save 模块不存在）

- [ ] **Step 3: 写实现**

`crates/game-core/src/save.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::GameError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LevelState {
    #[default]
    Locked,
    Unlocked,
    Passed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelProgress {
    #[serde(default)]
    pub state: LevelState,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub completed_at: Option<String>,
}

impl Default for LevelProgress {
    fn default() -> Self {
        Self { state: LevelState::Locked, attempts: 0, completed_at: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SaveData {
    #[serde(default)]
    pub xp: u32,
    #[serde(default)]
    pub combo: u32,
    #[serde(default)]
    pub max_combo: u32,
    #[serde(default)]
    pub level_states: HashMap<String, LevelProgress>,
    #[serde(default)]
    pub total_errors: u32,
}

pub fn load(path: &Path) -> Result<SaveData, GameError> {
    if !path.exists() {
        return Ok(SaveData::default());
    }
    let content = std::fs::read_to_string(path)?;
    toml::from_str(&content).map_err(|e| GameError::CorruptSave(e.to_string()))
}

pub fn save(data: &SaveData, path: &Path) -> Result<(), GameError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("toml.tmp");
    let content =
        toml::to_string_pretty(data).map_err(|e| GameError::CorruptSave(e.to_string()))?;
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
```

`crates/game-core/src/lib.rs`（在现有内容后追加）:
```rust
pub mod save;

pub use save::{load as load_save, save as save_game, LevelProgress, LevelState, SaveData};
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-core`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): 存档模块（原子写 + 损坏恢复）"
```

---

### Task 4: 编译器错误解析器

**Files:**
- Create: `crates/game-core/src/validate/mod.rs`
- Create: `crates/game-core/src/validate/error_parser.rs`
- Modify: `crates/game-core/src/lib.rs`
- Test: `crates/game-core/src/validate/error_parser.rs`（内联测试）

**Interfaces:**
- Consumes: 无（纯函数）
- Produces:
  - `pub struct CompileError { pub code: String, pub line: Option<u32>, pub message: String }`
  - `pub fn parse_rustc_stderr(stderr: &str) -> Vec<CompileError>` —— **只按错误码 `error[E0xxx]` 提取**，行号从 `--> ...:行:列` 提取，不匹配任何报错文本

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_error_with_line() {
        let stderr = "\
error[E0308]: mismatched types
  --> src/main.rs:3:9
   |
3  | let x: i32 = \"a\";
   |              ^^^ expected `i32`, found `&str`
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0308");
        assert_eq!(errors[0].line, Some(3));
        assert!(errors[0].message.contains("mismatched types"));
    }

    #[test]
    fn multiple_errors() {
        let stderr = "\
error[E0502]: cannot borrow `s` as mutable because it is also borrowed as immutable
  --> src/main.rs:5:10
error[E0382]: use of moved value: `s`
  --> src/main.rs:9:20
";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].code, "E0502");
        assert_eq!(errors[1].code, "E0382");
    }

    #[test]
    fn no_error_when_clean() {
        assert!(parse_rustc_stderr("warning: unused variable\n").is_empty());
    }

    #[test]
    fn missing_line_ok() {
        let stderr = "error[E0106]: missing lifetime specifier\n  --> /tmp/rlg-x/main.rs:2:1\n";
        let errors = parse_rustc_stderr(stderr);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].line, Some(2));
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p game-core error_parser`
Expected: 编译失败（模块不存在）

- [ ] **Step 3: 写实现**

`crates/game-core/src/validate/error_parser.rs`:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub code: String,
    pub line: Option<u32>,
    pub message: String,
}

/// 解析 rustc stderr。只依赖错误码 `error[E0xxx]` 与 `--> path:line:col` 定位行，
/// 不匹配任何具体报错文本（rustc 版本可能微调措辞，错误码稳定）。
pub fn parse_rustc_stderr(stderr: &str) -> Vec<CompileError> {
    let mut errors: Vec<CompileError> = Vec::new();
    for line in stderr.lines() {
        let t = line.trim();
        if let Some(pos) = t.find("error[E") {
            // "error[E0308]: ..." -> code = 6 chars after "error[E"
            let code_end = (pos + 6 + 5).min(t.len());
            let code = t[pos + 6..code_end].to_string();
            // 去掉 "]: " 前缀得到纯消息（trim 掉开头的 ']' 与 ':'）
            let message = t[code_end..]
                .trim_start_matches(|c| c == ']' || c == ':')
                .trim()
                .to_string();
            errors.push(CompileError { code, line: None, message });
        } else if let Some(idx) = t.find("--> ") {
            if let Some(last) = errors.last_mut() {
                if last.line.is_none() {
                    // "--> /path/main.rs:3:5" -> 取最后两段 :分隔
                    let loc = &t[idx + 5..];
                    let mut parts = loc.rsplitn(3, ':');
                    let _col = parts.next();
                    if let Some(line) = parts.next().and_then(|s| s.parse::<u32>().ok()) {
                        last.line = Some(line);
                    }
                }
            }
        }
    }
    errors
}
```

`crates/game-core/src/validate/mod.rs`（本任务只挂模块，编排函数在 Task 7 补）:
```rust
pub mod error_parser;
pub mod mapper;
```

`crates/game-core/src/lib.rs`（追加）:
```rust
pub mod validate;

pub use validate::error_parser::{parse_rustc_stderr, CompileError};
```

注意：`mapper` 模块在 Task 5 创建，本任务结束前 validate/mod.rs 里的 `pub mod mapper;` 会编译失败——因此本任务 Step 4 先创建 `crates/game-core/src/validate/mapper.rs` 空文件占位（`// 占位，Task 5 实现`），Task 5 再填充。

- [ ] **Step 4: 创建 mapper 占位文件并跑测试**

Create `crates/game-core/src/validate/mapper.rs`:
```rust
// 占位：ErrorMapper 在 Task 5 实现
```

Run: `cargo test -p game-core`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): rustc stderr 解析器（按错误码提取）"
```

---

### Task 5: 错误码映射表 ErrorMapper

**Files:**
- Create: `crates/game-core/src/validate/mapper.rs`（填充占位）
- Test: `crates/game-core/src/validate/mapper.rs`（内联测试，用 fixture TOML）

**Interfaces:**
- Consumes: `GameError`
- Produces:
  - `pub struct ErrorInfo { pub zh: String, pub link: String }`（Deserialize）
  - `pub struct ErrorMapper`，`Default`、`is_empty()`
  - `ErrorMapper::load(path: &Path) -> Result<Self, GameError>`（TOML 表：`code = { zh = "...", link = "..." }`）
  - `ErrorMapper::lookup(&self, code: &str) -> Option<&ErrorInfo>`
  - `pub fn default_fallback() -> Self`：内置最小兜底表（E0308/E0382/E0502/E0596/E0106），防止 assets 缺失时全空

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
[E0308]
zh = "类型不匹配：表达式的实际类型与期望类型不一致"
link = "https://doc.rust-lang.org/error_codes/E0308.html"

[E0502]
zh = "同时存在不可变借用与可变借用，Rust 不允许"
link = "https://doc.rust-lang.org/error_codes/E0502.html"
"#;

    #[test]
    fn load_and_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("errors.toml");
        std::fs::write(&p, FIXTURE).unwrap();
        let m = ErrorMapper::load(&p).unwrap();
        assert_eq!(m.lookup("E0308").unwrap().zh, "类型不匹配：表达式的实际类型与期望类型不一致");
        assert!(m.lookup("E9999").is_none());
    }

    #[test]
    fn missing_file_is_default_empty() {
        // 调用方用 unwrap_or_default 兜底，这里验证 default 行为
        let m = ErrorMapper::default();
        assert!(m.is_empty());
    }

    #[test]
    fn fallback_has_common_codes() {
        let m = ErrorMapper::default_fallback();
        assert!(m.lookup("E0308").is_some());
        assert!(m.lookup("E0382").is_some());
        assert!(m.lookup("E0502").is_some());
        assert!(m.lookup("E0596").is_some());
        assert!(m.lookup("E0106").is_some());
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p game-core mapper`
Expected: 编译失败（ErrorMapper 未定义）

- [ ] **Step 3: 写实现**

`crates/game-core/src/validate/mapper.rs`（整体替换占位文件）:
```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::error::GameError;

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorInfo {
    pub zh: String,
    pub link: String,
}

#[derive(Debug, Clone, Default)]
pub struct ErrorMapper {
    map: HashMap<String, ErrorInfo>,
}

impl ErrorMapper {
    pub fn load(path: &Path) -> Result<Self, GameError> {
        let content = std::fs::read_to_string(path)?;
        let map: HashMap<String, ErrorInfo> = toml::from_str(&content)
            .map_err(|e| GameError::TomlParse(path.display().to_string(), e.to_string()))?;
        Ok(Self { map })
    }

    pub fn lookup(&self, code: &str) -> Option<&ErrorInfo> {
        self.map.get(code)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// 最小兜底表：assets 缺失或映射不全时保证常见错误仍有中文提示
    pub fn default_fallback() -> Self {
        let map = HashMap::from([
            (
                "E0308".to_string(),
                ErrorInfo { zh: "类型不匹配：表达式的实际类型与期望类型不一致".into(), link: "https://doc.rust-lang.org/error_codes/E0308.html".into() },
            ),
            (
                "E0382".to_string(),
                ErrorInfo { zh: "使用了已移动的值：所有权已转移，无法再使用原变量".into(), link: "https://doc.rust-lang.org/error_codes/E0382.html".into() },
            ),
            (
                "E0502".to_string(),
                ErrorInfo { zh: "同时存在不可变借用与可变借用，Rust 不允许".into(), link: "https://doc.rust-lang.org/error_codes/E0502.html".into() },
            ),
            (
                "E0596".to_string(),
                ErrorInfo { zh: "无法以可变方式借用：变量需要声明为 mut".into(), link: "https://doc.rust-lang.org/error_codes/E0596.html".into() },
            ),
            (
                "E0106".to_string(),
                ErrorInfo { zh: "缺少生命周期标注：需要为引用显式标注生命周期".into(), link: "https://doc.rust-lang.org/error_codes/E0106.html".into() },
            ),
        ]);
        Self { map }
    }
}
```

`crates/game-core/src/lib.rs`（追加导出）:
```rust
pub use validate::mapper::{ErrorInfo, ErrorMapper};
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-core`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): 错误码中文映射表 ErrorMapper"
```
---

### Task 6: 开发期沙盒 DevSandbox（编译/运行/超时/syn 拦截）

**Files:**
- Create: `crates/game-core/src/sandbox.rs`
- Modify: `crates/game-core/src/lib.rs`
- Test: `crates/game-core/src/sandbox.rs`（内联测试 + 真实 rustc 集成测试）

**Interfaces:**
- Consumes: `GameError`、`parse_rustc_stderr`/`CompileError`（Task 4）
- Produces:
  - `pub trait Sandbox { fn compile(&self, code: &str) -> Result<CompileOutcome, GameError>; fn run(&self, binary: &Path) -> Result<RunOutcome, GameError>; }`
  - `pub enum CompileOutcome { Success { binary: PathBuf }, Failed { errors: Vec<CompileError> } }`
  - `pub enum RunOutcome { Ok { stdout: String }, Panic { message: String }, Timeout }`
  - `pub struct DevSandbox { pub compile_timeout_secs: u64, pub run_timeout_secs: u64 }`，`new()` = (10, 2)
  - 编译前 syn 静态扫描：`std::fs` / `std::net` / `std::process` / `std::env` / `std::thread` 任一出现 → `Err(GameError::SandboxBlocked)`
  - 运行超时/编译超时用 `try_wait` 轮询 + `kill`；输出用 pipe 捕获（小输出，pipe 不阻塞）

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::GameError;

    fn sandbox() -> DevSandbox {
        DevSandbox::new()
    }

    #[test]
    fn compile_success_then_run_ok() {
        let code = "fn main() { println!(\"hello {}\", 42); }";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Success { binary } => match sandbox().run(&binary).unwrap() {
                RunOutcome::Ok { stdout } => assert_eq!(stdout.trim(), "hello 42"),
                other => panic!("expected Ok, got {:?}", other),
            },
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn compile_failed_parses_error_code() {
        // E0502: 同时存在不可变与可变借用
        let code = "fn main() {\n    let mut s = String::from(\"hi\");\n    let r1 = &s;\n    let r2 = &mut s;\n    println!(\"{} {}\", r1, r2);\n}";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Failed { errors } => {
                assert!(!errors.is_empty());
                assert!(errors.iter().any(|e| e.code == "E0502"));
            }
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn run_panic_captures_message() {
        let code = "fn main() { panic!(\"boom {}\", 1); }";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Success { binary } => match sandbox().run(&binary).unwrap() {
                RunOutcome::Panic { message } => assert!(message.contains("boom"), "msg: {message}"),
                other => panic!("expected Panic, got {:?}", other),
            },
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn run_timeout_kills_infinite_loop() {
        let code = "fn main() { loop {} }";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Success { binary } => {
                let mut sb = sandbox();
                sb.run_timeout_secs = 1; // 测试用短超时
                assert!(matches!(sb.run(&binary).unwrap(), RunOutcome::Timeout));
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn blocked_fs_api_rejected() {
        let code = "fn main() { let _ = std::fs::read_to_string(\"/etc/passwd\"); }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn blocked_use_statement_rejected() {
        let code = "use std::net::TcpStream;\nfn main() { let _ = TcpStream::connect(\"x\"); }";
        assert!(matches!(sandbox().compile(code), Err(GameError::SandboxBlocked(_))));
    }

    #[test]
    fn string_mention_of_fs_not_blocked() {
        let code = "fn main() { println!(\"std::fs is not executed\"); }";
        assert!(matches!(sandbox().compile(code), Ok(CompileOutcome::Success { .. })));
    }

    #[test]
    fn output_mismatch_is_ok_with_expected_out() {
        // 运行输出由调用方比对；这里验证 stdout 原样返回
        let code = "fn main() { println!(\"a\\nb\"); }";
        match sandbox().compile(code).unwrap() {
            CompileOutcome::Success { binary } => match sandbox().run(&binary).unwrap() {
                RunOutcome::Ok { stdout } => assert_eq!(stdout, "a\nb\n"),
                other => panic!("expected Ok, got {:?}", other),
            },
            other => panic!("expected Success, got {:?}", other),
        }
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p game-core sandbox`
Expected: 编译失败（sandbox 模块不存在）

- [ ] **Step 3: 写实现**

`crates/game-core/src/sandbox.rs`:
```rust
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::GameError;
use crate::validate::error_parser::{parse_rustc_stderr, CompileError};

pub trait Sandbox {
    fn compile(&self, code: &str) -> Result<CompileOutcome, GameError>;
    fn run(&self, binary: &Path) -> Result<RunOutcome, GameError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileOutcome {
    Success { binary: PathBuf },
    Failed { errors: Vec<CompileError> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    Ok { stdout: String },
    Panic { message: String },
    Timeout,
}

/// 开发期沙盒：临时目录 + 超时 + syn 静态拦截。
/// 真隔离（bwrap）在计划②实现，实现新的 Sandbox 类型即可替换。
pub struct DevSandbox {
    pub compile_timeout_secs: u64,
    pub run_timeout_secs: u64,
}

impl DevSandbox {
    pub fn new() -> Self {
        Self { compile_timeout_secs: 10, run_timeout_secs: 2 }
    }
}

impl Default for DevSandbox {
    fn default() -> Self {
        Self::new()
    }
}

const BLOCKED_PREFIXES: [&str; 5] = ["std::fs", "std::net", "std::process", "std::env", "std::thread"];

fn use_tree_str(t: &syn::UseTree) -> String {
    match t {
        syn::UseTree::Path(p) => format!("{}::{}", p.ident, use_tree_str(&p.tree)),
        syn::UseTree::Name(n) => n.ident.to_string(),
        syn::UseTree::Rename(r) => r.ident.to_string(),
        syn::UseTree::Glob(_) => "*".to_string(),
        syn::UseTree::Group(g) => g.items.iter().map(use_tree_str).collect::<Vec<_>>().join(","),
    }
}

/// 粗略静态拦截：玩家代码中禁止访问文件系统、网络、进程、环境变量、线程。
/// 只扫描 AST 路径（注释/字符串不会误报）。
fn check_blocked_apis(code: &str) -> Result<(), GameError> {
    let ast: syn::File = syn::parse_file(code)
        .map_err(|e| GameError::SandboxBlocked(format!("代码语法错误: {e}")))?;
    let mut blocked: Option<String> = None;

    struct Scan<'a> {
        blocked: &'a mut Option<String>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Scan<'_> {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            let s = node
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            for p in BLOCKED_PREFIXES {
                if s.starts_with(p) {
                    *self.blocked = Some(s);
                    return;
                }
            }
            syn::visit::visit_expr_path(self, node);
        }
        fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
            let s = node
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            for p in BLOCKED_PREFIXES {
                if s.starts_with(p) {
                    *self.blocked = Some(s);
                    return;
                }
            }
            syn::visit::visit_type_path(self, node);
        }
        fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
            let s = use_tree_str(&node.tree);
            for p in BLOCKED_PREFIXES {
                if s.starts_with(p) {
                    *self.blocked = Some(s);
                    return;
                }
            }
            syn::visit::visit_item_use(self, node);
        }
    }

    let mut scan = Scan { blocked: &mut blocked };
    syn::visit::Visit::visit_file(&mut scan, &ast);

    if let Some(s) = blocked {
        return Err(GameError::SandboxBlocked(format!("检测到被禁用的 API：{s}")));
    }
    Ok(())
}

fn wait_with_timeout(child: &mut std::process::Child, secs: u64) -> Result<std::process::ExitStatus, GameError> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(GameError::CompileTimeout(secs));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read_piped(mut child: &mut std::process::Child) -> (String, String) {
    use std::io::Read;
    let mut out = String::new();
    let mut err = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    (out, err)
}

impl Sandbox for DevSandbox {
    fn compile(&self, code: &str) -> Result<CompileOutcome, GameError> {
        check_blocked_apis(code)?;

        let dir = tempfile::Builder::new()
            .prefix("rlg-")
            .tempdir()
            .map_err(|e| GameError::CompileEnv(e.to_string()))?;
        let src = dir.path().join("main.rs");
        let out = dir.path().join("main");
        std::fs::write(&src, code)?;

        let mut child = Command::new("rustc")
            .arg("--edition")
            .arg("2021")
            .arg(&src)
            .arg("-o")
            .arg(&out)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GameError::CompileEnv(format!("无法启动 rustc: {e}")))?;

        let status = wait_with_timeout(&mut child, self.compile_timeout_secs)?;

        if status.success() {
            Ok(CompileOutcome::Success { binary: out })
        } else {
            let (_, stderr) = read_piped(&mut child);
            Ok(CompileOutcome::Failed { errors: parse_rustc_stderr(&stderr) })
        }
    }

    fn run(&self, binary: &Path) -> Result<RunOutcome, GameError> {
        let mut child = Command::new(binary)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GameError::RunEnv(format!("无法启动玩家程序: {e}")))?;

        let deadline = Instant::now() + Duration::from_secs(self.run_timeout_secs);
        let status = loop {
            if let Some(st) = child.try_wait()? {
                break st;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(RunOutcome::Timeout);
            }
            std::thread::sleep(Duration::from_millis(50));
        };

        let (stdout, stderr) = read_piped(&mut child);
        if status.success() {
            Ok(RunOutcome::Ok { stdout })
        } else {
            let msg = if stderr.trim().is_empty() {
                format!("程序以非零退出码退出（code {:?}）", status.code())
            } else {
                stderr.trim().to_string()
            };
            Ok(RunOutcome::Panic { message: msg })
        }
    }
}
```

`crates/game-core/src/lib.rs`（追加）:
```rust
pub mod sandbox;

pub use sandbox::{CompileOutcome, DevSandbox, RunOutcome, Sandbox};
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-core sandbox -- --nocapture`
Expected: 全部 PASS（真实 rustc 编译，约需数秒）。若 `run_timeout_kills_infinite_loop` 耗时 >5s 说明超时逻辑失效，需排查

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): DevSandbox 编译/运行/超时/syn 静态拦截"
```

---

### Task 7: 校验编排 validate()

**Files:**
- Modify: `crates/game-core/src/validate/mod.rs`（补编排函数）
- Modify: `crates/game-core/src/lib.rs`
- Test: `crates/game-core/src/validate/mod.rs`（内联测试，用真实 rustc + fixture 关卡）

**Interfaces:**
- Consumes: `Level`、`Sandbox`/`CompileOutcome`/`RunOutcome`、`ErrorMapper`、`parse_rustc_stderr`
- Produces:
  - `pub enum Validation { Pass, Fail { feedback: Vec<String> } }`
  - `pub fn validate(level: &Level, code: &str, mapper: &ErrorMapper, sandbox: &dyn Sandbox) -> Result<Validation, GameError>`
  - 逻辑：编译失败 → allow_compile_fail 时比对 `expect_error_code`（相等→Pass，否则 Fail 说明实际码）；否则把错误翻译成中文反馈。编译成功 → allow_compile_fail 时 Fail（"要求制造错误但编译成功"）；否则运行：Ok 且 stdout（trim）== expect_output（trim）或 expect_output 为空 → Pass；Ok 但输出不符 → Fail 显示期望/实际；Panic → Fail 显示 panic 信息；Timeout → `Err(RunTimeout)`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Level, LevelTier};
    use crate::sandbox::DevSandbox;
    use crate::validate::mapper::ErrorMapper;

    fn level(id: &str, expect_output: &str, allow_fail: bool, expect_code: &str) -> Level {
        Level {
            id: id.into(),
            title: "t".into(),
            tier: LevelTier::L1,
            description: "d".into(),
            hint: String::new(),
            hints: Vec::new(),
            starter_code: String::new(),
            expect_output: expect_output.into(),
            allow_compile_fail: allow_fail,
            expect_error_code: expect_code.into(),
            source: "test".into(),
        }
    }

    fn sb() -> DevSandbox {
        DevSandbox::new()
    }

    #[test]
    fn pass_when_output_matches() {
        let lv = level("t1", "hello 42", false, "");
        let code = "fn main() { println!(\"hello {}\", 42); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn pass_when_no_output_required() {
        let lv = level("t2", "", false, "");
        let code = "fn main() { println!(\"anything\"); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn fail_when_output_mismatch_shows_expectation() {
        let lv = level("t3", "wanted", false, "");
        let code = "fn main() { println!(\"got\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("wanted"), "feedback: {feedback:?}");
                assert!(feedback[0].contains("got"));
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn fail_compile_error_mapped_to_chinese() {
        let lv = level("t4", "", false, "");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("E0382"), "feedback: {feedback:?}");
                assert!(feedback[0].contains("所有权"), "中文映射缺失: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn allow_compile_fail_matches_code() {
        let lv = level("t5", "", true, "E0382");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        assert_eq!(validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap(), Validation::Pass);
    }

    #[test]
    fn allow_compile_fail_wrong_code_fails() {
        let lv = level("t6", "", true, "E0502");
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("E0382"), "feedback: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn allow_compile_fail_but_success_fails() {
        let lv = level("t7", "", true, "E0308");
        let code = "fn main() { println!(\"ok\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => assert!(feedback[0].contains("编译成功")),
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn panic_reported() {
        let lv = level("t8", "", false, "");
        let code = "fn main() { panic!(\"kaboom\"); }";
        match validate(&lv, code, &ErrorMapper::default_fallback(), &sb()).unwrap() {
            Validation::Fail { feedback } => {
                assert!(feedback[0].contains("panic"), "feedback: {feedback:?}");
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p game-core validate::tests`
Expected: 编译失败（Validation/validate 未定义）

- [ ] **Step 3: 写实现**

`crates/game-core/src/validate/mod.rs`（整体替换为）:
```rust
pub mod error_parser;
pub mod mapper;

use crate::error::GameError;
use crate::level::Level;
use crate::sandbox::{CompileOutcome, RunOutcome, Sandbox};
use crate::validate::mapper::ErrorMapper;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Pass,
    Fail { feedback: Vec<String> },
}

/// 核心校验：编译 → （失败按 allow_compile_fail 分支；成功）→ 运行 → 比对 stdout
pub fn validate(
    level: &Level,
    code: &str,
    mapper: &ErrorMapper,
    sandbox: &dyn Sandbox,
) -> Result<Validation, GameError> {
    let compile = sandbox.compile(code)?;
    match compile {
        CompileOutcome::Failed { errors } => {
            if level.allow_compile_fail {
                let got = errors.first().map(|e| e.code.clone()).unwrap_or_default();
                if !level.expect_error_code.is_empty() && got == level.expect_error_code {
                    return Ok(Validation::Pass);
                }
                let shown = if got.is_empty() { "无错误".to_string() } else { got };
                return Ok(Validation::Fail {
                    feedback: vec![format!(
                        "需要制造编译错误 {}，实际得到 {}\n（先看第一条错误，再调整代码）",
                        level.expect_error_code, shown
                    )],
                });
            }
            let feedback = errors
                .iter()
                .map(|e| {
                    let loc = e.line.map(|l| format!("（第 {l} 行）")).unwrap_or_default();
                    let zh = mapper
                        .lookup(&e.code)
                        .map(|i| format!("  💡 {}（{}）", i.zh, i.link))
                        .unwrap_or_default();
                    format!("{}{} {}: {}", e.code, loc, e.message, zh)
                })
                .collect();
            Ok(Validation::Fail { feedback })
        }
        CompileOutcome::Success { binary } => {
            if level.allow_compile_fail {
                return Ok(Validation::Fail {
                    feedback: vec!["该关卡要求制造编译错误，但代码编译成功了".to_string()],
                });
            }
            match sandbox.run(&binary)? {
                RunOutcome::Ok { stdout } => {
                    if level.expect_output.trim().is_empty() || stdout.trim() == level.expect_output.trim() {
                        Ok(Validation::Pass)
                    } else {
                        Ok(Validation::Fail {
                            feedback: vec![format!(
                                "编译通过，但输出不符合要求。\n期望输出：{}\n实际输出：{}",
                                level.expect_output.trim(),
                                stdout.trim()
                            )],
                        })
                    }
                }
                RunOutcome::Panic { message } => Ok(Validation::Fail {
                    feedback: vec![format!("程序运行时出错（panic）：\n{}", message)],
                }),
                RunOutcome::Timeout => Err(GameError::RunTimeout(2)),
            }
        }
    }
}
```

`crates/game-core/src/lib.rs`（追加）:
```rust
pub use validate::{validate, Validation};
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-core validate`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): 校验编排 compile-run-compare + 制造错误关卡分支"
```

---

### Task 8: 代码着色 tokenizer（syn 分词 → 颜色片段）

**Files:**
- Create: `crates/game-core/src/editor.rs`
- Modify: `crates/game-core/src/lib.rs`
- Test: `crates/game-core/src/editor.rs`（内联测试）

**Interfaces:**
- Consumes: 无
- Produces:
  - `pub enum TokenKind { Keyword, Comment, String, Number, Normal }`（Clone, Copy, PartialEq, Eq）
  - `pub struct TokenSpan { pub kind: TokenKind, pub start: usize, pub end: usize }` —— **字节偏移**（可直接用于 `&text[start..end]` 切片）
  - `pub fn tokenize(code: &str) -> Vec<TokenSpan>` —— 轻量词法器：行注释 `//`、块注释 `/* */`（跨行）、字符串 `"..."`（含转义）、字符 `'x'`（3 字符形式；生命周期 `'a` 不当字符串处理）、数字、关键字、普通

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(code: &str) -> Vec<TokenKind> {
        tokenize(code).iter().map(|s| s.kind).collect()
    }

    fn slice<'a>(code: &'a str, spans: &[TokenSpan]) -> Vec<&'a str> {
        spans.iter().map(|s| &code[s.start..s.end]).collect()
    }

    #[test]
    fn keyword_and_normal() {
        let code = "let mut x = 5;";
        let spans = tokenize(code);
        let words = slice(code, &spans);
        assert_eq!(words, vec!["let", "mut", "x", "5"]);
        assert_eq!(kinds(code), vec![TokenKind::Keyword, TokenKind::Keyword, TokenKind::Normal, TokenKind::Number]);
    }

    #[test]
    fn line_comment() {
        let code = "// 注释\nlet x = 1;";
        let spans = tokenize(code);
        let words = slice(code, &spans);
        assert_eq!(words[0], "// 注释");
        assert_eq!(spans[0].kind, TokenKind::Comment);
    }

    #[test]
    fn block_comment_multiline() {
        let code = "/* a\nb */ let x = 1;";
        let spans = tokenize(code);
        assert_eq!(spans[0].kind, TokenKind::Comment);
        assert_eq!(&code[spans[0].start..spans[0].end], "/* a\nb */");
    }

    #[test]
    fn string_with_escape() {
        let code = "let s = \"a\\\"b\";";
        let spans = tokenize(code);
        assert!(spans.iter().any(|s| s.kind == TokenKind::String));
        let words = slice(code, &spans);
        assert_eq!(words[2], "\"a\\\"b\"");
    }

    #[test]
    fn char_literal() {
        let code = "let c = 'x';";
        let spans = tokenize(code);
        assert!(spans.iter().any(|s| s.kind == TokenKind::String && &code[s.start..s.end] == "'x'"));
    }

    #[test]
    fn lifetime_is_normal_not_string() {
        let code = "fn f<'a>(x: &'a str) -> &'a str { x }";
        let spans = tokenize(code);
        assert!(!spans.iter().any(|s| s.kind == TokenKind::String));
        assert!(spans.iter().any(|s| s.kind == TokenKind::Normal && &code[s.start..s.end] == "'a"));
    }

    #[test]
    fn raw_string_supported() {
        // 原始字符串内嵌引号不会提前终止
        let code = "let s = r#\"a \\\"b\\\" c\"#;";
        let spans = tokenize(code);
        assert!(spans.iter().any(|s| s.kind == TokenKind::String && &code[s.start..s.end] == "r#\"a \\\"b\\\" c\"#"));
    }

    #[test]
    fn bool_literal_keyword_colored() {
        let code = "let b = true;";
        let spans = tokenize(code);
        assert!(spans.iter().any(|s| s.kind == TokenKind::Keyword && &code[s.start..s.end] == "true"));
    }

    #[test]
    fn byte_offsets_valid_slices() {
        let code = "let 中文 = 1;";
        let spans = tokenize(code);
        for s in &spans {
            let _ = &code[s.start..s.end];
        }
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p game-core editor`
Expected: 编译失败（editor 模块不存在）

- [ ] **Step 3: 写实现**

`crates/game-core/src/editor.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Comment,
    String,
    Number,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSpan {
    pub kind: TokenKind,
    pub start: usize, // 字节偏移
    pub end: usize,
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "if", "else", "for", "while", "loop", "match", "struct", "enum",
    "impl", "trait", "pub", "use", "mod", "const", "static", "return", "move", "ref", "self",
    "Self", "super", "crate", "as", "in", "where", "async", "await", "dyn", "type", "unsafe",
    "extern", "macro_rules",
];

/// 基于 rustc_lexer（rustc 官方词法器，rust-lang/rust 仓库发布）的着色分词。
/// 输出字节偏移片段，可直接切片 &code[start..end]。
/// 支持：注释、原始字符串 r#"..."#、生命周期 'a、全部字面量形式。
pub fn tokenize(code: &str) -> Vec<TokenSpan> {
    use rustc_lexer::{tokenize, LiteralKind, TokenKind as LexKind};
    let mut spans = Vec::new();
    let mut pos = 0;
    for tok in tokenize(code) {
        let kind = match tok.kind {
            LexKind::LineComment { .. } | LexKind::BlockComment { .. } => TokenKind::Comment,
            LexKind::Ident => {
                let word = &code[pos..pos + tok.len];
                if KEYWORDS.contains(&word) {
                    TokenKind::Keyword
                } else {
                    TokenKind::Normal
                }
            }
            LexKind::RawIdent | LexKind::Lifetime { .. } => TokenKind::Normal,
            LexKind::Literal { kind, .. } => match kind {
                LiteralKind::Str { .. }
                | LiteralKind::ByteStr { .. }
                | LiteralKind::RawStr { .. }
                | LiteralKind::ByteStrRaw { .. }
                | LiteralKind::CStr { .. }
                | LiteralKind::Char { .. }
                | LiteralKind::Byte { .. } => TokenKind::String,
                LiteralKind::Int { .. } | LiteralKind::Float { .. } => TokenKind::Number,
                LiteralKind::Bool => TokenKind::Keyword,
            },
            LexKind::Whitespace => {
                pos += tok.len;
                continue;
            }
            LexKind::Punct | LexKind::Unknown => TokenKind::Normal,
            _ => TokenKind::Normal,
        };
        spans.push(TokenSpan { kind, start: pos, end: pos + tok.len });
        pos += tok.len;
    }
    spans
}

`crates/game-core/src/lib.rs`（追加）:
```rust
pub mod editor;

pub use editor::{tokenize, TokenKind, TokenSpan};
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-core editor`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): 代码着色 tokenizer（字节偏移片段）"
```

---

### Task 9: 引擎状态机 Engine

**Files:**
- Create: `crates/game-core/src/engine.rs`
- Modify: `crates/game-core/src/lib.rs`
- Test: `crates/game-core/src/engine.rs`（内联测试，真实 rustc + fixture 关卡）

**Interfaces:**
- Consumes: `LevelSet`、`SaveData`/`LevelState`/`LevelProgress`、`Validation`/`validate`、`ErrorMapper`、`Sandbox`/`DevSandbox`、`XP_PER_PASS`
- Produces:
  - `pub const XP_PER_PASS: u32 = 20;`
  - `pub struct Engine { pub level_set: LevelSet, pub save: SaveData, pub current: Option<usize>, pub mapper: ErrorMapper, pub sandbox: Box<dyn Sandbox> }`
  - `Engine::new(level_set, save, mapper, sandbox) -> Self`
  - `new_game(&mut self)`：save 重置 + `unlock_first`
  - `unlock_first(&mut self)`：第一关 → Unlocked
  - `start_level(&mut self, index: usize) -> Result<(), GameError>`：校验未锁定，设置 `current`
  - `submit(&mut self, code: &str) -> Result<Validation, GameError>`：Pass → XP+20、combo+1、标记 Passed、completed_at=unix 秒、解锁下一关；Fail → combo=0、total_errors+1、attempts+1
  - `current_level(&self) -> Option<&Level>`、`can_continue(&self) -> bool`、`save_ref(&self) -> &SaveData`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{parse_levels, LevelSet, LevelTier};
    use crate::save::{LevelState, SaveData};
    use crate::sandbox::DevSandbox;
    use crate::validate::mapper::ErrorMapper;
    use crate::validate::Validation;

    const LEVELS: &str = r#"
[[level]]
id = "l0-hello"
title = "hello"
tier = "l0"
description = "d"
starter_code = "fn main() { x = 5; println!(\"x has the value {}\", x); }"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l1-move"
title = "move"
tier = "l1"
description = "d"
starter_code = "fn main() { let s = String::from(\"hi\"); take(s); println!(\"{}\", s); } fn take(x: String) {}"
expect_output = ""
source = "rustlings"
"#;

    fn engine() -> Engine {
        let set = LevelSet { levels: parse_levels(LEVELS).unwrap() };
        Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()))
    }

    #[test]
    fn new_game_unlocks_first() {
        let mut e = engine();
        e.new_game();
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().state, LevelState::Unlocked);
        assert_eq!(e.save.level_states.get("l1-move").unwrap().state, LevelState::Locked);
    }

    #[test]
    fn locked_level_rejected() {
        let mut e = engine();
        e.new_game();
        assert!(matches!(e.start_level(1), Err(GameError::LevelLocked(_))));
    }

    #[test]
    fn pass_updates_xp_combo_and_unlocks_next() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        assert_eq!(e.submit(code).unwrap(), Validation::Pass);
        assert_eq!(e.save.xp, XP_PER_PASS);
        assert_eq!(e.save.combo, 1);
        assert_eq!(e.save.level_states.get("l0-hello").unwrap().state, LevelState::Passed);
        assert_eq!(e.save.level_states.get("l1-move").unwrap().state, LevelState::Unlocked);
        assert!(e.save.level_states.get("l0-hello").unwrap().completed_at.is_some());
    }

    #[test]
    fn fail_resets_combo_and_counts_error() {
        let mut e = engine();
        e.new_game();
        e.start_level(0).unwrap();
        // 先通关一关拿到 combo
        let code = "fn main() { println!(\"x has the value {}\", 5); }";
        e.submit(code).unwrap();
        assert_eq!(e.save.combo, 1);
        // 然后在 l1-move 上故意写错
        e.start_level(1).unwrap();
        let bad = "fn main() { println!(\"wrong\"); }";
        assert!(matches!(e.submit(bad).unwrap(), Validation::Fail { .. }));
        assert_eq!(e.save.combo, 0);
        assert_eq!(e.save.total_errors, 1);
        assert_eq!(e.save.level_states.get("l1-move").unwrap().attempts, 1);
        // 失败不改变关卡状态
        assert_eq!(e.save.level_states.get("l1-move").unwrap().state, LevelState::Unlocked);
    }

    #[test]
    fn allow_compile_fail_level_passes_with_right_error() {
        let set = LevelSet {
            levels: parse_levels(
                "[[level]]\nid = \"l1-bug\"\ntitle = \"制造错误\"\ntier = \"l1\"\ndescription = \"d\"\nstarter_code = \"\"\nallow_compile_fail = true\nexpect_error_code = \"E0382\"\nsource = \"rust-quiz\"\n",
            )
            .unwrap(),
        };
        let mut e = Engine::new(set, SaveData::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        e.new_game();
        e.start_level(0).unwrap();
        let code = "fn main() { let s = String::from(\"hi\"); let t = s; println!(\"{}\", s); }";
        assert_eq!(e.submit(code).unwrap(), Validation::Pass);
    }

    #[test]
    fn can_continue_after_progress() {
        let mut e = engine();
        assert!(!e.can_continue());
        e.new_game();
        assert!(!e.can_continue());
        e.start_level(0).unwrap();
        e.submit("fn main() { println!(\"x has the value {}\", 5); }").unwrap();
        assert!(e.can_continue());
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p game-core engine`
Expected: 编译失败（engine 模块不存在）

- [ ] **Step 3: 写实现**

`crates/game-core/src/engine.rs`:
```rust
use crate::error::GameError;
use crate::level::{Level, LevelSet};
use crate::sandbox::Sandbox;
use crate::save::{LevelProgress, LevelState, SaveData};
use crate::validate::mapper::ErrorMapper;
use crate::validate::{validate, Validation};

pub const XP_PER_PASS: u32 = 20;

pub struct Engine {
    pub level_set: LevelSet,
    pub save: SaveData,
    pub current: Option<usize>,
    pub mapper: ErrorMapper,
    pub sandbox: Box<dyn Sandbox>,
}

impl Engine {
    pub fn new(
        level_set: LevelSet,
        save: SaveData,
        mapper: ErrorMapper,
        sandbox: Box<dyn Sandbox>,
    ) -> Self {
        Self { level_set, save, current: None, mapper, sandbox }
    }

    pub fn new_game(&mut self) {
        self.save = SaveData::default();
        self.current = None;
        self.unlock_first();
    }

    pub fn unlock_first(&mut self) {
        if let Some(first) = self.level_set.levels.first() {
            self.save
                .level_states
                .entry(first.id.clone())
                .or_insert(LevelProgress { state: LevelState::Unlocked, attempts: 0, completed_at: None });
        }
    }

    pub fn start_level(&mut self, index: usize) -> Result<(), GameError> {
        let level = self
            .level_set
            .levels
            .get(index)
            .ok_or_else(|| GameError::LevelNotFound(format!("index {index}")))?;
        let state = self
            .save
            .level_states
            .get(&level.id)
            .map(|p| p.state)
            .unwrap_or(LevelState::Locked);
        if state == LevelState::Locked {
            return Err(GameError::LevelLocked(level.id.clone()));
        }
        self.current = Some(index);
        Ok(())
    }

    pub fn submit(&mut self, code: &str) -> Result<Validation, GameError> {
        let idx = self
            .current
            .ok_or_else(|| GameError::LevelNotFound("无当前关卡".into()))?;
        let level = self
            .level_set
            .levels
            .get(idx)
            .cloned()
            .ok_or_else(|| GameError::LevelNotFound(format!("index {idx}")))?;

        let result = validate(&level, code, &self.mapper, self.sandbox.as_ref())?;

        match &result {
            Validation::Pass => {
                self.save.xp += XP_PER_PASS;
                self.save.combo += 1;
                self.save.max_combo = self.save.max_combo.max(self.save.combo);
                let entry = self
                    .save
                    .level_states
                    .entry(level.id.clone())
                    .or_insert_with(|| LevelProgress {
                        state: LevelState::Unlocked,
                        attempts: 0,
                        completed_at: None,
                    });
                entry.state = LevelState::Passed;
                entry.attempts += 1;
                entry.completed_at = Some(unix_secs());
                if let Some(next) = self.level_set.levels.get(idx + 1) {
                    let n = self
                        .save
                        .level_states
                        .entry(next.id.clone())
                        .or_insert_with(LevelProgress::default);
                    if n.state == LevelState::Locked {
                        n.state = LevelState::Unlocked;
                    }
                }
            }
            Validation::Fail { .. } => {
                self.save.combo = 0;
                self.save.total_errors += 1;
                let entry = self
                    .save
                    .level_states
                    .entry(level.id.clone())
                    .or_insert_with(LevelProgress::default);
                entry.attempts += 1;
            }
        }
        Ok(result)
    }

    pub fn current_level(&self) -> Option<&Level> {
        self.current.and_then(|i| self.level_set.levels.get(i))
    }

    pub fn can_continue(&self) -> bool {
        self.save.xp > 0
            || self.save.level_states.values().any(|p| p.state == LevelState::Passed)
    }

    pub fn save_ref(&self) -> &SaveData {
        &self.save
    }
}

fn unix_secs() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}
```

`crates/game-core/src/lib.rs`（追加）:
```rust
pub mod engine;

pub use engine::{Engine, XP_PER_PASS};
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-core engine`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): 引擎状态机（解锁/通关/失败/combo）"
```

---

### Task 10: GameApp 状态机（Input → Screen）

**Files:**
- Create: `crates/game-core/src/app.rs`
- Create: `crates/game-core/src/ui.rs`
- Modify: `crates/game-core/src/lib.rs`
- Test: `crates/game-core/src/app.rs`（内联测试，真实 rustc + fixture 关卡）

**Interfaces:**
- Consumes: `Engine`、`Level`、`LevelState`、`Validation`、`XP_PER_PASS`
- Produces:
  - `pub enum Input { Up, Down, Enter, Esc, Submit, Hint, Reset }`
  - `pub enum GameFlow { Continue, Quit }`
  - `pub struct MenuData { pub selected: usize, pub can_continue: bool }`
  - `pub struct MapEntry { pub level: Level, pub state: LevelState }`
  - `pub struct ChapterMapData { pub selected: usize, pub entries: Vec<MapEntry> }`
  - `pub struct LevelData { pub level: Level, pub code: String, pub show_hint: bool, pub xp: u32, pub combo: u32, pub total: usize, pub index: usize }`
  - `pub struct FeedbackData { pub passed: bool, pub level_id: String, pub feedback: Vec<String>, pub xp_gained: u32 }`
  - `pub enum Screen { Menu(MenuData), ChapterMap(ChapterMapData), Level(LevelData), Feedback(FeedbackData) }`
  - `pub struct GameApp { pub engine: Engine, screen: Screen, last_level: Option<LevelData> }`
  - `GameApp::new(engine) -> Self`（unlock_first + 进入 ChapterMap）
  - `screen(&self) -> &Screen`、`save_ref(&self) -> &SaveData`、`set_code(&mut self, code: String)`
  - `handle(&mut self, input: Input) -> Result<GameFlow, GameError>`
  - `pub trait UiBackend { fn run(&mut self, app: &mut GameApp) -> Result<(), GameError>; }`（ui.rs，异步 trait 方法，1.75+ 原生）

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::level::{parse_levels, LevelSet};
    use crate::sandbox::DevSandbox;
    use crate::save::LevelState;
    use crate::validate::mapper::ErrorMapper;

    const LEVELS: &str = r#"
[[level]]
id = "l0-hello"
title = "hello"
tier = "l0"
description = "d"
starter_code = "fn main() { println!(\"x has the value {}\", 5); }"
expect_output = "x has the value 5"
source = "rustlings"

[[level]]
id = "l1-move"
title = "move"
tier = "l1"
description = "d"
starter_code = "fn main() { let s = String::from(\"hi\"); take(s); println!(\"{}\", s); } fn take(x: String) {}"
expect_output = ""
source = "rustlings"
"#;

    fn app() -> GameApp {
        let set = LevelSet { levels: parse_levels(LEVELS).unwrap() };
        let engine = Engine::new(set, Default::default(), ErrorMapper::default_fallback(), Box::new(DevSandbox::new()));
        GameApp::new(engine)
    }

    fn menu_selected(a: &GameApp) -> usize {
        match a.screen() {
            Screen::Menu(m) => m.selected,
            _ => panic!("not menu"),
        }
    }

    #[test]
    fn starts_in_chapter_map() {
        let a = app();
        match a.screen() {
            Screen::ChapterMap(m) => {
                assert_eq!(m.entries.len(), 2);
                assert_eq!(m.entries[0].state, LevelState::Unlocked);
                assert_eq!(m.entries[1].state, LevelState::Locked);
            }
            other => panic!("expected ChapterMap, got {:?}", other),
        }
    }

    #[test]
    fn esc_to_menu_then_quit() {
        let mut a = app();
        assert_eq!(a.handle(Input::Esc).unwrap(), GameFlow::Continue);
        let sel = menu_selected(&a);
        assert_eq!(sel, 0);
        // 无进度：菜单只有 [新游戏][退出]
        assert_eq!(a.handle(Input::Esc).unwrap(), GameFlow::Quit);
    }

    #[test]
    fn enter_level_submit_pass_feedback_then_next() {
        let mut a = app();
        a.handle(Input::Enter).unwrap(); // 进入第一关
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"x has the value {}\", 5); }"),
            other => panic!("expected Level, got {:?}", other),
        }
        a.handle(Input::Submit).unwrap();
        match a.screen() {
            Screen::Feedback(f) => {
                assert!(f.passed);
                assert_eq!(f.xp_gained, XP_PER_PASS);
            }
            other => panic!("expected Feedback, got {:?}", other),
        }
        // 回车 → 自动进入下一关
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.level.id, "l1-move"),
            other => panic!("expected Level l1-move, got {:?}", other),
        }
    }

    #[test]
    fn fail_keeps_code_and_returns_to_level() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        // 写错代码：输出不符
        a.set_code("fn main() { println!(\"wrong\"); }".into());
        a.handle(Input::Submit).unwrap();
        match a.screen() {
            Screen::Feedback(f) => {
                assert!(!f.passed);
                assert!(!f.feedback.is_empty());
            }
            other => panic!("expected Feedback fail, got {:?}", other),
        }
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"wrong\"); }"), // 代码保留
            other => panic!("expected Level, got {:?}", other),
        }
    }

    #[test]
    fn reset_restores_starter_code() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        a.set_code("fn main() { println!(\"whatever\"); }".into());
        a.handle(Input::Reset).unwrap();
        match a.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"x has the value {}\", 5); }"),
            other => panic!("expected Level, got {:?}", other),
        }
    }

    #[test]
    fn hint_toggles() {
        let mut a = app();
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::Level(d) => assert!(!d.show_hint),
            other => panic!("expected Level, got {:?}", other),
        }
        a.handle(Input::Hint).unwrap();
        match a.screen() {
            Screen::Level(d) => assert!(d.show_hint),
            other => panic!("expected Level, got {:?}", other),
        }
    }

    #[test]
    fn menu_new_game_flow() {
        let mut a = app();
        // 先通关一关制造进度
        a.handle(Input::Enter).unwrap();
        a.handle(Input::Submit).unwrap();
        a.handle(Input::Enter).unwrap();
        // 回到地图再 Esc 到菜单
        a.handle(Input::Esc).unwrap();
        assert_eq!(menu_selected(&a), 0);
        // 继续游戏（Enter）回地图
        a.handle(Input::Enter).unwrap();
        match a.screen() {
            Screen::ChapterMap(m) => assert!(m.entries[0].state == LevelState::Passed),
            other => panic!("expected ChapterMap, got {:?}", other),
        }
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p game-core app`
Expected: 编译失败（app/ui 模块不存在）

- [ ] **Step 3: 写实现**

`crates/game-core/src/ui.rs`:
```rust
use crate::app::GameApp;
use crate::error::GameError;

/// UI 后端抽象：核心产出 Screen/Input 纯数据，后端负责渲染与事件采集。
/// macroquad+egui 是实现之一；未来 ratatui 版再实现一份。
/// async：macroquad 的主循环依赖 async（next_frame().await），trait 方法用原生 async fn（Rust 1.75+）。
pub trait UiBackend {
    async fn run(&mut self, app: &mut GameApp) -> Result<(), GameError>;
}
```

`crates/game-core/src/app.rs`:
```rust
use crate::engine::{Engine, XP_PER_PASS};
use crate::error::GameError;
use crate::level::Level;
use crate::save::{LevelState, SaveData};
use crate::validate::Validation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Enter,
    Esc,
    Submit,
    Hint,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameFlow {
    Continue,
    Quit,
}

#[derive(Debug, Clone)]
pub struct MenuData {
    pub selected: usize,
    pub can_continue: bool,
}

#[derive(Debug, Clone)]
pub struct MapEntry {
    pub level: Level,
    pub state: LevelState,
}

#[derive(Debug, Clone)]
pub struct ChapterMapData {
    pub selected: usize,
    pub entries: Vec<MapEntry>,
}

#[derive(Debug, Clone)]
pub struct LevelData {
    pub level: Level,
    pub code: String,
    pub show_hint: bool,
    pub xp: u32,
    pub combo: u32,
    pub total: usize,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct FeedbackData {
    pub passed: bool,
    pub level_id: String,
    pub feedback: Vec<String>,
    pub xp_gained: u32,
}

#[derive(Debug, Clone)]
pub enum Screen {
    Menu(MenuData),
    ChapterMap(ChapterMapData),
    Level(LevelData),
    Feedback(FeedbackData),
}

pub struct GameApp {
    pub engine: Engine,
    screen: Screen,
    last_level: Option<LevelData>,
}

impl GameApp {
    pub fn new(mut engine: Engine) -> Self {
        engine.unlock_first();
        let screen = Self::build_map(&engine, 0);
        Self { engine, screen, last_level: None }
    }

    fn build_map(engine: &Engine, selected: usize) -> Screen {
        let entries = engine
            .level_set
            .levels
            .iter()
            .map(|l| {
                let state = engine
                    .save
                    .level_states
                    .get(&l.id)
                    .map(|p| p.state)
                    .unwrap_or(LevelState::Locked);
                MapEntry { level: l.clone(), state }
            })
            .collect();
        Screen::ChapterMap(ChapterMapData { selected, entries })
    }

    fn build_menu(engine: &Engine) -> Screen {
        Screen::Menu(MenuData { selected: 0, can_continue: engine.can_continue() })
    }

    fn build_level(&mut self, index: usize) -> Result<Screen, GameError> {
        let level = self
            .engine
            .level_set
            .levels
            .get(index)
            .cloned()
            .ok_or_else(|| GameError::LevelNotFound(format!("index {index}")))?;
        let d = LevelData {
            code: level.starter_code.clone(),
            show_hint: false,
            xp: self.engine.save.xp,
            combo: self.engine.save.combo,
            total: self.engine.level_set.len(),
            index,
            level,
        };
        self.last_level = Some(d.clone());
        Ok(Screen::Level(d))
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn save_ref(&self) -> &SaveData {
        &self.engine.save
    }

    pub fn set_code(&mut self, code: String) {
        if let Screen::Level(d) = &mut self.screen {
            d.code = code;
            self.last_level = Some(d.clone());
        }
    }

    pub fn handle(&mut self, input: Input) -> Result<GameFlow, GameError> {
        match self.screen.clone() {
            Screen::Menu(m) => self.handle_menu(m, input),
            Screen::ChapterMap(m) => self.handle_map(m, input),
            Screen::Level(d) => self.handle_level(d, input),
            Screen::Feedback(f) => self.handle_feedback(f, input),
        }
    }

    fn handle_menu(&mut self, m: MenuData, input: Input) -> Result<GameFlow, GameError> {
        match input {
            Input::Up => {
                self.screen = Screen::Menu(MenuData { selected: m.selected.saturating_sub(1), ..m });
            }
            Input::Down => {
                let max = if m.can_continue { 2 } else { 1 };
                self.screen = Screen::Menu(MenuData { selected: (m.selected + 1).min(max), ..m });
            }
            Input::Enter => {
                if m.can_continue {
                    match m.selected {
                        0 => self.screen = Self::build_map(&self.engine, 0),
                        1 => {
                            self.engine.new_game();
                            self.screen = Self::build_map(&self.engine, 0);
                        }
                        _ => return Ok(GameFlow::Quit),
                    }
                } else {
                    match m.selected {
                        0 => {
                            self.engine.new_game();
                            self.screen = Self::build_map(&self.engine, 0);
                        }
                        _ => return Ok(GameFlow::Quit),
                    }
                }
            }
            Input::Esc => return Ok(GameFlow::Quit),
            _ => {}
        }
        Ok(GameFlow::Continue)
    }

    fn handle_map(&mut self, m: ChapterMapData, input: Input) -> Result<GameFlow, GameError> {
        match input {
            Input::Up => {
                self.screen = Screen::ChapterMap(ChapterMapData { selected: m.selected.saturating_sub(1), ..m });
            }
            Input::Down => {
                let max = m.entries.len().saturating_sub(1);
                self.screen = Screen::ChapterMap(ChapterMapData { selected: (m.selected + 1).min(max), ..m });
            }
            Input::Enter => {
                self.engine.start_level(m.selected)?;
                self.screen = self.build_level(m.selected)?;
            }
            Input::Esc => self.screen = Self::build_menu(&self.engine),
            _ => {}
        }
        Ok(GameFlow::Continue)
    }

    fn handle_level(&mut self, d: LevelData, input: Input) -> Result<GameFlow, GameError> {
        match input {
            Input::Submit => {
                let result = self.engine.submit(&d.code)?;
                match result {
                    Validation::Pass => {
                        self.screen = Screen::Feedback(FeedbackData {
                            passed: true,
                            level_id: d.level.id.clone(),
                            feedback: Vec::new(),
                            xp_gained: XP_PER_PASS,
                        });
                    }
                    Validation::Fail { feedback } => {
                        self.screen = Screen::Feedback(FeedbackData {
                            passed: false,
                            level_id: d.level.id.clone(),
                            feedback,
                            xp_gained: 0,
                        });
                    }
                }
            }
            Input::Hint => {
                if let Screen::Level(cur) = &mut self.screen {
                    cur.show_hint = !cur.show_hint;
                    self.last_level = Some(cur.clone());
                }
            }
            Input::Reset => {
                if let Screen::Level(cur) = &mut self.screen {
                    cur.code = cur.level.starter_code.clone();
                    self.last_level = Some(cur.clone());
                }
            }
            Input::Esc => self.screen = Self::build_map(&self.engine),
            _ => {}
        }
        Ok(GameFlow::Continue)
    }

    fn handle_feedback(&mut self, f: FeedbackData, input: Input) -> Result<GameFlow, GameError> {
        match input {
            Input::Enter => {
                if f.passed {
                    let idx = self.engine.current.unwrap_or(0);
                    let next = idx + 1;
                    if next < self.engine.level_set.len() {
                        self.engine.start_level(next)?;
                        self.screen = self.build_level(next)?;
                    } else {
                        self.screen = Self::build_map(&self.engine);
                    }
                } else if let Some(prev) = self.last_level.clone() {
                    self.screen = Screen::Level(prev);
                } else {
                    self.screen = Self::build_map(&self.engine);
                }
            }
            Input::Esc => self.screen = Self::build_map(&self.engine),
            _ => {}
        }
        Ok(GameFlow::Continue)
    }
}
```

`crates/game-core/src/lib.rs`（追加）:
```rust
pub mod app;
pub mod ui;

pub use app::{ChapterMapData, FeedbackData, GameApp, GameFlow, Input, LevelData, MapEntry, MenuData, Screen};
pub use ui::UiBackend;
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-core`
Expected: 全部 PASS

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(core): GameApp 状态机（菜单/地图/关卡/反馈流转）"
```
---

### Task 11: game-data 资源（errors.toml + 15 个关卡 TOML）

**Files:**
- Create: `crates/game-data/src/lib.rs`
- Create: `crates/game-data/tests/levels.rs`
- Create: `assets/errors.toml`
- Create: `assets/levels/00-l0-hello.toml` … `assets/levels/14-l4-lifetime-trap.toml`（15 个文件）

**Interfaces:**
- Consumes: `LevelSet::load`、`Level`、`LevelTier`（game-core）
- Produces:
  - `game_data::assets_dir() -> PathBuf`、`levels_dir() -> PathBuf`、`errors_path() -> PathBuf`
  - 15 个关卡（L0×4、L1×4、L2×3、L3×2、L4×2），线性解锁顺序由文件名排序决定（`00-` … `14-`）
  - `assets/errors.toml`：20 个高频错误码 → 中文解释 + 官方链接

- [ ] **Step 1: 写 game-data lib 与测试**

`crates/game-data/src/lib.rs`:
```rust
use std::path::PathBuf;

/// 资源目录（workspace 根下 assets/）
pub fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub fn levels_dir() -> PathBuf {
    assets_dir().join("levels")
}

pub fn errors_path() -> PathBuf {
    assets_dir().join("errors.toml")
}
```

`crates/game-data/tests/levels.rs`:
```rust
#[test]
fn all_levels_parse_and_consistent() {
    let set = game_core::LevelSet::load(&game_data::levels_dir()).expect("关卡目录加载失败");
    assert_eq!(set.len(), 15, "第一版应有 15 关");
    let mut tiers = std::collections::BTreeSet::new();
    for l in &set.levels {
        tiers.insert(l.tier.order());
        assert!(!l.starter_code.trim().is_empty(), "{} 缺少 starter_code", l.id);
        assert!(!l.source.is_empty(), "{} 缺少 source", l.id);
        if l.allow_compile_fail {
            assert!(!l.expect_error_code.is_empty(), "{} 需 expect_error_code", l.id);
        }
    }
    assert_eq!(tiers.len(), 5, "应覆盖 L0-L4 全部难度层");
}

#[test]
fn errors_toml_has_required_codes() {
    let mapper = game_core::ErrorMapper::load(&game_data::errors_path()).expect("errors.toml 解析失败");
    for code in ["E0308", "E0382", "E0502", "E0596", "E0106"] {
        assert!(mapper.lookup(code).is_some(), "缺少错误码 {code}");
    }
}
```

- [ ] **Step 2: 写 assets/errors.toml（20 个错误码）**

`assets/errors.toml`:
```toml
[E0004]
zh = "match 分支不穷尽：没有覆盖所有可能的情况（缺少通配符 _ 分支）"
link = "https://doc.rust-lang.org/error_codes/E0004.html"

[E0061]
zh = "函数参数数量不匹配：调用时传入的参数个数与定义不符"
link = "https://doc.rust-lang.org/error_codes/E0061.html"

[E0106]
zh = "缺少生命周期标注：引用类型需要显式标注生命周期（如 <'a>）"
link = "https://doc.rust-lang.org/error_codes/E0106.html"

[E0204]
zh = "类型未实现 Copy：不能对含堆数据（如 String、Vec）的类型派生 Copy"
link = "https://doc.rust-lang.org/error_codes/E0204.html"

[E0277]
zh = "trait 约束未满足：某个类型没有实现所需的 trait（如 Display、Clone）"
link = "https://doc.rust-lang.org/error_codes/E0277.html"

[E0308]
zh = "类型不匹配：表达式的实际类型与期望类型不一致"
link = "https://doc.rust-lang.org/error_codes/E0308.html"

[E0369]
zh = "二元运算（如 +、-）不能用于这两个类型：需要实现对应运算符的 trait"
link = "https://doc.rust-lang.org/error_codes/E0369.html"

[E0382]
zh = "使用了已移动的值：所有权已转移，无法再使用原变量（可 clone 或借用）"
link = "https://doc.rust-lang.org/error_codes/E0382.html"

[E0412]
zh = "找不到该类型：类型名未定义或未导入"
link = "https://doc.rust-lang.org/error_codes/E0412.html"

[E0425]
zh = "找不到该变量/函数：名字未定义或不在当前作用域"
link = "https://doc.rust-lang.org/error_codes/E0425.html"

[E0433]
zh = "路径解析失败：引用了不存在的模块或 crate"
link = "https://doc.rust-lang.org/error_codes/E0433.html"

[E0499]
zh = "同一作用域内多次可变借用：一个变量同时只能有一个可变借用"
link = "https://doc.rust-lang.org/error_codes/E0499.html"

[E0502]
zh = "借用冲突：变量同时存在不可变借用与可变借用，Rust 不允许"
link = "https://doc.rust-lang.org/error_codes/E0502.html"

[E0505]
zh = "值被借用时不能移动：有活跃借用的情况下尝试 move 所有权"
link = "https://doc.rust-lang.org/error_codes/E0505.html"

[E0507]
zh = "不能移出借用的内容：无法从借用中 move 走所有权"
link = "https://doc.rust-lang.org/error_codes/E0507.html"

[E0596]
zh = "无法以可变方式借用：变量（或参数）需要声明为 mut"
link = "https://doc.rust-lang.org/error_codes/E0596.html"

[E0597]
zh = "借用活得不够久：被借用的值比引用先被释放（生命周期问题）"
link = "https://doc.rust-lang.org/error_codes/E0597.html"

[E0599]
zh = "没有该方法：类型上不存在这个方法的实现（或未实现对应 trait）"
link = "https://doc.rust-lang.org/error_codes/E0599.html"

[E0618]
zh = "把不可调用的值当函数调用：该值不是函数"
link = "https://doc.rust-lang.org/error_codes/E0618.html"

[E0623]
zh = "生命周期不一致：函数签名中两个引用的生命周期无法统一"
link = "https://doc.rust-lang.org/error_codes/E0623.html"
```

- [ ] **Step 3: 写 15 个关卡 TOML**

`assets/levels/00-l0-hello.toml`:
```toml
[[level]]
id = "l0-hello"
title = "你好，变量"
tier = "l0"
description = "修复代码使其编译：变量在 Rust 中需要用 let 声明。"
hint = "变量需要 let 声明：let x = 5;"
starter_code = '''
fn main() {
    x = 5;
    println!("x has the value {}", x);
}
'''
expect_output = "x has the value 5"
source = "rustlings (variables1)"
```

`assets/levels/01-l0-print.toml`:
```toml
[[level]]
id = "l0-print"
title = "格式化输出"
tier = "l0"
description = "println! 的占位符 {} 需要对应数量的参数。补上缺失的参数。"
hint = "三个 {} 需要三个参数，当前只传了两个。"
starter_code = '''
fn main() {
    println!("{} + {} = {}", 1, 2);
}
'''
expect_output = "1 + 2 = 3"
source = "rustlings (print 系列)"
```

`assets/levels/02-l0-function.toml`:
```toml
[[level]]
id = "l0-function"
title = "定义函数"
tier = "l0"
description = "main 调用了 call_me，但它还不存在。定义一个打印 Call me 的 call_me 函数。"
hint = "在 main 下方定义 fn call_me() { println!(\"Call me\"); }"
starter_code = '''
fn main() {
    call_me();
}
'''
expect_output = "Call me"
source = "rustlings (functions1)"
```

`assets/levels/03-l0-loop.toml`:
```toml
[[level]]
id = "l0-loop"
title = "循环范围"
tier = "l0"
description = "当前循环计算 1+2+3+4=10，但要求输出 15（1 到 5 的和）。修改循环范围。"
hint = "1..5 不含 5；使用 1..=5 表示包含端点。"
starter_code = '''
fn main() {
    let mut sum = 0;
    for i in 1..5 {
        sum += i;
    }
    println!("{}", sum);
}
'''
expect_output = "15"
source = "rustlings (loops 系列)"
```

`assets/levels/04-l1-move.toml`:
```toml
[[level]]
id = "l1-move"
title = "所有权与可变参数"
tier = "l1"
description = "fill_vec 接收 Vec 后要 push 元素，但参数不是 mut。修复编译错误。"
hint = "函数参数需要 mut：fn fill_vec(mut vec: Vec<i32>)"
starter_code = '''
fn main() {
    let vec0 = Vec::new();
    let mut vec1 = fill_vec(vec0);
    println!("{} has length {} content {:?}", "vec1", vec1.len(), vec1);
    vec1.push(88);
    println!("{} has length {} content {:?}", "vec1", vec1.len(), vec1);
}

fn fill_vec(vec: Vec<i32>) -> Vec<i32> {
    vec.push(22);
    vec.push(44);
    vec.push(66);
    vec
}
'''
expect_output = '''
vec1 has length 3 content [22, 44, 66]
vec1 has length 4 content [22, 44, 66, 88]
'''
source = "rustlings (move_semantics1)"
```

`assets/levels/05-l1-borrow.toml`:
```toml
[[level]]
id = "l1-borrow"
title = "借用而不是移动"
tier = "l1"
description = "calculate_length 拿走了 s 的所有权，导致 main 里无法再使用 s。改为借用（引用）。"
hint = "调用处传 &s，函数签名改为 fn calculate_length(s: &String) -> usize"
starter_code = '''
fn main() {
    let s = String::from("hello");
    let len = calculate_length(s);
    println!("The length of '{}' is {}.", s, len);
}

fn calculate_length(s: String) -> usize {
    s.len()
}
'''
expect_output = "The length of 'hello' is 5."
source = "rustlings (ownership 系列)"
```

`assets/levels/06-l1-mut-borrow.toml`:
```toml
[[level]]
id = "l1-mut-borrow"
title = "可变借用"
tier = "l1"
description = "add_world 要在 s 上追加内容，需要可变借用。修复所有权与可变性问题。"
hint = "三处改动：let mut s、调用处 &mut s、签名 fn add_world(s: &mut String)"
starter_code = '''
fn main() {
    let s = String::from("hello");
    add_world(s);
    println!("{}", s);
}

fn add_world(s: String) {
    s.push_str(" world");
}
'''
expect_output = "hello world"
source = "rustlings (move_semantics 系列)"
```

`assets/levels/07-l1-clone.toml`:
```toml
[[level]]
id = "l1-clone"
title = "克隆 vs 移动"
tier = "l1"
description = "let s2 = s1 把所有权移给了 s2，s1 无法再用。让两个变量都可使用。"
hint = "使用 s1.clone() 创建副本：let s2 = s1.clone();"
starter_code = '''
fn main() {
    let s1 = String::from("hello");
    let s2 = s1;
    println!("{} {}", s1, s2);
}
'''
expect_output = "hello hello"
source = "rustlings (move_semantics 系列)"
```

`assets/levels/08-l2-vec.toml`:
```toml
[[level]]
id = "l2-vec"
title = "数组越界 panic"
tier = "l2"
description = "程序会 panic：下标 3 超出了 vec![1,2,3] 的范围。修复下标使输出为 3。"
hint = "Vec 下标从 0 开始：vec[2] 是 3。"
starter_code = '''
fn main() {
    let v = vec![1, 2, 3];
    println!("{}", v[3]);
}
'''
expect_output = "3"
source = "rustlings (vecs 系列)"
```

`assets/levels/09-l2-option.toml`:
```toml
[[level]]
id = "l2-option"
title = "Option 与 get"
tier = "l2"
description = "v.get(5) 返回 None（越界），当前输出 none。修改参数使输出 3，并体会 Option 的 match 处理。"
hint = "v.get(2) 返回 Some(3)，match 的 Some 分支会打印 3。"
starter_code = '''
fn main() {
    let v = vec![1, 2, 3];
    let item = v.get(5);
    match item {
        Some(x) => println!("{}", x),
        None => println!("none"),
    }
}
'''
expect_output = "3"
source = "rustlings (option1)"
```

`assets/levels/10-l2-result.toml`:
```toml
[[level]]
id = "l2-result"
title = "Result 与错误处理"
tier = "l2"
description = "parse_number(\"not a number\") 返回 Err，result.unwrap() 会 panic。用 match 处理错误，输出「解析失败」。"
hint = "用 match result { Ok(n) => println!(\"{}\", n), Err(_) => println!(\"解析失败\") } 替换 unwrap。"
starter_code = '''
fn main() {
    let result = parse_number("not a number");
    println!("{}", result.unwrap());
}

fn parse_number(s: &str) -> Result<i32, std::num::ParseIntError> {
    s.parse()
}
'''
expect_output = "解析失败"
source = "rustlings (result 系列)"
```

`assets/levels/11-l3-lifetime.toml`:
```toml
[[level]]
id = "l3-lifetime"
title = "生命周期标注"
tier = "l3"
description = "longest 返回的引用缺少生命周期标注。补上 <'a> 使返回的引用与输入参数同寿命。"
hint = "签名改为 fn longest<'a>(x: &'a str, y: &'a str) -> &'a str"
starter_code = '''
fn main() {
    let string1 = String::from("long string is long");
    let result;
    {
        let string2 = String::from("xyz");
        result = longest(string1.as_str(), string2.as_str());
        println!("The longest string is '{}'", result);
    }
}

fn longest(x: &str, y: &str) -> &str {
    if x.len() > y.len() {
        x
    } else {
        y
    }
}
'''
expect_output = "The longest string is 'long string is long'"
source = "rustlings (lifetimes1)"
```

`assets/levels/12-l3-trait.toml`:
```toml
[[level]]
id = "l3-trait"
title = "实现 Trait"
tier = "l3"
description = "Rectangle 调用了 r.area()，但 Area trait 还没有为 Rectangle 实现。补上 impl。"
hint = "实现 impl Area for Rectangle { fn area(&self) -> u32 { self.width * self.height } }"
starter_code = '''
struct Rectangle {
    width: u32,
    height: u32,
}

trait Area {
    fn area(&self) -> u32;
}

fn main() {
    let r = Rectangle { width: 4, height: 5 };
    println!("area: {}", r.area());
}
'''
expect_output = "area: 20"
source = "rustlings (traits 系列)"
```

`assets/levels/13-l4-drop-order.toml`:
```toml
[[level]]
id = "l4-drop-order"
title = "挑战：Drop 释放顺序"
tier = "l4"
description = "写出代码，使输出依次为 end、drop 2、drop 1。提示：给结构体实现 Drop trait，局部变量按声明逆序释放（后声明的先释放）。"
hint = "struct S(u32) + impl Drop for S 打印 drop {}，main 中先创建 S(1) 再创建 S(2)，最后 println!(\"end\")。"
starter_code = '''
fn main() {
    println!("drop 1");
    println!("drop 2");
    println!("end");
}
'''
expect_output = '''
end
drop 2
drop 1
'''
source = "rust-quiz (drop 顺序主题)"
```

`assets/levels/14-l4-lifetime-trap.toml`:
```toml
[[level]]
id = "l4-lifetime-trap"
title = "挑战：借用的存活范围"
tier = "l4"
description = "修复代码：s 借用了一个在花括号内就释放的值。让 t 的存活范围覆盖 s 的使用。"
hint = "把 let t = String::from(\"hi\"); 移到块外（在 let s 之前声明）。"
starter_code = '''
fn main() {
    let s;
    {
        let t = String::from("hi");
        s = &t;
    }
    println!("{}", s);
}
'''
expect_output = "hi"
source = "rust-quiz (生命周期主题)"
```

- [ ] **Step 4: 跑 game-data 校验测试**

Run: `cargo test -p game-data`
Expected: 全部 PASS（15 关、L0-L4 全覆盖、错误码齐全）

- [ ] **Step 5: 手动抽查 2 个关卡的 starter 确实失败**

Run: `cd /tmp && printf 'fn main() {\n    x = 5;\n    println!("x has the value {}", x);\n}\n' > t.rs && rustc --edition 2021 t.rs 2>&1 | head -3`
Expected: 输出包含 `error[E0425]`（确认 starter 有错）

- [ ] **Step 6: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(data): 15 关卡 + 20 错误码映射表"
```

---

### Task 12: game-ui 前端（macroquad + egui）

**Files:**
- Create: `crates/game-ui/src/lib.rs`
- Create: `crates/game-ui/src/app_ui.rs`
- Test: `crates/game-ui/src/app_ui.rs`（内联测试，headless egui Context 冒烟）

**Interfaces:**
- Consumes: `GameApp`/`Screen`/`Input`/`GameFlow`（Task 10）、`UiBackend`、`tokenize`/`TokenKind`（Task 8）、`LevelTier`
- Produces:
  - `pub struct GameUi { code_buf: String, last_level_id: Option<String>, busy: Busy, quit: bool }`，`GameUi::new()`
  - `impl UiBackend for GameUi`：macroquad 主循环，渲染 Screen，采集键盘/鼠标事件 → `app.handle`，Quit 时退出
  - 编辑器 = `egui::TextEdit::multiline` + 自定义 layouter（tokenize 着色）+ 左侧行号 gutter
  - 提交按钮 → 两帧「编译中」提示 → 真正提交（提交阻塞期间用户能看到 loading）
  - 关卡数据同步：进入新关卡时 `code_buf` 从 `Screen::Level.code` 载入；`TextEdit` 改动 → `app.set_code`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use game_core::app::{GameApp, Input, Screen};
    use game_core::engine::Engine;
    use game_core::level::{parse_levels, LevelSet};
    use game_core::sandbox::DevSandbox;
    use game_core::validate::mapper::ErrorMapper;

    fn test_app() -> GameApp {
        let levels = parse_levels(
            "[[level]]\nid = \"t\"\ntitle = \"t\"\ntier = \"l0\"\ndescription = \"d\"\nstarter_code = \"fn main() { println!(1); }\"\nsource = \"x\"\n",
        )
        .unwrap();
        let engine = Engine::new(
            LevelSet { levels },
            Default::default(),
            ErrorMapper::default_fallback(),
            Box::new(DevSandbox::new()),
        );
        GameApp::new(engine)
    }

    #[test]
    fn renders_menu_headless() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Esc).unwrap(); // 进入菜单
        ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(matches!(app.screen(), Screen::Menu(_)));
    }

    #[test]
    fn renders_level_and_syncs_code_buffer() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap(); // 进入关卡
        assert!(ui.code_buf.is_empty());
        ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        assert!(ui.code_buf.contains("fn main"), "code_buf 未同步: {}", ui.code_buf);
        assert!(matches!(app.screen(), Screen::Level(_)));
    }

    #[test]
    fn editor_edit_syncs_back_to_app() {
        let ctx = egui::Context::default();
        let mut app = test_app();
        let mut ui = GameUi::new();
        app.handle(Input::Enter).unwrap();
        ctx.run(egui::RawInput::default(), |ctx| {
            ui.draw(ctx, &mut app);
        });
        // 模拟 TextEdit 改动后 UI 回写
        ui.code_buf = "fn main() { println!(\"edited\"); }".into();
        ui.sync_code(&mut app);
        match app.screen() {
            Screen::Level(d) => assert_eq!(d.code, "fn main() { println!(\"edited\"); }"),
            other => panic!("expected Level, got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd /home/elite/Projects/rust-learning-game && cargo test -p game-ui`
Expected: 编译失败（app_ui 模块不存在）

- [ ] **Step 3: 写实现**

`crates/game-ui/src/lib.rs`:
```rust
pub mod app_ui;

pub use app_ui::GameUi;
```

`crates/game-ui/src/app_ui.rs`:
```rust
use egui_macroquad::egui;
use game_core::app::{GameApp, GameFlow, Input, LevelData, MenuData, Screen};
use game_core::editor::{tokenize, TokenKind};
use game_core::error::GameError;
use game_core::level::LevelTier;
use game_core::ui::UiBackend;
use macroquad::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Busy {
    None,
    Show, // 显示「编译中」一帧
    Do,   // 下一帧真正执行提交
}

pub struct GameUi {
    code_buf: String,
    last_level_id: Option<String>,
    busy: Busy,
    quit: bool,
}

impl GameUi {
    pub fn new() -> Self {
        Self { code_buf: String::new(), last_level_id: None, busy: Busy::None, quit: false }
    }

    /// 把 code_buf 回写进 app（TextEdit 每次改动后调用）
    fn sync_code(&mut self, app: &mut GameApp) {
        app.set_code(self.code_buf.clone());
    }

    fn act(&mut self, app: &mut GameApp, input: Input) {
        match app.handle(input) {
            Ok(GameFlow::Quit) => self.quit = true,
            Ok(GameFlow::Continue) => {}
            Err(e) => eprintln!("[ui] 错误: {e}"),
        }
    }

    fn key(ctx: &egui::Context, k: egui::Key) -> bool {
        ctx.input(|i| i.key_pressed(k))
    }

    fn draw(&mut self, ctx: &egui::Context, app: &mut GameApp) {
        let screen = app.screen().clone();
        // 进入新关卡时同步 code_buf
        if let Screen::Level(d) = &screen {
            if self.last_level_id.as_deref() != Some(d.level.id.as_str()) {
                self.code_buf = d.code.clone();
                self.last_level_id = Some(d.level.id.clone());
            }
        } else {
            self.last_level_id = None;
        }
        match self.busy {
            Busy::Show => {
                self.draw_busy(ctx);
                self.busy = Busy::Do;
                return;
            }
            Busy::Do => {
                self.draw_busy(ctx);
                self.busy = Busy::None;
                self.act(app, Input::Submit);
                return;
            }
            Busy::None => {}
        }
        match screen {
            Screen::Menu(m) => self.draw_menu(ctx, app, &m),
            Screen::ChapterMap(m) => self.draw_map(ctx, app, &m),
            Screen::Level(d) => self.draw_level(ctx, app, &d),
            Screen::Feedback(f) => self.draw_feedback(ctx, app, &f),
        }
    }

    fn draw_busy(&self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("⏳ 编译中，请稍候...");
                ui.label("（首次编译可能较慢）");
            });
        });
    }

    fn draw_menu(&mut self, ctx: &egui::Context, app: &mut GameApp, m: &MenuData) {
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        if Self::key(ctx, egui::Key::ArrowUp) {
            self.act(app, Input::Up);
        }
        if Self::key(ctx, egui::Key::ArrowDown) {
            self.act(app, Input::Down);
        }
        if Self::key(ctx, egui::Key::Enter) {
            self.act(app, Input::Enter);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.heading(egui::RichText::new("🦀 Rust 学习游戏").size(44.0));
                ui.add_space(8.0);
                ui.label("闯关学 Rust：修好代码，让编译器闭嘴，让程序输出正确结果。");
                ui.add_space(30.0);
                let mut clicked: Option<usize> = None;
                let mut idx = 0;
                if m.can_continue {
                    if ui.selectable_label(m.selected == 0, "▶ 继续游戏").clicked() {
                        clicked = Some(idx);
                    }
                    idx += 1;
                }
                if ui.selectable_label(m.selected == idx, "🆕 新游戏").clicked() {
                    clicked = Some(idx);
                }
                idx += 1;
                if ui.selectable_label(m.selected == idx, "🚪 退出").clicked() {
                    clicked = Some(idx);
                }
                if let Some(target) = clicked {
                    self.select_and_enter(app, target);
                }
                ui.add_space(20.0);
                ui.label(egui::RichText::new("↑↓ 选择 · Enter 确认 · Esc 退出").weak());
            });
        });
    }

    fn select_and_enter(&mut self, app: &mut GameApp, target: usize) {
        loop {
            let cur = match app.screen() {
                Screen::Menu(m) => m.selected,
                _ => break,
            };
            if cur < target {
                self.act(app, Input::Down);
            } else if cur > target {
                self.act(app, Input::Up);
            } else {
                break;
            }
        }
        self.act(app, Input::Enter);
    }

    fn draw_map(&mut self, ctx: &egui::Context, app: &mut GameApp, m: &game_core::app::ChapterMapData) {
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        if Self::key(ctx, egui::Key::ArrowUp) {
            self.act(app, Input::Up);
        }
        if Self::key(ctx, egui::Key::ArrowDown) {
            self.act(app, Input::Down);
        }
        if Self::key(ctx, egui::Key::Enter) {
            self.act(app, Input::Enter);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🗺️ 关卡地图");
            ui.label("按 L0 → L4 顺序推进，解锁前一关后才能进入下一关。");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut clicked: Option<usize> = None;
                for (i, entry) in m.entries.iter().enumerate() {
                    let (icon, state_str) = match entry.state {
                        game_core::save::LevelState::Passed => ("✅", "已通关"),
                        game_core::save::LevelState::Unlocked => ("🔓", "可挑战"),
                        game_core::save::LevelState::Locked => ("🔒", "未解锁"),
                    };
                    let tier = match entry.level.tier {
                        LevelTier::L0 => "L0 入门",
                        LevelTier::L1 => "L1 所有权",
                        LevelTier::L2 => "L2 集合/错误",
                        LevelTier::L3 => "L3 生命周期/trait",
                        LevelTier::L4 => "L4 挑战",
                    };
                    let text = format!("{icon} {}. {}（{tier}）{state_str}", i + 1, entry.level.title);
                    if ui.selectable_label(m.selected == i, text).clicked() {
                        clicked = Some(i);
                    }
                }
                if let Some(target) = clicked {
                    // 移动选中到 target 再确认
                    loop {
                        let cur = match app.screen() {
                            Screen::ChapterMap(mm) => mm.selected,
                            _ => break,
                        };
                        if cur < target {
                            self.act(app, Input::Down);
                        } else if cur > target {
                            self.act(app, Input::Up);
                        } else {
                            break;
                        }
                    }
                    self.act(app, Input::Enter);
                }
            });
        });
    }

    fn draw_level(&mut self, ctx: &egui::Context, app: &mut GameApp, d: &LevelData) {
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(&d.level.title);
                ui.label(format!(
                    "L{} · {}/{} · XP {} · 连击 {}x",
                    d.level.tier.order(),
                    d.index + 1,
                    d.total,
                    d.xp,
                    d.combo
                ));
            });
            ui.label(&d.level.description);
            if d.show_hint && !d.level.hint.is_empty() {
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::from_rgb(255, 200, 80), format!("💡 {}", d.level.hint));
            }
            ui.separator();
            ui.horizontal(|ui| {
                // 行号 gutter（对齐用 monospace 字体）
                let line_count = self.code_buf.lines().count().max(1);
                let gutter = (1..=line_count).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(gutter).monospace().color(egui::Color32::from_rgb(120, 120, 120)),
                    )
                    .selectable(false),
                );
                // 编辑器
                let mut layouter = |ui: &egui::Ui, text: &str, _wrap_width: f32| {
                    let mut job = egui::text::LayoutJob::default();
                    for span in tokenize(text) {
                        job.append(
                            &text[span.start..span.end],
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::monospace(14.0),
                                color: color_for(span.kind),
                                ..Default::default()
                            },
                        );
                    }
                    ui.fonts(|f| f.layout_job(job))
                };
                let resp = ui.add(
                    egui::TextEdit::multiline(&mut self.code_buf)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(20)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter),
                );
                if resp.changed() {
                    self.sync_code(app);
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("▶ 提交运行").clicked() {
                    self.busy = Busy::Show;
                }
                if ui.button("💡 提示").clicked() {
                    self.act(app, Input::Hint);
                }
                if ui.button("↺ 重置代码").clicked() {
                    self.act(app, Input::Reset);
                    self.last_level_id = None; // 下一帧重新同步 starter_code
                }
            });
        });
    }

    fn draw_feedback(&mut self, ctx: &egui::Context, app: &mut GameApp, f: &game_core::app::FeedbackData) {
        if Self::key(ctx, egui::Key::Enter) {
            self.act(app, Input::Enter);
            return;
        }
        if Self::key(ctx, egui::Key::Escape) {
            self.act(app, Input::Esc);
            return;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(30.0);
            if f.passed {
                ui.vertical_centered(|ui| {
                    ui.heading(
                        egui::RichText::new("✅ 通关！")
                            .color(egui::Color32::from_rgb(90, 220, 130))
                            .size(40.0),
                    );
                    ui.add_space(8.0);
                    ui.label(format!("获得 {} XP，已自动保存进度", f.xp_gained));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("按 Enter 进入下一关").weak());
                });
            } else {
                ui.heading(egui::RichText::new("❌ 未通过").color(egui::Color32::from_rgb(240, 90, 90)));
                ui.separator();
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for line in &f.feedback {
                        ui.label(egui::RichText::new(line).color(egui::Color32::from_rgb(235, 190, 190)));
                        ui.add_space(6.0);
                    }
                });
                ui.separator();
                ui.label(egui::RichText::new("按 Enter 返回编辑继续修改，Esc 回地图").weak());
            }
        });
    }
}

impl Default for GameUi {
    fn default() -> Self {
        Self::new()
    }
}

fn color_for(kind: TokenKind) -> egui::Color32 {
    match kind {
        TokenKind::Keyword => egui::Color32::from_rgb(86, 156, 214),
        TokenKind::Comment => egui::Color32::from_rgb(106, 153, 85),
        TokenKind::String => egui::Color32::from_rgb(206, 145, 120),
        TokenKind::Number => egui::Color32::from_rgb(181, 206, 168),
        TokenKind::Normal => egui::Color32::from_rgb(220, 220, 220),
    }
}

impl UiBackend for GameUi {
    fn run(&mut self, app: &mut GameApp) -> Result<(), GameError> {
        loop {
            clear_background(Color::from_rgba(30, 30, 30, 255));
            egui_macroquad::ui(|ctx| {
                self.draw(ctx, app);
            });
            egui_macroquad::draw();
            if self.quit {
                break;
            }
            next_frame().await;
        }
        Ok(())
    }
}
```

注意：`game_core::app::ChapterMapData` / `FeedbackData` 需要是 `pub`（Task 10 已导出 `pub use app::{...ChapterMapData, FeedbackData...}`），此处用完整路径引用，避免额外导入。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p game-ui`
Expected: 全部 PASS（headless egui 渲染冒烟）

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(ui): macroquad+egui 前端（菜单/地图/关卡/反馈 + 着色编辑器）"
```

---

### Task 13: 集成 main.rs + 手动试玩

**Files:**
- Modify: `crates/game-ui/src/main.rs`
- Create: `crates/game-ui/src/lib.rs` 已含 `GameUi`；main 用 workspace 二进制

**Interfaces:**
- Consumes: 全部已有模块

- [ ] **Step 1: 写 main.rs**

`crates/game-ui/src/main.rs`:
```rust
use game_core::app::GameApp;
use game_core::engine::Engine;
use game_core::level::LevelSet;
use game_core::sandbox::DevSandbox;
use game_core::save;
use game_core::ui::UiBackend;
use game_core::validate::mapper::ErrorMapper;
use game_ui::GameUi;

fn save_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home).join(".local/share/rust-learning-game/save.toml")
}

#[macroquad::main("Rust 学习游戏")]
async fn main() {
    let level_set = match LevelSet::load(&game_data::levels_dir()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("关卡加载失败: {e}");
            std::process::exit(1);
        }
    };
    let save_data = save::load(&save_path()).unwrap_or_default();
    let mapper = match ErrorMapper::load(&game_data::errors_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("错误码映射加载失败（使用兜底表）: {e}");
            ErrorMapper::default_fallback()
        }
    };
    let engine = Engine::new(level_set, save_data, mapper, Box::new(DevSandbox::new()));
    let mut app = GameApp::new(engine);
    let mut ui = GameUi::new();
    if let Err(e) = ui.run(&mut app).await {
        eprintln!("运行错误: {e}");
    }
    if let Err(e) = save::save(app.save_ref(), &save_path()) {
        eprintln!("存档失败: {e}");
    }
}
```

- [ ] **Step 2: 全量构建**

Run: `cargo build --workspace`
Expected: 编译成功（首次编译 macroquad 依赖较慢，设 timeout 600s）

- [ ] **Step 3: 全量测试**

Run: `cargo test --workspace`
Expected: 全部 PASS

- [ ] **Step 4: 手动试玩清单**（有显示环境时执行）

Run: `cargo run -p game-ui`，逐项核对：
- [ ] 窗口打开，主菜单显示「新游戏/退出」（无存档时）
- [ ] 新游戏 → 关卡地图，第一关 🔓，其余 🔒
- [ ] 进入 l0-hello，编辑器内已有 starter_code，关键字/注释/字符串有颜色，左侧有行号
- [ ] 不改代码直接「提交运行」→ 两帧「编译中」→ 红色失败反馈，含 `E0425` 与中文解释
- [ ] 改成 `let x = 5;` 提交 → 绿色通关 + XP，自动进入下一关
- [ ] 故意提交错误代码 → 失败后 Enter 返回编辑，代码保留
- [ ] 「💡 提示」显示/隐藏提示，「↺ 重置代码」恢复 starter
- [ ] Esc 回地图、回菜单；菜单「退出」后进程结束
- [ ] 重跑游戏 → 菜单出现「继续游戏」，已通关关卡显示 ✅
- [ ] 通关到 l2-vec 时验证 panic 反馈（v[3] 越界）

若某步不符合，用 systematic-debugging 定位修复后再提交。

- [ ] **Step 5: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "feat(app): 集成 main.rs，完整可玩闭环"
```

---

### Task 14: README + 许可致谢 + 收尾

**Files:**
- Create: `README.md`

- [ ] **Step 1: 写 README**

`README.md`:
```markdown
# Rust 学习游戏

闯关式 Rust 学习游戏：每一关给出任务描述与初始代码（通常有 bug），玩家修改代码提交，
游戏调用 rustc 编译、运行并比对输出，把编译器报错翻译成中文提示与学习链接。

## 运行

```bash
cargo run -p game-ui
```

要求：rustc 1.75+（编译校验用系统 rustc）、macroquad 所需系统库
（Linux 桌面：libx11-dev libxi-dev libgl1-mesa-dev 等，见 macroquad 文档）。

## 玩法

- L0 入门：变量、函数、格式化输出、循环
- L1 所有权核心：move、借用、可变借用、clone
- L2 集合与错误处理：Vec 越界、Option、Result
- L3 难点：生命周期标注、trait 实现
- L4 挑战：Drop 顺序、借用的存活范围
- 线性解锁；通关获得 XP；失败记录错误次数；提示按钮给线索

## 关卡与数据

- 关卡：`assets/levels/*.toml`（`[[level]]` 数组，字段见各文件）
- 错误码中文映射：`assets/errors.toml`（E0xxx → 中文解释 + 官方文档链接）
- 存档：`~/.local/share/rust-learning-game/save.toml`
- 新增关卡 = 在 `assets/levels/` 放一个 TOML，文件名前缀决定顺序

## 代码校验与安全（开发期）

- 编译：裸 `rustc --edition 2021`，仅 std
- 超时：编译 10s / 运行 2s，超时终止
- 静态拦截：syn 扫描玩家代码中的 `std::fs` / `std::net` / `std::process` / `std::env` / `std::thread`
- ⚠️ 开发期无进程隔离（bwrap 沙盒在计划②实现）。**本版本仅限本地学习使用，禁止公开分发。**

## 素材与许可

- 关卡题目改编自 rustlings（MIT / Apache-2.0）：https://github.com/rust-lang/rustlings
- 挑战关卡主题参考 rust-quiz：https://github.com/dtolnay/rust-quiz
- 提示参考 The Book 与 course.rs，均已改写精简
- 每个关卡 TOML 的 `source` 字段标注具体出处

## 架构

```
game-core   纯逻辑：关卡/校验/错误解析/存档/沙盒抽象/着色/引擎/GameApp 状态机（零 UI 依赖）
game-ui     macroquad + egui 前端（实现 UiBackend trait，可替换）
game-data   关卡与错误码资源路径
```
```

- [ ] **Step 2: 最终验证**

Run: `cargo test --workspace && cargo build --workspace`
Expected: 全部 PASS / 编译成功

- [ ] **Step 3: 提交**

```bash
git add -A && git -c user.name="pi" -c user.email="pi@local" commit -m "docs: README 与素材许可致谢"
```

---

## 计划①完成标准

- [ ] `cargo test --workspace` 全绿（core 单测 + 真实 rustc 集成测试 + data 校验 + ui 冒烟）
- [ ] `cargo run -p game-ui` 可完整体验：菜单 → 地图 → 15 关 → 存档 → 继续游戏
- [ ] 15 个关卡 TOML 全部在 `assets/levels/`，errors.toml 20 个错误码
- [ ] 编译错误以「错误码 + 中文解释 + 行号 + 链接」展示，不显示 rustc 原文
- [ ] 存档在 `~/.local/share/rust-learning-game/save.toml`

## 计划②（后续，未在本计划实现）

- bwrap 真隔离沙盒（`--unshare-all` + 只读系统 + tmpfs 工作区 + 禁网络）
- 多级提示（`hints` 数组，第 3 级给参考代码）
- 「制造指定编译错误」关卡内容（数据模型 `allow_compile_fail` + `expect_error_code` 已就绪，Task 9/7 已实现逻辑）
- 计分细化、关卡完成时间展示
- 自定义关卡导入（外部关卡目录）
