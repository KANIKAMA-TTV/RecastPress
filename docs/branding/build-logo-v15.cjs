const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const sourceFile = path.join(__dirname,'warm-dark-study-v3','03-D1-brick-red.svg');
const outputFile = path.join(__dirname,'recastpress-logo-concept-v15.svg');
const source = fs.readFileSync(sourceFile,'utf8');
const capY = 800;
const clip = '<clipPath id="short-right-leg" clipPathUnits="userSpaceOnUse"><path d="M 0 0 H 1000 V 800 H 500 V 1000 H 0 Z"/></clipPath>';
const originalUse = source.match(/<use href="#normal-r"[^>]+\/>/)[0];
const revisedUse = originalUse.replace('<use ','<use clip-path="url(#short-right-leg)" ');
const svg = source.replace('</defs>',clip+'</defs>').replace(originalUse,revisedUse)
  .replace(/<title id="title">[\s\S]*?<\/title>/,'<title id="title">RecastPress — diagonal leg revision 15</title>')
  .replace(/<metadata>[\s\S]*?<\/metadata>/,'<metadata>Adopted colour #79332B, candidate 03-D1. Revision 15: only the distal right diagonal leg is clipped at y=800. The upright remains at baseline y=840 and touches the ring. Original outlined glyph, ring, stream, shared circular cut and square pixels are preserved.</metadata>');
const defs = s => s.match(/<defs>([\s\S]*?)<\/defs>/)[1];
const isolate = s => '<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="1000"><defs>'+defs(s)+'</defs><g clip-path="url(#within-white-band)">'+s.match(/<use [^>]*href="#normal-r"[^>]+\/>/)[0]+'</g></svg>';

(async()=>{
  const [oldRaw,newRaw] = await Promise.all([source,svg].map(s=>sharp(Buffer.from(isolate(s))).ensureAlpha().raw().toBuffer()));
  let changedLeft=0,changedAbove=0,ringContactRight=0,ringContactLeft=0,outermostRight=0;
  for(let y=0;y<1000;y++) for(let x=0;x<1000;x++){
    const i=(y*1000+x)*4+3;
    if(x<500 && oldRaw[i]!==newRaw[i]) changedLeft++;
    if(y<800 && oldRaw[i]!==newRaw[i]) changedAbove++;
    if(newRaw[i]>127){
      const radius=Math.hypot(x+.5-500,y+.5-500);
      if(x>=500 && y>=700){
        outermostRight=Math.max(outermostRight,radius);
        if(radius>=374.4) ringContactRight++;
      }
      if(x<500 && y>=700 && radius>=374.4) ringContactLeft++;
    }
  }
  if(changedLeft || changedAbove || ringContactRight || !ringContactLeft) throw new Error(JSON.stringify({changedLeft,changedAbove,ringContactRight,ringContactLeft}));
  fs.writeFileSync(outputFile,svg);
  await sharp(Buffer.from(svg),{density:96}).resize(1200,1200).png().toFile(path.join(__dirname,'recastpress-logo-concept-v15-white.png'));
  console.log(JSON.stringify({output:outputFile,rightLegBaseline:capY,leftStemBaseline:840,rightLegGapToInnerRing:374.4-outermostRight,changedLeftPixels:changedLeft,changedPixelsAboveNewCap:changedAbove,rightLegRingContactPixels:ringContactRight,leftStemStillTouchesRing:ringContactLeft>0}));
})();
