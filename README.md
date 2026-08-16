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

## 代码校验与安全

玩家代码在 **bwrap（bubblewrap）真隔离沙盒** 中编译与运行（v3 §9.1）：

- 隔离：`--unshare-all`（用户/pid/网络等全新命名空间）；整棵根文件系统只读挂载（`--ro-bind / /`，禁写主目录与系统目录）；沙盒内 `/tmp` 为 tmpfs 工作区；最小 `/proc` 与最小设备集（urandom/random/null/zero/tty，不暴露块设备）；禁网络。
- 资源限制：编译 10s / 运行 2s 超时终止；内存上限 `ulimit -v`（编译 1 GiB / 运行 512 MiB）。
- 纵深防御：syn 静态拦截保留（`std::fs` / `std::net` / `std::process` / `std::env` / `std::thread::spawn` / `unsafe` / `extern`）；bwrap 是进程级兜底——即使静态拦截被绕过，沙盒仍隔离网络、写入与资源耗尽。
- 安全优先：bwrap 缺失或不可用时游戏**拒绝运行**并给出中文错误（不静默降级到无隔离模式）。
- 🤝 欢迎贡献：本项目公开协作，PR 请附带测试。

## 素材与许可

- 关卡题目改编自 rustlings（MIT / Apache-2.0）：https://github.com/rust-lang/rustlings
- 挑战关卡主题参考 rust-quiz：https://github.com/dtolnay/rust-quiz
- 提示参考 The Book 与 course.rs，均已改写精简
- 每个关卡 TOML 的 `source` 字段标注具体出处
- UI 字体：JetBrains Maple Mono —— JetBrains Mono（OFL 1.1）与 Maple Mono（OFL 1.1）合并版，
  内嵌于 `crates/game-ui/assets/JetBrainsMapleMono-Regular.ttf`，覆盖 CJK 中文字形；
  许可全文见 `crates/game-ui/assets/OFL.txt`（SIL Open Font License 1.1，含双版权声明）
- 编辑器不支持中文输入法（IME 不可用），中文内容请复制粘贴；关卡设计已保证玩家无需手输中文

## 架构

```
game-core   纯逻辑：关卡/校验/错误解析/存档/沙盒抽象/着色/引擎/GameApp 状态机（零 UI 依赖）
game-ui     macroquad + egui 前端（实现 UiBackend trait，可替换）
game-data   关卡与错误码资源路径
```
