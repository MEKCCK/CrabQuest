# Rust 学习游戏 — 设计文档 v3（整合版）

日期：2026-08-16（整合定稿）
状态：整合 docs-review/ 全部 L1-L3 产出后的正式设计；v2 文件保留备查，本版取代其作为开发依据。
依据：docs-review/L1-plan.md（策略层）、L2-exercises.md / L2-error.md / L2-gamification.md / L2-chinese.md（研究层）、L3-A1/A2/B1/B2/C1/C2/D1/D2（定稿层）。全部结论可追溯到对应文件，本章末标注来源。

说明：本版相对 v2 的增量全部来自 docs-review/ 的实测结论，重点修正 v2 的三处事实错误——①rustlings 许可是 MIT only（非 MIT/Apache-2.0）；②course.rs 在线站已失效（404），中文教材链接改指 rustwiki.org；③存档无 version 字段，schema 演化会整档丢失。同时补齐 v2 的五大缺口（素材改编流水线 / 错误反馈闭环 / 中文内容与教学法 / 游戏化机制 / 许可合规）。

---

## 1. 概述与目标用户

闯关式学习游戏：玩家编写/修改 Rust 代码，系统校验（编译 → 运行 → 输出比对），推进关卡。面向从零到初级、被所有权/借用/生命周期困扰的学习者。核心目标：**把编译错误、借用检查报错包装成游戏反馈，而不是冷冰冰的报错文本**（保留 v2 §1 原义）。

### 1.1 受众细分与学习路径

- **受众**：零基础起步的 Rust 学习者；L0-L3 覆盖语法与所有权主线，L4 陷阱挑战面向已掌握基础、想验证理解深度的进阶玩家。
- **学习路径图**：L0 语法入门（变量/函数/控制流/打印）→ L1 所有权体系（移动/借用/可变借用/Clone）→ L2 组合类型与错误处理（Vec/Option/Result/match/溢出）→ L3 抽象机制（生命周期/trait/泛型/迭代器）→ L4 陷阱挑战（rust-quiz 主题：Drop 顺序/悬垂引用/惰性迭代器/闭包语义）。关卡线性解锁 L0→L4（保留 v2）。

### 1.2 差异化定位（v3 新增）

| 对比对象 | 形态 | 本游戏差异 |
|---|---|---|
| rustlings | 终端内 watch 模式改错练习，测试驱动（`#[cfg(test)]` 断言，bug 常藏在测试模块内），单条 hint | 本游戏把断言搬进 main 做 stdout 化改写（见 §2 流水线），错误反馈为「错误码 + 中文解释卡片 + 三级 hints + 链接」，且带 XP/rank/hearts/成就等游戏化层 |
| rust-course（Rust 语言圣经） | 系统教程书；**No License 禁止修改后包装分发**，且 course.rs 在线站已失效 | 本游戏只借鉴其章节结构与教学法思路（轻语言多例子、与编译器战斗），不复制任何文本；在线链接指向 rustwiki.org 的中文官方书 |
| 100-exercises-to-learn-rust | 渐进式习题（lib + 内联测试），CC BY-NC 4.0 | 只借鉴「错误先行、讲解后置、项目贯穿」教学法与结构，不复制习题文本；非商业约束见 §10 |
| rust-quiz | 高阶陷阱题（选择题/输出预测/编译失败），CC BY-SA 4.0 | 只改编 .rs 题目代码，解释文案全部自写（规避 SA 传染）；题目经转化规则进入 L4 |

教学法总原则（来源：L2-chinese D2、L3-D2 §1）：**错误先行、讲解后置**——先让玩家撞上真实编译错误（游戏关卡天然如此），再通过错误码卡片解释机制，最后给修复方向；rust-course「轻语言、多例子」的密度作为文案参考（不复制）。

来源：docs-review/L1-plan.md §4、L2-exercises.md 结论摘要、L2-chinese.md D2、L3-D2-pedagogy.md §1。

---

## 2. 素材改编流水线

### 2.1 素材适配总览（哪些源可用、怎么改）

| 素材仓库 | 许可 | 关卡适配方式 | 关键约束 |
|---|---|---|---|
| rustlings 6.5.0（24 目录 94 题） | MIT only | 主力来源：修复编译关 + 输出比对关；测试驱动题需 stdout 化改写 | 52 个文件含 `#[cfg(test)]`，裸 rustc 直编时测试模块被剥离，bug 不可见 → 必须 W1 改写；保留版权声明 |
| 100-exercises-to-learn-rust | CC BY-NC 4.0 | 结构借鉴 + 代码重写（02/03/04 章无 helpers 依赖，优先） | 07_threads/08_futures 排除；06 章依赖 helpers 需内联；非商业 |
| rust-quiz（37 题，35 有效） | CC BY-SA 4.0 | 输出比对关（21 题 D1）/ 选择题关（kind=quiz）/ allow_compile_fail 关（011 → E0794） | 005/007 为 tombstone 不采用；解释文案必须自写 |
| rust-course | No License | 仅参考章节结构与知识点框架；fight-with-compiler 编译失败示例的思路可借鉴 | **不得改编进游戏分发**；course.rs 已失效，链接引用也不可行，只能作本地编写参考 |

（来源：L1-plan §1 缺口一、L2-exercises A1-A4、L3-A2 §1。）

### 2.2 八步改编流程（S1-S8）

```
选素材 → 分析 → stdout 化改写 → expect_output 提取 → 错误码确认 → 三级 hints → source 标注 → 质量检查
 (S1)     (S2)        (S3)             (S4)              (S5)          (S6)        (S7)         (S8)
```

- **S1 选素材**：按优先级表选取（L0 首选 rustlings 01_variables/02_functions/03_if/04_primitive_types；L1 首选 06_move_semantics；L2 首选 13_error_handling；L3 首选 16_lifetimes/15_traits/18_iterators；L4 首选 rust-quiz）。硬排除：rustlings 17_tests/19_smart_pointers/20_threads/21_macros/22_clippy；100-exercises 07_threads/08_futures/依赖 helpers 章节；rust-course advance-practice、too-many-lists。
- **S2 分析（三查）**：`grep -c "fn main"`（0 = 纯 lib 需补 main）；`grep -c "#\[cfg(test)\]"`（>0 = bug 可能藏在测试内）；`grep use common/ticket_fields`（命中外部依赖 → 内联或排除）。
- **S3 stdout 化改写**：执行 W1-W10 十规则（见 2.3）。
- **S4 expect_output 提取**：编译运行 fixed 版，捕获 stdout 原文即为期望值；期望值只写进 expect_output，绝不进 starter_code。
- **S5 错误码确认**：编译运行 broken 版，`rustc --edition 2021 2>&1 | grep -o 'error\[E[0-9]*\]' | head -1` 即该关错误码；无 E 码 → 改写题干或排除，不得用模糊文案替代。
- **S6 三级 hints 撰写**：概念 → 章节链接 → 代码片段（规范见 §6.2）；rustlings hint 可改写（MIT），rust-quiz Hint 段必须自写中文。
- **S7 source 标注**：统一格式 `<仓库> (<相对路径>, <版本>, <许可后缀>，[改编说明])`（规范见 §3.5）。
- **S8 质量检查**：执行 Q1-Q10（见 2.4）。

### 2.3 stdout 化改写规则集（W1-W10 摘要）

| 规则 | 内容 | 要点 |
|---|---|---|
| W1 测试体→main 输出 | 把 `#[cfg(test)]` 内断言搬进 main，一条断言 ↔ 一条 println | bug 在裸 rustc 下不可见（实测 exit 0 无输出），必须搬出才暴露 |
| W2 assert_eq!→println! | 数值/字符串用 Display `{}`；`{x}` 内联与 `{}` 等价可互转 | 输出无引号 |
| W3 `{:?}` Debug 对照 | 集合/复合类型用 `{:?}`（`[22, 44, 66, 88]`/`Some(3)`/`Ok(5)`） | **浮点 Display 丢 `.0` 是高频坑**；`String` Debug 带引号，必须用 `{}` |
| W4 输出行序敏感 | 逐字节比对，行序不可调换 | drop 逆序是合法题型；禁依赖未定义顺序（HashMap 迭代序、多线程）的输出 |
| W5 字符串转义 | expect_output 写**运行后可见的真实字符**，不写源码转义 | 拿不准 `./out \| xxd` 核对字节 |
| W6 中文输出 | UTF-8 逐字节精确比对（10-l2-result「解析失败」先例） | 全角/半角、空格差异直接判失败 |
| W7 保留骨架删答案痕迹 | starter_code 保留 bug 与 TODO；删 assert 期望值、已修复写法、空 main 注释 | grep starter_code 不得出现期望值；TODO 注释中文化 |
| W8 allow_compile_fail 型 | starter 故意不编译；expect_error_code 必须**本机实测**首条 E 码 | 每次改编重测，不抄旧表（抗 rustc 版本漂移） |
| W9 panic 型 | 方案 A：`expect_panic` 字段（子串匹配 panic 消息）；方案 B（引擎未支持前）：改写成修复后正常输出关 | panic 分支优先于输出比对，不能靠 expect_output 验收 panic 修复 |
| W10 CRLF 归一化 | 比对前 `expect.replace("\r\n", "\n")` 先于 trim | 不引入整行 trim（行尾空格区分是合理严格性） |

（来源：L3-A2-pipeline.md §2 全文；对应 L2-exercises A5 的 R1-R10。）

### 2.4 质量检查清单（Q1-Q10）

1. starter 编译状态符合题型（code 关编译失败或失败原因即题干 bug；allow_compile_fail 关 starter 编译失败）。
2. 预期错误码 = `--edition 2021` 实测首条 E 码；无 E 码题已改写/排除。
3. expect_output 与 fixed 版 trim+CRLF 归一化后逐字节相等。
4. expect_output 无 `\r`、无行尾空格（除非 trim_lines=true）、行序与 fixed 一致、末尾无刻意 `\n`。
5. hints 第 1/2 级不泄露答案；hint_unlock 与 hints 等长。
6. starter 无答案痕迹（grep 不到期望值/修复写法/assert 常量）；TODO 已中文化。
7. source 标注精确（仓库/路径/许可后缀）；rust-course 素材带 link 且无复制文本；rust-quiz 解释自写并注明。
8. edition 2021 兼容 + 仅 std：无外部 crate、无 `use common/ticket_fields`、无 thread::spawn/unsafe/async。
9. 输出确定性：无时间戳、无 HashMap 迭代序、无多线程乱序；print! 拼接已显式预期。
10. quiz 型合法 + 中文化完整：options 2-6 项无重复、answer_index 界内实测对应、description/hints/expect_output 全中文。

来源：docs-review/L3-A2-pipeline.md §1（S1-S8）与 §4（Q1-Q10）、§2（W1-W10）、L2-exercises.md A1-A5。

---

## 3. 关卡数据模型 v2

### 3.1 命名规范（修正 v2）

- 关卡 `id` 无数字前缀：`l0-integers`、`l1-move`、`l4-boss`（v2 示例 `ownership_01` 与实际不符，废弃）。
- 文件名为排序号前缀：`NN-lX-主题.toml`（00-14 已占用，扩展新关从 04 起连续编排，最终以 game-data 实际排序为准）。
- `tier` 取值 `l0`/`l1`/`l2`/`l3`/`l4`。

### 3.2 关卡 TOML schema v2 草案（相对 v1 新增字段全部 `#[serde(default)]`，旧文件无需改动）

```toml
[[level]]
id = "l4-mutable-zst"                  # 必填：全局唯一
title = "挑战：可变零大小类型"          # 必填
tier = "l4"                            # 必填：l0|l1|l2|l3|l4
kind = "quiz"                          # 可选，缺省 "code"；"quiz"=选择题，"code"=编译/输出关
description = "S 是零大小类型（ZST）。执行 let [x, y] = &mut [S, S]; 后……结果为？"  # 必填（中文，规范见 §6.2）

# ---- 提示（三级；解锁见 hint_unlock）----
hint = "ZST 没有运行时表示。"           # 可选：无 hints 时的单条兜底
hints = [                              # 可选：三级提示（概念→链接→代码）
  "这与类型的「运行时大小」有关。",                 # 1 概念级：不含符号与修复方向
  "&mut [S, S] 中两个元素的地址是什么关系？",        # 2 定位级：指向机制，不给答案
  "ZST 占 0 字节，两个可变引用可指向同一地址，故选 1。", # 3 解法级：给出答案与解释
]
hint_unlock = [1, 3, 5]                # 可选：失败次数阈值；缺省=手动逐级揭示（现状）

# ---- code 关字段 ----
starter_code = '''                     # code 型必填；quiz 型为展示代码（须可编译）
struct S;
fn main() {
    let [x, y] = &mut [S, S];
    let eq = x as *mut S == y as *mut S;
    print!("{}", eq as u8);
}
'''
expect_output = "1"                    # code 型：fixed 版运行输出（trim+CRLF 归一化后比对）
expect_panic = "The journey took no time"  # 可选：panic 消息子串匹配（优先于 expect_output，本期预留）
allow_compile_fail = true              # 可选，缺省 false
expect_error_code = "E0425"            # allow_compile_fail=true 时必填（实测 E 码）

# ---- quiz 型字段（kind="quiz" 时必填）----
options = ["0", "1", "编译错误", "不确定"]
answer_index = 1                       # 0-based；越界在 game-data 加载时校验

# ---- 来源与许可（必填）----
source = "rust-quiz (questions/013-mutable-zst.rs, CC BY-SA 4.0，解释自写)"
link = "https://dtolnay.github.io/rust-quiz/013"   # 可选；rust-course 素材必填

# ---- 可选 ----
trim_lines = false                     # true 时每行去尾随空格后再比对（缺省 false）
is_boss = false                        # true = Boss 关（机制见 §7.5，显式标注不靠推断）
```

### 3.3 expect_output 规范化语义（落码于 validate/mod.rs）

| 语义 | 规则 |
|---|---|
| 两端 trim | 保持现状 `stdout.trim() == expect_output.trim()` |
| CRLF 归一化 | 比对前 `expect.replace("\r\n", "\n")`，先于 trim |
| 行序 | 敏感：多行逐字节比对，行序不可调换 |
| 内部空行 | 参与比对（严格）；expect_output 每一行必须有对应输出行 |
| 行尾空格 | 敏感（不整行 trim）；可选 `trim_lines: bool` 打开时每行先 trim 再比对 |
| 尾随换行 | 被 trim 吞掉，expect_output 末尾不要刻意写 `\n` |
| 空 expect_output | 视为「只要求编译运行成功」 |

### 3.4 hints 三级语义与解锁

- 三级内容：**1 概念级**（知识点名/错误类别/方向，禁具体符号、行号、修复代码）→ **2 定位级**（出错符号/行号、错误码含义、为什么错，禁修复写法与期望值）→ **3 解法级**（最小修复代码片段、期望输出提示，全关卡唯一允许出现修复代码的位置）。
- `hint_unlock: Vec<u32>`（与 hints 等长）：第 i 条在 `attempts >= hint_unlock[i]` 时自动解锁可见；未达阈值时按键揭示到已解锁下一条。缺省 = 现状手动揭示。推荐默认阈值 `[1, 3, 5]`。
- 失败联动行为表（L3-C1 §4.1 定稿，见 §7.6）：0 次失败可主动看 hint1；2 次失败解锁 hint1-2 并自动展开 hint1；3 次失败解锁 hint1-3 并自动推进到 hint2；≥4 次失败自动推进到 hint3 并出现「查看参考答案」按钮（需二次确认）。

### 3.5 source 字段格式规范

统一格式：`<仓库> (<相对路径>, <版本/主题>, <许可后缀>，[改编说明])`。

| 素材 | 许可后缀 | 复制边界 |
|---|---|---|
| rustlings | `MIT` | 代码直接搬（保留版权声明）；hint 可改写 |
| 100-exercises | `CC BY-NC 4.0` | 非商业；建议「只借鉴结构 + 代码重写 + 署名」 |
| rust-quiz | `CC BY-SA 4.0` | 代码可搬；**解释文案必须自写中文**，改编说明标注「解释自写」 |
| rust-course | `No License` | 只链接不复制（link 字段）；但 course.rs 已失效，实际仅作本地参考 |
| The Rust Book（book-cn） | `MIT/Apache-2.0` | 代码可搬（用于 05-l1-borrow 类来源修正） |

存量 15 关的 source 需复核：05-l1-borrow 实为 The Book ch4、11-l3-lifetime 混两题、08-l2-vec 无原题（L2 已点名）。

来源：docs-review/L1-plan.md 约束 11/12、L3-A2-pipeline.md §3（schema v2 草案、expect_output 语义、hints 解锁、source 规范）、L3-C1-gamification.md §2（is_boss 字段）。

---

## 4. 关卡内容大纲

### 4.1 53 关主线分布总表（现有 15 + 新增 38；另 7 个扩展槽）

> **修订（2026-08-16）**：为满足「从 Hello World 起步、学习曲线平缓」，L0 新增前置关
> `00-l0-hello-world`（空 main + 目标输出 `Hello, world!`，无编译错误，只要求补一行 println!）。
> 原 00-53 关号全部 +1（原 00-l0-hello → 02；01-l0-print 保持 01；32a → 33a），
> 各层内顺序不变，L1-L4 关号顺延。关卡总数 55 → 56。

图例：`★` = 已有完整 TOML 草案（10 个，见 §4.4）；`现有` = 已存在 15 关。错误码 = broken 版实测首错误码（`—` = broken 编译通过、错误在运行期/逻辑层）。

**L0 层（10 关：现有 4 + 新增 6）——语法入门**

| 关号 | id | 主题 | 素材来源 | 预期错误码/输出 |
|---|---|---|---|---|
| 00 | l0-hello（现有） | let 变量 | rustlings 01_variables/variables1.rs | E0425 |
| 01 | l0-print（现有） | 打印 | 自编 | —（无 E 码 → EUNKNOWN 兜底） |
| 02 | l0-function（现有） | 函数调用 | rustlings 02_functions/functions1.rs | E0425 |
| 03 | l0-loop（现有） | 循环 | 自编 | — |
| 04 | l0-integers ★ | 整数类型 | 100-exercises 02/01_integers（R1） | E0308+E0277；输出 `1 + 2 * 4 = 9` |
| 05 | l0-variables2 ★ | 类型推断 | rustlings 01_variables/variables2.rs | E0283；输出 `x is ten!` |
| 06 | l0-if | if 表达式 | rustlings 03_if/if1.rs | E0308 |
| 07 | l0-primitives | 基础类型 | rustlings 04_primitive_types/p1.rs | E0425 |
| 08 | l0-functions2 | 函数签名 | rustlings 02_functions/functions3.rs | E0061 |
| 09 | l0-boss | 综合：变量+函数+分支 | 自编 | E0425；输出 `总和：15` |

**L1 层（12 关：现有 4 + 新增 8）——所有权、借用、Clone**

| 关号 | id | 主题 | 素材来源 | 预期错误码/输出 |
|---|---|---|---|---|
| 10 | l1-move（现有） | 可变参数 | rustlings 06_move_semantics/move_semantics1.rs | E0596 |
| 11 | l1-borrow（现有） | 借用 | The Book ch4（source 待复核） | E0308 |
| 12 | l1-mut-borrow（现有） | 可变借用 | 自编 | E0596 |
| 13 | l1-clone（现有） | Clone | rustlings 06/move_semantics5.rs 主题 | — |
| 14 | l1-move2 ★ | 移动后使用 | rustlings 06/move_semantics2.rs（R1） | E0382；两行 len/内容 |
| 15 | l1-move3 | 可变参数 2 | rustlings 06/move_semantics3.rs | E0596 |
| 16 | l1-strings | String 基础 | rustlings 09_strings/strings1.rs | E0308 |
| 17 | l1-structs ★ | 结构体实例化 | rustlings 07_structs/structs1.rs（R1） | E0063；输出 `(0, 255, 0)` |
| 18 | l1-options1 | Option 初识 | rustlings 12_options/options1.rs | E0308 |
| 19 | l1-enums | 枚举变体 | rustlings 08_enums/enums1.rs | E0599 |
| 20 | l1-ownership-ticket | 访问器借用 | 100-exercises 03/06_ownership（R1+内联） | E0382 |
| 21 | l1-boss | Boss：move+borrow+clone | 自编 | E0596 |

**L2 层（12 关：现有 3 + 新增 9）——组合类型、Option、Result、错误处理**

| 关号 | id | 主题 | 素材来源 | 预期错误码/输出 |
|---|---|---|---|---|
| 22 | l2-vec（现有） | Vec | rustlings 主题（source 待复核） | —（运行期 panic 越界） |
| 23 | l2-option（现有） | Option | 自编 | — |
| 24 | l2-result（现有） | Result | 自编（中文输出先例） | —（运行期 panic unwrap） |
| 25 | l2-errors3 ★ | `?` 与 main 返回类型 | rustlings 13/errors3.rs | E0277；输出 `You now have 59 tokens.` |
| 26 | l2-errors2 | Result 内 `?` | rustlings 13/errors2.rs | E0369 |
| 27 | l2-saturating ★ | 整数溢出修复 | 100-exercises 02/09_saturating（R1） | 运行期 overflow panic；输出 `120`/`4294967295` |
| 28 | l2-errors4 | 自定义错误 | rustlings 13/errors4.rs（R1） | 输出 `Ok(PositiveNonzeroInteger(10))` 等 |
| 29 | l2-vecs2 | Vec 遍历 | rustlings 05_vecs/vecs2.rs（R1） | 输出 `[2, 4, 6]` 类 |
| 30 | l2-hashmap | HashMap | rustlings 11_hashmaps/hashmaps1.rs | E0425 |
| 31 | l2-strings2 | String 拼接 | rustlings 09_strings/strings2.rs | E0308 |
| 32 | l2-match | match 穷尽 | 自编（少分支） | E0004 |
| 33 | l2-boss | Boss：Option+Result+match | 自编 | E0308 |

**L3 层（11 关：现有 2 + 新增 9）——生命周期、trait、泛型、迭代器**

| 关号 | id | 主题 | 素材来源 | 预期错误码/输出 |
|---|---|---|---|---|
| 34 | l3-lifetime（现有） | 生命周期标注 | 自编（longest） | E0106 |
| 35 | l3-trait（现有） | trait 定义 | 自编 | — |
| 36 | l3-lifetime3 ★ | struct 生命周期 | rustlings 16/lifetimes3.rs | E0106；输出 `1984 by George Orwell` |
| 37 | l3-lifetime1 | 函数生命周期 | rustlings 16/lifetimes1.rs（R1） | E0106 |
| 38 | l3-generics | 泛型推断 | rustlings 14_generics/generics1.rs | E0282 |
| 39 | l3-traits1 | trait 方法实现 | rustlings 15_traits/traits1.rs | E0046 |
| 40 | l3-iterators ★ | 迭代器实现阶乘 | rustlings 18_iterators/iterators4.rs（R1） | E0308；输出 0!/5!/10! 三行 |
| 41 | l3-iterators2 | 迭代器消费 | rustlings 18_iterators/iterators2.rs | E0308 |
| 42 | l3-conversions | From 转换 | rustlings 23_conversions/conversions1.rs | E0277 |
| 43 | l3-enums3 | 枚举综合 | rustlings 08_enums/enums3.rs（R1） | 输出 `Move to (1, 2)` 类 |
| 44 | l3-boss | Boss：lifetime+generic+trait | 自编 | E0106 |

**L4 层（9 关：现有 2 + 新增 7）——陷阱挑战（rust-quiz 为主）**

| 关号 | id | 主题 | 素材来源 | 预期错误码/输出 |
|---|---|---|---|---|
| 45 | l4-drop-order（现有） | Drop 顺序 | rust-quiz 012 主题（自编） | — |
| 46 | l4-lifetime-trap（现有） | 悬垂引用 | rust-quiz 037 主题（自编） | — |
| 47 | l4-lazy-map | 惰性迭代器 | rust-quiz 026 | 输出 `112031` |
| 48 | l4-fnptr ★ | 函数指针比较（allow_compile_fail） | rust-quiz 011 | E0794 |
| 49 | l4-mutable-zst | 零大小类型 | rust-quiz 013（kind=quiz） | 选项索引 1 |
| 50 | l4-drop-underscore | `_` 与 `_s` 释放时机 | rust-quiz 019 | 输出 `21` |
| 51 | l4-lifetime-ext | 临时值延长 | rust-quiz 037 | 输出 `1001` |
| 52 | l4-fnmut-copy | FnMut+Copy 闭包 | rust-quiz 036 | 输出 `1223` |
| 53 | l4-boss ★ | Boss：借用+所有权+Option 综合（购物车） | 自编（参考 100-exercises ticket 结构） | E0596；两行中文输出 |

**扩展槽（7 个，达 60 关上限）**：l0-functions4（无 E 码→兜底）、l1-move4（→E0499/E0502，L2 点名收）、l2-panics（需 expect_panic 字段）、l3-traits5（→E0425）、l4-break-return（rust-quiz 020 复议）、l4-iterator-lazy2（rust-quiz 026 变体）、l4-macro-count（rust-quiz 001）。均需按 L2 A1/A3 结论复核后再定。

### 4.2 Boss 关设计

- **分布**：大纲中 L0-L4 各 1 个综合关（09/21/33/44/53）；**Boss 机制（is_boss=true）仅启用 L1-L4 层末关（21/33/44/53）**——L3-C1 明确「L0 不设 Boss（入门层全是教学关）」，09-l0-boss 按普通综合关处理（不启用尝试配额分档/提示禁用）。现有 15 关结构下对应 07-l1-clone / 10-l2-result / 12-l3-trait / 14-l4-lifetime-trap。
- **混合概念**：Boss 关覆盖本层 ≥3 个知识点，但 broken 版只允许一个预期错误码（或一组同码），避免「一次编译多个错误码取哪个」的歧义（L1 约束 19）。例：53-l4-boss 覆盖结构体+impl+`&mut self`+Vec<(String,u32)>+迭代器 sum+Option+match，修复点只有一处（`&self`→`&mut self`）。
- **机制数值**（L3-C1 §3）：尝试配额分档 XP（首通 ≤4 次 +50，>4 次 +30）、失败不扣心、提示默认禁用（fail_count≥5 解锁兜底）、完美/连击加成照常。
- **验收标准**：Boss broken 版只有 1 个预期错误码（5 个 Boss 已按此设计：E0425/E0596/E0308/E0106/E0596）。

### 4.3 难度曲线

- **层间**：L0 单关 1 处小改动（补 let/类型/参数）→ L1 1 处签名或调用改动（mut/&/clone）→ L2 1 处签名或逻辑（?/返回类型/saturating）→ L3 1 处标注或实现体（'a/impl/迭代器）→ L4 理解型修复或制造错误（输出比对/allow_compile_fail/quiz）。
- **rust-quiz Difficulty 映射**：D1 = 知识冷门程度低（≠题目简单），D1 进 L4 前半段（47/49/50/51），D2 进 L4 中段（52），D3 进 L4 末尾与 Boss（48/53）；013 需前置 recap。D2/D3 其余题不进第一版。
- **题型分布（53 关）**：编译错误修复型 43 关（含 5 Boss）；逻辑/运行期修复型 2 关（27/29）；allow_compile_fail 型 1 关（48）；选择题型 1 关（49，v1 若不做 quiz 类型则暂缓，L2 明确 v1 不引入选择题）。

### 4.4 十个完整关卡 TOML 草案（引用）

已用 rustc 1.97.0 `--edition 2021` 实测（broken 报预期错误码、fixed 输出逐字节一致）：04-l0-integers、05-l0-variables2、14-l1-move2、17-l1-structs、25-l2-errors3、27-l2-saturating、36-l3-lifetime3、40-l3-iterators、53-l4-boss（Boss）、48-l4-fnptr（allow_compile_fail）。完整草案见 docs-review/L3-A1-levels.md §5；改编执行时按 §2 流水线重测（S5 错误码确认不抄旧表）。

来源：docs-review/L3-A1-levels.md §2-§6（分布表/Boss/难度曲线/草稿）、L2-exercises.md A1/A2/A3、L3-C1-gamification.md §3.1（四段 Boss 与 L0 不设 Boss 的裁决）。

---

## 5. 校验与错误反馈体系

### 5.1 错误码全集（30 活跃码 + 2 死码）

severity 语义：**P0** = 现有关卡已触发或 L0-L3 教学主线必踩（UI 常驻展开、高亮）；**P1** = 扩展路线确定会触发（默认展示、可折叠）；**P2** = 可选/低优先（折叠展示）。

| 组 | 码 | severity | concept |
|---|---|---|---|
| A. 现有关卡触发（P0，v2 化补字段） | E0425（找不到名字） / E0596（无法可变借用） / E0382（使用已移动值） / E0106（缺生命周期标注） / E0599（没有该方法） / E0597（借用活得不够久） | P0 | name / borrow / ownership / lifetime / trait / lifetime |
| B. 新增 P0（教学主线，实测发射） | E0282（类型推断失败） / E0384（不可变变量重新赋值） / E0594（修改只读借用后的值） / E0621（签名需显式生命周期） / E0601（缺 main） | P0 | type / variable / borrow / lifetime / main |
| C. P1（borrow 家族补全） | E0506（赋值给被借用变量） | P1 | borrow |
| D. rustlings 缺口新增 | E0046（trait 方法未实现） / E0283（类型推断歧义） / E0381（可能未初始化） / E0603（访问私有项） | P1 | trait / type / variable / module |
| D2. rustlings 缺口新增（P2） | E0072（递归类型需 Box） / E0423（宏名/值名误用） | P2 | type / macro |
| E. 留存 13 码（补 severity/concept） | E0004 / E0061 / E0204 / E0277 / E0308 / E0369 / E0433 / E0499 / E0502 / E0505 / E0507 / E0618 / E0623 | P1/P2 混合 | match/function/trait/type/trait/trait/path/borrow×4/call/lifetime |
| F. 死码 → [deprecated] | E0412（找不到类型现报 E0425）、E0504（不再发射，语义相近用 E0506） | — | — |

**关键实测结论**（必须遵守）：
1. E0277 与 E0369 **双条目共存**：`i32+f64`/`1+"x"` 现报 E0277，只有 `&str+&str` 仍报 E0369（运算符错误码已漂移）。
2. E0425 已扩张覆盖「值/函数/类型」三种找不到（吞并原 E0412 场景）。
3. E0412/E0504 是死码（`--explain` 明示 no longer emitted），删除活跃条目、登记 [deprecated]，防关卡作者设计无法通过的死码关。
4. **最大的覆盖盲区是无 E 码错误**（format 参数数量、let chains 版本、常量溢出），必须走 EUNKNOWN 兜底（见 5.3）。
5. 关卡大纲新增码：E0063（17-l1-structs 缺字段，高）、E0794（48-l4-fnptr，中）、E0170/E0560/E0060（扩展槽低优先）——随关卡收录时补 errors.toml 条目。

### 5.2 errors.toml schema v2

```toml
[E0382]                       # 现有码 v2 化样板
zh = "使用了已移动的值：所有权已转移给别的变量/函数，原变量不能再使用"   # 必填（≤60 字，本地内置）
link = "https://doc.rust-lang.org/error_codes/E0382.html"            # 必填（官方页）
link_zh = "https://rustwiki.org/zh-CN/book/ch04-01-what-is-ownership.html"  # 可选（中文概念页，rustwiki 白名单）
severity = "P0"               # 可选：P0|P1|P2，默认 P1
concept = "ownership"         # 可选：name|ownership|borrow|lifetime|type|trait|macro|module|match|variable|function|panic
fix = "改用引用 &s（只借用）或 s.clone()，或调整使用顺序"  # 可选（≤40 字）
example = '''                 # 可选（≤8 行最小复现 + 一处修复标注）
let s1 = String::from("hi");
let s2 = &s1;
println!("{} {}", s1, s2);
'''

[fallback]                    # 无 E 码错误（EUNKNOWN）与未收录码兜底
zh = "编译出错但没有标准错误码（可能是 rustc 版本差异或语法/宏类问题）。请对照面板中的原文与行号，逐行检查最近的改动；仍无法解决可尝试简化代码。"
link = "https://doc.rust-lang.org/error_codes/index.html"

[deprecated]                  # 死码登记（rustc 1.97 起 no longer emitted）
E0412 = "该错误码已不再由 rustc 发射（找不到类型现报 E0425），请勿用于关卡设计"
E0504 = "该错误码已不再由 rustc 发射，请勿用于关卡设计（语义相近可用 E0506）"
```

兼容规则：新字段全部 `#[serde(default)]`；`[fallback]`/`[deprecated]` 用独立结构解析，旧文件缺省不报错；`mapper.rs` 的 `default_fallback()` 扩到全部 P0 码 + EUNKNOWN（assets 缺失时最小可用）。

### 5.3 解析器边界规则（L3-B2 定稿）

1. **只匹配错误码 E0xxx + error: 行，不匹配报错文本**（抗 rustc 版本差异，策略不变；实测 E 码语义稳定、措辞会漂移）。
2. **无 E 码捕获（EUNKNOWN）**：trim 后以 `error:` 开头且不含 `[E` 的行 → `CompileIssue { code: "EUNKNOWN", line: 其后第一个 --> 行, kind: NoCode, message }`；`validate()` 编译失败且 errors 为空 → **强制兜底文案，禁止空反馈**（01-l0-print 第一关就会触发，现网级 bug 的回归锁）。`-D warnings` 提升的 error 也走此路径。
3. **多错误码取舍**：`allow_compile_fail=true` 只取 rustc 输出顺序第一条（版本间稳定，关卡作者可控）；展示全列出但**上限 3 条**，超出折叠为「+N 条」；**不做行号重排**（E0621 的 --> 在返回表达式行、E0601 在文件末尾行，反直觉定位是实测事实）。
4. **同码多 `-->`**：每条错误只取第一个 `-->`（首使用点，从上往下改最顺）；warning 的 `-->` 不得附加到错误（错误块边界防御）。
5. **warning vs error**：`warning:` 行不生成 CompileIssue、不判失败（裸 rustc 下 warning 退出码 0）；clippy lint 裸 rustc 完全不触发（22_clippy 与管线不兼容 → 方案 A：把 lint 违规改写成等价编译错误，如 `let pi: i32 = 3.14;` 触发 E0308；方案 B lint_mode 第二版再议）。
6. **panic 净化与分类**：净化三步——先 strip 行首空白（实测 panic stderr 以空行开头）→ 剥临时目录路径（`(?:/tmp/)?rlg-[A-Za-z0-9_]+/`）→ 剥 `thread 'main' (线程id) panicked at` 头与 note 行；保留 `main.rs:3:21` 定位行。8 类关键词分类（index out of bounds / Option::unwrap on None / Result::unwrap on Err / ParseIntError / overflow / divide by zero / 显式 panic / 分配失败）→ 各自中文提示 + 修复方向；不命中走通用文案。**panic 分支优先级 `panic > 输出比对 > 错误码`**。

### 5.4 「错误码 → 概念 → 修复」三屏递进反馈（与 §7.3 反馈面板配合）

| 屏 | 内容 | 数据来源 |
|---|---|---|
| 第一屏 | 错误码徽章 + 一句话人话 + 首个出错行号 | `zh` + parser 首条 `-->`；编译失败自动弹出、默认展开 |
| 第二屏 | 概念卡（为什么错）+ 中文概念链接 | 「为什么错」文本 + `link_zh`；点击「为什么」展开 |
| 第三屏 | 修复方向 + 最小修复代码 | `fix` + `example`；hints[2] 解锁后叠加关卡代码片段；「再试一次」按钮 |

交互规则：卡片是**被动反馈**（编译失败自动出现），三级 hints 是**主动索取**，共享同一术语与链接体系、文案不重复（卡片讲机制通用、hints 讲本题具体）；第二/三屏不自动展开；无 E 码错误第一屏显示 fallback 兜底 + 原文；死码不出卡片。

来源：docs-review/L3-B1-errorcodes.md §1-§3、L3-B2-parser.md §1-§4、L3-D2-pedagogy.md §3、L2-error.md B1-B4、L3-A1-levels.md §7。

---

## 6. 中文内容规范

### 6.1 术语基准（L3-D1 定稿：book-cn 为主、reference-cn 补充）

裁决基准：lifetime→**生命周期**（book-cn 234 处/rust-course 561/rbe-cn 90 一致；reference-cn「生存期」内部自相矛盾不采）；trait→**保留英文 trait**（book-cn 与官方 reference 均保留；「特征/特质」互斥且非官方，首次出现括注「（trait，可译为特征）」）；dangling→**悬垂引用**（禁「悬挂」）；move→**移动（move）**（「转移所有权」仅作解释性短语）；lifetime elision→**省略**；variable→**变量**（讲解技术细节可用「绑定」作动词）；borrow checker→借用检查器；可变借用/可变引用按语境（动作用借用、`&mut T` 类型用可变引用）。nomicon-cn 全英文未翻译，不作术语来源。

**50 词条摘要**（裁决译名；全部词条见 L3-D1 §1 全文）：
所有权/所有者、移动（move）、借用、可变借用、借用检查器、悬垂引用、生命周期、生命周期省略、切片、变量、遮蔽、枚举、结构体、模式匹配、Result（保留）、Option（保留）、panic!（保留）、unwrap/expect（保留）、trait（保留）、trait bound（保留）、trait 对象、泛型、类型推断、迭代器、闭包、类型别名、强制转换（coercion，与 cast「类型转换」区分）、模块、crate（保留）/包、路径、use（保留）、Box<T>（保留）、Rc<T>（保留）、Arc<T>（保留）、解引用（Deref 保留）、Drop（保留）、内部可变性、不安全 Rust、宏、`?` 运算符（传播错误）、Vec（向量，首括注动态数组）、哈希 map、字符串、智能指针、if let（保留）、Copy（保留）、引用循环、Send/Sync（保留）、元组、数组。

### 6.2 讲解文案字数规范（全游戏统一）

| 文本位 | 上限 | 强制规则 |
|---|---|---|
| description | ≤ 80 字 | 只写「症状 + 目标输出 + 涉及概念名」三要素，**禁止修复动作**（改哪行/用什么 API/调什么方法） |
| hints[0] 概念引导 | ≤ 40 字 | 无代码、无 API 名，一句话点出底层概念 |
| hints[1] 章节链接 | ≤ 60 字（链接不计） | 仅定位（「见 book-cn ch04-02『可变引用』」）+ rustwiki.org 白名单链接 |
| hints[2] 代码片段 | ≤ 3 行代码 + ≤ 40 字说明 | 全关卡唯一允许出现修复代码的位置 |
| 整关 hints 总量 | ≤ 200 字 | 与现状 120-170 字量级一致 |
| 错误码卡片标题 | ≤ 16 字（含错误码） | 折叠时可辨识 |
| 卡片「一句话人话」 | ≤ 30 字 | 见 §5.4 第一屏 |
| 卡片「为什么错」 | ≤ 60 字 | 每卡只讲一个概念 |
| 卡片「修复方向」 | ≤ 40 字 | 不给整段代码 |
| 卡片整卡 | ≤ 200 汉字（不含代码与链接） | 「是什么/为什么/怎么改」三段之和 |
| Boss 关前置 recap | ≤ 100 字 | 用基准术语重述相关概念（rust-quiz 题必备） |

**现有关卡必改清单**：10-l2-result 的 description/hint 去掉「用 match 处理错误」与整段答案代码；07-l1-clone 的 description 去掉「使用 s1.clone()」剧透；为 11/15 关补全三级 hints（现有仅 3 关用了 hints 数组，且 08 只有 2 条）。CI 启发式：description 出现 `clone()`/`&mut`/`let mut` 等关键词告警人工确认。

### 6.3 链接策略（link / link_zh 双轨 + 离线降级）

- **白名单**：`link` = `https://doc.rust-lang.org/error_codes/E0xxx.html`（官方错误码页，实测 200，必填）；`link_zh` = `https://rustwiki.org/zh-CN/book/...` | `rustwiki.org/zh-CN/rust-by-example/...`（中文概念页，实测 45+25+12 全部 200，可选）。
- **禁用**：`course.rs` 及子域（实测 404）、`doc.rust-lang.org/zh-CN/book/*`（实测 404）、`rust-lang.org/zh-CN/*`（实测 404）、`practice.course.rs`（已跳转第三方 beatai.org）。
- 解释文本**全部本地内置**（errors.toml zh 字段），离线可玩；链接只作「可打开/可复制」入口。
- **离线降级**：启动时对 rustwiki 一个页面做非阻塞 HEAD（超时 ≤3s）→ 缓存 `offline` 标志；offline 时链接区整体隐藏为灰字提示「当前离线：概念已内置在讲解卡片中」，hints[1] 链接降级为纯文本；点击链接失败弹 toast「无法打开在线教材」，不崩溃不阻塞。配置项 `[ui] online_links` 可手动关闭。
- UI 展示：中文页优先（link_zh），官方英文页折叠为「英文官方文档」。

### 6.4 原创改写自查清单（每篇文案落地前逐条过）

① 术语对照 §6.1 裁决表（禁「特征/特质/生存期/悬挂」）；② 短句重述、不逐句翻译；③ source 字段与许可表一致；④ 红线：不复制 rust-course/100-exercises 文本、不直接采用 rust-quiz Hint/Explanation；⑤ diff 抽查与来源 ≥3 连词雷同即返工。

来源：docs-review/L3-D1-terms.md §1-§5、L3-D2-pedagogy.md §1、L2-chinese.md D1-D4。

---

## 7. 游戏化与 UI 规范

### 7.1 设计原则（L3-C1 定稿）

纯规则零 IO（XP/rank/hearts/streak/成就/Boss 全部是 game-core 纯函数，UI 只读状态）；数值照搬 rust-quest 但**不照搬全部**（保留本游戏编译校验特性，无选择题，hearts/≥75% 语义映射为尝试次数）；防挫败优先（XP 只增不减、hint 零成本、Boss 失败不扣心）；rank 与 XP 解耦（rank 按完成关卡数判定，不读 XP）；**不新增关卡类型**（v1 无选择题，Boss 关 = 内容更混合 + 机制不同的普通关）。

### 7.2 XP 定价表

| 事件 | XP | 发放条件 | 一次/多次 |
|---|---|---|---|
| 首次通关（普通关） | +25 | `completed_steps` 无 `"{level_id}:pass"` | 一次 |
| 重复通关 | +0 | 已有记录 | —（combo 仍更新，练习价值保留） |
| 完美通关（首次提交即通过） | +10 | 该关 `fail_count == 0` | 一次（随首通） |
| 连击加成 | +5 | 首通且通过后 `combo ≥ 3` | 一次 |
| Boss 首通（≤4 次尝试） | +50 | 无 `pass` 记录 | 一次 |
| Boss 首通（>4 次尝试） | +30 | 该关 `fail_count > 4` | 一次（≥75% 语义惩罚档） |
| 查看 hint / 复习回血 | +0 | — | 防挫败工具不奖不罚 |

单关上限：普通 25+10+5=40；Boss 50+10+5=65。全游戏 XP 区间 475-700（11 普通 + 4 Boss；现有 15 关结构）。现有 `XP_PER_PASS=20` 替换为 `XP_PASS=25 / XP_PERFECT=10 / XP_COMBO=5 / XP_BOSS=50 / XP_BOSS_FALLBACK=30`；实现为 `award_xp()` 四步累加 + `completed_steps` 写入。

### 7.3 Rank 表（10 级，按完成关卡数判定）

| Rank | 称号 | 判定条件 | 解锁内容 |
|---|---|---|---|
| R1 | 见习学徒 | 开局 | 游戏开始 |
| R2 | 输出新手 | 完成 00-l0-hello | XP 进度条；错误码图鉴 L0 条目 |
| R3 | 语法学徒 | L0 全部（00-03） | 图鉴 L0 全码 |
| R4 | 所有权新兵 | 04-l1-move | 图鉴所有权码 |
| R5 | 借用骑士 | L1 全部（含第一段 Boss） | 图鉴借用码；连击徽章 |
| R6 | 集合行者 | 08-l2-vec | 图鉴集合码 |
| R7 | 错误猎人 | L2 全部（含第二段 Boss） | **图鉴全量开放** |
| R8 | 特质学徒 | 11-l3-lifetime | 图鉴特质/生命周期码 |
| R9 | 生命周期贤者 | L3 全部（含第三段 Boss） | 统计页开放 |
| R10 | 铁锈冠军 | 全部 15 关（含第四段 Boss） | 自由模式 + 一次性通关庆典 |

rank 不解锁关卡（关卡保持线性解锁链），只解锁元内容；新增 `game-core/src/rank.rs` 纯模块，不新增存档字段。

### 7.4 Hearts / Streak / 成就

- **Hearts**：初始 3、上限 5；提交失败 −1（floor 0）、通关 +1（cap 5）、复习回血 +1（心 <5 且每关每局一次，记 `"{level_id}:lore"`）、**Boss 失败 0**；0 心禁提交（按钮置灰 +「❤️ 已空：复习关卡说明可回 1 心」），不禁止编辑不扣 XP。复习 = 新增 `engine.review_lore(level_id)`，幂等。
- **Streak**：活跃行为 = 通关/查看 hint/复习回血；昨日活跃 → +1，同日幂等，更早/首日 → 重置 1；连续游玩日纯展示无奖励。**日期算法禁 rust-quest 的 `y*372+m*31+d`（跨月 bug 实测）**，用 chrono `num_days_from_ce()`（推荐）或 std 纯函数 Hinnant 公式；单测必含 2-28→3-01、12-31→1-01。
- **成就（10 个，静态表 + HashSet 存档，无 XP 奖励）**：first_steps（首次通关）/ no_hint_perfect（首次提交即通过且未看 hint）/ combo_5 / combo_10 / owner_guard（完美通关 04-l1-move）/ boss_slayer（击败任意 Boss）/ boss_all（击败全部 4 个 Boss）/ error_collector（累计见过 ≥10 种不同错误码，需 `seen_error_codes` 字段）/ never_give_up（单关失败 ≥10 次后仍通过）/ champion（全部 15 关通关）。触发点挂 `engine.submit` Pass/Fail 分支，`check_achievements` 纯函数。

### 7.5 Boss 关机制（四段）

| 段 | 位置（现有 15 关） | 主题 | 混合概念示例 |
|---|---|---|---|
| 第一段 | L1 末 07-l1-clone | 所有权与克隆 | 移动后再用（E0382）+ 借用冲突（E0505/E0507） |
| 第二段 | L2 末 10-l2-result | 错误处理 | unwrap on None 路径 + `?` 在 main 类型不符（E0277） |
| 第三段 | L3 末 12-l3-trait | Trait 与泛型 | trait bound 缺失（E0277）+ 泛型推断（E0282/E0308） |
| 第四段 | L4 末 14-l4-lifetime-trap | 生命周期 | 悬垂引用（E0597）+ 缺标注（E0106）+ 借用逃逸（E0505） |

规则：`is_boss = true` 显式标注（不靠 tier 末关推断）；尝试配额（首通 ≤4 次 +50 / >4 次 +30，**不设硬上限**）；失败不扣心；提示默认禁用（反馈的错误码解释卡仍显示——那是教学核心不豁免；fail_count ≥5 解锁提示兜底）；失败仍 combo 清零 + total_errors +1；完美/连击加成照常。

### 7.6 防挫败设计

- 惩罚落在 combo（清零，可恢复）与 hearts（−1，软惩罚），**不扣已得 XP、不重置代码**；失败文案「❌ 未通过」改「🔧 还差一点」。
- hint 联动表（§3.4 行为表）：0/1 次失败 → hint1 可看；2 次 → hint1-2 解锁且自动展开 hint1；3 次 → hint1-3 解锁且自动推进 hint2；≥4 次 → hint3 + 「查看参考答案」按钮（二次确认「先自己试试？」）。hint 查看永远零成本（不扣心/XP、不影响完美判定）。
- 0 心转引导：禁提交 + 「📖 复习回血」按钮（重看描述 +1 心）——惩罚自动转为学习引导。

### 7.7 反馈面板信息架构（ErrorCard）

```rust
pub struct FeedbackData {
    pub passed: bool, pub level_id: String, pub xp_gained: u32,
    pub combo: u32, pub hearts: u32,                 // 新增
    pub errors: Vec<ErrorCard>,                      // 编译错误分支（结构化卡片）
    pub expectation: Option<OutputDiff>,             // 输出不符分支
    pub panic: Option<String>,                       // panic 分支（分类标题 + 净化消息）
    pub unlocked_next: Option<String>,               // 通关时下一关标题（末关 None → 全通关庆典）
}
pub struct ErrorCard {
    pub code: String, pub line: Option<u32>, pub summary: String,  // rustc 原文（折叠）
    pub zh: String, pub fix: String, pub example: Option<String>,
    pub link: Option<String>, pub hint_index: Option<u32>,         // 「💡 与提示 2/3 相关」
}
```

展示优先级（自上而下）：① 头行「E0308 · 第 3 行 · 类型不匹配」（错误码徽章 + 行号 + 中文标题，每错误一卡）；② 中文解释默认展开；③「怎么改」默认折叠（fix + example + 相关 hint 序号）；④ rustc 原文默认折叠（CollapsingHeader，monospace 灰字）；⑤ 多错误默认只展开第一条，其余折叠为「还有 N 个错误」；⑥ panic 卡片「❗ 程序运行崩溃（分类）」+ 净化消息折叠区；⑦ 输出不符两栏 diff 对照（期望 vs 实际，逐行着色）。

交互：卡片「第 N 行」可点击 → 光标跳到编辑器对应行（focus_line 状态 + CCursorRange）；返回编辑时反馈面板保持（底部固定区域）；egui 组件用 `Frame` 徽章 + `CollapsingHeader` + `ScrollArea::vertical().max_height(420)` + `ui.hyperlink_to`。

通关正反馈最小集（每项 ≤1 秒）：XP 数字跳动 + ProgressBar 增长（+25 XP）→ 下一关标题 + 🔓 徽章（末关 →「🏆 全部通关！」一次性庆典，`victory_celebrated` 防重复）→「已自动保存」小字 + 首次显示存档路径 → hearts +1 提示 → Enter 进下一关 / Esc 回地图。

### 7.8 中文字体方案与 IME 风险

- **现状**：内嵌 JetBrains Maple Mono（JetBrains Mono + Maple Mono 合并版，SIL OFL 1.1，33,331 字形，CJK 统一表意区 + 全角标点 + 制表符全覆盖；emoji 全部缺失回退单色）。epaint 0.31 原生支持 CJK 断行。
- **必修**：① 补 `crates/game-ui/assets/OFL.txt`（OFL 全文 + 双版权声明，OFL 义务）；② layouter 修复一行：`job.wrap.max_width = wrap_width;`（现为 INFINITY 超宽行不换行）；③ 字号统一 `TextStyle::Monospace.resolve(ui.style())`（现 14.0 vs 12.0 不一致）。
- **【高】IME 不可用**：egui-miniquad 0.16 源码明示 `// no IME`，miniquad 无 IME 事件通道 → 编辑器内 fcitx/ibus 中文输入不可用（可粘贴）。设计约束：**关卡验收标准加一条——所有关卡可编辑代码不要求玩家输入 CJK 字符**（需要中文输出的关，starter_code 直接给出字符串字面量）；编辑器首次进入显示一行弱提示「中文请复制粘贴」；P3 调研换窗口后端或「插入片段」按钮。
- **P3 优化**：双字体（Proportional = Noto Sans SC OFL + maple 兜底，Monospace 保持 maple）；emoji 减少依赖或接受单色；tokenize 缓存；字体子集化（需用字覆盖冒烟测试）。

来源：docs-review/L3-C1-gamification.md §1-§4、L3-C2-save-ui.md §2-§4、L2-gamification.md C1/C4/C5。

---

## 8. 存档 schema v2

### 8.1 问题（实测）

现有 `SaveData` 5 字段**无 version**：`load()` 直读 `toml::from_str`，字段改名/类型变更 → `CorruptSave` → 整档归零静默丢失（无备份无提示）。save() 已有 tmp+rename 原子写（优于 rust-quest），但无 `.bak`。**version 字段是发布前必修项**。

### 8.2 目标 schema v1 struct 草案（L3-C2 定稿）

```rust
pub const CURRENT_SAVE_VERSION: u32 = 1;
fn default_version() -> u32 { 0 }   // 旧存档（无 version）读出 0，进入迁移链

pub struct SaveData {
    #[serde(default = "default_version")] pub version: u32,
    #[serde(default)] pub player_name: String,          // 新增（P2 名称输入，v1 恒空串）
    #[serde(default)] pub xp: u32,                      // 已有
    #[serde(default)] pub combo: u32,                   // 已有
    #[serde(default)] pub max_combo: u32,               // 已有
    #[serde(default)] pub total_errors: u32,            // 已有（累计失败次数）
    #[serde(default)] pub hearts: u32,                  // 新增：3-5（默认值由迁移链显式补 3）
    #[serde(default)] pub streak_days: u32,             // 新增：连续活跃日
    #[serde(default)] pub last_played_date: Option<String>,  // 新增：上次活跃日 "yyyy-mm-dd"
    #[serde(default)] pub completed_steps: HashSet<String>,  // 新增：XP-once 去重 "{level_id}:pass"/"{level_id}:lore"
    #[serde(default)] pub achievements: HashSet<String>,     // 新增：成就 id
    #[serde(default)] pub practice_unlock_all: bool,         // 新增：自由模式（P3）
    #[serde(default)] pub victory_celebrated: bool,          // 新增：全通关庆典一次性标志
    #[serde(default)] pub level_states: HashMap<String, LevelProgress>,  // 已有
    #[serde(default)] pub boss_states: HashMap<String, BossProgress>,    // 新增：Boss 状态（按 level_id）
}

pub struct LevelProgress {
    #[serde(default)] pub state: LevelState,            // 已有（locked/unlocked/passed）
    #[serde(default)] pub attempts: u32,                // 已有（提交次数 = hint 推进计数）
    #[serde(default)] pub completed_at: Option<String>, // 已有
    #[serde(default)] pub best_time_ms: Option<u64>,    // 新增：通关最快用时（engine.submit 通过分支记录）
    #[serde(default)] pub hints_used: Vec<u32>,         // 新增：看过的 hint 序号（与 attempts 联动）
}

pub struct BossProgress { #[serde(default)] pub defeated: bool, #[serde(default)] pub best_attempts: u32 }
```

TOML 形状注意：`HashMap<String, LevelProgress>` 序列化为 `[level_states."<id>"]` 表（迁移 fixture 必须用真实形状）。

### 8.3 迁移链（load 流程）

```
load(path):
    不存在 → SaveData::default()          # 首次启动
    toml::from_str(raw)                   # 新字段全 serde(default) → v0 文件不失败
    migrate(data) → 迁移后立即回写落盘     # 纯函数逐级升
migrate(data):
    version > CURRENT → Err(CorruptSave("存档版本 {v} 高于游戏版本，请升级游戏"))  # fail-fast，不静默丢档
    while version < CURRENT: 0 => migrate_v0_to_v1
migrate_v0_to_v1:
    version = 1; hearts = 3               # serde default 给 0，需显式补起始心
    streak_days = 0; last_played_date = None   # 无法回推历史，从零开始（不惩罚）
    completed_steps = level_states 中 state==Passed 的 id → "{id}:pass"   # 老玩家记录不丢 XP 语义
save() 增强：写入前把旧档复制为 save.toml.bak（人工恢复通道）
```

### 8.4 字段口径差异（L3 内部冲突，L4 落地时二选一）

1. **last_played_date**（L3-C2 草案 String）vs **last_day**（L3-C1 §1.4 整数 i64，免解析无歧义）——推荐整数方案（C1），存档字段名以 C2 草案为准时二选一即可，均为 `#[serde(default)]`。
2. **boss_states**（C2 草案保留）vs C1 §3.3「并行表不采用」（可由 `level_states[id].fail_count ≤ 4` 推断 defeated）——v1 可保留 C2 草案的显式表，P3 再裁减。
3. **seen_error_codes**（C1 成就 8 需要）未列入 C2 草案 —— L4 落地按 C1 补充 `#[serde(default)]` 字段。
4. **hints_used**：C2 用 `Vec<u32>`（序号），C1 用 `u32`（计数）——Vec 兼容计数语义（`is_empty()` = 未看过），成就 2/5 判定用 `is_empty()`。
5. **fail_count**（C1 新增）vs 现有 `attempts`（C2 直接用作失败计数）——若 attempts 语义已是「失败次数」则不必新增；否则按 C1 补 `fail_count`，两字段均 `#[serde(default)]`。

### 8.5 迁移测试矩阵

① `tests/fixtures/save_v0.toml`（现无 version 真实形状）→ version==1、hearts==3、`completed_steps == {"l0-hello:pass"}`、xp/combo/attempts 保留、未通关关保持 unlocked；② `version = 99` 未来存档 → Err(CorruptSave) 且原文件未被修改；③ v1 roundtrip 全字段相等；④ completed_steps 仅 state==Passed 进集合；⑤ 极简缺字段 TOML（只有 `xp = 5`）→ 不报错迁移成完整 v1。

来源：docs-review/L3-C2-save-ui.md §1、L3-C1-gamification.md §1.4/§2/§3.3、L2-gamification.md C2。

---

## 9. 沙盒安全

### 9.1 目标架构（保留 v2 §10）

- **计划②（最终目标）**：bwrap（bubblewrap）真隔离——`--unshare-all`（用户命名空间）、系统目录只读挂载（--ro-bind）、tmpfs 工作区、最小化 /proc /dev、禁网络、禁写主目录；叠加 `timeout`（编译 10s / 运行 2s）+ 内存限制（ulimit -v）。
- **开发期兜底（现状）**：timeout + 临时目录 + syn 静态拦截。
- **风险验证**：实现时先验证 `bwrap --unshare-all true` 能否运行（用户命名空间需内核支持）。
- **分发门槛**：不做 bwrap 隔离前，游戏不得公开分发（开发期本地可跑）。

### 9.2 syn 静态拦截清单（L1 补充，v2 未列全）

黑名单式拦截（默认放行 std 基础类型与集合）：

| 类别 | 拦截符号/模式 |
|---|---|
| 文件 IO | `std::fs` 全部 |
| 进程 | `std::process` 全部 |
| 环境 | `std::env` 全部 |
| 网络 | `std::net` 全部 |
| 并发 | `std::thread::spawn` |
| 内存不安全 | `unsafe` 块、FFI / `extern` |
| 恐慌策略 | `panic=abort`（禁止覆盖沙盒默认 unwind 判定） |

拦截规则由 `game-core/editor` 的 syn 扫描实现（纯逻辑可测）；白名单 vs 黑名单策略：黑名单拦截 IO/进程/网络，其余 std 基础类型与集合放行。

### 9.3 warning 与 error 区分（L1 补充，实测结论）

- warning 不判通关失败：裸 rustc 下 warning 退出码 0（实测），当前只匹配 `error[E` 天然正确；warning 行不得生成错误卡片。
- **clippy 类关卡与当前管线不兼容**（22_clippy 在裸 rustc 下零警告零错误）：方案 A（推荐）把 lint 违规改写成等价编译错误（如 `let pi: i32 = 3.14;` → E0308）；方案 B `lint_mode` 新关卡类型第二版再议。
- `-D warnings` 把 warning 提升为**无 E 码** error → 走 §5.3 EUNKNOWN 兜底（若未来启用 lint_mode 自然覆盖）。

来源：docs-review/L1-plan.md 约束 6/13/14、L3-B2-parser.md §1.3、L2-error.md B3.4。

---

## 10. 素材与许可清单

### 10.1 全仓库许可表（v3 修正版）

| 素材 | 实际许可 | 可否改写引用 | 要求与风险 |
|---|---|---|---|
| rustlings | **MIT only**（单 LICENSE，Copyright Carol Nichols；v2 写「MIT / Apache-2.0」不准确） | ✅ 可自由改写 | 保留版权声明；source 字段标注 |
| rust-quiz | CC BY-SA 4.0 | ⚠️ 题代码可改写；**解释文本衍生须 CC BY-SA** | 只改编 .rs + 全部自写中文解释（规避 SA 传染）；source 标注「解释自写」 |
| 100-exercises-to-learn-rust | **CC BY-NC 4.0（非商业）** | ⚠️ 非商业可引用 | 只借鉴结构与教学法、不复制习题文本；有赞助/售卖即违规 |
| rust-course | **No License**（README 明示不能修改后再包装分发） | ❌ 不可复制进游戏 | **course.rs 已失效（404）**，连链接引用都不可行；仅作本地编写参考；如需使用须联系作者 |
| error-docs | CC BY 4.0 | ✅ 可引用 | 署名；无 SA 传染，作错误码解释底稿 |
| book-cn / rust-by-example-cn / nomicon-cn / rust-wiki / api-guidelines | MIT + Apache-2.0 双协议 | ✅ 可自由改写 | 保留版权声明（README 致谢） |
| reference-cn | **英文部分 MIT/Apache-2.0；src 中文翻译部分 Mulan PSL v2（木兰宽松许可证）** | ✅ 可改写 | 三份 LICENSE 并存；改写 src 中文内容时按 §6.1 基准转「生命周期」（其正文用「生存期」） |
| rust-quest | MIT | ✅ 机制代码可借鉴 | 借鉴 game/ 模块时保留版权声明 |
| happy_ruster | 无许可声明（默认保留版权） | ❌ 不可复制代码/文案 | 仅借鉴分层反馈思路；README 为 UTF-16 编码勿直接处理 |
| 字体 | JetBrains Maple Mono（JetBrains Mono + Maple Mono 合并） | ✅ | **SIL OFL 1.1**；补 `crates/game-ui/assets/OFL.txt`（OFL 全文 + 双版权声明）；README 致谢升级 |
| 图片 | rust-course img/（No License 仓库内）、rust-quest media/ | ❌ | **不采用任何第三方图片素材**，全部 macroquad 自绘或 OFL 字体渲染 |

### 10.2 在线链接修正（v2 §13 表述废弃）

- v2 的「The Book / course.rs：提示文本来源」→ **废弃**：course.rs 已失效（实测 404），且 No License；中文教材在线入口 = **rustwiki.org**（book-cn / rust-by-example-cn / reference-cn 三本同站，实测 92 个 URL 全部 200）。
- doc.rust-lang.org/zh-CN/book 实测 404，禁用；官方错误码页 doc.rust-lang.org/error_codes/E0xxx.html 稳定（200），作为 link 必填。
- 链接白名单/禁用清单见 §6.3；CI 检查 `link_zh` 域名白名单 + 映射表清单核对。

### 10.3 README「素材与许可」章节要求

列出全部来源与许可（含 rust-quiz CC BY-SA 提醒、100-exercises CC BY-NC 声明、rust-course No License 声明与致谢联系渠道、字体 OFL 全文位置）；注明 course.rs 失效仅作本地参考；关卡 TOML 必须有 `source` 字段（CI 可查）。

来源：docs-review/L1-plan.md §5、L2-chinese.md D4、L3-D1-terms.md §3-§4、L3-C2-save-ui.md §3.2。

---

## 11. 测试策略

### 11.1 错误码 fixture 矩阵（L3-B2 定稿）

目录结构（`crates/game-core/tests/fixtures/`）：`errors/`（现有 6 码 + P0 5 码，每场景 `broken.rs` + `fixed.rs` + `expected.toml`）、`nocode/`（NCODE_format_args_count = 01-l0-print 场景、NCODE_let_chains_edition）、`panic/`（PANIC_index_out_of_bounds = 08-l2-vec 场景、PANIC_unwrap_option_none）、`dead_codes/`（E0412/E0504 负面断言）。`expected.toml` 元数据：`code`（断言 errors.first().code）/ `line`（实测首条 --> 行号）/ `kind`（compile|nocode|panic）/ `message_contains`（稳定子串，不断言全文）/ `classification`（panic 分类 id）。

测试分层与成本：

| 层 | 内容 | 成本 |
|---|---|---|
| Tier 1 解析器单测 | 静态 stderr 快照：无码错误→EUNKNOWN、warning 交错（不产生 issue、--> 不误附）、双 `-->` 取首条、空 stderr、`error: aborting due to` 忽略 | ≈ 0 |
| Tier 2 真实编译矩阵 | 遍历 errors/ + nocode/（DevSandbox 编译，**只比 E 码与行号不比文本**）；panic/ 运行断言分类命中；dead_codes/ 断言 errors 为空 | ~8-10s 串行（15 场景 × ~0.5s），4 路并行 ≈ 3s |
| Tier 3 关卡回归 | game-data tests：15 关 starter 编译断言「预期错误码集合」（现 6 码 + 01-l0-print 的 EUNKNOWN 断言，锁死空反馈 bug） | ≈ 0 |
| 版本矩阵 | weekly job：rustup 装 1.8x/1.9x 跑 Tier 2，验证「只比 E 码」策略不碎 | 不作为 PR 门禁 |

### 11.2 改编关卡回归与数据校验

- `game-data/tests/levels.rs`：关卡 TOML 解析（合法/非法）、quiz 型 options 2-6 项且 answer_index 界内、hint_unlock 与 hints 等长、expect_output 无 `\r`、**source 字段存在性**。
- 改编回归：新关落地走 §2 流水线 S5 实测 + Q1-Q10 核对；rust-quiz 改编关的 fixture 复用 `docs-review/L3-D2-cards/` 8 卡代码（E0382/E0502/E0499/E0596/E0106/E0308/E0277/E0507）。

### 11.3 解析器边界用例（Tier 1 必含）

多错误码（06-l1-mut-borrow：E0382 → E0596 顺序）、同码双 `-->`（E0382 取首条）、warning 与错误交错、`-D warnings` 无码 error、E0621 定位返回表达式行（非签名行）、E0601 定位文件末尾行、E0282 定位 let 声明行、panic stderr 以空行开头。

### 11.4 存档迁移与游戏化纯逻辑单测

- 迁移矩阵（§8.5）：save_v0.toml → v1 断言；未来版本 fail-fast 不改原文件；roundtrip；`.bak` 存在。
- 游戏化纯函数（照搬 rust-quest tests/game_state.rs 风格）：XP-once 去重、完美/连击加成、Boss 分档（≤4 → +50 / >4 → +30，失败不扣心）、hearts 增减与触底、review_lore 幂等、streak 跨月/跨年（2-28→3-01、12-31→1-01）、rank 里程碑边界（4/8/11/15 关）、10 个成就触发条件、hint 失败联动推进、Boss hint 拦截。

### 11.5 UI 冒烟与字体

窗口初始化 → 渲染一帧 → 退出（保留 v2）；字体字形覆盖冒烟（可选，fontTools 脚本验证关卡文案用字在内嵌字体中）；layouter 修复后超宽中文行换行验证（截图冒烟）。

来源：docs-review/L3-B2-parser.md §2-§3、L2-error.md B5、L3-C1-gamification.md §6、L3-C2-save-ui.md §5、L2-gamification.md C2.5。

---

## 12. 迭代路线

### 12.1 现状盘点（v2 已实现，保留）

macroquad+egui 窗口与界面（菜单/关卡/反馈）、TextEdit + 行号 + rustc_lexer 着色、TOML 关卡加载（15 关）、compile→run→compare 闭环、错误码解析映射（errors.toml 20 码）、存档（无 version，tmp+rename 原子写）、开发期安全兜底（timeout + 临时目录 + syn）、三级 hints 部分关卡（hint_step 手动逐级）、combo/XP_PER_PASS=20、地图三态（✅🔓🔒）、通关反馈（✅通关 + XP + 存档确认 + Enter 下一关）。

### 12.2 P1（MVP 必修，全部新增或修正）

1. **EUNKNOWN 无码错误捕获 + [fallback] 兜底**（现网 bug：01-l0-print 空反馈）；`validate()` 编译失败且 errors 为空 → 强制兜底文案。
2. **panic 净化 + 8 类关键词分类**（剥路径/线程 id/note 行，08/10 关反馈自动升级）。
3. **errors.toml schema v2**：severity/fix/example/concept/link_zh + [fallback] + [deprecated]；删除 E0412/E0504 死码；新增 P0 5 码（E0282/E0384/E0594/E0621/E0601）；default_fallback 扩到全部 P0 + EUNKNOWN。
4. **反馈面板结构化**：FeedbackData → ErrorCard 数组；默认展开第一条、其余折叠；「🔧 还差一点」语气。
5. **XP 定价替换**：25/10/5/50/30 + completed_steps 去重（替换 XP_PER_PASS=20）。
6. **rank 模块**（game-core/src/rank.rs，按完成关卡数，10 级中文称号）。
7. **存档 version + v0→v1 迁移 + .bak 备份**（发布前必修，否则 schema 演化整档丢失）。
8. **layouter 修复**：`job.wrap.max_width = wrap_width`（一行）；字号统一。
9. **OFL.txt 补发**（`crates/game-ui/assets/OFL.txt` + 双版权声明）。
10. **现有关卡修订**：10-l2-result / 07-l1-clone 去剧透（description/hint 按 §6.2）。
11. **解析器单测回归锁**：多错误码首条、双 `-->`、warning 交错、EUNKNOWN 捕获、01-l0-print 断言反馈非空。

### 12.3 P2（机制与内容扩展）

- hearts 3-5 + 0 心禁提交 + 复习回血；streak（chrono Unix day）；成就表（10 个，触发挂 engine.submit）；hint 与失败次数联动（§7.6 表）；错误卡片「第 N 行」点击跳转编辑器。
- 新增 P1 码（E0506 等）；为 11/15 关补全三级 hints（概念→链接→代码，rustwiki 白名单）。
- 内容扩展第一批：rustlings 06_move_semantics（ms2/3/4/5）> 16_lifetimes（l1/l2/l3）> 13_error_handling（e1-e6）> 100-exercises 02/03/04 章（无 helpers）> rust-quiz D1 输出比对题（21 题实测零成本）。
- fixture 矩阵 Tier 2 全量（15+ 场景）；clippy 类关卡决策（方案 A 改写为编译错误关）。

### 12.4 P3（玩法与体验增强）

- Boss 关机制（is_boss、四段、尝试配额分档、失败不扣心、提示禁用）；通关庆典动画（victory_celebrated）；自由模式（practice_unlock_all，R10 解锁）。
- 双字体方案（Noto Sans SC + maple）；IME 调研（换窗口后端或插入片段按钮）；tokenize 缓存；字体子集化（3-5MB，需用字覆盖测试）。
- quiz 关卡类型（kind=quiz）若采用（L2 明确 v1 不引入选择题，P3 复议）；expect_panic 字段启用（「制造指定 panic」关）；版本矩阵 weekly job。

### 12.5 P4（安全与发布）

- bwrap 真隔离沙盒（--unshare-all + 只读系统 + tmpfs + 禁网络 + ulimit；先验证 `bwrap --unshare-all true`）；自定义关卡导入（外部 TOML 关卡目录）。
- 53 关全量 + 7 扩展槽（达 60 关上限）；发布合规审查：§10 许可表全量核对、README「素材与许可」章节、source 字段 CI 检查、链接白名单 CI 检查。

### 12.6 明确不采纳（L2/L3 结论）

选择题关卡类型（v1）；AI 解释（happy_ruster 的 Gemini 方案，永远不引入——依赖网络/API key、无确定性、与沙盒禁网哲学冲突）；TUI 盒式排版（retro.rs）；rodio 音乐（P3+ 若做用 macroquad::audio）；rust-quest 的 `y*372+m*31+d` 日期算法（跨月 bug）；boss_states 并行表（P3 裁减，见 §8.4）。

来源：docs-review/L2-error.md 给 L4 建议、L2-gamification.md 给 L4 建议、L3-C1-gamification.md §2、L3-C2-save-ui.md §4、L3-A1-levels.md §2/§8、L2-exercises.md 给 L4 建议。

---

## 附：关键事实修正汇总（相对 v2）

| 项 | v2 说法 | v3 修正（实测依据） |
|---|---|---|
| rustlings 许可 | MIT / Apache-2.0 | **MIT only**（单 LICENSE） |
| course.rs 链接 | 提示文本来源 | **已失效（404）**；中文教材在线 = rustwiki.org；No License 不可复制 |
| 存档 | 无版本说明 | 无 version 字段 → 必须加 version + v0→v1 迁移 + .bak |
| 错误码 | 20 码（含 E0412/E0504） | E0412/E0504 死码删除入 [deprecated]；新增 P0 5 码 + P1/E 组；EUNKNOWN 兜底 |
| 关卡命名 | `ownership_01` | 固定 `NN-lX-主题`，id 无前缀 |
| XP | XP_PER_PASS=20 可重复刷 | 25/10/5/50/30 分档 + completed_steps 一次制 |
| egui 中文 | 未提风险 | IME 不可用（egui-miniquad 无 IME 通道）→ 关卡设计规避玩家手打中文 |
| 字体 | README 一句话 | OFL.txt 必修 + 双版权声明 |
| 链接地址 | doc.rust-lang.org/zh-CN/book | **404**；禁用 course.rs 与 doc.rust-lang.org/zh-CN |
| reference-cn 许可 | MIT/Apache 双协议 | src 中文翻译部分为 **Mulan PSL v2**（三份 LICENSE 并存） |

来源：全部 docs-review/ 文件（L1-plan.md §5、L2-exercises.md、L2-error.md、L2-chinese.md D3/D4、L2-gamification.md C2/C4、L3-A1/A2/B1/B2/C1/C2/D1/D2）。
