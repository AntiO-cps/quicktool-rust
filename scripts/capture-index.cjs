// Test oracle only: runs the unmodified original worker and native indexer on synthetic files.
const {Worker}=require('node:worker_threads');
const {DatabaseSync}=require('node:sqlite');
const fs=require('node:fs'),path=require('node:path'),os=require('node:os');
if(!process.versions.electron&&!process.argv.includes('--allow-system-node'))throw Error('Use the original Electron runtime with ELECTRON_RUN_AS_NODE=1');
const base=path.resolve('C:/Users/AntiO/.codex/visualizations/2026/10/19/01a154e8-f9c1-7e42-a649-266e9cecb914');
const option=(name,fallback)=>{const i=process.argv.indexOf(name);return i<0?fallback:process.argv[i+1]};
const bulk=Number(option('--count','520'));
const output=path.resolve(option('--output','tests/index-oracle.json'));
if(!Number.isSafeInteger(bulk)||bulk<1||bulk>20000)throw Error('count must be 1..20000');
const fixture=fs.mkdtempSync(path.join(base,'quicktool-index-fixture-'));
const root=path.join(fixture,'files'),database=path.join(fixture,'original.sqlite');
fs.mkdirSync(root);const files=[];
const put=(relative,directory=false)=>{const file=path.join(root,relative);fs.mkdirSync(directory?file:path.dirname(file),{recursive:true});if(!directory)fs.writeFileSync(file,'synthetic index fixture');files.push({relative,directory});};
const filesFrom=option('--files-from','');
if(filesFrom){for(const file of JSON.parse(fs.readFileSync(filesFrom,'utf8')).files)put(file.relative,file.directory);}else{
for(let i=0;i<bulk;i++)put(`000-very-long-needle-${String(i).padStart(5,'0')}.txt`);
for(const name of ['z-needle.txt','a.txt','a\u0345b.txt','𜳖B-index.txt','z.t_t','fullwidth.ＴＸＴ','alpha x beta.txt','alpha beta.txt','alphabeta.txt','alpha\u0085beta.txt','ΟΣ.txt','ος-test.txt','İstanbul.txt','istanbul.txt','100%_literal.txt','literal[one].md','len😀😀😀.txt','lenabcd.txt','len😀.txt','lenab.txt','sort-A.txt','sort-a.txt','sort-ä.txt','sort-z.txt','sort-Á.txt','sort-Ω.txt','😃-boundary.txt','file-fi.txt','ＡＢＣ.txt','abc.txt'])put(name);
put('dir-parent',true);put('dir-parent/nested-needle.md');put('ПРИВЕТ',true);put('ПРИВЕТ/данные.txt');put('excluded/hidden-needle.txt');
}
const original=path.resolve('../src/file-index-worker.js');
const native=path.resolve('../src/native/file-indexer.exe');
const worker=new Worker(original,{workerData:{database,native}});
let serial=0,status=null;const pending=new Map();const errors=[];
worker.on('message',message=>{if(message.status)status=message.status;if(message.error)errors.push(message.error);if(message.id&&pending.has(message.id)){const waiter=pending.get(message.id);pending.delete(message.id);message.error?waiter.reject(Error(message.error)):waiter.resolve(message.value)}});
worker.on('error',error=>{errors.push(error.message);for(const waiter of pending.values())waiter.reject(error)});
const request=(type,payload={})=>new Promise((resolve,reject)=>{const id=++serial;pending.set(id,{resolve,reject});worker.postMessage({id,type,...payload})});
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const replaceRoot=value=>value.toLowerCase().startsWith(root.toLowerCase())?'C:\\QuickToolSynthetic'+value.slice(root.length):value;
async function run(){
 const started=performance.now();await request('configure',{config:{roots:[root],excludes:['EXCLUDED']}});
 const until=Date.now()+30000;while(!status||status.building||!status.count){if(errors.length)throw Error(errors.join('; '));if(Date.now()>until)throw Error('original scan timeout');await delay(10)}
 const scanMs=performance.now()-started;
 const cases=[];const add=(name,query,limit=40)=>cases.push({name,query,limit});
 for(const [name,query] of Object.entries({empty:'',spaces:'  ',needle:'needle',quoted:'"alpha beta"',adjacentQuotes:'"alpha"beta',embeddedQuotes:'alpha" beta"',unclosedQuote:'"alpha beta',emptyQuote:'""',fourQuotes:'""""',doubleQuotesAndPrefix:'"alpha beta".txt',NEL:'alpha\u0085beta',BOM:'\uFEFFneedle\uFEFF',literal:'100%_',literalBracket:'literal[one]',wildcard:'*needle*.txt',wildcardQuestion:'len??.txt',folders:'folder:',files:'file:',extension:'ext:txt',extensionWildcard:'ext:t_t',extensionPercent:'ext:%',extensionEmpty:'ext:',extensionUnicode:'ext:ＴＸＴ',extensionQuoted:'"ext:txt"',fullwidthName:'ＡＢＣ',greekFinalSigma:'ος',greekUpper:'ΟΣ',turkish:'İSTANBUL',lengthSort:'len',unicode16Ascii:'ab-index',unicode16Literal:'𜳖b-index',markGram:'a\u0345',localeSort:'sort-',pathUpper:'\\ПРИВЕТ\\',pathLower:'\\привет\\',mixedPath:'\\ПРИВЕТ\\данные.txt',excluded:'hidden',negative:'no_such_file_7z9'}))add(name,query);
 for(const limit of [0,1,2,10,40,100,101,200])add('limit-'+limit,'needle',limit);
 for(const length of [511,512,513,1024,2049])add('asciiBoundary-'+length,'*'.repeat(length));
 add('utf16Boundary-512','😃'+'*'.repeat(510));add('utf16Boundary-513','😃'+'*'.repeat(511));
 add('nfkcBoundary-513','*'.repeat(510)+'ﬁ*');
 add('fullPathLower',root.toLowerCase()+'\\привет\\данные.txt');
 for(const row of cases){const t=performance.now();const result=await request('query',{query:row.query,limit:row.limit});row.elapsedMs=performance.now()-t;row.query=replaceRoot(row.query);row.expected=result.map(value=>({path:replaceRoot(value.path),name:value.name,isDir:value.isDir}));}
 const timings=[];for(let i=0;i<100;i++){const t=performance.now();await request('query',{query:i%3?'needle':'*needle*',limit:40});timings.push(performance.now()-t)}
 await request('status');worker.postMessage({type:'close'});await new Promise(resolve=>worker.once('exit',resolve));
 const db=new DatabaseSync(database,{readOnly:true});
 const entries=db.prepare('SELECT id,path,name,search,isDir,size FROM entries ORDER BY id').all().map(entry=>({...entry,isDir:Boolean(entry.isDir),path:replaceRoot(entry.path),tokens:db.prepare('SELECT token FROM tokens WHERE id=? ORDER BY token').all(entry.id).map(row=>row.token)}));db.close();
 timings.sort((a,b)=>a-b);
 const result={source:'../src/file-index-worker.js (unmodified Worker + ../src/native/file-indexer.exe)',node:process.version,runtime:{electron:process.versions.electron,node:process.versions.node,icu:process.versions.icu,unicode:process.versions.unicode},locale:Intl.DateTimeFormat().resolvedOptions().locale,bulk,entries,cases,files,performance:{scanMs,rows:entries.length,databaseBytes:fs.statSync(database).size,queryMedianMs:timings[50],queryP95Ms:timings[95]}};
 fs.mkdirSync(path.dirname(output),{recursive:true});fs.writeFileSync(output,JSON.stringify(result,null,2)+'\n');
 console.log(JSON.stringify({output,entries:entries.length,cases:cases.length,performance:result.performance}));
}
run().catch(async error=>{console.error(error);await worker.terminate();process.exitCode=1}).finally(()=>{const resolved=fs.realpathSync(fixture);if(!resolved.startsWith(base+path.sep)||!path.basename(resolved).startsWith('quicktool-index-fixture-'))throw Error('unsafe fixture cleanup');fs.rmSync(resolved,{recursive:true,force:true})});
