---
cargo/wh3-core: patch (Changed)
---

Update the desktop UI to `gpui-kit` 0.8 and render through its GPUI Fast 0.1.10 backend instead of gpui-pre 0.3.9; application imports are unchanged. A full redraw of the synthetic mod list takes 1.02 / 0.97 / 1.05 ms at the median for 1,000 / 10,000 / 100,000 mods instead of 1.59 / 1.56 / 1.58 ms (`visual` test, 50 forced redraws each, release mode, headless Metal on an Apple M5 Pro). This is an offscreen macOS measurement; Windows rendering was not measured.
