const fs=require('node:fs'),path=require('node:path'),C=require('../reference/core-utils.js');
const expressions=['1.','.5','1e3','2pi','2e','5%2','-0','round(-0.5)','min()','max()','hypot()','pow(2)','sin(1,2)','sin()','1 2','2(3)','pi2','2^-2','-2^2','--2','170!','171!','3!!','(2+3)!','5!^2','1,000.5+1','1.000,5+1','pow(2;3)','min(1;2;3)','sqrt(-1)','1/0','1..2','π+π'];
let seed=54;function next(){seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/2**32;}for(let i=0;i<250;i++){const a=(next()-.5)*40,b=(next()-.5)*10,c=Math.floor(next()*8);expressions.push(`(${a}+${b})*${c}^2`);}
const cases=[];for(const locale of ['en-US','de-DE'])for(const input of expressions){try{cases.push({input,locale,result:C.formatCalculationResult(C.calculateExpression(input,locale),locale)});}catch{cases.push({input,locale,error:true});}}
fs.writeFileSync(path.resolve(__dirname,'../tests/calculator-oracle.json'),JSON.stringify(cases,null,2));console.log(`Captured ${cases.length} calculator cases`);
