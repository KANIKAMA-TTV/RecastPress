const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const source = fs.readFileSync(path.join(__dirname,'recastpress-logo-concept-v16.svg'),'utf8');
const glyph = source.match(/<path id="normal-r" d="([^"]+)"/);
let outline = glyph[1];
const oldHead = 'M 776.903 840 L 616.726 840 L 413.628 559.469';
const newHead = 'M 413.628 559.469';
const oldTail = 'L 624.248 720.088 C 652.566 759.616 676.903 785.87 697.257 798.85 C 717.611 811.829 744.159 820.088 776.903 823.628 Z';
const newTail = 'L 413.628 559.469 Z';
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
const stream=`M ${xy(a)} C ${xy(aControl)} ${pt(303.540,213)} ${pt(303.540,249)} C ${pt(303.540,266)} ${pt(302.540,274)} ${pt(292.340,282.8)} C ${pt(270.490,306.95)} ${pt(280.840,343.75)} ${pt(310.740,354.1)} C ${pt(339.490,364.45)} ${pt(370.540,343.75)} ${pt(371.690,317.3)} C ${pt(372.840,282.8)} ${pt(345.540,282)} ${pt(345.540,249)} C ${pt(345.540,213)} ${xy(bControl)} ${xy(b)} L ${xy(outer(b))} A 376.4 376.4 0 0 0 ${xy(outer(a))} Z`;
let svg = source
  .replace(glyph[0],`<path id="normal-r" d="${outline}"`)
  .replace(/(<circle id="shared-cut-circle"[^>]* r=")[^"]+/,`$1${radius}`)
  .replace(/<g id="pixel-boundary"[^>]*>[\s\S]*?<\/g>/,`<g id="pixel-boundary" shape-rendering="crispEdges">${pixels.join('')}</g>`)
  .replace(/(<path id="continuous-injection" d=")[^"]+/,`$1${stream}`)
  .replace(/<title id="title">[\s\S]*?<\/title>/,'<title id="title">RecastPress — Caslon-inspired study</title>');

// The second S-curve is an exact translation of the first, not a normal offset.
const leftS=[[413.628,559.469],[443.628,647.469],[565,698],[595,786]];
const translation=[86.726,-11.504];
const rightS=leftS.map(p=>p.map((v,i)=>v+translation[i]));
const point=p=>p.map(v=>v.toFixed(3)).join(' ');
const cubic=p=>'C '+point(p[1])+' '+point(p[2])+' '+point(p[3]);
const direction=[30,88],length=Math.hypot(...direction),unit=direction.map(v=>v/length);
const endL=leftS[3],endR=rightS[3];
const bracket1=endL.map((v,i)=>v+unit[i]*10),flare2=endR.map((v,i)=>v+unit[i]*15);
const legPath='M '+point(leftS[0])+' '+cubic(leftS)+' C '+point(bracket1)+' 615 800 605 800 L 545 800 L 545 820 L 810 820 L 810 800 L 786 800 C 765 800 '+point(flare2)+' '+point(endR)+' C '+point(rightS[2])+' '+point(rightS[1])+' '+point(rightS[0])+' Z';
const curvedLeg='<path d="'+legPath+'" fill="#fff" stroke="#fff" stroke-width="6" stroke-linejoin="round"/>';
const freeLeg='<g clip-path="url(#within-white-band)"><g clip-path="url(#midpoint-foot-height)"><g transform="translate('+shiftX+' 0)">'+curvedLeg+'</g></g></g>';
svg = svg.replace('<path id="continuous-injection"',freeLeg+'<path id="continuous-injection"');
const output=path.join(__dirname,'strong-s-curve-study.png');
sharp(Buffer.from(svg),{density:96}).resize(1200,1200).png().toFile(output).then(()=>{
  console.log(JSON.stringify({output,images:1,reference:'Adobe Caslon in the user-supplied comparison image; custom outline interpretation, not the installed Adobe font',internalShiftX:shiftX,uprightSerifWidth:265,diagonalSerifWidth:265,curveConstruction:'rotationally symmetric cubic S, duplicated by translation',curveTranslation:translation,serifRightShift:45,additionalSerifShift:25,legEndLeftShift:12,dropletBodyScale:1.15,neckWidth:42,previousNeckWidth:30,ringOverlapAllowed:true,cutRadius:radius,previousCutRadius:300,cutAreaIncreasePercent:44,adoptedAssetsUpdated:false}));
});
