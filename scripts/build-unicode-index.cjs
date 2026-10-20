// Development-only conversion. Runtime lookup and search are entirely Rust.
const fs=require('node:fs'),path=require('node:path');
const root=path.resolve(__dirname,'..'),metadata=JSON.parse(fs.readFileSync(path.join(root,'assets/data/unicode-meta.json'))).meta;
// Object.entries deliberately preserves the original JavaScript enumeration.
const rows=Object.entries(metadata).filter(([,value])=>typeof value[0]==='string'&&value[0]).map(([hex,value])=>({cp:parseInt(hex,16),name:Buffer.from(value[0]),category:value[1]||'Cn'}));
const rowStart=16,lookupStart=rowStart+rows.length*16,stringStart=lookupStart+rows.length*8;
const output=Buffer.alloc(stringStart+rows.reduce((n,row)=>n+row.name.length,0));output.write('QTU1');output.writeUInt32LE(rows.length,4);output.writeUInt32LE(lookupStart,8);output.writeUInt32LE(stringStart,12);
let offset=0;rows.forEach((row,i)=>{if(!/^[A-Za-z]{2}$/.test(row.category))throw Error('Unexpected Unicode category');const at=rowStart+i*16;output.writeUInt32LE(row.cp,at);output.writeUInt32LE(offset,at+4);output.writeUInt32LE(row.name.length,at+8);output.write(row.category,at+12,2,'ascii');row.name.copy(output,stringStart+offset);offset+=row.name.length;});
rows.map((row,index)=>({cp:row.cp,index})).sort((a,b)=>a.cp-b.cp).forEach((row,i)=>{output.writeUInt32LE(row.cp,lookupStart+i*8);output.writeUInt32LE(row.index,lookupStart+i*8+4);});
fs.writeFileSync(path.join(root,'assets/unicode-index.bin'),output);console.log(`Indexed ${rows.length} named Unicode entries (${output.length} bytes)`);
