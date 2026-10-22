// Development oracle, executed by the original Electron runtime.
const fs=require('node:fs'),path=require('node:path');
const core=require('../../src/core-utils.js');
const pairs=[];
for(const query of ['𜳖','Ᲊ','ᲊ','a','ΟΣ','ος','οσ','ΟΣ\u0301','\ufeffalpha','alpha\u0085beta','alpha\u00a0beta','A B','a/b','Straße','İ','é','ｅ','fi','😀','😀a','---','\u0345','a\u0345','x'.repeat(2048)]) {
 for(const target of ['𜳖','Ᲊ','ᲊ','a','ΟΣ','ΟΣ.txt','ος','οσ','ΟΣ\u0301','alpha beta','\ufeffalpha','alpha\u0085beta','a-b','Straße','İstanbul','éclair','ﬁle','😀','😀abc']) pairs.push({query,target,result:core.fuzzySearch(query,target)});
}
const result={runtime:process.versions,cases:pairs};
fs.writeFileSync(path.join(__dirname,'../tests/search-oracle.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({cases:pairs.length,node:process.versions.node,electron:process.versions.electron}));
