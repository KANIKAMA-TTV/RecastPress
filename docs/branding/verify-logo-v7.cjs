const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

(async () => {
  const svg = fs.readFileSync(path.join(__dirname, 'recastpress-logo-concept-v7.svg'), 'utf8');
  const defs = svg.match(/<defs>([\s\S]*?)<\/defs>/)[1];
  const renderLetter = (masked) => {
    const letter = `<g clip-path="url(#within-white-band)"><use href="#normal-r" fill="#fff" ${masked ? 'mask="url(#single-circular-cut)"' : ''}/></g>`;
    const isolated = `<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="1000"><defs>${defs}</defs>${letter}</svg>`;
    return sharp(Buffer.from(isolated)).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  };
  const [normal, cut] = await Promise.all([renderLetter(false), renderLetter(true)]);
  let outsideGlyph = 0;
  let removed = 0;
  for (let i = 3; i < normal.data.length; i += 4) {
    if (normal.data[i] === 0 && cut.data[i] !== 0) outsideGlyph++;
    if (normal.data[i] > 0 && cut.data[i] === 0) removed++;
  }
  const circle = svg.match(/id="shared-cut-circle" cx="([\d.]+)" cy="([\d.]+)" r="([\d.]+)"/);
  const cx = Number(circle[1]);
  const cy = Number(circle[2]);
  const centerRadius = Math.hypot(cx - 500, cy - 500);
  const sourceGlyphs = [...svg.matchAll(/<path id="normal-r"/g)].length;
  const cutCircles = [...svg.matchAll(/id="shared-cut-circle"/g)].length;
  if (outsideGlyph || removed === 0 || Math.abs(centerRadius - 450) > 0.002 || sourceGlyphs !== 1 || cutCircles !== 1) {
    throw new Error(JSON.stringify({ outsideGlyph, removed, centerRadius, sourceGlyphs, cutCircles }));
  }
  console.log(JSON.stringify({ boundaryPixelsOutsideOriginalGlyph: outsideGlyph, removedGlyphPixels: removed, cutCenters: cutCircles, centerDistanceFromLogoCenter: centerRadius, position: 'upper-left 45 degrees', originalGlyphs: sourceGlyphs }));
})();
