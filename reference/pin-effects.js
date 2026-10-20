// Shared pixel pipeline for image preview and transparent PNG export.
(function(root) {
  function filters(raw={}) {
    const n=(key,min,max)=>Math.max(min,Math.min(max,Number(raw[key])||0));
    return {brightness:n('brightness',-150,150),contrast:n('contrast',-50,100),saturation:n('saturation',-100,100),exposure:n('exposure',-3,3),hue:n('hue',-180,180),colorize:!!raw.colorize,colorizeSaturation:Math.max(0,Math.min(100,Number(raw.colorizeSaturation ?? (raw.colorize?Math.max(25,Math.abs(Number(raw.saturation)||0)):25))))};
  }
  // Adapted from PhotoCraft crates/compose/src/adjust.rs, revision b07a2e5.
  // Copyright (c) 2026 ArtCraft Team and PhotoCraft contributors. MIT; see licenses/photocraft-MIT.txt.
  const clamp=v=>Math.max(0,Math.min(1,v));
  function modernBrightness(v,b) {
    if(!b)return v;const slope=1.375**(b/50),p=b>=0?Math.max(2,4.5-.013*b):5-.072*b;
    v=clamp(v);return clamp(slope*v+(1-slope)*v**p);
  }
  function modernContrast(v,c) {
    if(!c)return v;const k=c/128,end=1-k,mid=1+k;
    const half=t=>end*.5*(t**3-2*t*t+t)+(-2*t**3+3*t*t)*.5+mid*.5*(t**3-t*t);
    v=clamp(v);return v<=.5?half(v/.5):1-half((1-v)/.5);
  }
  function rgbToHsl([r,g,b]) {
    const max=Math.max(r,g,b),min=Math.min(r,g,b),d=max-min,l=(max+min)/2;
    if(d<1e-8)return [0,0,l];const sat=d/(1-Math.abs(2*l-1));
    const h=max===r?((g-b)/d+(g<b?6:0)):max===g?(b-r)/d+2:(r-g)/d+4;
    return [h/6,sat,l];
  }
  function hslToRgb(h,s,l) {
    const c=(1-Math.abs(2*l-1))*s,x=c*(1-Math.abs((h*6)%2-1)),m=l-c/2;
    const a=h<1/6?[c,x,0]:h<2/6?[x,c,0]:h<3/6?[0,c,x]:h<4/6?[0,x,c]:h<5/6?[x,0,c]:[c,0,x];return a.map(v=>v+m);
  }
  function hueSaturation(rgb,hue,s,colorize) {
    let [h,sat,l]=rgbToHsl(rgb);
    if(colorize){h=((hue%360)+360)%360/360;sat=Math.max(Math.abs(s),.25);}
    else {h=((h+hue/360)%1+1)%1;sat=clamp(sat*(1+s));}
    return hslToRgb(h,sat,l);
  }
  function processPixels(bytes,raw) {
    const f=filters(raw),gain=2**f.exposure,lut=new Float32Array(256);
    for(let v=0;v<256;v++){const tone=clamp(modernContrast(modernBrightness(v/255,f.brightness),f.contrast));lut[v]=clamp((tone**2.2*gain)**(1/2.2));}
    const changeHue=!!(f.hue || f.saturation || f.colorize);
    for(let i=0;i<bytes.length;i+=4){
      let r=lut[bytes[i]],g=lut[bytes[i+1]],b=lut[bytes[i+2]];
      if(changeHue){
        const max=Math.max(r,g,b),min=Math.min(r,g,b),d=max-min,l=(max+min)/2;
        let h=d<1e-8?0:(max===r?((g-b)/d+(g<b?6:0)):max===g?(b-r)/d+2:(r-g)/d+4)/6;
        let s=d<1e-8?0:d/(1-Math.abs(2*l-1));
        if(f.colorize){h=((f.hue%360)+360)%360/360;s=f.colorizeSaturation/100;}else{h=((h+f.hue/360)%1+1)%1;s=clamp(s*(1+f.saturation/100));}
        const c=(1-Math.abs(2*l-1))*s,x=c*(1-Math.abs((h*6)%2-1)),m=l-c/2;
        if(h<1/6){r=c;g=x;b=0;}else if(h<2/6){r=x;g=c;b=0;}else if(h<3/6){r=0;g=c;b=x;}else if(h<4/6){r=0;g=x;b=c;}else if(h<5/6){r=x;g=0;b=c;}else{r=c;g=0;b=x;}
        r+=m;g+=m;b+=m;
      }
      bytes[i]=Math.round(clamp(r)*255);bytes[i+1]=Math.round(clamp(g)*255);bytes[i+2]=Math.round(clamp(b)*255);
    }return bytes;
  }
  function render(image,{width,height,radius=0,filters:raw={}}) {
    const canvas=typeof OffscreenCanvas==='function'?new OffscreenCanvas(1,1):document.createElement('canvas');canvas.width=Math.max(1,Math.round(width));canvas.height=Math.max(1,Math.round(height));
    const ctx=canvas.getContext('2d',{willReadFrequently:true});
    ctx.drawImage(image,0,0,canvas.width,canvas.height);
    const f=filters(raw);if(f.brightness||f.contrast||f.saturation||f.exposure||f.hue||f.colorize){const pixels=ctx.getImageData(0,0,canvas.width,canvas.height);processPixels(pixels.data,f);ctx.putImageData(pixels,0,0);}
    // Mask after processing, keeping antialiased corners and source transparency.
    ctx.save();ctx.globalCompositeOperation='destination-in';ctx.fillStyle='#fff';ctx.beginPath();ctx.roundRect(0,0,canvas.width,canvas.height,Math.min(radius,canvas.width/2,canvas.height/2));ctx.fill();ctx.restore();
    return canvas;
  }
  const api={filters,processPixels,render,modernBrightness,modernContrast,hueSaturation};
  if(typeof module==='object' && module.exports) module.exports=api;
  else root.PinEffects=api;
})(typeof window==='object'?window:globalThis);
