(function(root){
 const locales=['en','en-GB','de-AT','fr-CA','es-MX','zh','ja','ko','fr','de','es','pt','it','ru','uk','pl','nl','sv','da','nb','fi','cs','sk','hu','ro','tr','el','ar','he','hi','bn','th','vi','id','ms','fa','ta'],months=new Map(),cache=new Map();let ready=false;
 const fold=s=>s.normalize('NFKD').replace(/\p{M}/gu,'').toLocaleLowerCase().replace(/[.,\u200e\u200f]/g,'').trim();
 function digits(s){return s.normalize('NFKC').replace(/[٠-٩۰-۹०-९০-৯๐-๙]/g,c=>{const n=c.charCodeAt(0);for(const base of [0x660,0x6f0,0x966,0x9e6,0xe50])if(n>=base&&n<base+10)return String(n-base);return c;});}
 function monthNames(){if(ready)return;ready=true;for(const locale of locales)for(const style of ['long','short']){const fmt=new Intl.DateTimeFormat(locale,{month:style,calendar:'gregory',timeZone:'UTC'});const contextual=new Intl.DateTimeFormat(locale,{month:style,day:'numeric',calendar:'gregory',timeZone:'UTC'});for(let m=0;m<12;m++){const d=new Date(Date.UTC(2026,m,1));for(const key of [fold(fmt.format(d)),fold(contextual.formatToParts(d).find(p=>p.type==='month')?.value||'')])if(key&&!/^\d/.test(key))months.set(key,m+1);}}}
 function date(y,m=1,d=1,h=0,mi=0,se=0){if(y<1||y>9999||m<1||m>12||d<1||d>31||h>23||mi>59||se>59)return null;const value=new Date(0);value.setFullYear(y,m-1,d);value.setHours(h,mi,se,0);return value.getFullYear()===y&&value.getMonth()===m-1&&value.getDate()===d?value:null;}
 const year=n=>n<100?(n<69?2000:1900)+n:n,pad=n=>String(n).padStart(2,'0');
 function recognized(d){return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;}
 function parse(raw,order='YMD'){
  const key=String(raw)+'|'+order;if(cache.has(key))return cache.get(key);const result=parseRaw(raw,order);cache.set(key,result);if(cache.size>128)cache.delete(cache.keys().next().value);return result;
 }
 function parseRaw(raw,order){const original=String(raw??'').trim(),s=digits(original);if(!s||s.length>160)return null;
  const results=[];function add(y,m,d,format,precision='day',clock=[]){const value=date(String(y).length<=2?year(+y):+y,+m,+d,...clock.map(Number));if(value&&!results.some(r=>r.date.getTime()===value.getTime()))results.push({date:value,format,precision,recognized:recognized(value),kind:clock.length?'datetime':'date'});}
  // Offset-bearing ISO input remains independent of regional preferences.
  if(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d{1,3})?)?(?:Z|[+-]\d{2}:?\d{2})$/i.test(s)){const base=s.match(/^(\d+)-(\d+)-(\d+)T(\d+):(\d+)(?::(\d+))?/);if(date(+base[1],+base[2],+base[3],+base[4],+base[5],+(base[6]||0))){const d=new Date(s);if(!isNaN(d))return {date:d,kind:'iso',format:'ISO 8601',precision:'second',recognized:recognized(d),alternatives:[]};}return null;}
  let m=s.match(/^(\d{4})\s*[年년]\s*(?:(\d{1,2})\s*[月월]\s*(?:(\d{1,2})\s*[日일])?)?(?:\s+(\d{1,2}):(\d{2})(?::(\d{2}))?)?$/);
  if(m)add(m[1],m[2]||1,m[3]||1,'年/月/日',m[3]?'day':m[2]?'month':'year',m[4]?[m[4],m[5],m[6]||0]:[]);
  if(/^\d{4}$/.test(s)&&+s>=1000&&+s<=2999)add(s,1,1,'YYYY','year');
  if(/^\d{6}$/.test(s))add(s.slice(0,2),s.slice(2,4),s.slice(4,6),'YYMMDD');
  if(/^\d{8}$/.test(s))add(s.slice(0,4),s.slice(4,6),s.slice(6,8),'YYYYMMDD');
  m=s.match(/^(\d{4})[-/.](\d{1,2})$/);if(m)add(m[1],m[2],1,'YYYY-MM','month');
  m=s.match(/^(\d{1,4})([-/.])(\d{1,2})\2(\d{1,4})(?:[ T](\d{1,2})(?::(\d{2}))?(?::(\d{2}))?)?$/);
  if(m){const tokens=[m[1],m[3],m[4]],clock=m[5]?[m[5],m[6]||0,m[7]||0]:[],first=tokens[0].length===4,last=tokens[2].length===4;
   const orders=first?(order==='YDM'?['YDM','YMD']:['YMD','YDM']):last?(order==='MDY'?['MDY','DMY']:['DMY','MDY']):[order,...['YMD','YDM','DMY','MDY'].filter(x=>x!==order)];
   for(const o of orders){const y=tokens[o.indexOf('Y')],mo=tokens[o.indexOf('M')],d=tokens[o.indexOf('D')];add(y,mo,d,o.split('').map((c,i)=>c==='Y'?(tokens[i].length===2?'YY':'YYYY'):c+c).join(m[2]),'day',clock);}
  }
  if(!results.length&&/\d/.test(s)&&!/[\@?#\\:]/.test(s)&&/\p{L}/u.test(s)){monthNames();let clean=fold(s).replace(/\//g,' ').replace(/\b(\d{1,2})(?:st|nd|rd|th)\b/g,'$1').replace(/\b(?:de|del|of|г|года|roku)\b/g,' ').replace(/\s+/g,' ').trim();
   for(const [name,mo] of months){const at=clean.indexOf(name);if(at<0)continue;const before=clean.slice(0,at).trim(),after=clean.slice(at+name.length).trim();if(!/^[\d\s-]*$/.test(before+' '+after))continue;const nums=(before+' '+after).match(/\d+/g)||[];
    if(nums.length===1&&nums[0].length<=2)add(new Date().getFullYear(),mo,nums[0],'月份名称 DD');
    else if(nums.length===1&&nums[0].length===4)add(nums[0],mo,1,'月份名称 YYYY','month');
    else if(nums.length===2){const y=nums.find(n=>n.length===4)||nums.at(-1),d=nums.find((n,i)=>n!==y||i===0&&nums[1]===y);if(d)add(y,mo,d,'本地化月份日期');}
    if(results.length)break;
   }
  }
  if(!results.length)return null;return {...results[0],alternatives:results.slice(1),original};
 }
 const api={parse,recognized,monthNames};if(typeof module==='object'&&module.exports)module.exports=api;else root.DateParser=api;
})(typeof window==='object'?window:globalThis);
