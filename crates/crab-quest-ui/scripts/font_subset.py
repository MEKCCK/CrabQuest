#!/usr/bin/env python3
"""P3-20 双字体：Noto Sans SC 子集化 + 用字覆盖验证（开发期工具，不参与游戏运行）。

用途
----
1. 从游戏实际文案（levels/*.toml + errors.toml + 两 crate 的 Rust 源码字符串）
   提取全部用字（CJK + 全角标点 + ASCII），用 fontTools/pyftsubset 把
   Noto Sans SC 子集化到该字符集，产出 crates/game-ui/assets/NotoSansSC-Regular.ttf
   （验收要求 ≤5MB）。
2. 覆盖验证：断言子集 cmap 覆盖全部 CJK 用字；并以 maple 全量 CJK cmap 兜底
   （Proportional 家族 = [noto 子集, maple, egui 默认]），保证「无缺字」。
   emoji（✅💡🔓 等）不在 Noto/maple cmap 内，属预期回退到 egui 内置
   NotoEmoji/emoji-icon-font（单色），仅统计报告、不参与缺字断言。

用法
----
    python3 crates/game-ui/scripts/font_subset.py            # 完整流程：取源字体→实例化→子集化→验证
    python3 crates/game-ui/scripts/font_subset.py --check    # 只验证：断言 assets 内已提交子集覆盖全部用字
    python3 crates/game-ui/scripts/font_subset.py --download # 源字体缺失时从官方源下载（需网络）

依赖：python3 + fonttools（pip install fonttools；含 pyftsubset / varLib.instancer）。

源字体：Noto Sans SC（SIL OFL 1.1，Google Fonts 官方仓库，变量字体 wght 100-900）：
    https://github.com/google/fonts/raw/main/ofl/notosanssc/NotoSansSC%5Bwght%5D.ttf
默认实例为 wght=100（Thin），本脚本固定 wght=400（Regular）后再子集化。
"""
from __future__ import annotations

import argparse
import glob
import os
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
CRATE_DIR = SCRIPT_DIR.parent                     # crates/game-ui
REPO_ROOT = CRATE_DIR.parent.parent               # 仓库根
ASSETS_DIR = CRATE_DIR / "assets"
SUBSET_FONT = ASSETS_DIR / "NotoSansSC-Regular.ttf"   # 提交产物（≤5MB）
SOURCE_FONT = ASSETS_DIR / "NotoSansSC-variable.ttf"  # 全量变量字体（不提交，脚本按需取）
SOURCE_URL = (
    "https://github.com/google/fonts/raw/main/ofl/notosanssc/"
    "NotoSansSC%5Bwght%5D.ttf"
)
MAPLE_FONT = ASSETS_DIR / "JetBrainsMapleMono-Regular.ttf"
FONT_WEIGHT = 400  # Regular

# 非 ASCII 字符中，视为「正文用字」的区段（其余按 emoji 处理，仅统计）
CJK_RANGES = (
    (0x3400, 0x4DBF),   # CJK 扩展 A
    (0x4E00, 0x9FFF),   # CJK 统一表意
    (0x3000, 0x303F),   # CJK 标点（「」、、。等）
    (0xFF00, 0xFFEF),   # 全角/半角形式
    (0x2010, 0x2027),   # –—… 等印刷标点
    (0x2028, 0x202E),
    (0x2030, 0x205E),   # 通用标点（·、→ 等）
    (0x00B0, 0x00BF),   # °·× 等拉丁补充标点
)


def is_cjk_char(cp: int) -> bool:
    return any(lo <= cp <= hi for lo, hi in CJK_RANGES)


def collect_corpus() -> set[str]:
    """提取游戏全部渲染用字：关卡 TOML + 错误码 TOML + Rust 源码（含注释，保守超集）。"""
    texts: list[str] = []
    for pat in ("assets/levels/*.toml", "assets/errors.toml"):
        texts.extend(
            Path(p).read_text(encoding="utf-8")
            for p in glob.glob(str(REPO_ROOT / pat))
            if Path(p).is_file()
        )
    for pat in ("crates/game-ui/src/**/*.rs", "crates/game-core/src/**/*.rs"):
        texts.extend(
            Path(p).read_text(encoding="utf-8")
            for p in glob.glob(str(REPO_ROOT / pat), recursive=True)
        )
    chars: set[str] = set()
    for text in texts:
        chars.update(text)
    # 始终保留 ASCII 可打印区（标题/错误码/XP 等拉丁字符）
    chars.update(chr(c) for c in range(0x20, 0x7F))
    return chars


def instantiate_regular(src: Path, dst: Path) -> None:
    """变量字体固定为 FONT_WEIGHT 实例（否则默认 Thin 太细，UI 不可读）。"""
    subprocess.run(
        [
            sys.executable, "-m", "fontTools.varLib.instancer",
            str(src), f"wght={FONT_WEIGHT}", "-o", str(dst),
        ],
        check=True,
    )


def subset_font(src: Path, dst: Path, text_file: Path) -> None:
    """pyftsubset 到用字集；保留许可/名称表与布局特性（OFL 义务）。"""
    subprocess.run(
        [
            sys.executable, "-m", "fontTools.subset",
            str(src),
            f"--text-file={text_file}",
            f"--output-file={dst}",
            "--layout-features=*",
            "--name-IDs=*",
            "--name-legacy",
            "--name-languages=*",
            "--glyph-names",
            "--symbol-cmap",
            "--legacy-cmap",
        ],
        check=True,
    )


def load_cmap(font_path: Path) -> set[int]:
    from fontTools.ttLib import TTFont
    with TTFont(font_path, fontNumber=0) as font:
        cmap: set[int] = set()
        for table in font["cmap"].tables:
            cmap.update(table.cmap.keys())
        return cmap


def fix_name_table(font_path: Path) -> None:
    """变量字体默认实例名为「Thin」，实例化后 name 表仍保留旧名；修正为 Regular。"""
    from fontTools.ttLib import TTFont

    replace = {
        1: "Noto Sans SC Regular",
        3: "2.004;ADBO;NotoSansSC-Regular;ADOBE",
        4: "Noto Sans SC Regular",
        6: "NotoSansSC-Regular",
        17: "Regular",
    }
    with TTFont(font_path) as font:
        for nid, value in replace.items():
            for rec in font["name"].names:
                if rec.nameID == nid:
                    if rec.platformID == 3:
                        rec.string = value.encode("utf-16-be")
                    else:
                        rec.string = value.encode("mac_roman", errors="replace")
        font.save(font_path)


def verify(subset: Path, corpus: set[str], maple: Path) -> tuple[int, int, list[str]]:
    """验证：CJK 用字 ∈ noto 子集（主），且 ∈ noto ∪ maple（兜底）。返回 (缺字, emoji 数, 缺字清单)。"""
    subset_cmap = load_cmap(subset)
    maple_cmap = load_cmap(maple)
    cjk_chars = {c for c in corpus if is_cjk_char(ord(c))}
    emoji_chars = {c for c in corpus if ord(c) > 0x7F and not is_cjk_char(ord(c))}
    missing_noto = sorted(c for c in cjk_chars if ord(c) not in subset_cmap)
    missing_all = sorted(c for c in cjk_chars if ord(c) not in subset_cmap and ord(c) not in maple_cmap)
    if missing_noto:
        print(f"  [warn] 以下用字不在 Noto 子集（将由 maple 兜底）：{''.join(missing_noto)}")
    if missing_all:
        print(f"  [FAIL] 以下用字不在 noto∪maple，会渲染成方框：{''.join(missing_all)}")
        return len(missing_all), len(emoji_chars), missing_all
    print(
        f"  [OK] CJK 用字 {len(cjk_chars)} 个全部覆盖"
        f"（noto 子集 {len(subset_cmap)} 字；noto 缺失 {len(missing_noto)} 个走 maple 兜底）"
    )
    print(f"  [info] emoji/其他非 CJK 字符 {len(emoji_chars)} 个：{''.join(sorted(emoji_chars))}")
    print(f"         （回退 egui 内置 NotoEmoji/emoji-icon-font 单色，P3-20 决策：接受单色回退）")
    return 0, len(emoji_chars), missing_all


def main() -> int:
    ap = argparse.ArgumentParser(description="Noto Sans SC 子集化 + 用字覆盖验证（P3-20）")
    ap.add_argument("--check", action="store_true", help="只验证已提交子集，不重新子集化")
    ap.add_argument("--download", action="store_true", help="源字体缺失时自动下载（需网络）")
    args = ap.parse_args()

    corpus = collect_corpus()
    print(f"用字语料：{len(corpus)} 个字符（CJK 正文 + ASCII + 全角标点）")

    if args.check:
        if not SUBSET_FONT.exists():
            print(f"[FAIL] 缺少已提交子集 {SUBSET_FONT}，请先运行完整流程")
            return 1
        print(f"验证 {SUBSET_FONT}（{SUBSET_FONT.stat().st_size} bytes）…")
        missing, emoji, _ = verify(SUBSET_FONT, corpus, MAPLE_FONT)
        return 1 if missing else 0

    if not SOURCE_FONT.exists():
        if not args.download:
            print(
                f"[FAIL] 源字体 {SOURCE_FONT} 缺失。\n"
                f"       请手动下载（OFL 1.1）:\n        {SOURCE_URL}\n"
                f"       或加 --download 自动获取。"
            )
            return 1
        print(f"下载源字体（{SOURCE_URL}）…")
        urllib.request.urlretrieve(SOURCE_URL, SOURCE_FONT)
    src_size = SOURCE_FONT.stat().st_size
    print(f"源字体 {SOURCE_FONT.name}: {src_size} bytes")

    with tempfile.TemporaryDirectory() as tmp:
        tmpdir = Path(tmp)
        regular = tmpdir / "NotoSansSC-Regular.ttf"
        text_file = tmpdir / "corpus.txt"
        text_file.write_text("".join(sorted(corpus)), encoding="utf-8")
        instantiate_regular(SOURCE_FONT, regular)
        print(f"实例化 wght={FONT_WEIGHT} → {regular.stat().st_size} bytes")
        subset_font(regular, SUBSET_FONT, text_file)
        fix_name_table(SUBSET_FONT)
    subset_size = SUBSET_FONT.stat().st_size
    print(f"子集产物 {SUBSET_FONT.name}: {subset_size} bytes")
    if subset_size > 5 * 1024 * 1024:
        print("[FAIL] 子集产物超过 5MB 验收上限")
        return 1

    missing, emoji, _ = verify(SUBSET_FONT, corpus, MAPLE_FONT)
    if missing:
        return 1
    print("完成：双字体子集已更新（≤5MB 且用字全覆盖）。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
