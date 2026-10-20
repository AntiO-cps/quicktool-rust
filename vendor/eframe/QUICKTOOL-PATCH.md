# Immediate viewport actions

Source: crates.io eframe 0.31.1, copied from the existing Cargo registry.
Upstream: https://github.com/emilk/egui/tree/0.31.1/crates/eframe
License: MIT OR Apache-2.0. Upstream licenses are included.

The only runtime patch is in `src/native/glow_integration.rs`: the immediate
viewport render path now drains the same Screenshot/Cut/Copy/Paste actions as
the root viewport. Screenshot reads the current back buffer before swapping;
the event keeps the originating viewport ID and caller data.

This fixes the Pin/effects screenshot timeout and makes editing commands work
consistently for independent windows. Both main and independent viewport
screenshots must be verified when upgrading this dependency. The upstream
root/deferred path and rendering defaults remain unchanged.
