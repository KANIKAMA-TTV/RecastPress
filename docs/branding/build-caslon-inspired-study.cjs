const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const source = fs.readFileSync(path.join(__dirname,'recastpress-logo-concept-v16.svg'),'utf8');
const glyph = source.match(/<path id="normal-r" d="([^"]+)"/);
let outline = glyph[1];
const oldHead = 'M 776.903 840 L 616.726 840 L 413.628 559.469';
const newHead = 'M 650 820 L 560 820 L 560 806 L 575 806 C 585 806 585 797 578 783 C 550 731 511 666 474 615 C 451 582 433 565 413.628 559.469';
const oldTail = 'L 624.248 720.088 C 652.566 759.616 676.903 785.87 697.257 798.85 C 717.611 811.829 744.159 820.088 776.903 823.628 Z';
const newTail = 'C 536 591 566 649 593 705 C 608 741 610 769 620 787 C 630 799 639 802 650 802 Z';
const oldSerif = 'C 374.395 797.139 391.209 803.628 413.628 803.628 L 437.08 803.628 L 437.08 820.000';
const newSerif = 'C 374.395 795 393 800 416 800 L 445 800 L 445 820.000';
for(const s of [oldHead,oldTail,oldSerif]) if(!outline.includes(s)) throw new Error('Approved glyph contour not found.');
outline = outline.replace(oldHead,newHead).replace(oldTail,newTail).replace(oldSerif,newSerif);
const shiftX = 20;
outline = outline.replace(/(-?\d+(?:\.\d+)?)\s+(-?\d+(?:\.\d+)?)/g,(_,x,y)=>`${(Number(x)+shiftX).toFixed(3)} ${y}`);

// A single, larger frame-fixed circle drives both the cut and its square-pixel edge.
const cx=500-450/Math.sqrt(2),cy=cx,radius=360,cell=10,band=40;
const bayer=[[0,8,2,10],[12,4,14,6],[3,11,1,9],[15,7,13,5]];
const pixels=[];
for(let row=0;row<100;row++) for(let col=0;col<100;col++){
  const x=col*cell,y=row*cell,d=Math.hypot(x+cell/2-cx,y+cell/2-cy);
  if(Math.abs(d-radius)>band/2) continue;
  const coverage=(d-(radius-band/2))/band;
  const fill=coverage>=(bayer[row%4][col%4]+.5)/16?'#fff':'#000';
  pixels.push(`<rect x="${x}" y="${y}" width="${cell}" height="${cell}" fill="${fill}"/>`);
}

// Shift the pendant and letter equally, while reattaching the root to the fixed ring.
const innerRadius=374.4;
const pointAtX = x => [x,500-Math.sqrt(innerRadius*innerRadius-(x-500)*(x-500))];
const a=pointAtX(241.466+shiftX),b=pointAtX(405+shiftX);
const outer=p=>p.map(v=>500+(v-500)*376.4/innerRadius);
const xy=p=>p.map(v=>v.toFixed(3)).join(' ');
const pt=(x,y)=>`${(x+shiftX).toFixed(3)} ${y}`;
const aControl=[a[0]+30.534,a[1]+30.534*(-(a[0]-500)/(a[1]-500))];
const bControl=[b[0]-30,b[1]-30*(-(b[0]-500)/(b[1]-500))];
const stream=`M ${xy(a)} C ${xy(aControl)} ${pt(313.540,213)} ${pt(313.540,249)} C ${pt(313.540,265)} ${pt(306.540,274)} ${pt(296.540,287)} C ${pt(277.540,308)} ${pt(286.540,340)} ${pt(312.540,349)} C ${pt(337.540,358)} ${pt(364.540,340)} ${pt(365.540,317)} C ${pt(366.540,287)} ${pt(335.540,282)} ${pt(335.540,249)} C ${pt(335.540,213)} ${xy(bControl)} ${xy(b)} L ${xy(outer(b))} A 376.4 376.4 0 0 0 ${xy(outer(a))} Z`;
const svg = source
  .replace(glyph[0],`<path id="normal-r" d="${outline}"`)
  .replace(/(<circle id="shared-cut-circle"[^>]* r=")[^"]+/,`$1${radius}`)
  .replace(/<g id="pixel-boundary"[^>]*>[\s\S]*?<\/g>/,`<g id="pixel-boundary" shape-rendering="crispEdges">${pixels.join('')}</g>`)
  .replace(/(<path id="continuous-injection" d=")[^"]+/,`$1${stream}`)
  .replace(/<title id="title">[\s\S]*?<\/title>/,'<title id="title">RecastPress — Caslon-inspired study</title>');

const output=path.join(__dirname,'caslon-inspired-study.png');
sharp(Buffer.from(svg),{density:96}).resize(1200,1200).png().toFile(output).then(()=>{
  console.log(JSON.stringify({output,images:1,reference:'Adobe Caslon in the user-supplied comparison image; custom outline interpretation, not the installed Adobe font',internalShiftX:shiftX,cutRadius:radius,previousCutRadius:300,cutAreaIncreasePercent:44,adoptedAssetsUpdated:false}));
});
