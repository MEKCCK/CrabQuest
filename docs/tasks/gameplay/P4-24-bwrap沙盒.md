# 需求 P4-24：bwrap 真隔离沙盒

- 优先级：P4（安全，发布门槛）｜ 前置：P4-23 ｜ 来源：v3 §9.1；docs-review/L1-plan.md 约束 6

## 目标

bwrap（bubblewrap）真隔离沙盒，替代开发期兜底（timeout + 临时目录 + syn），达到可分发标准。

## 背景（实测事实）

- 当前 DevSandbox = 临时目录 + timeout（编译 10s/运行 2s）+ syn 拦截，**无进程隔离**——玩家代码可尝试耗尽资源或绕过（README 明示「正式发布前必须完成 bwrap 沙盒」）。
- bwrap 已安装（无需 root）；风险：用户命名空间需内核支持，实现前先验证 `bwrap --unshare-all true`。

## 需求范围

1. **可行性验证先行**：`bwrap --unshare-all true` 本机跑通；失败则记录内核/配置限制并给出替代（如 firejail 不可用时的 userns 启用步骤）。
2. **bwrap 参数集**：`--unshare-all`（用户命名空间）+ 系统目录只读挂载（--ro-bind /usr /lib /etc 等）+ tmpfs 工作区 + 最小化 /proc /dev + 禁网络 + 禁写主目录；叠加 timeout（编译 10s/运行 2s）+ 内存限制（ulimit -v）。
3. **实现方式**：新 Sandbox 实现（如 BwrapSandbox），替换 DevSandbox；Sandbox trait 不变，engine/validate 零改动。
4. **降级策略**：bwrap 不可用时显式报错（GameError::CompileEnv「沙盒初始化失败」），**不回退到无隔离模式**（安全优先）。
5. **分发门槛**：README 沙盒说明更新为已隔离；关卡内容无变化（校验流程一致）。

## 验收标准

- [ ] `bwrap --unshare-all true` 验证通过（记录输出）。
- [ ] 隔离环境内：编译/运行成功路径与 DevSandbox 结果一致（现有 15 关全过回归）。
- [ ] 攻击用例：网络访问（`std::net::TcpStream::connect`）失败、写主目录失败、`fork` 炸弹类被 timeout 终止（若隔离内可运行）。
- [ ] bwrap 缺失时游戏启动/提交给出中文错误，不静默降级。
- [ ] `cargo test --workspace` 全绿（测试用 DevSandbox 保持，BwrapSandbox 单独集成测试）。

## 参考素材

- v3 §9.1（目标架构 + 风险验证 + 分发门槛）
- README.md「代码校验与安全（开发期）」段落
