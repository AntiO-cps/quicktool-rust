# QuickTool variable-font patch

Source: crates.io epaint 0.31.1 (egui 0.31.1), copied from the existing Cargo registry.
Upstream: https://github.com/emilk/egui/tree/0.31.1/crates/epaint
License: MIT OR Apache-2.0; unmodified upstream licenses are included here.

Only src/text/fonts.rs is patched: FontData gains optional variation coordinates,
and its existing ab_glyph FontRef/FontVec parser applies these coordinates before
constructing FontArc. Empty coordinates preserve upstream behavior. Static font
bytes remain borrowed and shared between weights. The two regression tests use
the project's existing MiSans variable font, with no network or extra dependency.

Remove this local patch when upgrading to an upstream release that exposes
variable-font coordinates. Recheck rendering and the regression tests on upgrades.
