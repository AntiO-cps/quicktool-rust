(function(root){
 const PG=typeof module==='object'&&module.exports?require('./pin-geometry'):root.PinGeometry;
 const A=typeof module==='object'&&module.exports?require('./pin-annotations'):root.PinAnnotations;
 function snap(a,p,w,h){const dx=(p.x-a.x)*w,dy=(p.y-a.y)*h,angle=Math.round(Math.atan2(dy,dx)/(Math.PI/4))*Math.PI/4,ux=Math.cos(angle),uy=Math.sin(angle);let length=Math.hypot(dx,dy);if(Math.abs(ux)>1e-8)length=Math.min(length,(ux>0?1-a.x:a.x)*w/Math.abs(ux));if(Math.abs(uy)>1e-8)length=Math.min(length,(uy>0?1-a.y:a.y)*h/Math.abs(uy));return {x:a.x+ux*length/w,y:a.y+uy*length/h};}
 function handles(a){if(['line','arrow'].includes(a.type))return [['start',a.x,a.y],['middle',(a.x+a.x2)/2,(a.y+a.y2)/2],['end',a.x2,a.y2]];const l=Math.min(a.x,a.x2),r=Math.max(a.x,a.x2),t=Math.min(a.y,a.y2),b=Math.max(a.y,a.y2);return [['nw',l,t],['n',(l+r)/2,t],['ne',r,t],['e',r,(t+b)/2],['se',r,b],['s',(l+r)/2,b],['sw',l,b],['w',l,(t+b)/2]];}
 function resize(a,handle,p,w,h,shift,start){if(['line','arrow'].includes(a.type)){if(handle==='middle'){let delta={x:p.x-start.x,y:p.y-start.y};if(shift){const v=snap({x:.5,y:.5},{x:.5+delta.x/2,y:.5+delta.y/2},w,h);delta={x:(v.x-.5)*2,y:(v.y-.5)*2};}return A.translate(a,delta.x,delta.y);}const anchor=handle==='start'?{x:a.x2,y:a.y2}:{x:a.x,y:a.y},end=shift?snap(anchor,p,w,h):p;return handle==='start'?{...a,x:end.x,y:end.y}:{...a,x2:end.x,y2:end.y};}
  let l=Math.min(a.x,a.x2),r=Math.max(a.x,a.x2),t=Math.min(a.y,a.y2),b=Math.max(a.y,a.y2);if(handle.includes('w'))l=p.x;if(handle.includes('e'))r=p.x;if(handle.includes('n'))t=p.y;if(handle.includes('s'))b=p.y;
  if(shift){const proportional=PG.proportional(a,handle,p);l=proportional.x;r=proportional.x2;t=proportional.y;b=proportional.y2;}
  const radius=Math.min(a.radius||0,Math.min(Math.abs(r-l)*w,Math.abs(b-t)*h)/(2*w));return {...a,x:Math.min(l,r),y:Math.min(t,b),x2:Math.max(l,r),y2:Math.max(t,b),radius};
 }
 const api={snap,handles,resize};if(typeof module==='object'&&module.exports)module.exports=api;else root.AnnotationGeometry=api;
})(typeof window==='object'?window:globalThis);
