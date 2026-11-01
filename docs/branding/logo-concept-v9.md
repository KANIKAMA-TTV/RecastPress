# RecastPress logo concept v9

Revision: start the stream broadly and with smooth curves from the white circular band, suggesting a viscous liquid pouring from its source. Enlarge the round terminal, and increase its separation from the R.

The stream root spans the inner band's upper-left arc from -140 to -116 degrees. The two sides transition from that arc into the narrowing stream using cubic curves. The root overlaps the white band by 2 units to avoid an antialiased seam between adjoining white shapes. The neck remains narrow; its larger rounded terminal ends at y = 391 instead of v8's y = 420.

The type weight, glyph placement, both feet touching the band, common circular cut, and glyph-confined square pixel boundary remain as in v8. This is a native SVG revision, not a generative bitmap edit.

## Verification

At 1000 × 1000, `verify-logo-v9.cjs` checks isolated letter and stream layers:

- Minimum edge-pixel separation increases from **21px to 50px**.
- Stream / letter overlap: **0 pixels**.
- Cut-letter pixels outside the revised original glyph: **0 pixels**.
- One shared circular cut; center remains at upper-left 45 degrees on the badge circumference.

## Files

- `recastpress-logo-concept-v9.svg`: vector result.
- `recastpress-logo-concept-v9-white.png`: 1200 × 1200 PNG export.
- `recastpress-logo-v9-uncut-master.svg`: complete letter before the cut.
- `logo-comparison-v9.html`, `logo-comparison-v9.png`: WordPress comparison with 64px and 32px previews.
- `build-logo-v9.ps1`: reproducible SVG source construction.
- `verify-logo-v9.cjs`: separation, containment, and common-cut checks.

Current revision: 2026-10-26. WordPress official reference remains unchanged: https://wordpress.org/about/logos/ .
