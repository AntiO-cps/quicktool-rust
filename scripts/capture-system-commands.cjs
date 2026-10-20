const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const root=path.resolve(__dirname,'..'),source=fs.readFileSync(path.resolve(root,'../src/renderer.js'),'utf8');
function array(name){const start=source.indexOf(`const ${name}=[`)+`const ${name}=`.length;return vm.runInNewContext('('+source.slice(start,source.indexOf('\n]',start)+2)+')');}
fs.writeFileSync(path.join(root,'assets/windows-settings.json'),JSON.stringify(array('windowsSettings'),null,2));
const systems=array('systemCommands');fs.writeFileSync(path.join(root,'assets/system-commands.json'),JSON.stringify(systems,null,2));
const commands=JSON.parse(fs.readFileSync(path.join(root,'assets/commands.json'))).filter(c=>!c.id.startsWith('system:'));
commands.push(...systems.map(s=>({id:`system:${s.id}`,name:s.name,icon:'mdi:console',primaryAlias:s.aliases[0],aliases:s.aliases})));
fs.writeFileSync(path.join(root,'assets/commands.json'),JSON.stringify(commands,null,2));
const C=require('../reference/core-utils.js');const context={module:{exports:{}},windowsSettings:array('windowsSettings'),fuzzyMatchInfo:(text,query)=>C.fuzzySearch(query,text)};
vm.runInNewContext(source.slice(source.indexOf('function windowsSettingsSearch('),source.indexOf('const systemCommands=['))+';module.exports=windowsSettingsSearch;',context);
const cases=['','显示','声音','wifi','网络','主题','update','clipboard','yuyan','startup','xyz','WINDOWS','developers'].map(input=>({input,results:context.module.exports(input,12).map(({item,score})=>({command:item.command,score}))}));
fs.writeFileSync(path.join(root,'tests/windows-settings-oracle.json'),JSON.stringify(cases,null,2));
