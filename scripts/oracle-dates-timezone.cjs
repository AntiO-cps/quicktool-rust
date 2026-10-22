// Rebuild with original runtime: ELECTRON_RUN_AS_NODE=1 node_modules/electron/dist/electron.exe rust-rewrite/scripts/oracle-dates-timezone.cjs
// Development-only oracle: executes the original DateParser/Core and exact renderer wrappers.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const root=path.resolve(__dirname,'../..');
const Core=require(path.join(root,'src/core-utils.js'));
const renderer=fs.readFileSync(path.join(root,'src/renderer.js'),'utf8');
const wrappers=renderer.slice(renderer.indexOf('function parseDateInput('),renderer.indexOf('function colorToolResults('));
const custom='Local=YYYY-MM-DD HH:mm:ss.SSS WW W X x\nUTC=UTC:YYYY-MM-DD HH:mm:ss.SSS WW W';
const zones=['Asia/Shanghai','America/New_York','Europe/Berlin','Australia/Lord_Howe','Pacific/Apia','Africa/Casablanca','Africa/Cairo','America/Asuncion','America/Coyhaique','Asia/Gaza','UTC'];
const inputs=['1991-06-01 12:34:56','1991-01-01 00:00:00','1991-04-14 02:30:00','1991-09-15 01:30:00','1991年6月1日 12:34:56','1991년 6월 1일 12:34:56','2024.01','1900-01-01 00:00:00','2024-03-10 02:30:00','2024-11-03 01:30:00','2024-03-31 02:30:00','2024-10-27 02:30:00','2024-10-06 02:15:00','2011-12-30 12:00:00','2040-07-01 12:00:00','2026-01-01T00:15:00+14:00','1991-06-01T12:34+0800','1969-12-31T23:59:59.999Z','0001-01-01T00:00:00Z','0675566400','ft: 123456789012345678','ft: -1','ft: 1','ft: 116444735999999999','oa: 33389.5','exc: 45658.75','exf: 40000.25','oa: NaN','ft: 1.5','2024/01/02','20240102','2024年1月2日'];
// Convert proleptic Gregorian years without asking JS Date to represent an out-of-TimeClip year.
function daysFromCivil(y,m,d){y-=m<=2?1:0;const era=Math.floor(y/400),yoe=y-era*400,doy=Math.floor((153*(m+(m>2?-3:9))+2)/5)+d-1;return era*146097+yoe*365+Math.floor(yoe/4)-Math.floor(yoe/100)+doy-719468;}
for(const year of [10000,100000,-100000,275000,-275000]){const ms=daysFromCivil(year,7,1)*86400000+43200000;inputs.push('ft: '+(BigInt(ms)*10000n+116444736000000000n));inputs.push('oa: '+(ms/86400000+25569));}
for(const ms of [8640000000000000,-8640000000000000,8640000000000001,-8640000000000001])inputs.push('ft: '+(BigInt(ms)*10000n+116444736000000000n));
inputs.push('2100-07-01 12:00:00','2400-07-01 12:00:00','2026-04-01 12:00:00','9999-12-31T23:59:59-23:59');
const output=[];
for(const zone of zones){process.env.TZ=zone;delete require.cache[require.resolve(path.join(root,'src/date-parser.js'))];const DateParser=require(path.join(root,'src/date-parser.js'));const settings={dateOrder:'YMD',timeCustomFormats:custom};const ctx=vm.createContext({Core,DateParser,settings,Date});vm.runInContext(wrappers,ctx);
for(const input of inputs){let result=null;try{const parsed=ctx.parseDateInput(input);if(parsed)result=ctx.dateResults(parsed).map(({title,value})=>({title,value}));}catch(e){result={error:e.message};}output.push({runtime:{node:process.versions.node,electron:process.versions.electron||null,icu:process.versions.icu,tz:process.versions.tz},zone,input,order:'YMD',custom,result});}}
const outputPath=path.join(__dirname,'../tests/dates-timezone-oracle.json');fs.writeFileSync(outputPath,JSON.stringify(output,null,2)+'\n');console.log(JSON.stringify({cases:output.length,path:outputPath,example:output.find(r=>r.zone==='Asia/Shanghai'&&r.input==='1991-06-01 12:34:56')}));
