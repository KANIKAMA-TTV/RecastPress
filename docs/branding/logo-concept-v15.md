# RecastPress logo revision v15

User instruction: float the R's diagonal right foot slightly above the circular frame; its length need not match the vertical foot.

The adopted colour remains #79332B (03-D1). Only the distal right leg is shortened: a glyph-only clip removes pixels below y=800 for x>=500. The complete outlined glyph is retained in the SVG. The vertical foot remains at y=840 and still joins the ring.

The circular frame, stream, common circular cut and square-pixel treatment are unchanged. `build-logo-v15.cjs` verifies the left half and all pixels above the new cap match the previous version exactly. At 1000px, the right foot is approximately 12.3px clear of the inner ring, with zero contact pixels; left-foot contact remains.

The canonical SVG, both PNG backgrounds at all 16 sizes, web favicon and Apple Touch / 192px / 512px icons are rebuilt by `build-final-brand-assets.cjs`. The stable asset directory remains `../../assets/branding/recastpress/`.

The original distribution is retained as `recastpress-logo-final-brick-red-v1.zip`. The updated distribution is `recastpress-logo-final-brick-red-v2.zip`, also available under the stable filename `recastpress-logo-final-brick-red.zip`.
