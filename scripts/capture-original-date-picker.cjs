const {app,BrowserWindow}=require('electron');
const fs=require('node:fs'),path=require('node:path');
const qa=path.resolve(__dirname,'../.qa');fs.mkdirSync(qa,{recursive:true});
const root=fs.mkdtempSync(path.join(qa,'original-picker-'));
const empty=path.join(root,'empty');fs.mkdirSync(empty);
app.setPath('userData',root);
fs.writeFileSync(path.join(root,'settings.json'),JSON.stringify({globalShortcut:'Control+Alt+Shift+F24',hideOnBlur:false,theme:'dark',reduceMotion:true,fileIndexRoots:empty}));
require('../../src/main.js');
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(fn){for(let i=0;i<150;i++){const value=await fn();if(value)return value;await wait(30);}throw Error('capture timeout');}
async function save(window,name){await wait(500);fs.writeFileSync(path.join(qa,`original-${name}.png`),(await window.webContents.capturePage()).toPNG());}
async function run(){
 await app.whenReady();
 const main=await until(()=>BrowserWindow.getAllWindows().find(w=>w.webContents.getURL().endsWith('/index.html')));
 await until(()=>!main.webContents.isLoading());await wait(600);main.show();
 const records={};
 for(const query of ['0xFF','CN','2026-10-20 12:34:56']){
  await main.webContents.executeJavaScript(`mode={toolId:null,args:[]};search.value=${JSON.stringify(query)};render();true`);
  await wait(900);
  records[query]=await main.webContents.executeJavaScript(`(()=>{const r=e=>{if(!e)return null;const b=e.getBoundingClientRect();const s=getComputedStyle(e);return {x:b.x,y:b.y,width:b.width,height:b.height,padding:s.padding,margin:s.margin,gap:s.gap}};return {devicePixelRatio,scroll:resultsEl.scrollTop,window:[innerWidth,innerHeight],results:r(resultsEl),context:r(contextLabel.parentElement),groups:[...resultsEl.children].map(r),rows:[...resultsEl.querySelectorAll('.result,.country-card')].slice(0,3).map(r)}})()`);
  await save(main,query==='0xFF'?'base-picker-reference':query==='CN'?'country-picker-reference':'date-before-native-picker');
 }
 records.dateOpen=await main.webContents.executeJavaScript(`(()=>{datePicker.value='2026-10-20T12:34';datePicker.showPicker();return {value:datePicker.value,active:document.activeElement?.id,inputRect:datePicker.getBoundingClientRect().toJSON()}})()`,true);
 await save(main,'date-native-picker');
 records.dateWindows=BrowserWindow.getAllWindows().map(w=>({url:w.webContents.getURL(),visible:w.isVisible(),bounds:w.getBounds()}));
 for(const [name,value] of [['leap','2024-02-29T23:59'],['month-start','2026-11-01T00:00']]){
  main.hide();await wait(100);main.show();
  await main.webContents.executeJavaScript(`datePicker.value=${JSON.stringify(value)};datePicker.showPicker();true`,true);
  await save(main,`date-native-picker-${name}`);
 }
 // Closing/hiding the owner dismisses Chromium's native picker without OS input.
 main.hide();await wait(150);main.show();
 await main.webContents.executeJavaScript("mode={toolId:null,args:[]};search.value='#7c3aed';render();openColorPicker(Core.parseColorCandidates(search.value,cssColors)[0].color);true");
 await save(main,'color-before-native-picker');
 records.colorPanel=await main.webContents.executeJavaScript(`(()=>{const r=s=>{const e=document.querySelector(s),b=e.getBoundingClientRect(),c=getComputedStyle(e);return {rect:b.toJSON(),padding:c.padding,border:c.border,font:c.font,background:c.backgroundColor}};return Object.fromEntries(['.color-picker-panel','.color-picker-head','.picker-controls','.native-color-wrap','.alpha-control','.picker-formats','.picker-format'].map(s=>[s,r(s)]))})()`);
 records.colorOpen=await main.webContents.executeJavaScript(`(()=>{colorPickerInput.showPicker();return {value:colorPickerInput.value,inputRect:colorPickerInput.getBoundingClientRect().toJSON()}})()`,true);
 await save(main,'color-native-picker');
 fs.writeFileSync(path.join(qa,'original-picker-layout.json'),JSON.stringify(records,null,2));
 console.log('Captured native picker attempts and DOM geometry in isolated profile');app.quit();
}
setTimeout(()=>app.exit(1),45000).unref();run().catch(error=>{console.error(error);app.exit(1)});
