// Pure original renderer functions, captured with its exact Electron runtime.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.join(__dirname,'../../src/renderer.js'),'utf8');
const context=vm.createContext({});vm.runInContext(source.slice(source.indexOf('function titleCase('),source.indexOf('function textResults(')),context);
const inputs=['','Rust 性能','ΟΣ οσ ος','Ο\u0345Σ','abc\u0345 def','aⅰ abc','a𜳖 def','Ᲊᲊ','aᲉb','𐐀𐐨 hello','hello 𐐀𐐨','𐐨 GREEK ΟΣ','ß STRAẞE','İ I ı i','a\u0301b c','foo_bar baz','\ufeffalpha\u0085beta','e\u0301 o\u0308','ＡＢＣ Ｄｅｆ','123ⅰx','αΒΓ ABC'];
const cases=inputs.map(input=>({input,result:context.caseResults(input)}));
const output={runtime:process.versions,cases};fs.writeFileSync(path.join(__dirname,'../tests/case-oracle.json'),JSON.stringify(output,null,2)+'\n');console.log(JSON.stringify({cases:cases.length,electron:process.versions.electron}));
