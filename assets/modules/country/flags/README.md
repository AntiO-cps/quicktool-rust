# Country flags

`svg/` and `png250/` are populated by `npm run sync:flags` from:
https://hampusborgos.github.io/country-flags/

The Windows release build verifies and bundles both representations offline.
The country card displays SVG and **copies PNG directly from the bundled `png250/` asset**, avoiding runtime SVG rasterization failures.
