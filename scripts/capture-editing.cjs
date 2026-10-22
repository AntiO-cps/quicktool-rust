// Test-only Chromium editing oracle. Events stay inside the isolated application.
const {app,BrowserWindow}=require('electron');
const fs=require('node:fs'),path=require('node:path');
const qa=path.resolve(__dirname,'../.qa');fs.mkdirSync(qa,{recursive:true});
const profile=fs.mkdtempSync(path.join(qa,'original-editing-')),empty=path.join(profile,'empty');fs.mkdirSync(empty);
app.setPath('userData',profile);fs.writeFileSync(path.join(profile,'settings.json'),JSON.stringify({globalShortcut:'Control+Alt+Shift+F24',hideOnBlur:false,theme:'dark',reduceMotion:true,fileIndexRoots:empty}));
require('../../src/main.js');const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn){for(let i=0;i<150;i++){const v=await fn();if(v)return v;await delay(50)}throw Error('editing oracle timeout')}
async function run(){await app.whenReady();const main=await until(()=>BrowserWindow.getAllWindows().find(w=>w.webContents.getURL().endsWith('/index.html')));await until(()=>!main.webContents.isLoading());await delay(400);main.show();
const inputs=['alpha beta gamma','abc_def  ghi','alpha...beta','你好世界 abc','e\u0301 emoji 👨‍👩‍👧‍👦 test','A𐐀B Ᲊc','\ufeffalpha\u0085beta','YWJjZGVmZ2hp/ab+CD== hello'];const cases=[];const plain=process.argv.includes('--plain');
for(const value of inputs){const boundaries=[0];for(const c of value)boundaries.push(boundaries.at(-1)+c.length);const positions=plain?boundaries:[...new Set([0,boundaries[Math.floor(boundaries.length/2)],value.length])];
for(const caret of positions)for(const key of ['Left','Right','Backspace','Delete'])for(const shift of (plain&&key==='Delete'?[false]:[false,true])){
await main.webContents.executeJavaScript(`mode={toolId:null,args:[]};completion='';search.value=${JSON.stringify(value)};render();search.focus();search.setSelectionRange(${caret},${caret});true`);
const modifiers=plain?(shift?['shift']:[]):(shift?['control','shift']:['control']);main.webContents.sendInputEvent({type:'keyDown',keyCode:key,modifiers});main.webContents.sendInputEvent({type:'keyUp',keyCode:key,modifiers});await delay(8);
const actual=await main.webContents.executeJavaScript('({value:search.value,start:search.selectionStart,end:search.selectionEnd,direction:search.selectionDirection})');cases.push({value,caret,key,control:!plain,shift,actual});}}
const result={runtime:process.versions,method:'Original Electron search input, webContents.sendInputEvent only; no OS input, clipboard, or user profile',cases};fs.writeFileSync(path.join(__dirname,plain?'../tests/editing-plain-oracle.json':'../tests/editing-oracle.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({cases:cases.length,node:process.versions.node,electron:process.versions.electron}));app.quit()}
setTimeout(()=>app.exit(1),60000).unref();run().catch(error=>{console.error(error);app.exit(1)});
