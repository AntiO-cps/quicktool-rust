// Development only: captures the original implementation as a differential oracle.
const fs=require('node:fs'), path=require('node:path'), vm=require('node:vm'), crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'), original=path.resolve(root,'..','src');
const C=require('../reference/core-utils.js');
const source=fs.readFileSync(path.join(original,'core-utils.js'),'utf8');
function evaluate(text){return vm.runInNewContext('('+text+')',{Math});}
const units=evaluate(source.match(/const UNIT_CATEGORIES = (\{[\s\S]*?\n  \});/)[1]);
fs.writeFileSync(path.join(root,'assets','units.json'),JSON.stringify(units,null,2));
const renderer=fs.readFileSync(path.join(original,'renderer.js'),'utf8');
const commands=[...renderer.matchAll(/\{id:'([^']+)',name:'([^']+)',icon:'([^']+)',primaryAlias:'([^']+)'[^\n]*?aliases:(\[[^\]]*\])/g)]
 .map(m=>({id:m[1],name:m[2],icon:m[3],primaryAlias:m[4],aliases:evaluate(m[5])}));
fs.writeFileSync(path.join(root,'assets','commands.json'),JSON.stringify(commands,null,2));
const main=fs.readFileSync(path.join(original,'main.js'),'utf8');
fs.writeFileSync(path.join(root,'assets','defaults.json'),JSON.stringify(evaluate(main.match(/const DEFAULT_SETTINGS = (\{[\s\S]*?\n\});/)[1]),null,2));
const cases=[];
function add(operation,args){try{cases.push({operation,args,result:C[operation](...args)});}catch(e){cases.push({operation,args,error:true});}}
for(const input of ['0xFF','2:1010','-0b100','0','16:ff','1_000','+42','2 : 1010','9999999999999999999999999999999999','xyz','0x','-','2:2'])add('baseResults',[input]);
for(const input of ['ＡＢＣ１２３！　','ABC 123!','中😀&<>"\'','', '\\u4E2D','%E4%B8%AD'])for(const operation of ['toFullWidth','toHalfWidth','percentEncode','percentDecode','unicodeEscapeEncode','htmlEncode'])add(operation,[input]);
for(const [a,b] of [['visual','Visual'],['vis','Visual Studio Code'],['vsc','Visual Studio Code'],['zzz','Visual Studio Code'],['cafe','Café'],['中','中国'],['foo bar','foo bar baz'],['😀','😀x'],['ft','Foo-test']])add('fuzzySearch',[a,b]);
for(const input of ['2(1+2)','(2+3)(3+4)','2pi','sqrt(9)+max(2,5)','1,000.5+0.5','5!','3!+2','(2+3)!','5!^2','0!','-4!','2.5!','171!','2^3^2','-2^2','sin(pi/2)','1/0'])add('calculateExpression',[input,'en-US']);
add('calculateExpression',['1,5+2,5','de-DE']);
for(const input of ['10 km to mi','32 F to C','1 GB','1 kg to g','1 m² to ft²','1 rad to deg','100 C','1 kWh to J','1 ms','2.5e3 mm to m'])add('unitResults',[input]);
fs.writeFileSync(path.join(root,'tests','oracle.json'),JSON.stringify(cases,null,2));
const files=[];
function visit(dir){for(const entry of fs.readdirSync(dir,{withFileTypes:true})){const f=path.join(dir,entry.name);if(entry.isDirectory())visit(f);else files.push({path:path.relative(original,f).replaceAll('\\','/'),bytes:fs.statSync(f).size,sha256:crypto.createHash('sha256').update(fs.readFileSync(f)).digest('hex')});}}
visit(original);fs.writeFileSync(path.join(root,'reference','manifest.json'),JSON.stringify({capturedAt:new Date().toISOString(),files},null,2));
const icons=require('../../node_modules/@iconify-json/mdi/icons.json');
fs.writeFileSync(path.join(root,'assets','icons.json'),JSON.stringify({width:icons.width,height:icons.height,icons:Object.fromEntries(commands.map(c=>c.icon.replace('mdi:','')).filter(n=>icons.icons[n]).map(n=>[n,icons.icons[n]]))}));
console.log(`Captured ${commands.length} commands, ${cases.length} oracle cases, ${files.length} source hashes.`);
const effects=require('../reference/pin-effects.js');
let seed=0x51475549;function next(){seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/2**32;}
const pixelCases=[];
for(let n=0;n<48;n++){const filters={brightness:next()*300-150,contrast:next()*150-50,saturation:next()*200-100,exposure:next()*6-3,hue:next()*360-180,colorize:n%3===0,colorizeSaturation:next()*100};const input=Array.from({length:256*4},()=>Math.floor(next()*256));pixelCases.push({filters,input,output:[...effects.processPixels(Uint8ClampedArray.from(input),filters)]});}
fs.writeFileSync(path.join(root,'tests','pixels-oracle.json'),JSON.stringify(pixelCases));
const colorCases=['#1','#12','#123','#1234','#7c3aed','#000000','#ffffff','#00223377','red','transparent','rgb(255 0 0 / 50%)','rgba(12,34,56,.8)','hsl(220 40% 50%)','hsv(120 80% 60%)','hsi(0 100% 33.333%)','yuv(0.299 -0.14713 0.615)','color(display-p3 1 0 0 / .5)','ycbcr709(0.5 0.5 0.5)','lab(40 10 -20)','oklch(.6 .2 140)','xyz(20 30 40)','cmyk(10% 20% 30% 40%)'].map(input=>({input,colors:C.parseColorCandidates(input,require('../assets/data/css-colors.json')).map(c=>({color:c.color,hex:C.rgbaHex(c.color,true),formats:C.colorFormats(c.color).map(({title,value})=>({title,value}))}))}));
fs.writeFileSync(path.join(root,'tests','colors-oracle.json'),JSON.stringify(colorCases,null,2));
const DP=require('../reference/date-parser.js');const dateSource=fs.readFileSync(path.join(root,'reference/date-parser.js'),'utf8').replace('parse,recognized,monthNames','parse,recognized,monthNames,monthEntries:()=>[...months]');const dateContext={module:{exports:{}},Intl,Date,Map};vm.runInNewContext(dateSource,dateContext);dateContext.module.exports.monthNames();fs.writeFileSync(path.join(root,'assets/months.json'),JSON.stringify(dateContext.module.exports.monthEntries()));
const dateCases=[];for(const input of ['2026年1月20日','2020年','jan 2026','Jan/2026','2026/Jan/20','20 января 2026','2026-20-01','26-01-20','2026/20/01','20 janvier 2026','20 Januar 2026','20 de marzo de 2026','2026년 1월 20일','٢٠٢٦/٢٠/٠١','2024-02-29','2026年2月29日','2026-13-32','2026-01-20 25:00','2026-02-30T12:00Z','https://jan2026.com','jan 2026@test.com','2026-01-20T12:00:00+08:00','20260120','260120','2026-01'])for(const order of ['YMD','YDM','DMY','MDY']){const p=DP.parse(input,order);dateCases.push({input,order,result:p?{millis:p.date.getTime(),format:p.format,precision:p.precision,recognized:p.recognized,alternatives:p.alternatives.map(a=>a.recognized)}:null});}fs.writeFileSync(path.join(root,'tests/dates-oracle.json'),JSON.stringify(dateCases,null,2));
