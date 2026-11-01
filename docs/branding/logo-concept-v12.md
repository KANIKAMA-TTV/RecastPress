# RecastPress logo concept v12

Revision: offset the approved droplet slightly right and make its connection to the circular frame more balanced left-to-right.

The droplet neck axis moves 16 units right, from x = 308.539823 to x = 324.539823. The rounded terminal is translated without changing its shape or height. Both root joins now use tangent circular fillets with the same 45-unit radius. Because the source frame is sloped at this position, the two tangency heights differ; matching the radii gives both shoulders the same local curvature.

The root is constructed against the actual inner frame circle. A 2-unit white overlap along its top arc avoids antialiasing seams. All type geometry, type weight, R placement, shared circular cut, and square pixel boundary remain unchanged.

Validation checks the stream is separated from the letter and that the pixel boundary remains inside the glyph. The single cut-circle center is unchanged.

## Files

- `recastpress-logo-concept-v12.svg`: vector result.
- `recastpress-logo-concept-v12-white.png`: 1200 × 1200 PNG.
- `recastpress-logo-v12-uncut-master.svg`: complete glyph master.
- `logo-comparison-v12.html`, `logo-comparison-v12.png`: WordPress comparison with 64px / 32px previews.
- `build-logo-v12.ps1`, `verify-logo-v12.cjs`: reproducible construction and checks.

Native SVG revision, 2026-10-26. Official WordPress reference remains unchanged: https://wordpress.org/about/logos/ .
