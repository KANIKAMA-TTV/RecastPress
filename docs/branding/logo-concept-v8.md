# RecastPress logo concept v8

Revision: slightly increase the type weight; keep the hanging stream separated from the letter; make it thick at its connection to the white band, taper down to a narrow neck, and finish with a gently swollen round terminal.

The ordinary Times New Roman Regular outline from v7 is preserved, with a centered 6-unit white outline weight increase (3 units per side) before applying the shared circular mask. At the upright this is a modest weight increase of approximately 7%. The letter placement, frame and single-circle pixel mask remain the same as v7. The revised original glyph includes this weight adjustment; the pixel boundary remains confined to that glyph.

The stream is one continuous filled Bezier shape from the circular white band. It narrows from its upper attachment to an approximately 11-unit neck, ending in a round terminal approximately twice the neck width. It ends above the cut letter with a visible blue gap.

## Validation

`verify-logo-v8.cjs` rasterizes the isolated letter and stream layers at 1000 × 1000. Results:

- Stream / letter overlap: **0 pixels**.
- Cut-letter pixels outside the revised original glyph: **0 pixels**.
- One shared cut circle; its center remains on the logo circumference at upper-left 45 degrees.

## Deliverables

- `recastpress-logo-concept-v8.svg`: portable vector artwork, with no installed-font dependency.
- `recastpress-logo-concept-v8-white.png`: 1200 × 1200 PNG export.
- `recastpress-logo-v8-uncut-master.svg`: complete thickened glyph before the cut.
- `logo-comparison-v8.html`, `logo-comparison-v8.png`: comparison and 64px / 32px previews.
- `build-logo-v8.ps1`: reproducible SVG construction.
- `verify-logo-v8.cjs`: containment and separation validation.

Built as native SVG, not with image generation. Current revision: 2026-10-26.

WordPress comparison uses the unmodified official W mark retrieved 2026-10-25 from https://s.w.org/style/images/about/WordPress-logotype-wmark.png . Source page: https://wordpress.org/about/logos/ .
