const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const source = fs.readFileSync(path.join(__dirname,'warm-dark-study-v3','03-D1-brick-red.svg'),'utf8');
const previous = fs.readFileSync(path.join(__dirname,'recastpress-logo-concept-v15.svg'),'utf8');
const glyph = source.match(/<path id="normal-r" d="([^"]+)"/);
const original = glyph[1];
const from = original.indexOf('L 350.796 733.805');
const to = original.indexOf('L 266.283 346.195',from);
if(from<0 || to<0) throw new Error('Upright foot contour not found.');
const foot = original.slice(from,to);
const raisedFoot = foot.replace(/(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)/g,(_,x,y)=>`${x} ${(Number(y)-20).toFixed(3)}`);
const revisedGlyph = original.slice(0,from)+raisedFoot+original.slice(to);
const capY=820,clearanceRadius=362;
const clips = `<clipPath id="midpoint-foot-height" clipPathUnits="userSpaceOnUse"><rect width="1000" height="${capY}"/></clipPath><clipPath id="right-foot-clearance" clipPathUnits="userSpaceOnUse"><rect width="1000" height="700"/><rect width="500" height="1000"/><circle cx="500" cy="500" r="${clearanceRadius}"/></clipPath>`;
const oldUse = source.match(/<use href="#normal-r"[^>]+\/>/)[0];
const newUse = '<g clip-path="url(#right-foot-clearance)">'+oldUse.replace('<use ','<use clip-path="url(#midpoint-foot-height)" ')+'</g>';
const svg = source.replace(glyph[0],`<path id="normal-r" d="${revisedGlyph}"`).replace('</defs>',clips+'</defs>').replace(oldUse,newUse)
  .replace(/<title id="title">[\s\S]*?<\/title>/,'<title id="title">RecastPress — midpoint feet revision 16</title>')
  .replace(/<metadata>[\s\S]*?<\/metadata>/,'<metadata>Adopted colour #79332B, candidate 03-D1. Revision 16: both visible foot ends are at y=820, midway between the prior upright y=840 and diagonal cap y=800. The upright foot profile moves up 20 units. The right foot extends down 20 units and remains clear of the circular band. All upper letter contours, the stream, ring and circular pixel cut are retained.</metadata>');
const isolate = s => {
  const defs=s.match(/<defs>([\s\S]*?)<\/defs>/)[1];
  const body=s.slice(s.indexOf('</defs>')+7);
  const letter=body.match(/<g clip-path="url\(#within-white-band\)">[\s\S]*?<\/g>\s*(?=<path id="continuous-injection")/)[0];
  return '<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="1000"><defs>'+defs+'</defs>'+letter+'</svg>';
};

(async()=>{
  const [oldRaw,newRaw]=await Promise.all([previous,svg].map(s=>sharp(Buffer.from(isolate(s))).ensureAlpha().raw().toBuffer()));
  const bottom=[-1,-1],oldBottom=[-1,-1];
  let rightAdded=0,rightContact=0,leftContact=0,outermostRight=0;
  for(let y=700;y<1000;y++) for(let x=0;x<1000;x++){
    const i=(y*1000+x)*4+3,side=x<500?0:1;
    if(oldRaw[i]>127) oldBottom[side]=Math.max(oldBottom[side],y);
    if(newRaw[i]>127){
      bottom[side]=Math.max(bottom[side],y);
      const radius=Math.hypot(x+.5-500,y+.5-500);
      if(side===1){
        outermostRight=Math.max(outermostRight,radius);
        if(radius>=374.4) rightContact++;
        if(oldRaw[i]<=127) rightAdded++;
      } else if(radius>=374.4) leftContact++;
    }
  }
  if(bottom[0]!==819 || bottom[1]!==819 || bottom[0]>=oldBottom[0] || bottom[1]<=oldBottom[1] || rightAdded===0 || rightContact!==0 || leftContact===0) throw new Error(JSON.stringify({bottom,oldBottom,rightAdded,rightContact,leftContact}));
  fs.writeFileSync(path.join(__dirname,'recastpress-logo-concept-v16.svg'),svg);
  await sharp(Buffer.from(svg),{density:96}).resize(1200,1200).png().toFile(path.join(__dirname,'recastpress-logo-concept-v16-white.png'));
  console.log(JSON.stringify({previousBottomPixels:oldBottom,revisedBottomPixels:bottom,sharedFootHeight:capY,rightLegGapToInnerRing:374.4-outermostRight,rightAddedPixels:rightAdded,rightLegRingContactPixels:rightContact,leftStemStillTouchesRing:leftContact>0}));
})();
