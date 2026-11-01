const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const sharp = require('C:/Users/nodew/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');

const repository = path.resolve(__dirname,'../..');
const root = path.join(repository,'assets','branding','recastpress');
const selectedFile = path.join(__dirname,'recastpress-logo-concept-v16.svg');
const sizes = [16,20,24,32,40,48,64,96,128,180,192,256,384,512,1024,2048];
const color = '#79332B';
const foreground = '#FFFFFF';
const files = [];
for (const dir of ['', 'png/transparent','png/white','web','preview']) fs.mkdirSync(path.join(root,dir),{recursive:true});

function record(relative,kind,extra={}) {
  const buffer = fs.readFileSync(path.join(root,relative));
  files.push({file:relative,kind,bytes:buffer.length,sha256:crypto.createHash('sha256').update(buffer).digest('hex'),...extra});
}
function svgGeometry(svg,id) {
  return svg.match(new RegExp(`<path id="${id}" d="([^"]+)`))[1];
}
function icoFromPngs(images) {
  const header = Buffer.alloc(6+images.length*16);
  header.writeUInt16LE(1,2);
  header.writeUInt16LE(images.length,4);
  let offset=header.length;
  images.forEach(({size,data},i)=>{
    const e=6+i*16;
    header[e]=size===256?0:size;
    header[e+1]=size===256?0:size;
    header.writeUInt16LE(1,e+4);
    header.writeUInt16LE(32,e+6);
    header.writeUInt32LE(data.length,e+8);
    header.writeUInt32LE(offset,e+12);
    offset+=data.length;
  });
  return Buffer.concat([header,...images.map(i=>i.data)]);
}

(async()=>{
  const source = fs.readFileSync(selectedFile,'utf8');
  if (!source.includes('fill="'+color+'"')) throw new Error('Wrong adopted candidate colour.');
  const finalWhite = source
    .replace(/<title id="title">[\s\S]*?<\/title>/,'<title id="title">RecastPress</title>')
    .replace(/<desc id="desc">[\s\S]*?<\/desc>/,'<desc id="desc">RecastPress adopted brick-red logo. A Roman serif R, a hanging viscous stream, and a shared circular cut with a square-pixel edge. The right diagonal leg is separated slightly from the circular band.</desc>')
    .replace(/<metadata>[\s\S]*?<\/metadata>/,'<metadata>Adopted 2026-10-26. Candidate 03-D1, Brick Red Dark. Brand colour #79332B, RGB 121 51 43. Foreground #FFFFFF. Concept v16 geometry: both feet end at y=820, the midpoint of the preceding upright and diagonal lengths; the upright is raised, the diagonal is lowered, and clearance from the band is retained on the right. Glyph is outlined; no installed font is required. ViewBox 0 0 1000 1000, outer circle radius 450 with 50 units of canvas margin.</metadata>');
  const defsEnd = finalWhite.indexOf('</defs>')+'</defs>'.length;
  const body = finalWhite.slice(defsEnd);
  const canvas = '<rect width="1000" height="1000" fill="#fff"/>';
  if (!body.includes(canvas)) throw new Error('Expected one white canvas rect after the definitions.');
  const finalTransparent = finalWhite.slice(0,defsEnd)+body.replace(canvas,'');
  for (const id of ['normal-r','continuous-injection']) {
    if (svgGeometry(source,id)!==svgGeometry(finalTransparent,id)) throw new Error('Approved logo geometry changed.');
  }
  fs.writeFileSync(path.join(root,'recastpress.svg'),finalTransparent);
  fs.writeFileSync(path.join(root,'recastpress-white-bg.svg'),finalWhite);
  record('recastpress.svg','svg',{background:'transparent',viewBox:'0 0 1000 1000'});
  record('recastpress-white-bg.svg','svg',{background:'#FFFFFF',viewBox:'0 0 1000 1000'});

  for (const size of sizes) {
    for (const [background,svg] of [['transparent',finalTransparent],['white',finalWhite]]) {
      const relative=`png/${background}/recastpress-${size}.png`;
      let image=sharp(Buffer.from(svg),{density:Math.ceil(72*Math.max(1,size/1000))}).resize(size,size);
      if(background==='white') image=image.flatten({background:'#FFFFFF'});
      await image.png({compressionLevel:9,adaptiveFiltering:true}).toFile(path.join(root,relative));
      const metadata=await sharp(path.join(root,relative)).metadata();
      if(metadata.width!==size || metadata.height!==size) throw new Error('Incorrect PNG dimensions: '+relative);
      const corner=await sharp(path.join(root,relative)).ensureAlpha().extract({left:0,top:0,width:1,height:1}).raw().toBuffer();
      if(background==='transparent' && corner[3]!==0) throw new Error('Missing transparency: '+relative);
      if(background==='white' && corner.some(v=>v!==255)) throw new Error('White canvas is not opaque white: '+relative);
      record(relative,'png',{width:size,height:size,background});
    }
  }
  const sample=await sharp(path.join(root,'png/transparent/recastpress-256.png')).extract({left:128,top:44,width:1,height:1}).removeAlpha().raw().toBuffer();
  if(sample[0]!==121 || sample[1]!==51 || sample[2]!==43) throw new Error('Adopted brand RGB was not preserved.');

  fs.copyFileSync(path.join(root,'recastpress.svg'),path.join(root,'web/favicon.svg'));
  record('web/favicon.svg','svg',{background:'transparent'});
  for (const [from,to,size] of [
    ['png/white/recastpress-180.png','web/apple-touch-icon.png',180],
    ['png/transparent/recastpress-192.png','web/icon-192.png',192],
    ['png/transparent/recastpress-512.png','web/icon-512.png',512],
  ]) {
    fs.copyFileSync(path.join(root,from),path.join(root,to));
    record(to,'png',{width:size,height:size});
  }
  const icoSizes=[16,24,32,48,64,256];
  const ico=icoFromPngs(icoSizes.map(size=>({size,data:fs.readFileSync(path.join(root,`png/transparent/recastpress-${size}.png`))})));
  if(ico.readUInt16LE(2)!==1 || ico.readUInt16LE(4)!==icoSizes.length) throw new Error('Invalid ICO header.');
  for(let i=0;i<icoSizes.length;i++){
    const e=6+i*16,offset=ico.readUInt32LE(e+12),length=ico.readUInt32LE(e+8);
    const m=await sharp(ico.subarray(offset,offset+length)).metadata();
    if(m.width!==icoSizes[i] || m.height!==icoSizes[i]) throw new Error('Invalid ICO image size.');
  }
  fs.writeFileSync(path.join(root,'web/favicon.ico'),ico);
  record('web/favicon.ico','ico',{sizes:icoSizes});
  const webmanifest={name:'RecastPress',short_name:'RecastPress',theme_color:color,background_color:'#FFFFFF',icons:[{src:'icon-192.png',sizes:'192x192',type:'image/png',purpose:'any'},{src:'icon-512.png',sizes:'512x512',type:'image/png',purpose:'any'}]};
  fs.writeFileSync(path.join(root,'web/site.webmanifest'),JSON.stringify(webmanifest,null,2));
  record('web/site.webmanifest','manifest');

  const brand={name:'RecastPress',status:'adopted',adopted:'2026-10-26',selectedCandidate:'03-D1',selectedName:'ブリックレッド・暗め',primary:{hex:color,rgb:[121,51,43]},foreground:{hex:foreground,rgb:[255,255,255]},svg:'recastpress.svg',svgWhiteBackground:'recastpress-white-bg.svg',pngSizes:sizes,source:'docs/branding/recastpress-logo-concept-v16.svg',geometry:'concept-v16',originalColourCandidate:'docs/branding/warm-dark-study-v3/03-D1-brick-red.svg',canvas:{viewBox:[0,0,1000,1000],badgeCenter:[500,500],badgeRadius:450,margin:50}};
  fs.writeFileSync(path.join(root,'brand.json'),JSON.stringify(brand,null,2));
  fs.writeFileSync(path.join(root,'manifest.json'),JSON.stringify({brand:'RecastPress',colour:color,files},null,2));
  const shortlistPath=path.join(__dirname,'current-candidates.json');
  const shortlist=JSON.parse(fs.readFileSync(shortlistPath,'utf8'));
  shortlist.status='adopted';
  shortlist.currentGeometry='concept-v16';
  shortlist.adopted={candidate:'03-D1',name:'ブリックレッド・暗め',hex:color,date:'2026-10-26',geometry:'concept-v16',sourceSVG:'docs/branding/recastpress-logo-concept-v16.svg',assetDirectory:'assets/branding/recastpress',svg:'assets/branding/recastpress/recastpress.svg'};
  fs.writeFileSync(shortlistPath,JSON.stringify(shortlist,null,2));

  const readme=`# RecastPress ロゴデータ\n\n採用案：ブリックレッドの中央案（03-D1、暗め）。採用日：2026-10-26。\n\n- 地色・外周色：**${color}** / RGB **121, 51, 43**\n- 文字・円環・タレ：**#FFFFFF**\n- 形：縦足を上げ、斜めの足を下げて、両方を中間の高さに揃えた第16案。右足と円枠の隙間は維持しています。\n\n## SVG\n\n- **recastpress.svg**：標準版。円の外側が透過。文字と円環の白は不透明です。\n- **recastpress-white-bg.svg**：白背景付き。\n\n文字は輪郭データになっており、フォントのインストールは不要です。\n\n## PNG\n\n- **png/transparent/**：円の外側が透過。\n- **png/white/**：白い背景付き。\n- 各フォルダーに **${sizes.join(' / ')} px** の正方形PNGを収録。\n\nSVGとPNGは同じ1000×1000の構図を使用し、円の外側に5%ずつ余白があります。ファイルサイズは円だけの直径ではなく、余白を含めた画像全体の寸法です。\n\n## Web用\n\n- **web/favicon.svg**：SVG favicon。\n- **web/favicon.ico**：16 / 24 / 32 / 48 / 64 / 256pxを内包。\n- **web/apple-touch-icon.png**：180px、白背景。\n- **web/icon-192.png**、**web/icon-512.png**：透過PNG。\n- **web/site.webmanifest**：アイコンと採用色の定義。\n\nこれらは配置用のデータです。アプリやサイトへの組み込みは行っていません。\n\n## 確認用・色定義\n\n- **preview/size-preview.html**、**preview/size-preview.png**：サイズごとの見え方。\n- **brand.json**：採用案と色・サイズの定義。\n- **manifest.json**：収録アセットの寸法、背景、SHA-256。\n\n採用色の元案：docs/branding/warm-dark-study-v3/03-D1-brick-red.svg\n形の改訂元：docs/branding/recastpress-logo-concept-v16.svg\n`;
  fs.writeFileSync(path.join(root,'README.ja.md'),readme);

  const samples=[16,24,32,48,64,96,128,192];
  const preview=`<!doctype html><html lang="ja"><meta charset="utf-8"><title>RecastPress 採用ロゴとサイズ確認</title><style>*{box-sizing:border-box}body{margin:0;background:#F5F3EF;color:#302B28;font-family:Arial,"Yu Gothic",sans-serif}.page{width:1400px;padding:40px 45px}.kicker{font-size:13px;letter-spacing:.12em;color:#8B7B71;margin:0 0 12px}h1{font-size:32px;margin:0 0 12px}.sub{font-size:16px;color:#7B6C63;margin:0 0 28px}.layout{display:grid;grid-template-columns:420px 1fr;gap:34px}.hero{background:#fff;border:1px solid #DDD5CE;border-radius:12px;padding:18px}.hero .checker{width:382px;height:382px;background-color:white;background-image:linear-gradient(45deg,#ECE9E5 25%,transparent 25%),linear-gradient(-45deg,#ECE9E5 25%,transparent 25%),linear-gradient(45deg,transparent 75%,#ECE9E5 75%),linear-gradient(-45deg,transparent 75%,#ECE9E5 75%);background-size:20px 20px;background-position:0 0,0 10px,10px -10px,-10px 0px}.hero img{width:382px;height:382px}.hero h2{font-size:22px;margin:22px 0 10px}.hero p{font-size:14px;line-height:1.9;color:#857368;margin:0}.samples{display:grid;grid-template-columns:repeat(4,1fr);gap:12px}.sample{background:white;border:1px solid #DED7D0;border-radius:10px;text-align:center;padding:9px 3px 13px}.sample .area{height:197px;display:flex;align-items:center;justify-content:center}.sample img{max-width:none}.sample p{font-size:14px;margin:8px 0 0;color:#887466}.foot{font-size:13px;border-top:1px solid #DCD3C9;margin-top:24px;padding-top:17px;color:#8D7C70}</style><main class="page"><p class="kicker">RECASTPRESS · ADOPTED LOGO / 03-D1 · REV.16</p><h1>ブリックレッド — 採用版</h1><p class="sub">#79332B ／ SVG・透過PNG・白背景PNG・favicon</p><div class="layout"><section class="hero"><div class="checker"><img src="../png/transparent/recastpress-512.png"></div><h2>RecastPress</h2><p>地色 RGB 121, 51, 43<br>円の外側は透過、字・円環・タレは白。<br>縦足を上げ、斜めの足を下げて揃えています。</p></section><section class="samples">${samples.map(n=>`<article class="sample"><div class="area"><img src="../png/transparent/recastpress-${n}.png" width="${n}" height="${n}"></div><p>${n} × ${n} px</p></article>`).join('')}</section></div><p class="foot">右側は画像全体を実寸表示。配布PNGは16–2048pxの16サイズ、白背景版も同梱。SVGはフォント不要です。</p></main></html>`;
  fs.writeFileSync(path.join(root,'preview/size-preview.html'),preview);
  console.log(JSON.stringify({directory:root,adoptedCandidate:'03-D1',hex:color,svg:2,pngSizes:sizes,pngExports:sizes.length*2,icoSizes,verifiedAssetFiles:files.length,sourceGeometry:'unchanged',transparentCorners:'verified',whiteCorners:'verified',brandRgb:'verified'}));
})();
