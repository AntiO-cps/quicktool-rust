// Development-only screenshots from an isolated original Electron profile.
const {app,BrowserWindow}=require('electron');
const fs=require('node:fs'),path=require('node:path');
const qa=path.resolve(__dirname,'../.qa');fs.mkdirSync(qa,{recursive:true});
const profile=fs.mkdtempSync(path.join(qa,'original-history-'));const empty=path.join(profile,'empty');fs.mkdirSync(empty);
app.setPath('userData',profile);fs.writeFileSync(path.join(profile,'settings.json'),JSON.stringify({globalShortcut:'Control+Alt+Shift+F24',hideOnBlur:false,theme:'dark',reduceMotion:true,fileIndexRoots:empty}));
require('../../src/main.js');
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn){for(let i=0;i<150;i++){const value=await fn();if(value)return value;await delay(50)}throw Error('history capture timeout')}
async function run(){await app.whenReady();const main=await until(()=>BrowserWindow.getAllWindows().find(w=>w.webContents.getURL().endsWith('/index.html')));await until(()=>!main.webContents.isLoading());await delay(500);main.show();
await main.webContents.executeJavaScript('showHistory();true');await delay(500);fs.writeFileSync(path.join(qa,'original-history-empty.png'),(await main.webContents.capturePage()).toPNG());
const records=[{id:'qa-history-base',toolId:'base',toolName:'进制转换',input:'0xFF',action:'十进制',output:'255',outputType:'text',createdAt:Date.now()-60000,pinned:false,mode:null},{id:'qa-history-base64',toolId:'base64',toolName:'Base64',input:'Rust 性能',action:'Base64 编码',output:'UnVzdCDmgKfog70=',outputType:'text',createdAt:Date.now()-120000,pinned:true,mode:{toolId:'base64',args:[{id:'encode',label:'编码'}]}},{id:'qa-history-json',toolId:'json',toolName:'JSON',input:'{"hello":"你好","answer":42}',action:'JSON 美化',output:'{\n  "hello": "你好",\n  "answer": 42\n}',outputType:'text',createdAt:Date.now()-180000,pinned:false,mode:null}];
await main.webContents.executeJavaScript(`(async()=>{for(const record of ${JSON.stringify(records)})await api.addHistory(record);await showHistory();return true})()`);await delay(600);fs.writeFileSync(path.join(qa,'original-history-filled.png'),(await main.webContents.capturePage()).toPNG());
const layout=await main.webContents.executeJavaScript(`JSON.stringify([...document.querySelectorAll('#historyPanel,.history-head,.history-item,.history-title,.history-meta,.history-preview,.history-actions')].map(el=>{const r=el.getBoundingClientRect(),s=getComputedStyle(el);return{tag:el.tagName,class:el.className,text:el.innerText,rect:{x:r.x,y:r.y,w:r.width,h:r.height},font:s.font,padding:s.padding,gap:s.gap}}))`);fs.writeFileSync(path.join(qa,'original-history-layout.json'),layout);console.log('Captured empty and populated original history');app.quit()}
setTimeout(()=>app.exit(1),30000).unref();run().catch(e=>{console.error(e);app.exit(1)});
