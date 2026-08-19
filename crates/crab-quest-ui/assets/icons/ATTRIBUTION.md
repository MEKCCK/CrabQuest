# Tabler Icons attribution

This directory contains 12 selected outline icons from [Tabler Icons](https://tabler.io/icons), version 3.46.0:

- `map-2`, `code`, `bulb`, `heart`, `bolt`, `flame`
- `sword`, `trophy`, `settings`, `player-play`, `circle-check`, `lock`

The original SVG files are retained under `tabler-svg/`. The matching `tabler-png/` files are 48×48 local renderings made from those SVGs with their `currentColor` stroke set to white, so the application can tint them at runtime. They are embedded with `include_bytes!`; the game makes no network request for icons.

Tabler Icons are licensed under the MIT License. The exact upstream license text is preserved in [TABLER-ICONS-MIT.txt](TABLER-ICONS-MIT.txt).
