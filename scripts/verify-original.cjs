const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'),manifest=require('../reference/manifest.json');let failed=0;
for(const record of manifest.files){const filename=path.resolve(root,'../src',record.path);try{const actual=crypto.createHash('sha256').update(fs.readFileSync(filename)).digest('hex');if(actual!==record.sha256){console.error(`Changed: ${record.path}`);failed++;}}catch{console.error(`Missing: ${record.path}`);failed++;}}
console.log(`${manifest.files.length-failed}/${manifest.files.length} original files unchanged`);process.exitCode=failed?1:0;
