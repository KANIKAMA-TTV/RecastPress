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
const stream=`M ${xy(a)} C ${xy(aControl)} ${pt(313.540,213)} ${pt(313.540,249)} C ${pt(313.540,265)} ${pt(306.540,274)} ${pt(296.540,287)} C ${pt(277.540,308)} ${pt(286.540,340)} ${pt(312.540,349)} C ${pt(337.540,358)} ${pt(364.540,340)} ${pt(365.540,317)} C ${pt(366.540,287)} ${pt(335.540,282)} ${pt(335.540,249)} C ${pt(335.540,213)} ${xy(bControl)} ${xy(b)} L ${xy(outer(b))} A 376.4 376.4 0 0 0 ${xy(outer(a))} Z`;
let svg = source
  .replace(glyph[0],`<path id="normal-r" d="${outline}"`)
  .replace(/(<circle id="shared-cut-circle"[^>]* r=")[^"]+/,`$1${radius}`)
  .replace(/<g id="pixel-boundary"[^>]*>[\s\S]*?<\/g>/,`<g id="pixel-boundary" shape-rendering="crispEdges">${pixels.join('')}</g>`)
  .replace(/(<path id="continuous-injection" d=")[^"]+/,`$1${stream}`)
  .replace(/<title id="title">[\s\S]*?<\/title>/,'<title id="title">RecastPress — Caslon-inspired study</title>');

// One continuous outline avoids a seam between the curved shaft and serif.
const control=[[456.991,553.717],[465.619,618.762],[585,700],[650,760]];
const lerp=(a,b,t)=>a.map((v,i)=>v+(b[i]-v)*t);
const center=t=>lerp(lerp(lerp(control[0],control[1],t),lerp(control[1],control[2],t),t),lerp(lerp(control[1],control[2],t),lerp(control[2],control[3],t),t),t);
const velocity=t=>[0,1].map(i=>3*(1-t)*(1-t)*(control[1][i]-control[0][i])+6*(1-t)*t*(control[2][i]-control[1][i])+3*t*t*(control[3][i]-control[2][i]));
const half=(350.796-266.283)/2;
const sides=[[],[]];
for(let i=0;i<=48;i++){
  const t=i/48,p=center(t),v=velocity(t),len=Math.hypot(...v),normal=[-v[1]/len,v[0]/len];
  sides[0].push(p.map((x,j)=>x-normal[j]*half));
  sides[1].push(p.map((x,j)=>x+normal[j]*half));
}
const point=p=>p.map(x=>x.toFixed(3)).join(' ');
const curve=points=>{
  let d='';
  for(let i=0;i<points.length-1;i++){
    const a=i?points[i-1]:points[0].map((v,j)=>2*v-points[1][j]);
    const b=points[i],c=points[i+1];
    const z=i+2<points.length?points[i+2]:c.map((v,j)=>2*v-b[j]);
    const c1=b.map((v,j)=>v+(c[j]-a[j])/6),c2=c.map((v,j)=>v-(z[j]-b[j])/6);
    d+=' C '+point(c1)+' '+point(c2)+' '+point(c);
  }
  return d;
};
const outside=sides[0],inside=sides[1],outerEnd=outside.at(-1),innerEnd=inside.at(-1);
const tangent=velocity(1),norm=Math.hypot(...tangent),unit=tangent.map(v=>v/norm);
const flare1=outerEnd.map((v,i)=>v+unit[i]*35);
const bracket2=innerEnd.map((v,i)=>v+unit[i]*10);
const legPath='M '+point(outside[0])+curve(outside)+' C '+point(flare1)+' 724 800 743 800 L 765 800 L 765 820 L 500 820 L 500 800 L 580 800 C 600 800 '+point(bracket2)+' '+point(innerEnd)+curve([...inside].reverse())+' Z';
const curvedLeg='<path d="'+legPath+'" fill="#fff" stroke="#fff" stroke-width="6" stroke-linejoin="round"/>';
const freeLeg='<g clip-path="url(#within-white-band)"><g clip-path="url(#midpoint-foot-height)"><g transform="translate('+shiftX+' 0)">'+curvedLeg+'</g></g></g>';
svg = svg.replace('<path id="continuous-injection"',freeLeg+'<path id="continuous-injection"');
const output=path.join(__dirname,'caslon-weight-study.png');
sharp(Buffer.from(svg),{density:96}).resize(1200,1200).png().toFile(output).then(()=>{
  console.log(JSON.stringify({output,images:1,reference:'Adobe Caslon in the user-supplied comparison image; custom outline interpretation, not the installed Adobe font',internalShiftX:shiftX,uprightSerifWidth:265,diagonalSerifWidth:265,diagonalStrokeWidth:90,ringOverlapAllowed:true,cutRadius:radius,previousCutRadius:300,cutAreaIncreasePercent:44,adoptedAssetsUpdated:false}));
});
