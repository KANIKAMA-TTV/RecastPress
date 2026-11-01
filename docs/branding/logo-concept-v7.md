# RecastPress logo concept v7

Built as a new, deterministic SVG, replacing generative raster approximation for this geometric revision.

## User changes

- Replace the separated drops with continuous injection from the white annular frame.
- Enlarge the R until both feet meet the circular frame, and move it slightly left.
- Use a normal Roman serif R before cutting, allowing its bowl to open.
- Use exactly one common cutting circle, centered on the badge circumference 45 degrees left of the top.
- Finish its cut edge with square, 8bit-style pixels confined to the original glyph fill.

## Construction

The source is the actual outline of Times New Roman Regular's capital R, obtained through Windows GDI+. It is converted to SVG path data; the delivered SVG has no font dependency. Only a uniform scale and translation are applied before the subtractive mask.

Canvas: 1000 × 1000. Badge center: (500, 500), outer radius 450. White band outer radius: 423. Blue inner disk radius: 374.4. The frame uses the WordPress reference proportions: outer blue ring 3% of diameter, white annular gap 5.4% of diameter.

R bounds before clipping: left 180, top 240, width 596.903, height 600. Its bounding center is x = 478.451, approximately 21.5 units left of the badge center. Both feet intersect the white frame; glyph rendering is clipped to its outer edge.

One shared cut circle has center (181.801948, 181.801948) and radius 300. The center lies exactly on the badge's outer circumference at 10:30. Both the upright and bowl use the same mask. No stroke is added to preserve the bowl.

Square boundary pixels use a 10-unit grid, a 40-unit radial transition band, and deterministic 4 × 4 Bayer dithering. Their coverage is calculated from the same cutting circle. Because the mask is applied to the original glyph, no boundary pixel can extend outside its fill. The continuous injection is a separate white path connected to the white circular band and the letter.

## Verification

`verify-logo-v7.cjs` renders isolated original and cut letter layers and checks that cut-letter alpha never appears outside the original glyph. Result: **0 pixels outside the original glyph**, 24,573 glyph pixels removed at 1000 × 1000. It also verifies a single glyph and a single shared cutting circle, with its center 450 units from the badge center (within SVG rounding).

## Files

- `recastpress-logo-concept-v7.svg`: editable, portable vector result.
- `recastpress-logo-concept-v7-white.png`: 1200 × 1200 raster export of the SVG.
- `recastpress-logo-v7-uncut-master.svg`: normal letter before subtraction.
- `logo-comparison-v7.html`, `logo-comparison-v7.png`: WordPress comparison, with 64px and 32px previews.
- `build-logo-v7.ps1`: reproducible construction; Windows and Times New Roman required only to rebuild.
- `verify-logo-v7.cjs`: meaningful mask-containment verification using bundled Sharp.

## Reference

WordPress official W mark, unchanged; comparison only. Source: https://wordpress.org/about/logos/ . PNG: https://s.w.org/style/images/about/WordPress-logotype-wmark.png . Retrieved 2026-10-25. Current revision: 2026-10-26.
