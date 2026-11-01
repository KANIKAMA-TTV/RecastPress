const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const root = path.join(__dirname,'warm-dark-study-v3');
fs.mkdirSync(root,{recursive:true});
const source = fs.readFileSync(path.join(__dirname,'recastpress-logo-concept-v14.svg'),'utf8');
const previous = JSON.parse(fs.readFileSync(path.join(__dirname,'color-metal-comparison-v2','options.json'),'utf8'));
const selected = previous.colors.filter(c=>['01','02','03','04'].includes(c.id));
const darken = (hex, black) => '#'+hex.slice(1).match(/../g).map(v=>Math.round(parseInt(v,16)*(1-black)).toString(16).padStart(2,'0')).join('').toUpperCase();
const rows = [
  {suffix:'',name:'元案',mix:0},
  {suffix:'-D1',name:'暗め',mix:.18},
  {suffix:'-D2',name:'さらに暗め',mix:.35},
];
const variants = rows.flatMap(row=>selected.map(c=>({...c,id:c.id+row.suffix,baseId:c.id,hex:darken(c.hex,row.mix),level:row.name,blackMix:row.mix,filename:c.id+row.suffix+'-'+c.slug})));
const card = c => `<article class="card"><div class="top"><span>${c.id}</span><span>${c.level}</span></div><a href="${c.filename}.svg"><div class="box"><img src="${c.filename}.svg" alt="${c.name} ${c.level}"></div></a><h2>${c.name}</h2><p class="hex">${c.hex}</p></article>`;
const reference = `<article class="card reference"><div class="top"><span>WP</span><span>REFERENCE</span></div><div class="box"><img src="wordpress-official-wmark.png" alt="WordPress公式ロゴ"></div><h2>WordPress（公式）</h2><p class="hex">比較基準・配布色のまま</p></article>`;
const levelCard = (title,kicker,note) => `<aside class="level"><span>${kicker}</span><h2>${title}</h2><p>${note}</p><div class="rule"></div><p>右の4色を横に比較<br>各色の濃さを縦に比較</p></aside>`;
const css = `*{box-sizing:border-box}body{margin:0;background:#F5F3EF;color:#272B2E;font-family:Arial,"Yu Gothic",sans-serif}.page{width:1800px;margin:auto;padding:42px 54px 34px}.kicker{font-size:14px;letter-spacing:.14em;color:#867B70;margin:0 0 12px}h1{font-size:34px;margin:0 0 12px}.intro{font-size:16px;color:#756C63;margin:0}.head{margin-bottom:29px}.grid{display:grid;grid-template-columns:repeat(5,1fr);gap:19px}.card{background:white;border:1px solid #E4DED5;border-radius:12px;padding:16px 15px 17px;text-align:center}.top{font-size:13px;color:#92877C;display:flex;justify-content:space-between;line-height:20px}.box{height:278px;width:278px;position:relative;overflow:hidden;margin:auto}.box img{width:278px;height:278px;position:absolute;max-width:none;left:50%;top:50%;transform:translate(-50%,-50%)}.reference{border:2px solid #A9A19A;padding:15px 14px 16px}.reference .box img{width:375px;height:375px}h2{font-size:20px;line-height:1.3;margin:0 0 9px}.hex{font-size:14px;color:#7E7368;letter-spacing:.05em;line-height:20px;margin:0}.level{background:#ECE8E1;border:1px solid #E0D9CE;border-radius:12px;padding:45px 29px;display:flex;flex-direction:column;justify-content:center}.level>span{font-size:13px;letter-spacing:.14em;color:#958778;margin-bottom:16px}.level h2{font-size:30px;margin-bottom:18px}.level p{font-size:15px;color:#877969;line-height:1.85;margin:0}.rule{height:1px;background:#D6CBBE;margin:28px 0}.foot{display:flex;justify-content:space-between;border-top:1px solid #DBD3C9;padding-top:18px;margin-top:25px;font-size:13px;color:#8A7E71}a{color:inherit;text-decoration:none}`;

(async()=>{
  fs.copyFileSync(path.join(__dirname,'wordpress-official-wmark-reference.png'),path.join(root,'wordpress-official-wmark.png'));
  for(const v of variants){
    const svg = source.replaceAll('#24627a',v.hex).replace('RecastPress logo concept 14',`RecastPress ${v.id} ${v.name} ${v.level}`);
    fs.writeFileSync(path.join(root,v.filename+'.svg'),svg);
    await sharp(Buffer.from(svg)).resize(1000,1000).png().toFile(path.join(root,v.filename+'.png'));
  }
  const grid = reference+variants.slice(0,4).map(card).join('')+levelCard('暗め','DARKER','色味を残しながら黒を加える。')+variants.slice(4,8).map(card).join('')+levelCard('さらに暗め','DEEPER','もう一段、深い色にする。')+variants.slice(8,12).map(card).join('');
  const html = `<!doctype html><html lang="ja"><meta charset="utf-8"><title>RecastPress — 残した4色と暗色バリエーション</title><style>${css}</style><main class="page"><header class="head"><p class="kicker">RECASTPRESS · SHORTLIST / DARKER COLOUR STUDY</p><h1>残した4色と、さらに暗いバリエーション</h1><p class="intro">最上段は元の4案。2段目・3段目は、それぞれ同じ色をより暗くした案です。左上はWordPress公式ロゴ。</p></header><section class="grid">${grid}</section><footer class="foot"><span>ロゴの形・大きさ・白い部分は共通。円内の地色と外周色だけを変更。</span><span>RECASTPRESS · 2026.10.26</span></footer></main></html>`;
  fs.writeFileSync(path.join(root,'shortlist.html'),html);
  fs.writeFileSync(path.join(root,'variants.json'),JSON.stringify({base:'concept-v14',selectedOriginalIds:selected.map(c=>c.id),rows,variants},null,2));
  fs.writeFileSync(path.join(__dirname,'current-candidates.json'),JSON.stringify({base:'concept-v14',study:'warm-dark-study-v3',selectedOriginals:selected.map(({id,name,hex})=>({id,name,hex})),reference:'WordPress official W mark',variants:variants.map(({id,hex,level,filename})=>({id,hex,level,svg:'warm-dark-study-v3/'+filename+'.svg'}))},null,2));
  fs.writeFileSync(path.join(root,'README.md'),'# RecastPress selected warm colours\n\nThe four central options on the prior top row are retained: 01 Burnt Orange, 02 Dark Amber, 03 Brick Red, 04 Oxblood. The official WordPress mark remains at top left.\n\nThe comparison uses columns for the four colour families, and rows for original, darker, and deeper. Darker variants mix 18% or 35% black into the original encoded RGB channels, preserving their warm direction. All letter, stream and cut geometry is unchanged from concept v14.\n\nOpen shortlist.html. Individual SVG and PNG exports are included. Prior options remain in the earlier archive, but are not part of this shortlist.\n');
  console.log(JSON.stringify({selected:selected.map(c=>({id:c.id,name:c.name})),variants:variants.map(({id,hex})=>({id,hex})),directory:root}));
})();
