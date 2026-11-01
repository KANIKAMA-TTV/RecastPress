const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const source = fs.readFileSync(path.join(__dirname,'recastpress-logo-concept-v16.svg'),'utf8');
const glyph = source.match(/<path id="normal-r" d="([^"]+)"/);
let outline = glyph[1];
const oldHead = 'M 776.903 840 L 616.726 840 L 413.628 559.469';
const newHead = 'M 660 820 L 528 820 L 528 802 L 552 802 C 566 802 574 794 567 781 C 544 737 512 668 478 618 C 454 584 435 565 413.628 559.469';
const oldTail = 'L 624.248 720.088 C 652.566 759.616 676.903 785.87 697.257 798.85 C 717.611 811.829 744.159 820.088 776.903 823.628 Z';
const newTail = 'C 538 592 556 657 588 728 C 605 766 622 793 642 802 L 660 802 Z';
const oldUprightSerif = 'C 374.395 797.139 391.209 803.628 413.628 803.628 L 437.08 803.628 L 437.08 820.000';
const newUprightSerif = 'C 374.395 794.5 394.209 798 418 798 L 448 798 L 448 820.000';
for (const expected of [oldHead,oldTail,oldUprightSerif]) {
  if(!outline.includes(expected)) throw new Error('Expected approved contour not found.');
}
outline = outline.replace(oldHead,newHead).replace(oldTail,newTail).replace(oldUprightSerif,newUprightSerif);
const svg = source.replace(glyph[0],`<path id="normal-r" d="${outline}"`)
  .replace(/<title id="title">[\s\S]*?<\/title>/,'<title id="title">RecastPress Roman serif leg study</title>');
const output = path.join(__dirname,'roman-serif-leg-study.png');
sharp(Buffer.from(svg),{density:96}).resize(1200,1200).png().toFile(output).then(()=>{
  console.log(JSON.stringify({study:output,images:1,adoptedAssetsUpdated:false,colour:'#79332B',change:'Curved, tapered diagonal leg with a bracketed terminal serif; slightly stronger upright serif.'}));
});
