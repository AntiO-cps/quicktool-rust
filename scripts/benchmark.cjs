const fs=require('node:fs'),path=require('node:path'),{execFileSync}=require('node:child_process'),{performance}=require('node:perf_hooks');
const E=require('../reference/pin-effects.js');let seed=0x51475549;const input=Uint8ClampedArray.from({length:1024*1024*4},()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed>>>24;});
const filters={brightness:25,contrast:18,saturation:30,exposure:.35,hue:12,colorize:false,colorizeSaturation:25};
for(let i=0;i<3;i++)E.processPixels(input.slice(),filters);
let checksum;const samples=[];for(let i=0;i<7;i++){const start=performance.now(),data=input.slice();E.processPixels(data,filters);samples.push(performance.now()-start);checksum=data.reduce((a,b)=>a+b,0);}samples.sort((a,b)=>a-b);
const exe=process.argv[2];if(!exe)throw Error('Pass the release benchmark executable path');
const rust=JSON.parse(execFileSync(exe,{encoding:'utf8'}));if(rust.checksum!==checksum)throw Error('Benchmark outputs differ');
const result={measuredAt:new Date().toISOString(),scope:'CPU RGBA filter pipeline only; does not establish whole-app performance',pixels:rust.pixels,javascript:{runtime:process.version,samplesMs:samples,medianMs:samples[3],checksum},rust,speedup:samples[3]/rust.medianMs};
const output=path.resolve(__dirname,'../docs/pixel-benchmark.json');fs.writeFileSync(output,JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
