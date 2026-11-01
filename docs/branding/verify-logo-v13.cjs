const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

(async () => {
  const svg = fs.readFileSync(path.join(__dirname, 'recastpress-logo-concept-v13.svg'), 'utf8');
  const defs = svg.match(/<defs>([\s\S]*?)<\/defs>/)[1];
  const renderLetter = (masked) => {
    const letter = `<g clip-path="url(#within-white-band)"><use href="#normal-r" fill="#fff" ${masked ? 'mask="url(#single-circular-cut)"' : ''}/></g>`;
    const isolated = `<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="1000"><defs>${defs}</defs>${letter}</svg>`;
    return sharp(Buffer.from(isolated)).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  };
  const streamPath = svg.match(/<path id="continuous-injection"[\s\S]*?\/>/)[0];
  const streamSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="1000">${streamPath}</svg>`;
  const [normal, cut, stream] = await Promise.all([
    renderLetter(false), renderLetter(true),
    sharp(Buffer.from(streamSvg)).ensureAlpha().raw().toBuffer({ resolveWithObject: true })
  ]);
  let streamLetterOverlap = 0;
  for (let i = 3; i < cut.data.length; i += 4) {
    if (cut.data[i] > 0 && stream.data[i] > 0) streamLetterOverlap++;
  }
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

  const edgePoints = (raw) => {
    const pts = [];
    const alpha = (x,y) => raw[(y * 1000 + x) * 4 + 3];
    for (let y = 1; y < 999; y++) {
      for (let x = 1; x < 999; x++) {
        if (alpha(x,y) > 127 && (alpha(x-1,y) <= 127 || alpha(x+1,y) <= 127 || alpha(x,y-1) <= 127 || alpha(x,y+1) <= 127)) pts.push([x,y]);
      }
    }
    return pts;
  };
  const gap = (a,b) => {
    let min2 = Infinity;
    for (const p of a) for (const q of b) {
      const dx = p[0]-q[0], dy = p[1]-q[1], d2=dx*dx+dy*dy;
      if (d2 < min2) min2=d2;
    }
    return Math.sqrt(min2);
  };
  const previousSvg = fs.readFileSync(path.join(__dirname,'recastpress-logo-concept-v12.svg'),'utf8');
  const previousStream = previousSvg.match(/<path id="continuous-injection"[\s\S]*?\/>/)[0];
  const previousRaw = await sharp(Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="1000">${previousStream}</svg>`)).ensureAlpha().raw().toBuffer();
  const letterEdge = edgePoints(cut.data);
  const currentGap = gap(edgePoints(stream.data),letterEdge);
  const previousGap = gap(edgePoints(previousRaw),letterEdge);
  if (currentGap < 30) throw new Error('The stream must remain clearly separated from the letter.');

  if (streamLetterOverlap || outsideGlyph || removed === 0 || Math.abs(centerRadius - 450) > 0.002 || sourceGlyphs !== 1 || cutCircles !== 1) {
    throw new Error(JSON.stringify({ streamLetterOverlap, outsideGlyph, removed, centerRadius, sourceGlyphs, cutCircles }));
  }
  console.log(JSON.stringify({ streamLetterGapPixels: currentGap, previousGapPixels: previousGap, streamLetterOverlapPixels: streamLetterOverlap, boundaryPixelsOutsideOriginalGlyph: outsideGlyph, removedGlyphPixels: removed, cutCenters: cutCircles, centerDistanceFromLogoCenter: centerRadius, position: 'upper-left 45 degrees', originalGlyphs: sourceGlyphs }));
})();








