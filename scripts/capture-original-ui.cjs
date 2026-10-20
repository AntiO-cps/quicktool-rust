const {app,BrowserWindow}=require('electron');
const fs=require('node:fs'),path=require('node:path');
const qa=path.resolve(__dirname,'../.qa');fs.mkdirSync(qa,{recursive:true});
const root=fs.mkdtempSync(path.join(qa,'original-ui-'));const empty=path.join(root,'empty');fs.mkdirSync(empty);
app.setPath('userData',root);fs.writeFileSync(path.join(root,'settings.json'),JSON.stringify({globalShortcut:'Control+Alt+Shift+F24',hideOnBlur:false,theme:'dark',reduceMotion:true,fileIndexRoots:empty}));
require('../../src/main.js');
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn){for(let i=0;i<150;i++){const v=await fn();if(v)return v;await delay(50);}throw Error('UI capture timeout');}
async function capture(){await app.whenReady();const main=await until(()=>BrowserWindow.getAllWindows().find(w=>w.webContents.getURL().endsWith('/index.html')));await until(()=>!main.webContents.isLoading());await delay(500);main.show();
 for(const[name,query]of[['empty',''],['base','0xFF'],['unit','10 km to mi'],['color','#7c3aed'],['country','CN']]){await main.webContents.executeJavaScript(`mode={toolId:null,args:[]};search.value=${JSON.stringify(query)};render();true`);await delay(600);fs.writeFileSync(path.join(qa,'original-'+name+'.png'),(await main.webContents.capturePage()).toPNG());}
 await main.webContents.executeJavaScript('showSettings();true');await delay(600);fs.writeFileSync(path.join(qa,'original-settings.png'),(await main.webContents.capturePage()).toPNG());console.log('Captured original UI in isolated test profile');app.quit();}
setTimeout(()=>app.exit(1),30000).unref();capture().catch(e=>{console.error(e);app.exit(1);});
