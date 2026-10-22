// Test-only generator. Use the original Electron runtime with ELECTRON_RUN_AS_NODE=1.
const fs=require('node:fs'),path=require('node:path');
if(!process.versions.electron)throw Error('Run using the original Electron with ELECTRON_RUN_AS_NODE=1');
const isLetterOrNumber=/^[\p{L}\p{N}]$/u, isLetter=/^\p{L}$/u;
const ranges=[],letterRanges=[];let start=null,last=null,count=0,letterStart=null,letterLast=null,letterCount=0;
for(let cp=0;cp<=0x10ffff;cp++){
 const text=String.fromCodePoint(cp),matched=isLetterOrNumber.test(text);
 if(isLetter.test(text)){letterCount++;if(letterStart===null)letterStart=cp;letterLast=cp;}
 else if(letterStart!==null){letterRanges.push([letterStart,letterLast]);letterStart=null;}
 if(matched){count++;if(start===null)start=cp;last=cp;}
 else if(start!==null){ranges.push([start,last]);start=null;}
}
if(start!==null)ranges.push([start,last]);
if(letterStart!==null)letterRanges.push([letterStart,letterLast]);
const metadata={electron:process.versions.electron,node:process.versions.node,icu:process.versions.icu,unicode:process.versions.unicode,count,ranges:ranges.length};
const lines=[`// Generated from the original Electron RegExp \\p{L}|\\p{N}; do not edit.`,`// ${JSON.stringify(metadata)}`,'pub(super) const LETTER_OR_NUMBER_RANGES: &[(u32, u32)] = &[',...ranges.map(([first,last])=>`    (0x${first.toString(16)}, 0x${last.toString(16)}),`),'];', 'pub(super) const LETTER_RANGES: &[(u32, u32)] = &[',...letterRanges.map(([first,last])=>`    (0x${first.toString(16)}, 0x${last.toString(16)}),`),'];',String.raw`
fn contains(c:char,ranges:&[(u32,u32)])->bool {
    let value=c as u32;let next=ranges.partition_point(|&(first,_)|first<=value);
    next>0 && value<=ranges[next-1].1
}
pub(crate) fn is_letter(c:char)->bool {
    if c.is_ascii() {c.is_ascii_alphabetic()} else {contains(c,LETTER_RANGES)}
}
pub(crate) fn is_alphanumeric(c:char)->bool {
    if c.is_ascii() {c.is_ascii_alphanumeric()} else {contains(c,LETTER_OR_NUMBER_RANGES)}
}
`];
fs.writeFileSync(path.resolve(__dirname,'../src/index_categories.rs'),lines.join('\n'));
fs.writeFileSync(path.resolve(__dirname,'../tests/index-categories-oracle.json'),JSON.stringify({...metadata,ranges,letterRanges,letterCount},null,2)+'\n');
console.log(JSON.stringify(metadata));
