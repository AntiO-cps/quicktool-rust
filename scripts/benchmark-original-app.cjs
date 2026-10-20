// Development harness: uses an isolated profile, no user data or clipboard.
const started=performance.now();
const {app,BrowserWindow}=require('electron');
const fs=require('node:fs'),path=require('node:path');
const qa=path.resolve(__dirname,'../.qa');fs.mkdirSync(qa,{recursive:true});
const data=fs.mkdtempSync(path.join(qa,'original-metrics-'));const empty=path.join(data,'empty');fs.mkdirSync(empty);
app.setPath('userData',data);
fs.writeFileSync(path.join(data,'settings.json'),JSON.stringify({globalShortcut:'Control+Alt+Shift+F24',hideOnBlur:false,theme:'dark',reduceMotion:true,fileIndexRoots:empty}));
require('../../src/main.js');
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(fn){for(let i=0;i<200;i++){const value=await fn();if(value)return value;await delay(25);}throw Error('app readiness timeout');}
async function run(){
 await app.whenReady();const main=await until(()=>BrowserWindow.getAllWindows().find(w=>w.webContents.getURL().endsWith('/index.html')));
 await until(()=>!main.webContents.isLoading());main.show();
 await main.webContents.executeJavaScript('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');
 const startup_ms=performance.now()-started;
 await delay(5000);
 const processes=app.getAppMetrics();
 const working_set_bytes=processes.reduce((n,p)=>n+(p.memory?.workingSetSize||0)*1024,0);
 const private_bytes=processes.reduce((n,p)=>n+(p.memory?.privateBytes||0)*1024,0);
 const queries=['0xFF','100cm','#7c3aed','/时间 2026-10-20 12:34:56','/json {"a":1}','/base64 Rust 性能','/国家 CN','/大小写 QuickTool Rust','/unicode U+00FF','/帮助'];
 const samples=[];
 for(const query of queries){const sample=await main.webContents.executeJavaScript(`(async()=>{mode={toolId:null,args:[]};search.value=${JSON.stringify(query)};const start=performance.now();render();const rebuild_ms=performance.now()-start;await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return {rebuild_ms,query_to_stable_layout_ms:performance.now()-start}})()`);samples.push({query,...sample});await delay(350);}
 fs.writeFileSync(path.join(qa,'original-app-metrics.json'),JSON.stringify({measured_at:new Date().toISOString(),startup_ms,working_set_bytes,private_bytes,process_count:processes.length,queries:samples,notes:'Electron main entry to first loaded renderer and two animation frames. Memory sums Electron process tree after five seconds. Layout timing excludes OS presentation; renderer animations may continue.'},null,2));
 app.quit();
}
setTimeout(()=>app.exit(1),45000).unref();run().catch(error=>{console.error(error);app.exit(1);});
