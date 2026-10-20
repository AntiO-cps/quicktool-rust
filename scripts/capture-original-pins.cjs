// Isolated original application reference; does not read the user's clipboard.
const {app,BrowserWindow}=require('electron');
const fs=require('node:fs'),path=require('node:path');
const qa=path.resolve(__dirname,'../.qa');fs.mkdirSync(qa,{recursive:true});
const profile=fs.mkdtempSync(path.join(qa,'original-pins-'));const empty=path.join(profile,'empty');fs.mkdirSync(empty);
app.setPath('userData',profile);fs.writeFileSync(path.join(profile,'settings.json'),JSON.stringify({globalShortcut:'Control+Alt+Shift+F24',hideOnBlur:false,theme:'dark',reduceMotion:true,fileIndexRoots:empty,blurStrength:28,glassOpacity:.78}));
require('../../src/main.js');
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn){for(let i=0;i<200;i++){const value=await fn();if(value)return value;await delay(25);}throw Error('reference Pin timeout');}
async function run(){await app.whenReady();const main=await until(()=>BrowserWindow.getAllWindows().find(w=>w.webContents.getURL().endsWith('/index.html')));await until(()=>!main.webContents.isLoading());
 const image=await main.webContents.executeJavaScript(`(()=>{const c=document.createElement('canvas');c.width=320;c.height=240;const x=c.getContext('2d');const g=x.createLinearGradient(0,0,320,240);g.addColorStop(0,'#2463c8');g.addColorStop(1,'#e69436');x.fillStyle=g;x.fillRect(0,0,320,240);x.fillStyle='#fff';x.fillRect(100,75,120,90);return c.toDataURL('image/png')})()`);
 fs.writeFileSync(path.join(qa,'pin-reference-input.png'),Buffer.from(image.split(',')[1],'base64'));const captures=[];
 for(const[kind,payload]of[['text',{type:'text',text:'QuickTool Rust 测试'}],['image',{type:'image',dataUrl:image,name:'reference.png'}]]){
  const result=await main.webContents.executeJavaScript(`window.quicktool.createPin(${JSON.stringify(payload)})`);if(!result?.ok)throw Error(JSON.stringify(result));
  const pin=await until(()=>BrowserWindow.getAllWindows().find(w=>w.webContents.getURL().endsWith('/pin.html')));await until(()=>!pin.webContents.isLoading());await delay(500);
  await pin.webContents.executeJavaScript('window.quicktoolPin.toggleEffects()');const effects=await until(()=>BrowserWindow.getAllWindows().find(w=>w.webContents.getURL().endsWith('/effects-panel.html')));await until(()=>!effects.webContents.isLoading());await delay(500);
  fs.writeFileSync(path.join(qa,`original-pin-${kind}.png`),(await pin.webContents.capturePage()).toPNG());fs.writeFileSync(path.join(qa,`original-pin-${kind}-effects.png`),(await effects.webContents.capturePage()).toPNG());
  captures.push({kind,pin:pin.getBounds(),effects:effects.getBounds()});pin.close();await delay(300);
 }
 fs.writeFileSync(path.join(qa,'original-pin-captures.json'),JSON.stringify(captures,null,2));console.log('Captured original text/image Pin and effects');app.quit();}
setTimeout(()=>app.exit(1),45000).unref();run().catch(error=>{console.error(error);app.exit(1);});
