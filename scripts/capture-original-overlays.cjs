const {app,BrowserWindow}=require('electron');
const fs=require('node:fs'),path=require('node:path');
const qa=path.resolve(__dirname,'../.qa');fs.mkdirSync(qa,{recursive:true});
const root=fs.mkdtempSync(path.join(qa,'original-overlays-'));const empty=path.join(root,'empty');fs.mkdirSync(empty);
app.setPath('userData',root);fs.writeFileSync(path.join(root,'settings.json'),JSON.stringify({globalShortcut:'Control+Alt+Shift+F24',hideOnBlur:false,theme:'dark',reduceMotion:true,fileIndexRoots:empty}));
require('../../src/main.js');
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(fn){for(let i=0;i<150;i++){const value=await fn();if(value)return value;await delay(30);}throw Error('overlay capture timeout');}
async function save(window,name){await delay(400);fs.writeFileSync(path.join(qa,`original-${name}.png`),(await window.webContents.capturePage()).toPNG());}
async function capture(){
 await app.whenReady();const main=await until(()=>BrowserWindow.getAllWindows().find(window=>window.webContents.getURL().endsWith('/index.html')));await until(()=>!main.webContents.isLoading());await delay(600);main.show();
 await main.webContents.executeJavaScript("mode={toolId:'help',args:[]};search.value='';render();true");await delay(500);await main.webContents.executeJavaScript("document.querySelector('.results').scrollTop=0;true");await save(main,'help');
 await main.webContents.executeJavaScript("mode={toolId:null,args:[]};search.value='#7c3aed';render();openColorPicker(Core.parseColorCandidates(search.value,cssColors)[0].color);true");await save(main,'color-picker');
 await main.webContents.executeJavaScript('closeColorPickerPanel();showSettings();themeSelectTrigger.click();true');
 const popup=await until(()=>BrowserWindow.getAllWindows().find(window=>window.webContents.getURL().endsWith('/popup.html')));await until(()=>!popup.webContents.isLoading());await save(popup,'theme-popup');popup.hide();
 await main.webContents.executeJavaScript("confirmCancelSettings();mode={toolId:null,args:[]};search.value='';render();true");await delay(400);
 await main.webContents.executeJavaScript("search.dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,clientX:150,clientY:45}));true");await delay(300);
 const edit=BrowserWindow.getAllWindows().find(window=>window.webContents.getURL().endsWith('/popup.html')&&window.isVisible());if(edit)await save(edit,'edit-popup');else await save(main,'edit-menu');
 for(const window of BrowserWindow.getAllWindows())if(window!==main&&window.webContents.getURL().endsWith('/popup.html'))window.hide();
 await delay(900);main.showInactive();await delay(300);
 await main.webContents.executeJavaScript("window.quicktool.showTooltip({text:'设置',theme:'dark',anchor:{x:740,y:20,width:42,height:42}});true");
 await delay(200);const tooltip=BrowserWindow.getAllWindows().find(window=>window.webContents.getURL().endsWith('/tooltip.html'));if(tooltip){fs.writeFileSync(path.join(qa,'original-tooltip.png'),(await tooltip.webContents.capturePage()).toPNG());}else{console.warn('Tooltip unavailable after settled owner; remaining overlays captured');}
 console.log('Captured original help, color picker and native theme/edit menus in isolated profile; tooltip: '+(tooltip?'captured':'unavailable'));app.quit();
}
setTimeout(()=>app.exit(1),45000).unref();capture().catch(error=>{console.error(error);app.exit(1);});
