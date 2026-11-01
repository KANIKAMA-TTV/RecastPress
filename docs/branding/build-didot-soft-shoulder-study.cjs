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
const shiftX = 40;
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
const pt=(x,y)=>`${(x+shiftX).toFixed(3)} ${y-15}`;
const aControl=[a[0]+30.534,a[1]+30.534*(-(a[0]-500)/(a[1]-500))];
const bControl=[b[0]-30,b[1]-30*(-(b[0]-500)/(b[1]-500))];
const stream=`M ${xy(a)} C ${xy(aControl)} ${pt(297.540,213)} ${pt(297.540,249)} C ${pt(297.540,266)} ${pt(299.623,274.75)} ${pt(292.340,282.8)} C ${pt(270.490,306.95)} ${pt(280.840,343.75)} ${pt(310.740,354.1)} C ${pt(339.490,364.45)} ${pt(370.540,343.75)} ${pt(371.690,317.3)} C ${pt(372.840,282.8)} ${pt(351.540,282)} ${pt(351.540,249)} C ${pt(351.540,213)} ${xy(bControl)} ${xy(b)} L ${xy(outer(b))} A 376.4 376.4 0 0 0 ${xy(outer(a))} Z`;
let svg = source
  .replace(glyph[0],`<path id="normal-r" d="${outline}"`)
  .replace(/(<circle id="shared-cut-circle"[^>]* r=")[^"]+/,`$1${radius}`)
  .replace(/<g id="pixel-boundary"[^>]*>[\s\S]*?<\/g>/,`<g id="pixel-boundary" shape-rendering="crispEdges">${pixels.join('')}</g>`)
  .replace(/(<path id="continuous-injection" d=")[^"]+/,`$1${stream}`)
  .replace(/<title id="title">[\s\S]*?<\/title>/,'<title id="title">RecastPress — Didot soft-shoulder study</title>');

// Didot-inspired bowed leg: substantial vertical mass and a fine upturned terminal.
const legPath='M 413.628 559.469 C 486 565 531 592 538 642 C 552 691 544 744 563 780 C 577 809 604 817 637 815 C 680 814 721 794 745 779 L 742 771 C 714 787 680 801 669 793 C 653 785 654 765 654 734 C 654 679 648 640 622 604 C 592 574 548 558 500.354 547.965 Z';
const curvedLeg='<path d="'+legPath+'" fill="#fff" stroke="#fff" stroke-width="2" stroke-linejoin="round"/>';
const freeLeg='<g clip-path="url(#within-white-band)"><g clip-path="url(#midpoint-foot-height)"><g transform="translate('+shiftX+' 0)">'+curvedLeg+'</g></g></g>';
svg = svg.replace('<path id="continuous-injection"',freeLeg+'<path id="continuous-injection"');
const output=path.join(__dirname,'didot-soft-shoulder-study.png');
sharp(Buffer.from(svg),{density:96}).resize(1200,1200).png().toFile(output).then(()=>{
  console.log(JSON.stringify({output,images:1,reference:'Didot in the user-supplied comparison image; custom outline interpretation',legStyle:'heavier Didot-inspired bowed leg, body projecting beyond the upper bowl, fine upturned terminal',internalShiftX:shiftX,legCorrectionX:-10,upperShoulder:"gently lowered and relaxed",dropletShiftY:-15,cutRadius:radius,dropletBase:'strong-s-curve-study',neckWidth:54,previousNeckWidth:42,adoptedAssetsUpdated:false}));
});
