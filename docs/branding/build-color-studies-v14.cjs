const fs = require('fs');
const path = require('path');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const root = path.join(__dirname, 'color-studies-v14');
fs.mkdirSync(root, { recursive: true });
const source = fs.readFileSync(path.join(__dirname, 'recastpress-logo-concept-v14.svg'), 'utf8');
const originalDefs = source.match(/<defs>([\s\S]*?)<\/defs>/)[1];
const glyph = originalDefs.match(/<path id="normal-r"[^>]+\/>/)[0];
const stream = source.match(/<path id="continuous-injection"[^>]+\/>/)[0];
const annulus = '<path d="M 923 500 A 423 423 0 1 0 77 500 A 423 423 0 1 0 923 500 Z M 874.4 500 A 374.4 374.4 0 1 0 125.6 500 A 374.4 374.4 0 1 0 874.4 500 Z" fill-rule="evenodd"/>';
const letter = '<g clip-path="url(#within-white-band)"><use href="#normal-r" mask="url(#single-circular-cut)"/></g>';

const colors = [
  { id:'01', slug:'burnt-orange', name:'バーントオレンジ', hex:'#8F3F1F', note:'深い橙。鋳造・火・活字の温かさ。' },
  { id:'02', slug:'dark-amber', name:'ダークアンバー', hex:'#80551E', note:'落ち着いた琥珀色。古い真鍮の気配。' },
  { id:'03', slug:'brick-red', name:'ブリックレッド', hex:'#943E35', note:'赤茶のレンガ色。力強く、親しみやすい。' },
  { id:'04', slug:'oxblood', name:'オックスブラッド', hex:'#681E2D', note:'黒に近い深紅。重厚な出版物の印象。' },
  { id:'05', slug:'dark-plum', name:'ダークプラム', hex:'#4C2D46', note:'深い紫。古典的で、少し独特な佇まい。' },
  { id:'06', slug:'ink-navy', name:'インクネイビー', hex:'#1E3048', note:'濃いインクの青。端正で安定感がある。' },
  { id:'07', slug:'petrol', name:'ペトロール', hex:'#1E5059', note:'青緑。現行の青から自然につながる。' },
  { id:'08', slug:'forest', name:'フォレスト', hex:'#244B3B', note:'濃緑。静かな工房や書庫の印象。' },
  { id:'09', slug:'espresso', name:'エスプレッソ', hex:'#49372F', note:'濃い茶。紙・木・革となじむ温かさ。' },
  { id:'10', slug:'graphite', name:'グラファイト', hex:'#30353B', note:'青みの墨色。道具としての簡潔さ。' },
];

const gradient = (id, stops, x1='0',y1='0',x2='1000',y2='1000') => `<linearGradient id="${id}" gradientUnits="userSpaceOnUse" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}">${stops.map(([o,c])=>`<stop offset="${o}" stop-color="${c}"/>`).join('')}</linearGradient>`;
const mercury = gradient('metal', [
  ['0%','#FCFEFF'],['14%','#ADBFCB'],['28%','#F5FCFF'],['38%','#758B9A'],
  ['46%','#D4E4EE'],['49%','#FFFFFF'],['52%','#758A99'],['62%','#DCE8F0'],
  ['73%','#FAFDFF'],['84%','#8A9DAA'],['100%','#E7EFF4']
], '150','20','850','950');
const bronze = gradient('metal', [
  ['0%','#F8D4A0'],['20%','#A36B3B'],['39%','#E9B477'],['49%','#F6D4A1'],
  ['55%','#946038'],['73%','#CD965F'],['100%','#EAC394']
], '0','70','900','930');
const gunmetal = gradient('ground', [
  ['0%','#1B2730'],['27%','#495B68'],['44%','#26343F'],['53%','#5D6E7C'],
  ['65%','#34434E'],['100%','#17222B']
], '0','0','0','1000');
const grain = '<pattern id="grain" width="29" height="5" patternUnits="userSpaceOnUse"><path d="M0 1 H29" stroke="#fff" stroke-opacity=".12" stroke-width=".65"/><path d="M0 3.5 H29" stroke="#000" stroke-opacity=".10" stroke-width=".55"/></pattern>';

const finishes = [
  { id:'M1',slug:'mercury-mirror',name:'水銀・鏡面シルバー',hex:'#111C25',note:'R・円環・タレを、冷たい鏡面の銀に。',kind:'foreground',gradient:mercury },
  { id:'M2',slug:'brushed-gunmetal',name:'ヘアライン・ガンメタル',hex:'METALLIC GROUND',note:'円内の地を、細い研磨線のある金属に。',kind:'background',gradient:gunmetal },
  { id:'M3',slug:'satin-bronze',name:'サテン・ブロンズ',hex:'#261E1B',note:'R・円環・タレを、落ち着いた銅合金色に。',kind:'foreground',gradient:bronze },
];

function foreground(paint, glyphId='normal-r') {
  return `<g fill="${paint}">${annulus}${letter.replace('href="#normal-r"',`href="#${glyphId}"`)}${stream.replace('fill="#fff"',`fill="${paint}"`)}</g>`;
}
function metalSvg(item) {
  let defs = originalDefs;
  if (item.kind === 'foreground') defs = defs.replace(glyph,glyph.replace('stroke="#fff"','stroke="url(#metal)"'));
  const bg = item.kind === 'background' ? 'url(#ground)' : item.hex;
  const paint = item.kind === 'foreground' ? 'url(#metal)' : '#fff';
  const content = `<rect width="1000" height="1000" fill="#fff"/><circle cx="500" cy="500" r="450" fill="${bg}"/>`;
  const texture = item.kind === 'background' ? '<circle cx="500" cy="500" r="450" fill="url(#grain)"/>' : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000" width="1000" height="1000" role="img"><title>RecastPress ${item.id} ${item.name}</title><desc>Exact concept v14 geometry. ${item.note}</desc><defs>${defs}${item.gradient}${grain}</defs>${content}${texture}${foreground(paint)}</svg>`;
}

const htmlHeader = (title,kicker,subtitle) => `<!doctype html><html lang="ja"><meta charset="utf-8"><title>${title}</title><style>
*{box-sizing:border-box}body{margin:0;background:#F6F4F0;color:#242A2F;font-family:Arial,"Yu Gothic",sans-serif}.page{width:1800px;margin:auto;padding:43px 54px 36px}.kicker{font-size:14px;letter-spacing:.14em;color:#7A746E;margin:0 0 13px}h1{font-size:34px;line-height:1.2;margin:0 0 12px}.intro{font-size:16px;margin:0;color:#6A6763}.header{margin-bottom:30px}.grid{display:grid;grid-template-columns:repeat(5,1fr);gap:20px}.card{background:#fff;border:1px solid #E6E1DA;border-radius:12px;padding:19px 17px 20px;text-align:center;min-height:393px}.number{font-size:14px;letter-spacing:.07em;color:#90877F;display:block;text-align:left}.logo{width:278px;height:278px;display:block;margin:-4px auto -5px}.name{font-size:20px;font-weight:600;line-height:1.3;margin:0 0 7px}.code{font-size:14px;letter-spacing:.06em;color:#79736D;margin:0 0 9px}.note{font-size:13px;color:#716D67;line-height:1.6;margin:0}.foot{font-size:13px;color:#878078;border-top:1px solid #DDD7CE;padding-top:18px;margin-top:26px;display:flex;justify-content:space-between}a{color:inherit;text-decoration:none}.materials .grid{grid-template-columns:repeat(3,1fr);gap:28px}.materials .card{padding:22px 24px 26px;min-height:590px}.materials .logo{width:435px;height:435px;margin:0 auto 2px}.materials .name{font-size:25px}.materials .note{font-size:16px}.materials .code{font-size:13px;margin-bottom:11px}
</style><main class="page"><header class="header"><p class="kicker">${kicker}</p><h1>${title}</h1><p class="intro">${subtitle}</p></header>`;
function card(c) {
  return `<article class="card"><span class="number">${c.id}</span><a href="${c.id}-${c.slug}.svg"><img class="logo" src="${c.id}-${c.slug}.svg" alt="${c.name}"></a><h2 class="name">${c.name}</h2><p class="code">${c.hex}</p><p class="note">${c.note}</p></article>`;
}

(async () => {
  for (const c of colors) {
    const svg = source.replaceAll('#24627a',c.hex).replace('RecastPress logo concept 14',`RecastPress ${c.id} ${c.name}`);
    fs.writeFileSync(path.join(root,`${c.id}-${c.slug}.svg`),svg);
    await sharp(Buffer.from(svg)).resize(800,800).png().toFile(path.join(root,`${c.id}-${c.slug}.png`));
  }
  for (const c of finishes) {
    const svg = metalSvg(c);
    fs.writeFileSync(path.join(root,`${c.id}-${c.slug}.svg`),svg);
    await sharp(Buffer.from(svg)).resize(1000,1000).png().toFile(path.join(root,`${c.id}-${c.slug}.png`));
  }
  const colorHtml = htmlHeader('背景色の検討 — 10案','RECASTPRESS · COLOUR STUDY / CONCEPT 14','円内の地色と外周色を変更。字・円環・タレは白、形はすべて同じです。') + `<section class="grid">${colors.map(card).join('')}</section><footer class="foot"><span>暖色 01–04 ／ 紫 05 ／ 寒色 06–08 ／ 無彩色寄り 09–10</span><span>RECASTPRESS · 2026.10.26</span></footer></main></html>`;
  const finishHtml = htmlHeader('金属の仕上げ — 3案','RECASTPRESS · MATERIAL STUDY / CONCEPT 14','鏡面・研磨された地・サテン調の銅。背景10色とは別の仕上げ案です。').replace('<main class="page">','<main class="page materials">') + `<section class="grid">${finishes.map(card).join('')}</section><footer class="foot"><span>形と切り欠きは第14案と同一。金属の反射・研磨線をSVGで表現。</span><span>RECASTPRESS · 2026.10.26</span></footer></main></html>`;
  fs.writeFileSync(path.join(root,'background-colors.html'),colorHtml);
  fs.writeFileSync(path.join(root,'metallic-finishes.html'),finishHtml);
  fs.writeFileSync(path.join(root,'palette.json'),JSON.stringify({base:'concept-v14',colors,finishes:finishes.map(({gradient,...c})=>c)},null,2));
  fs.writeFileSync(path.join(root,'README.md'),'# RecastPress colour and finish studies\n\n10 flat background colours and 3 metal finishes. All based on concept v14.\n\nOpen background-colors.html and metallic-finishes.html for the visual sheets. Each mark links to its standalone SVG. Corresponding PNG files are included.\n\nFor M1 and M3, the letter, annular band and hanging stream receive the metal finish. M2 applies its brushed gunmetal finish to the circular ground, retaining the white mark. No glyph outline, cut mask, stream outline or ring proportion is changed. The SVG files preserve editable geometry and require no installed font.\n');
  console.log(JSON.stringify({directory:root,backgroundColors:colors.length,metalFinishes:finishes.length}));
})();
