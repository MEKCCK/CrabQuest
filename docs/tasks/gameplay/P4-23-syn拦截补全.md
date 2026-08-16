# 需求 P4-23：syn 静态拦截清单补全

- 优先级：P4（安全）｜ 前置：无（开发期兜底强化）｜ 依赖方：P4-24 ｜ 来源：v3 §9.2；docs-review/L1-plan.md 约束 13

## 目标

拦截清单从 5 类扩到 7 类，覆盖文件 IO / 进程 / 环境 / 网络 / 并发 / unsafe / FFI，全部经 syn AST 扫描实现（注释字符串不误报）。

## 背景（实测事实）

- 当前 BLOCKED_PREFIXES 仅 5 项：`std::fs / std::net / std::process / std::env / std::thread`。
- 缺项：`unsafe` 块、`extern`/FFI、`std::thread::spawn` 精确匹配（现整条 `std::thread` 拦截过宽——`std::thread::sleep` 等无害调用也被拦）。
- 设计文档 v3 §9.2 定稿为黑名单式：默认放行 std 基础类型与集合，拦截 IO/进程/网络/并发/不安全。

## 需求范围

1. **拦截清单 7 类**：
   | 类别 | 拦截符号/模式 |
   |---|---|
   | 文件 IO | `std::fs` 全部 |
   | 进程 | `std::process` 全部 |
   | 环境 | `std::env` 全部 |
   | 网络 | `std::net` 全部 |
   | 并发 | `std::thread::spawn`（精确，不拦 sleep/yield） |
   | 内存不安全 | `unsafe` 块 |
   | FFI | `extern` 块 |
2. **精确化**：`std::thread` 改为 `std::thread::spawn` 精确匹配（用例：`std::thread::sleep` 应放行）。
3. **拦截反馈**：沙盒拦截错误走 GameError::SandboxBlocked，UI 中文提示「该代码使用了游戏不允许的 API：X」。
4. **panic 策略**：禁 `panic=abort` 属性（防止覆盖沙盒默认 unwind 判定）——syn 扫描属性（可选，P1 内可延后）。

## 验收标准

- [ ] 每类至少 1 个触发用例被拦（单元测试：代码片段 → SandboxBlocked）。
- [ ] `std::thread::sleep` / `std::thread::yield_now` 放行（精确匹配回归）。
- [ ] `unsafe {}` 与 `extern "C" {}` 被拦。
- [ ] 注释/字符串中出现 `std::fs` 不误报（AST 扫描，已有行为回归）。
- [ ] 现有 15 关 starter_code 全部通过拦截（无合法关卡被误杀）。

## 参考素材

- v3 §9.2（拦截清单表全文）、§12.5
- docs-review/L1-plan.md 约束 13（syn 拦截清单扩充）
