const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const root = path.join(__dirname,'color-metal-comparison-v2');
const previous = path.join(__dirname,'color-studies-v14');
fs.mkdirSync(root,{recursive:true});
const palette = JSON.parse(fs.readFileSync(path.join(previous,'palette.json'),'utf8'));
const source = fs.readFileSync(path.join(__dirname,'recastpress-logo-concept-v14.svg'),'utf8');
const originalDefs = source.match(/<defs>([\s\S]*?)<\/defs>/)[1];
const glyph = originalDefs.match(/<path id="normal-r"[^>]+\/>/)[0];
const stream = source.match(/<path id="continuous-injection"[^>]+\/>/)[0];
const annulus = '<path d="M 923 500 A 423 423 0 1 0 77 500 A 423 423 0 1 0 923 500 Z M 874.4 500 A 374.4 374.4 0 1 0 125.6 500 A 374.4 374.4 0 1 0 874.4 500 Z" fill-rule="evenodd"/>';
const letter = '<g clip-path="url(#within-white-band)"><use href="#normal-r" mask="url(#single-circular-cut)"/></g>';
const gradient = (id,stops,x1=0,y1=0,x2=0,y2=1000) => `<linearGradient id="${id}" gradientUnits="userSpaceOnUse" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}">${stops.map(([o,c])=>`<stop offset="${o}%" stop-color="${c}"/>`).join('')}</linearGradient>`;
const mercury = [[0,'#FCFEFF'],[14,'#ADBFCB'],[28,'#F5FCFF'],[38,'#758B9A'],[46,'#D4E4EE'],[49,'#FFFFFF'],[52,'#758A99'],[62,'#DCE8F0'],[73,'#FAFDFF'],[84,'#8A9DAA'],[100,'#E7EFF4']];
const chrome = [[0,'#F3FAFF'],[10,'#6E879B'],[26,'#D4E1EB'],[40,'#FFFFFF'],[46,'#9EAFBE'],[49,'#263F55'],[50.5,'#071522'],[52,'#8AAAC4'],[55,'#F9FEFF'],[61,'#C6D4DF'],[79,'#607C94'],[88,'#F4FBFF'],[100,'#4D6376']];
const blackChrome = [[0,'#070D13'],[16,'#2B3C49'],[29,'#637B8D'],[31,'#A8BDCB'],[34,'#10202D'],[47,'#030A11'],[51,'#1F394D'],[54,'#68869B'],[59,'#243F53'],[76,'#101D28'],[90,'#455F72'],[100,'#07111B']];
const copper = [[0,'#F7CFA3'],[17,'#A45A2A'],[37,'#F7CA91'],[45,'#FFE3BC'],[49,'#8C4019'],[51,'#4F210F'],[54,'#E6A067'],[58,'#FFE0AD'],[76,'#AD632E'],[90,'#F3C387'],[100,'#945025']];
const gold = [[0,'#FFF3C0'],[16,'#C79223'],[36,'#F5D77D'],[45,'#FFF7D1'],[49,'#9A6411'],[51,'#563007'],[54,'#E9B53D'],[58,'#FFF0A8'],[78,'#B67C18'],[91,'#FBE1A0'],[100,'#A77417']];
const nickel = [[0,'#18242D'],[18,'#71818C'],[38,'#22323E'],[46,'#BBC8D0'],[49,'#273947'],[52,'#07131D'],[55,'#748C9F'],[66,'#283E4E'],[87,'#9AADB9'],[100,'#1B2D3B']];

const additions = [
  {id:'M4',slug:'mercury-ground-white',name:'水銀／白ベタ',note:'水銀の地・白い字',kind:'background',paint:'#FFFFFF',stops:mercury},
  {id:'M5',slug:'mercury-ground-ink',name:'水銀／墨ベタ',note:'水銀の地・墨色の字',kind:'background',paint:'#18242E',stops:mercury},
  {id:'M6',slug:'chrome-ground-white',name:'クローム／白ベタ',note:'強い鏡面の地・白い字',kind:'background',paint:'#FFFFFF',stops:chrome},
  {id:'M7',slug:'chrome-ground-ink',name:'クローム／墨ベタ',note:'強い鏡面の地・墨色の字',kind:'background',paint:'#14212D',stops:chrome},
  {id:'M8',slug:'black-chrome-ground-white',name:'黒クローム／白ベタ',note:'黒いメッキの地・白い字',kind:'background',paint:'#FFFFFF',stops:blackChrome},
  {id:'M9',slug:'copper-plated-ground-white',name:'銅メッキ／白ベタ',note:'銅メッキの地・白い字',kind:'background',paint:'#FFFFFF',stops:copper},
  {id:'M10',slug:'gold-plated-ground-ink',name:'金メッキ／墨ベタ',note:'金メッキの地・墨色の字',kind:'background',paint:'#281E11',stops:gold},
  {id:'M11',slug:'hard-chrome-letter',name:'硬質クローム文字',note:'強い鏡面の字・黒い地',kind:'foreground',ground:'#101820',stops:chrome},
  {id:'M12',slug:'black-nickel-letter',name:'黒ニッケル文字',note:'黒い鏡面の字・淡灰の地',kind:'foreground',ground:'#E7E8E5',stops:nickel},
  {id:'M13',slug:'gold-plated-letter-wine',name:'金メッキ文字',note:'金メッキの字・深紅の地',kind:'foreground',ground:'#501E2B',stops:gold},
];

function finishSvg(item) {
  const paint = item.kind==='background' ? item.paint : 'url(#finish)';
  const ground = item.kind==='background' ? 'url(#finish)' : item.ground;
  const defs = originalDefs.replace(glyph,glyph.replace('stroke="#fff"',`stroke="${paint}"`));
  const foreground = `<g fill="${paint}">${annulus}${letter}${stream.replace('fill="#fff"',`fill="${paint}"`)}</g>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000" width="1000" height="1000" role="img"><title>RecastPress ${item.id} ${item.name}</title><desc>Concept v14 geometry. ${item.note}. Solid paint applies to letter, annular band and hanging stream together.</desc><defs>${defs}${gradient('finish',item.stops)}</defs><rect width="1000" height="1000" fill="#fff"/><circle cx="500" cy="500" r="450" fill="${ground}"/>${foreground}</svg>`;
}
const originals = [
  ...palette.colors.map(c=>({...c,kind:'color',label:c.hex})),
  ...palette.finishes.map(c=>({...c,label:c.id==='M1'?'銀の文字・黒い地':c.id==='M2'?'研磨した地・白ベタ文字':'銅色の文字・黒茶の地'}))
];
const ref = {id:'WP',name:'WordPress（公式）',label:'比較基準・配布色のまま',kind:'reference',src:'wordpress-official-wmark.png'};

const css = `
*{box-sizing:border-box}body{margin:0;background:#F5F3EF;color:#262B30;font-family:Arial,"Yu Gothic",sans-serif}.page{width:2400px;margin:auto;padding:44px 50px 34px}.kicker{font-size:16px;letter-spacing:.14em;color:#82796F;margin:0 0 12px}h1{font-size:38px;margin:0 0 12px;font-weight:650}.intro{font-size:18px;color:#736C64;margin:0}.head{margin-bottom:30px}.grid{display:grid;grid-template-columns:repeat(6,1fr);gap:18px}.card{background:white;border:1px solid #E2DDD5;border-radius:12px;padding:16px 12px 18px;text-align:center;min-height:410px}.card.reference{border:2px solid #A7A39D;padding:15px 11px 17px}.top{display:flex;align-items:center;justify-content:space-between;font-size:15px;line-height:20px;padding:0 8px;color:#93897F}.kind{font-size:12px;letter-spacing:.08em;color:#AAA198}.box{width:310px;height:310px;position:relative;overflow:hidden;margin:0 auto}.box img{position:absolute;max-width:none;width:310px;height:310px;left:50%;top:50%;transform:translate(-50%,-50%)}.reference .box img{width:418px;height:418px}.name{font-size:22px;line-height:1.3;margin:-2px 0 8px;font-weight:650}.label{font-size:15px;line-height:1.4;color:#797169;margin:0}.foot{display:flex;justify-content:space-between;border-top:1px solid #DBD4CB;margin-top:25px;padding-top:17px;font-size:15px;color:#827A70}a{color:inherit;text-decoration:none}
`;
function card(c) {
  const kind = c.kind==='reference'?'REFERENCE':c.kind==='color'?'COLOR':c.kind==='background'?'METAL GROUND':'METAL LETTER';
  const src = c.src || `${c.id}-${c.slug}.svg`;
  return `<article class="card ${c.kind==='reference'?'reference':''}"><div class="top"><span>${c.id}</span><span class="kind">${kind}</span></div><a href="${src}"><div class="box"><img src="${src}" alt="${c.name}"></div></a><h2 class="name">${c.name}</h2><p class="label">${c.label || c.note}</p></article>`;
}

(async()=>{
  fs.copyFileSync(path.join(__dirname,'wordpress-official-wmark-reference.png'),path.join(root,ref.src));
  for (const c of originals) {
    for (const ext of ['svg','png']) fs.copyFileSync(path.join(previous,`${c.id}-${c.slug}.${ext}`),path.join(root,`${c.id}-${c.slug}.${ext}`));
  }
  for (const c of additions) {
    const svg = finishSvg(c);
    fs.writeFileSync(path.join(root,`${c.id}-${c.slug}.svg`),svg);
    await sharp(Buffer.from(svg)).resize(1000,1000).png().toFile(path.join(root,`${c.id}-${c.slug}.png`));
  }
  const cards = [ref,...originals,...additions];
  const html = `<!doctype html><html lang="ja"><meta charset="utf-8"><title>RecastPress — 色と金属仕上げの比較</title><style>${css}</style><main class="page"><header class="head"><p class="kicker">RECASTPRESS · COLOUR & PLATING STUDY / CONCEPT 14</p><h1>色と金属仕上げ — 一覧比較</h1><p class="intro">左上：WordPress公式ロゴ ／ カラー10案 ＋ 金属13案。M4–M10はベタ文字、M6以降はメッキ感を強めた追加案。</p></header><section class="grid">${cards.map(card).join('')}</section><footer class="foot"><span>ロゴ円の直径を揃えて比較。RecastPressの輪郭・切り欠きは全案共通。「ベタ」は文字・円環・タレを同じ単色に。</span><span>RECASTPRESS · 2026.10.26</span></footer></main></html>`;
  fs.writeFileSync(path.join(root,'all-options.html'),html);
  fs.writeFileSync(path.join(root,'options.json'),JSON.stringify({base:'concept-v14',reference:ref,colors:palette.colors,metals:[...palette.finishes,...additions.map(({stops,...c})=>c)]},null,2));
  fs.writeFileSync(path.join(root,'README.md'),'# RecastPress colour and plating comparison\n\nOne sheet: official WordPress reference at top left, 10 flat background colours, and 13 metal treatments.\n\nOpen all-options.html. Each card links to the standalone SVG or official reference PNG. PNG exports are included.\n\nM1–M3 preserve the earlier material options. M4–M10 add metal grounds with solid white or dark lettering. M6–M13 introduce harder chrome, black chrome, copper plating, gold plating and black nickel. Solid paint is applied to letter, annular band and stream together.\n\nAll RecastPress marks preserve concept v14 glyph, stream, cut and circular-frame geometry. WordPress is an unchanged official reference from https://wordpress.org/about/logos/ .\n');
  console.log(JSON.stringify({directory:root,cards:cards.length,colors:palette.colors.length,metals:13}));
})();
