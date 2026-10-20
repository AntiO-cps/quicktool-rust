// Test-time oracle only. The shipped Rust app never evaluates JavaScript.
const fs=require('node:fs');
const path=require('node:path');
const A=require('../reference/pin-annotations.js');
const G=require('../reference/annotation-geometry.js');
const PG=require('../reference/pin-geometry.js');
const P=require('../../src/pin-psd.js');
const vm=require('node:vm');
const pinRenderer=fs.readFileSync(path.join(__dirname,'../../src/pin-renderer.js'),'utf8');
const resizeSource=pinRenderer.match(/function resizeBounds\(g,x,y,shift=false\) \{[\s\S]*?\r?\n\}/)[0];
let seed=0x1199ab;
const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296;};
const fixtures=[];
for(let i=0;i<300;i++) {
 const type=['rect','ellipse','line','arrow','pen','text','number','mosaic','highlight'][i%9];
 const a=A.normalize([{id:String(i),type,x:random(),y:random(),x2:random(),y2:random(),stroke:.001+random()*.03,fontSize:.01+random()*.1,radius:random()*.04,color:['#ff4f63','#000000','#ffffff','#9bc82a'][i%4],text:'测试\nRust',number:i%99+1,points:type==='pen'?Array.from({length:12},()=>({x:random(),y:random()})):[]}])[0];
 const width=100+Math.round(random()*1500),height=100+Math.round(random()*1200),point={x:random(),y:random()},delta={x:random()*4-2,y:random()*4-2};
 fixtures.push({op:'hit',a,width,height,point,expected:A.hit(a,point,width,height)});
 fixtures.push({op:'translate',a,delta,expected:A.translate(a,delta.x,delta.y)});
 fixtures.push({op:'arrow',a,width,height,expected:A.arrow(a,width,height)});
 fixtures.push({op:'square',a,width,height,point,expected:A.square(a,point,width,height)});
 fixtures.push({op:'snap',a,width,height,point,expected:G.snap(a,point,width,height)});
 fixtures.push({op:'handles',a,expected:G.handles(a)});
 const hs=G.handles(a),handle=hs[i%hs.length][0],start={x:random(),y:random()},shift=i%2===0;
 fixtures.push({op:'resize',a,width,height,point,handle,start,shift,expected:G.resize(a,handle,point,width,height,shift,start)});
 fixtures.push({op:'direction',delta,expected:PG.direction(delta.x,delta.y)});
 fixtures.push({op:'numberColor',color:a.color,expected:A.numberTextColor(a.color)});
}
for(let i=0;i<160;i++) {
 const type=i%2?'text':'image',editing=i%3===0,shift=i%4<2,natural={width:100+Math.round(random()*2000),height:100+Math.round(random()*2000)};
 const start={x:Math.round(random()*1200)-300,y:Math.round(random()*900)-100,width:316+Math.round(random()*1500),height:136+Math.round(random()*900)};
 const g={start,corner:['nw','ne','sw','se'][i%4],startX:Math.round(random()*1600),startY:Math.round(random()*900)};
 const pointer={x:g.startX+Math.round(random()*1600)-800,y:g.startY+Math.round(random()*1200)-600};
 const resize=vm.runInNewContext(resizeSource+';resizeBounds',{pinState:{type},pinNatural:natural,annotationEditing:editing});
 fixtures.push({op:'windowResize',type,editing,shift,natural,gesture:g,point:pointer,expected:resize(g,pointer.x,pointer.y,shift)});
}
const composite=Uint8Array.from([1,2,3,255,1,2,3,255]);
const psd=Array.from(new Uint8Array(P.encode({width:2,height:1,layers:[{name:'原图',width:2,height:1,data:composite,hidden:true}],composite})));
fs.writeFileSync(path.join(__dirname,'../tests/annotations-oracle.json'),JSON.stringify({fixtures,psd}));
console.log(`Captured ${fixtures.length} annotation geometry fixtures and ${psd.length} PSD bytes from the original implementation.`);
