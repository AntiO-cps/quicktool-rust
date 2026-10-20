(function(root){
 function direction(dx,dy){const angle=Math.round(Math.atan2(dy,dx)/(Math.PI/4))*Math.PI/4,length=Math.hypot(dx,dy);return {x:Math.cos(angle)*length,y:Math.sin(angle)*length};}
 function proportional(a,handle,p){const l=Math.min(a.x,a.x2),r=Math.max(a.x,a.x2),t=Math.min(a.y,a.y2),b=Math.max(a.y,a.y2),w=r-l,h=b-t;if(w<1e-6||h<1e-6)return a;const west=handle.includes('w'),east=handle.includes('e'),north=handle.includes('n'),south=handle.includes('s'),x=west?r:east?l:(l+r)/2,y=north?b:south?t:(t+b)/2;
  let scale=(west||east)&&(north||south)?Math.max(Math.abs(p.x-x)/w,Math.abs(p.y-y)/h):(west||east)?Math.abs(p.x-x)/w:Math.abs(p.y-y)/h;
  const availableX=west?x:east?1-x:Math.min(x,1-x)*2,availableY=north?y:south?1-y:Math.min(y,1-y)*2;scale=Math.max(.001,Math.min(scale,availableX/w,availableY/h));const nw=w*scale,nh=h*scale,left=west?x-nw:east?x:x-nw/2,top=north?y-nh:south?y:y-nh/2;return {...a,x:left,y:top,x2:left+nw,y2:top+nh};
 }
 const api={direction,proportional};if(typeof module==='object'&&module.exports)module.exports=api;else root.PinGeometry=api;
})(typeof window==='object'?window:globalThis);
