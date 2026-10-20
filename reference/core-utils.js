(function(global){
  'use strict';

  const CATEGORY_NAMES = {
    Lu:'Uppercase Letter', Ll:'Lowercase Letter', Lt:'Titlecase Letter', Lm:'Modifier Letter', Lo:'Other Letter',
    Mn:'Nonspacing Mark', Mc:'Spacing Mark', Me:'Enclosing Mark', Nd:'Decimal Number', Nl:'Letter Number', No:'Other Number',
    Pc:'Connector Punctuation', Pd:'Dash Punctuation', Ps:'Open Punctuation', Pe:'Close Punctuation', Pi:'Initial Punctuation', Pf:'Final Punctuation', Po:'Other Punctuation',
    Sm:'Math Symbol', Sc:'Currency Symbol', Sk:'Modifier Symbol', So:'Other Symbol', Zs:'Space Separator', Zl:'Line Separator', Zp:'Paragraph Separator',
    Cc:'Control', Cf:'Format', Cs:'Surrogate', Co:'Private Use', Cn:'Unassigned'
  };

  const normalizeSearch = s => String(s ?? '')
    .normalize('NFKC')
    .toLocaleLowerCase()
    .replace(/[\s_\-–—()（）\[\]【】.,，。'"·]+/g,'');


  // PowerToys-inspired fuzzy scoring: require every query token to match,
  // then reward compact/early/word-start/exact matches. This is intentionally
  // implemented independently rather than copied from PowerToys source.
  function foldSearchChar(ch){
    return String(ch ?? '').normalize('NFKD').toLocaleLowerCase().replace(/\p{M}+/gu,'');
  }
  function fuzzySearch(query,target){
    const raw=String(target ?? '');
    const qRaw=String(query ?? '').trim();
    if(!raw || !qRaw) return {success:false,score:0,indexes:[]};
    const chars=Array.from(raw), flat=[], map=[], boundaries=new Set([0]);
    let logical=0;
    for(let i=0;i<chars.length;i++){
      const ch=chars[i], folded=foldSearchChar(ch);
      if(i===0 || /[\s_\-–—()（）\[\]【】.,，。/\\:;]+/u.test(chars[i-1])) boundaries.add(logical);
      for(const c of folded){
        if(/[\s_\-–—()（）\[\]【】.,，。'"·/\\:;]+/u.test(c)) continue;
        flat.push(c); map.push(i); logical++;
      }
    }
    const hay=flat.join('');
    const tokens=qRaw.split(/\s+/).map(x=>Array.from(foldSearchChar(x)).filter(c=>!/[\s_\-–—()（）\[\]【】.,，。'"·/\\:;]+/u.test(c)).join('')).filter(Boolean);
    if(!hay || !tokens.length) return {success:false,score:0,indexes:[]};
    const allIndexes=[]; let total=0; let searchFrom=0;
    for(const token of tokens){
      let best=null;
      const direct=hay.indexOf(token,searchFrom);
      if(direct>=0){
        const end=direct+token.length-1;
        let score=.82;
        if(direct===0) score+=.10;
        if(boundaries.has(direct)) score+=.055;
        const extra=Math.max(0,hay.length-token.length);
        score+=extra<5?.035:extra<10?.018:0;
        best={score:Math.min(1,score),positions:Array.from({length:token.length},(_,i)=>direct+i),end};
      }
      // Subsequence candidate. We check a few plausible starts and keep the densest.
      for(let start=searchFrom;start<hay.length;start++){
        if(hay[start]!==token[0]) continue;
        const positions=[start]; let qi=1;
        for(let i=start+1;i<hay.length&&qi<token.length;i++) if(hay[i]===token[qi]){positions.push(i);qi++}
        if(qi!==token.length) continue;
        const span=positions.at(-1)-positions[0]+1;
        const density=token.length/span;
        const contiguous=positions.reduce((n,v,i)=>i&&v===positions[i-1]+1?n+1:n,0)/Math.max(1,token.length-1);
        let score=.48+density*.22+contiguous*.10;
        score+=Math.max(0,.08-positions[0]*.006);
        if(boundaries.has(positions[0])) score+=.055;
        if(hay===token) score+=.10;
        const candidate={score:Math.min(.995,score),positions,end:positions.at(-1)};
        if(!best||candidate.score>best.score) best=candidate;
        if(best.score>.97) break;
      }
      if(!best) return {success:false,score:0,indexes:[]};
      total+=best.score;
      for(const pos of best.positions) allIndexes.push(map[pos]);
      searchFrom=Math.min(hay.length,best.end+1);
    }
    let score=total/tokens.length;
    const foldedTarget=normalizeSearch(raw), foldedQuery=normalizeSearch(qRaw);
    if(foldedTarget===foldedQuery) score=Math.min(1,score+.08);
    return {success:true,score:Math.min(1,score),indexes:[...new Set(allIndexes)]};
  }

  function utf8Bytes(ch){
    return [...new TextEncoder().encode(ch)].map(x=>x.toString(16).toUpperCase().padStart(2,'0'));
  }

  function utf16Units(ch){
    const out=[];
    for(let i=0;i<ch.length;i++) out.push(ch.charCodeAt(i).toString(16).toUpperCase().padStart(4,'0'));
    return out;
  }

  function parseCodePointSequence(raw){
    const s=String(raw ?? '').trim();
    if(!s) return null;
    const tokens=s.split(/[\s,]+/).filter(Boolean);
    const parsed=[];
    for(const token of tokens){
      let m=token.match(/^U\+([0-9A-F]{1,6})$/i)
        || token.match(/^\\u\{([0-9A-F]{1,6})\}$/i)
        || token.match(/^\\u([0-9A-F]{4})$/i)
        || token.match(/^\\U([0-9A-F]{8})$/i)
        || token.match(/^0x([0-9A-F]{1,6})$/i);
      if(!m && tokens.length===1) m=token.match(/^([0-9A-F]{4,6})$/i);
      if(!m) return null;
      const cp=parseInt(m[1],16);
      if(cp<0 || cp>0x10FFFF || (cp>=0xD800&&cp<=0xDFFF)) return null;
      parsed.push(cp);
    }
    return parsed.length?parsed:null;
  }

  function describeUnicode(raw, unicodeMeta){
    const meta=unicodeMeta?.meta || unicodeMeta || {};
    const cps=parseCodePointSequence(raw);
    let chars;
    if(cps){
      chars=cps.map(cp=>String.fromCodePoint(cp));
    }else{
      const s=String(raw ?? '');
      if(!s) return {text:'',items:[],unicodeVersion:unicodeMeta?.unicodeVersion||''};
      chars=Array.from(s);
    }
    const items=chars.map(ch=>{
      const cp=ch.codePointAt(0);
      const hex=cp.toString(16).toUpperCase();
      const row=meta[hex] || ['', 'Cn', 0];
      return {
        char:ch,
        codePoint:`U+${hex.padStart(4,'0')}`,
        hex,
        decimal:cp,
        name:row[0] || (cp>=0x4E00&&cp<=0x9FFF?`CJK UNIFIED IDEOGRAPH-${hex}`:'<unnamed>'),
        category:row[1] || 'Cn',
        categoryName:CATEGORY_NAMES[row[1]] || row[1] || 'Unassigned',
        combiningClass:row[2] || 0,
        utf8:utf8Bytes(ch).join(' '),
        utf16:utf16Units(ch).join(' '),
        jsEscape:cp<=0xFFFF?`\\u${hex.padStart(4,'0')}`:`\\u{${hex}}`,
        htmlEntity:`&#x${hex};`
      };
    });
    return {text:chars.join(''),items,unicodeVersion:unicodeMeta?.unicodeVersion||''};
  }

  function parseInteger(raw){
    const s=String(raw ?? '').trim().replace(/_/g,'');
    if(!s) throw new Error('请输入整数');
    let sign=1n, body=s;
    if(body.startsWith('-')){sign=-1n; body=body.slice(1);} else if(body.startsWith('+')) body=body.slice(1);
    let base=10;
    if(/^0x/i.test(body)){base=16;body=body.slice(2)}
    else if(/^0b/i.test(body)){base=2;body=body.slice(2)}
    else if(/^0o/i.test(body)){base=8;body=body.slice(2)}
    else {
      const pref=body.match(/^(2|8|10|16)\s*:\s*(.+)$/i);
      if(pref){base=Number(pref[1]); body=pref[2];}
    }
    const re=base===2?/^[01]+$/:base===8?/^[0-7]+$/:base===16?/^[0-9a-f]+$/i:/^[0-9]+$/;
    if(!re.test(body)) throw new Error('无法识别该进制整数');
    let n=0n;
    for(const ch of body.toUpperCase()){
      const d=ch>='A'?ch.charCodeAt(0)-55:Number(ch);
      n=n*BigInt(base)+BigInt(d);
    }
    return sign*n;
  }

  function baseResults(raw){
    const n=parseInteger(raw);
    const sign=n<0n?'-':''; const v=n<0n?-n:n;
    const fmt=(base,prefix='')=>sign+prefix+v.toString(base).toUpperCase();
    return [
      {title:'二进制 BIN',value:fmt(2,'0b')},
      {title:'八进制 OCT',value:fmt(8,'0o')},
      {title:'十进制 DEC',value:n.toString(10)},
      {title:'十六进制 HEX',value:fmt(16,'0x')}
    ];
  }

  function toFullWidth(s, opts={}){
    const includeSpace=opts.space!==false;
    return Array.from(String(s)).map(ch=>{
      const cp=ch.codePointAt(0);
      if(ch===' ' && includeSpace) return '\u3000';
      if(cp>=0x21&&cp<=0x7E) return String.fromCodePoint(cp+0xFEE0);
      return ch;
    }).join('');
  }
  function toHalfWidth(s, opts={}){
    const includeSpace=opts.space!==false;
    return Array.from(String(s)).map(ch=>{
      const cp=ch.codePointAt(0);
      if(ch==='\u3000' && includeSpace) return ' ';
      if(cp>=0xFF01&&cp<=0xFF5E) return String.fromCodePoint(cp-0xFEE0);
      return ch;
    }).join('');
  }
  function widthResults(s){
    return [
      {title:'转换为半角',value:toHalfWidth(s)},
      {title:'转换为全角',value:toFullWidth(s)}
    ];
  }


  const UNIT_CATEGORIES = {
    length:[
      ['m',1,['m','meter','meters','metre','metres','米']],['km',1000,['km','kilometer','kilometers','kilometre','公里','千米']],['cm',.01,['cm','centimeter','厘米']],['mm',.001,['mm','millimeter','毫米']],['µm',1e-6,['um','µm','micrometer','微米']],['nm',1e-9,['nm','nanometer','纳米']],['in',.0254,['in','inch','inches','英寸']],['ft',.3048,['ft','foot','feet','英尺']],['yd',.9144,['yd','yard','yards','码']],['mi',1609.344,['mi','mile','miles','英里']]
    ],
    mass:[
      ['kg',1,['kg','kilogram','公斤','千克']],['g',.001,['g','gram','克']],['mg',1e-6,['mg','milligram','毫克']],['t',1000,['t','tonne','metricton','吨']],['lb',.45359237,['lb','lbs','pound','pounds','磅']],['oz',.028349523125,['oz','ounce','ounces','盎司']]
    ],
    duration:[
      ['ms',.001,['ms','millisecond','milliseconds','毫秒']],['s',1,['s','sec','second','seconds','秒']],['min',60,['min','minute','minutes','分钟']],['h',3600,['h','hr','hour','hours','小时']],['d',86400,['d','day','days','天']],['wk',604800,['wk','week','weeks','周','星期']]
    ],
    area:[
      ['m²',1,['m2','m²','sqm','平方米']],['km²',1e6,['km2','km²','平方公里','平方千米']],['cm²',1e-4,['cm2','cm²','平方厘米']],['ft²',.09290304,['ft2','ft²','sqft','平方英尺']],['in²',.00064516,['in2','in²','sqin','平方英寸']],['ha',10000,['ha','hectare','公顷']],['acre',4046.8564224,['acre','acres','英亩']]
    ],
    volume:[
      ['L',1,['l','liter','litre','liters','litres','升']],['mL',.001,['ml','milliliter','毫升']],['m³',1000,['m3','m³','立方米']],['cm³',.001,['cm3','cm³','cc','立方厘米']],['gal(US)',3.785411784,['gal','gallon','usgal','加仑']],['fl oz(US)',.0295735295625,['floz','fl oz','fluidounce','液盎司']]
    ],
    speed:[
      ['m/s',1,['m/s','mps','米每秒']],['km/h',1/3.6,['km/h','kph','kmh','公里每小时','千米每小时']],['mph',.44704,['mph','mileperhour','英里每小时']],['kn',.5144444444,['kn','knot','knots','节']],['ft/s',.3048,['ft/s','fps','英尺每秒']]
    ],
    pressure:[
      ['Pa',1,['pa','pascal','帕']],['kPa',1000,['kpa','千帕']],['MPa',1e6,['mpa','兆帕']],['bar',100000,['bar','巴']],['psi',6894.757293168,['psi']],['atm',101325,['atm','atmosphere','标准大气压']],['torr',133.322368421,['torr','托']]
    ],
    energy:[
      ['J',1,['j','joule','焦耳']],['kJ',1000,['kj','千焦']],['MJ',1e6,['mj','兆焦']],['Wh',3600,['wh','瓦时']],['kWh',3.6e6,['kwh','千瓦时','度电']],['cal',4.184,['cal','calorie','卡']],['kcal',4184,['kcal','kilocalorie','千卡','大卡']]
    ],
    power:[
      ['W',1,['w','watt','瓦']],['kW',1000,['kw','kilowatt','千瓦']],['MW',1e6,['mw','megawatt','兆瓦']],['hp',745.699871582,['hp','horsepower','马力']]
    ],
    information:[
      ['bit',.125,['bit','bits','位','比特']],['B',1,['b','byte','bytes','字节']],['KB',1000,['kb','kilobyte']],['KiB',1024,['kib','kibibyte']],['MB',1e6,['mb','megabyte']],['MiB',1048576,['mib','mebibyte']],['GB',1e9,['gb','gigabyte']],['GiB',1073741824,['gib','gibibyte']],['TB',1e12,['tb','terabyte']],['TiB',1099511627776,['tib','tebibyte']]
    ],
    angle:[
      ['deg',1,['deg','degree','degrees','°','度']],['rad',180/Math.PI,['rad','radian','弧度']],['grad',.9,['grad','gon','百分度']],['turn',360,['turn','圈','周']]
    ],
    acceleration:[
      ['m/s²',1,['m/s2','m/s²','mps2','米每二次方秒']],['g',9.80665,['gforce','g-force','重力加速度']],['ft/s²',.3048,['ft/s2','ft/s²','fps2']]
    ]
  };
  const TEMP_UNITS={
    c:{label:'°C',aliases:['c','°c','celsius','摄氏','摄氏度'],toBase:v=>v,fromBase:v=>v},
    f:{label:'°F',aliases:['f','°f','fahrenheit','华氏','华氏度'],toBase:v=>(v-32)*5/9,fromBase:v=>v*9/5+32},
    k:{label:'K',aliases:['k','kelvin','开尔文'],toBase:v=>v-273.15,fromBase:v=>v+273.15}
  };
  function normUnit(s){return String(s??'').trim().normalize('NFKC').toLocaleLowerCase().replace(/\s+/g,'').replace(/²/g,'2').replace(/³/g,'3');}
  function unitLookup(raw){
    const n=normUnit(raw); if(!n)return null;
    for(const [cat,rows] of Object.entries(UNIT_CATEGORIES)) for(const [label,factor,aliases] of rows) if(aliases.some(a=>normUnit(a)===n)) return {cat,label,factor};
    for(const [key,u] of Object.entries(TEMP_UNITS)) if(u.aliases.some(a=>normUnit(a)===n)) return {cat:'temperature',label:u.label,key,...u};
    return null;
  }
  function formatUnitNumber(n){
    if(!Number.isFinite(n))return String(n);
    const a=Math.abs(n); if((a&&a<1e-6)||a>=1e12)return n.toExponential(8).replace(/0+e/,'e');
    return new Intl.NumberFormat('en-US',{maximumSignificantDigits:12,useGrouping:false}).format(n);
  }
  function unitConvert(value,from,to){
    if(from.cat!==to.cat)throw new Error('单位类别不一致');
    if(from.cat==='temperature'){const c=from.toBase(value);return to.fromBase(c);}
    return value*from.factor/to.factor;
  }
  function parseUnitQuery(raw){
    const s=String(raw??'').trim();
    const m=s.match(/^([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?)\s*([^\d]+?)(?:\s+(?:to|in|=|→|转|转换为|换算为)\s*([^\d]+))?$/i);
    if(!m)return null;
    const value=Number(m[1]); if(!Number.isFinite(value))return null;
    const from=unitLookup(m[2]); if(!from)return null;
    const to=m[3]?unitLookup(m[3]):null; if(m[3]&&!to)return null;
    if(to&&to.cat!==from.cat)return null;
    return{value,from,to};
  }
  function unitResults(raw){
    const p=parseUnitQuery(raw); if(!p)throw new Error('无法识别单位转换，例如：10 km to mi');
    if(p.to){const v=unitConvert(p.value,p.from,p.to);return[{title:`${p.from.label} → ${p.to.label}`,value:`${formatUnitNumber(v)} ${p.to.label}`}];}
    if(p.from.cat==='temperature'){
      return Object.values(TEMP_UNITS).filter(x=>x.label!==p.from.label).map(to=>({title:`${p.from.label} → ${to.label}`,value:`${formatUnitNumber(unitConvert(p.value,p.from,{cat:'temperature',...to}))} ${to.label}`}));
    }
    const rows=UNIT_CATEGORIES[p.from.cat]||[];
    return rows.filter(([label])=>label!==p.from.label).slice(0,8).map(([label,factor,aliases])=>{const to={cat:p.from.cat,label,factor};return{title:`${p.from.label} → ${label}`,value:`${formatUnitNumber(unitConvert(p.value,p.from,to))} ${label}`}});
  }

  function percentEncode(s){ return encodeURIComponent(String(s)); }
  function percentDecode(s){ try{return decodeURIComponent(String(s));}catch{throw new Error('无效的百分号编码');} }
  function unicodeEscapeEncode(s){
    return Array.from(String(s)).map(ch=>{
      const cp=ch.codePointAt(0), h=cp.toString(16).toUpperCase();
      return cp<=0xFFFF?`\\u${h.padStart(4,'0')}`:`\\u{${h}}`;
    }).join('');
  }
  function unicodeEscapeDecode(s){
    const raw=String(s);
    return raw
      .replace(/\\u\{([0-9a-fA-F]{1,6})\}/g,(_,h)=>String.fromCodePoint(parseInt(h,16)))
      .replace(/\\U([0-9a-fA-F]{8})/g,(_,h)=>String.fromCodePoint(parseInt(h,16)))
      .replace(/\\u([0-9a-fA-F]{4})/g,(_,h)=>String.fromCharCode(parseInt(h,16)));
  }
  function htmlEncode(s){
    return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;');
  }
  function htmlDecode(s){
    const named={amp:'&',lt:'<',gt:'>',quot:'"',apos:"'",nbsp:'\u00A0'};
    return String(s).replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi,(m,k)=>{
      if(k[0]==='#'){
        const cp=k[1].toLowerCase()==='x'?parseInt(k.slice(2),16):parseInt(k.slice(1),10);
        return Number.isFinite(cp)&&cp<=0x10FFFF?String.fromCodePoint(cp):m;
      }
      return Object.prototype.hasOwnProperty.call(named,k.toLowerCase())?named[k.toLowerCase()]:m;
    });
  }
  function jsonEscape(s){ return JSON.stringify(String(s)).slice(1,-1); }
  function jsonUnescape(s){ try{return JSON.parse(`"${String(s).replace(/"/g,'\\"')}"`)}catch{throw new Error('无效的 JSON 转义文本');} }

  function escapeResults(s){
    const out=[];
    const push=(title,value)=>{ if(!out.some(x=>x.value===value)) out.push({title,value}); };
    try{ if(/%[0-9a-fA-F]{2}/.test(s)) push('百分号 / URL 解码',percentDecode(s)); }catch{}
    if(/\\u\{|\\u[0-9a-fA-F]{4}|\\U[0-9a-fA-F]{8}/.test(s)) push('Unicode 转义解码',unicodeEscapeDecode(s));
    if(/&(?:#x[0-9a-f]+|#\d+|[a-z]+);/i.test(s)) push('HTML 实体解码',htmlDecode(s));
    push('百分号 / URL 编码',percentEncode(s));
    push('Unicode 转义编码',unicodeEscapeEncode(s));
    push('HTML 实体编码',htmlEncode(s));
    push('JSON 字符串转义',jsonEscape(s));
    return out;
  }

  function countryIndex(dataset){
    const countries=dataset?.countries || [];
    return countries.map(c=>{
      const fields=[c.nameZh,c.officialNameZh,c.nameEn,c.officialNameEn,c.alpha2,c.alpha3,c.numeric,c.ioc,c.fifa,c.itu,
        ...(c.callingCodes||[]).map(x=>'+'+String(x).replace(/^\+/,'')),
        ...(c.callingCodes||[]),...(c.tlds||[]),...(c.aliases||[])].filter(Boolean);
      return {...c,_search:fields.map(normalizeSearch)};
    });
  }

  function countryScore(c,q){
    const n=normalizeSearch(q);
    if(!n) return 0;
    let best=0;
    for(const f of c._search||[]){
      if(f===n) best=Math.max(best,1);
      else if(f.startsWith(n)) best=Math.max(best,.9-Math.min(.08,(f.length-n.length)*.002));
      else if(f.includes(n)) best=Math.max(best,.72);
    }
    return best;
  }

  function findCountries(index,q,limit=8){
    const raw=String(q ?? '').trim();
    if(!raw) return [];
    return index.map(c=>({country:c,score:countryScore(c,raw)})).filter(x=>x.score>0).sort((a,b)=>b.score-a.score||a.country.nameEn.localeCompare(b.country.nameEn)).slice(0,limit);
  }

  function countryResultItems(c){
    const calls=(c.callingCodes||[]).map(x=>'+'+String(x).replace(/^\+/,''));
    return [
      {title:`${c.flagEmoji||''} ${c.nameZh}`,value:c.nameEn,kind:'country-header'},
      {title:'ISO 3166-1 Alpha-2',value:c.alpha2},
      {title:'ISO 3166-1 Alpha-3',value:c.alpha3},
      {title:'ISO 3166-1 Numeric',value:c.numeric},
      ...(c.ioc?[{title:'IOC',value:c.ioc}]:[]),
      ...(c.fifa?[{title:'FIFA',value:c.fifa}]:[]),
      ...(c.itu?[{title:'ITU',value:c.itu}]:[]),
      ...(calls.length?[{title:'国际电话区号',value:calls.join(', ')}]:[]),
      ...((c.tlds||[]).length?[{title:'国家/地区顶级域名 ccTLD',value:c.tlds.join(', ')}]:[]),
      {title:'国旗 Emoji',value:c.flagEmoji||''},
      {title:'国旗 SVG',value:c.flagSvg||'',kind:'flag-url'},
      {title:'国旗 PNG 250px',value:c.flagPng250||'',kind:'flag-url'},
    ].filter(x=>x.value!==undefined&&x.value!==null&&x.value!=='');
  }


  // Calculator parser inspired by the mature launcher behavior in PowerToys Run,
  // but implemented independently for QuickTool. Supports locale-aware numbers,
  // constants/functions and implicit multiplication such as 2(1+2) and 2pi.
  function calculatorLocaleSeparators(locale='en-US'){
    try{
      const parts=new Intl.NumberFormat(locale).formatToParts(12345.6);
      return {decimal:parts.find(x=>x.type==='decimal')?.value||'.',group:parts.find(x=>x.type==='group')?.value||','};
    }catch{return{decimal:'.',group:','}}
  }
  function tokenizeCalculation(input,locale='en-US'){
    const s=String(input??'').trim(); if(!s)throw new Error('请输入表达式');
    const {decimal,group}=calculatorLocaleSeparators(locale), out=[];
    const isDigit=c=>c>='0'&&c<='9', isIdStart=c=>/[A-Za-z_π]/.test(c||''), isId=c=>/[A-Za-z0-9_π]/.test(c||'');
    let i=0;
    while(i<s.length){
      const ch=s[i]; if(/\s/.test(ch)){i++;continue}
      if(isDigit(ch)||ch===decimal||(decimal!=='.'&&ch==='.')){
        const start=i; let raw='',seenDecimal=false;
        while(i<s.length){
          const c=s[i];
          if(isDigit(c)){raw+=c;i++;continue}
          if((c===decimal||(decimal!=='.'&&c==='.'&&!seenDecimal))&&!seenDecimal){raw+='.';seenDecimal=true;i++;continue}
          if(c===group&&group!==decimal){
            // Treat a group separator as part of a number only for real 3-digit grouping.
            const tail=s.slice(i+1).match(/^(\d{3})(?!\d)/);
            if(tail){i++;continue}
          }
          break;
        }
        if(raw==='.'||!/^\d*\.?\d+(?:[eE][+-]?\d+)?$/.test(raw))throw new Error(`无法识别数字：${s.slice(start,i)}`);
        const value=Number(raw); if(!Number.isFinite(value))throw new Error('数字超出范围'); out.push({t:'num',v:value}); continue;
      }
      if(isIdStart(ch)){
        let j=i+1;while(j<s.length&&isId(s[j]))j++;
        out.push({t:'id',v:s.slice(i,j).toLowerCase().replace('π','pi')});i=j;continue;
      }
      if('+-*/%^(),;!'.includes(ch)){out.push({t:ch==='('? 'lp':ch===')'?'rp':(ch===','||ch===';')?'sep':'op',v:ch});i++;continue}
      throw new Error(`不支持的字符：${ch}`);
    }
    return out;
  }
  const CALC_CONSTANTS={pi:Math.PI,e:Math.E,tau:Math.PI*2};
  const FACTORIAL_LIMIT=170;
  function factorial(n){
    if(!Number.isInteger(n)||n<0) throw new Error('阶乘仅支持非负整数，例如 5!');
    if(n>FACTORIAL_LIMIT) throw new Error('阶乘结果超出范围');
    let v=1; for(let i=2;i<=n;i++) v*=i; return v;
  }
  const CALC_FUNCTIONS={
    sin:a=>Math.sin(a),cos:a=>Math.cos(a),tan:a=>Math.tan(a),asin:a=>Math.asin(a),acos:a=>Math.acos(a),atan:a=>Math.atan(a),
    sqrt:a=>Math.sqrt(a),abs:a=>Math.abs(a),floor:a=>Math.floor(a),ceil:a=>Math.ceil(a),round:a=>Math.round(a),
    ln:a=>Math.log(a),log:a=>Math.log10(a),log10:a=>Math.log10(a),exp:a=>Math.exp(a),
    min:(...a)=>Math.min(...a),max:(...a)=>Math.max(...a),pow:(a,b)=>Math.pow(a,b),hypot:(...a)=>Math.hypot(...a)
  };
  function calculateExpression(input,locale='en-US'){
    const tokens=tokenizeCalculation(input,locale);let pos=0;
    const peek=()=>tokens[pos],take=()=>tokens[pos++],beginsPrimary=t=>t&&(t.t==='num'||t.t==='id'||t.t==='lp');
    function primary(){
      const t=take(); if(!t)throw new Error('表达式不完整');
      if(t.t==='num')return t.v;
      if(t.t==='lp'){const v=expr();if(take()?.t!=='rp')throw new Error('括号不匹配');return v}
      if(t.t==='id'){
        const name=t.v;
        if(peek()?.t==='lp'){
          take();const args=[];
          if(peek()?.t!=='rp'){
            while(true){args.push(expr());if(peek()?.t==='sep'){take();continue}break}
          }
          if(take()?.t!=='rp')throw new Error('函数括号不匹配');
          const fn=CALC_FUNCTIONS[name];if(!fn)throw new Error(`未知函数：${name}`);
          const v=fn(...args);if(!Number.isFinite(v))throw new Error('计算结果无效');return v;
        }
        if(Object.prototype.hasOwnProperty.call(CALC_CONSTANTS,name))return CALC_CONSTANTS[name];
        throw new Error(`未知常量：${name}`);
      }
      throw new Error('表达式语法错误');
    }
    function unary(){const t=peek();if(t?.t==='op'&&(t.v==='+'||t.v==='-')){take();const v=postfix();return t.v==='-'?-v:v}return postfix()}
    // Factorial is a postfix operator and binds tighter than ^: 5!^2 === (5!)^2.
    function postfix(){let v=primary();while(peek()?.t==='op'&&peek().v==='!'){take();v=factorial(v)}return v}
    function power(){let left=unary();if(peek()?.t==='op'&&peek().v==='^'){take();left=Math.pow(left,power())}return left}
    function term(){let left=power();while(true){const t=peek();if(t?.t==='op'&&['*','/','%'].includes(t.v)){take();const r=power();left=t.v==='*'?left*r:t.v==='/'?left/r:left%r;continue}if(beginsPrimary(t)){left*=power();continue}break}return left}
    function expr(){let left=term();while(peek()?.t==='op'&&['+','-'].includes(peek().v)){const op=take().v,r=term();left=op==='+'?left+r:left-r}return left}
    const value=expr();if(pos!==tokens.length)throw new Error('表达式尾部存在无法解析的内容');if(!Number.isFinite(value))throw new Error('计算结果无效');return value;
  }
  function formatCalculationResult(value,locale='en-US'){
    if(!Number.isFinite(value))throw new Error('计算结果无效');
    if(Number.isInteger(value)&&Math.abs(value)<1e21)return new Intl.NumberFormat(locale,{useGrouping:false,maximumFractionDigits:0}).format(value);
    return new Intl.NumberFormat(locale,{useGrouping:false,maximumSignificantDigits:14}).format(value);
  }

  const WINDOWS_FILETIME_EPOCH_MS=11644473600000;
  function isoWeek(date){
    const d=new Date(Date.UTC(date.getFullYear(),date.getMonth(),date.getDate()));d.setUTCDate(d.getUTCDate()+4-(d.getUTCDay()||7));
    const y=new Date(Date.UTC(d.getUTCFullYear(),0,1));return{year:d.getUTCFullYear(),week:Math.ceil((((d-y)/86400000)+1)/7)};
  }
  function windowsFileTimeFromDate(date){return BigInt(Math.trunc(date.getTime()+WINDOWS_FILETIME_EPOCH_MS))*10000n}
  function dateFromWindowsFileTime(raw){const n=BigInt(String(raw).trim());return new Date(Number(n/10000n)-WINDOWS_FILETIME_EPOCH_MS)}
  function oleAutomationFromDate(date){return date.getTime()/86400000+25569}
  function dateFromOleAutomation(v){return new Date((Number(v)-25569)*86400000)}
  function excel1900FromDate(date){return oleAutomationFromDate(date)}
  function dateFromExcel1900(v){return dateFromOleAutomation(v)}
  function excel1904FromDate(date){return date.getTime()/86400000-(-24107)}
  function dateFromExcel1904(v){return new Date((Number(v)-24107)*86400000)}
  function formatDatePattern(date,pattern,utc=false){
    const get=(local,universal)=>utc?universal.call(date):local.call(date), pad=(n,w=2)=>String(n).padStart(w,'0');
    const values={
      YYYY:()=>String(get(Date.prototype.getFullYear,Date.prototype.getUTCFullYear)),
      YY:()=>String(get(Date.prototype.getFullYear,Date.prototype.getUTCFullYear)).slice(-2),
      MM:()=>pad(get(Date.prototype.getMonth,Date.prototype.getUTCMonth)+1),
      DD:()=>pad(get(Date.prototype.getDate,Date.prototype.getUTCDate)),
      HH:()=>pad(get(Date.prototype.getHours,Date.prototype.getUTCHours)),
      mm:()=>pad(get(Date.prototype.getMinutes,Date.prototype.getUTCMinutes)),
      ss:()=>pad(get(Date.prototype.getSeconds,Date.prototype.getUTCSeconds)),
      SSS:()=>pad(get(Date.prototype.getMilliseconds,Date.prototype.getUTCMilliseconds),3),
      X:()=>String(Math.floor(date.getTime()/1000)),x:()=>String(date.getTime()),
      WW:()=>String(isoWeek(date).week).padStart(2,'0'),W:()=>String(isoWeek(date).week)
    };
    return String(pattern).replace(/YYYY|SSS|YY|MM|DD|HH|mm|ss|WW|X|x|W/g,t=>values[t]());
  }
  function parseTimeSpecial(raw){
    const s=String(raw??'').trim(),m=s.match(/^(ft|filetime|oa|ole|exc|excel1900|exf|excel1904)\s*[:=]?\s*(-?\d+(?:\.\d+)?)$/i);if(!m)return null;
    const kind=m[1].toLowerCase(),value=m[2];let date;
    try{if(kind==='ft'||kind==='filetime')date=dateFromWindowsFileTime(value);else if(kind==='oa'||kind==='ole')date=dateFromOleAutomation(value);else if(kind==='exc'||kind==='excel1900')date=dateFromExcel1900(value);else date=dateFromExcel1904(value)}catch{return null}
    return Number.isFinite(date?.getTime())?{kind,date}:null;
  }
  function extendedTimeResults(date,customFormats=[]){
    const d=date instanceof Date?date:new Date(date);if(!Number.isFinite(d.getTime()))throw new Error('无效日期');const week=isoWeek(d);
    const rows=[
      {title:'Windows File Time',value:windowsFileTimeFromDate(d).toString()},
      {title:'OLE Automation Date',value:String(Number(oleAutomationFromDate(d).toFixed(10)))},
      {title:'Excel 1900',value:String(Number(excel1900FromDate(d).toFixed(10)))},
      {title:'Excel 1904',value:String(Number(excel1904FromDate(d).toFixed(10)))},
      {title:'ISO 周数',value:`${week.year}-W${String(week.week).padStart(2,'0')}`}
    ];
    for(const row of customFormats||[]){
      const line=String(row??'').trim();if(!line)continue;const eq=line.indexOf('=');if(eq<=0)continue;const name=line.slice(0,eq).trim(),rawPattern=line.slice(eq+1).trim();const utc=/^UTC:/i.test(rawPattern),pattern=rawPattern.replace(/^UTC:/i,'');
      try{rows.push({title:name,value:formatDatePattern(d,pattern,utc)})}catch{}
    }
    return rows;
  }


  const clamp01=x=>Math.min(1,Math.max(0,Number(x)||0));
  const round=(x,n=4)=>Number(Number(x).toFixed(n));
  function srgbToLinear(c){ c=clamp01(c); return c<=0.04045?c/12.92:Math.pow((c+0.055)/1.055,2.4); }
  function linearToSrgb(c){ c=Math.max(0,c); return clamp01(c<=0.0031308?12.92*c:1.055*Math.pow(c,1/2.4)-0.055); }
  function rgba(r,g,b,a=1){ return {r:clamp01(r),g:clamp01(g),b:clamp01(b),a:clamp01(a)}; }
  function hex2(n){ return Math.round(clamp01(n)*255).toString(16).toUpperCase().padStart(2,'0'); }
  function rgbaHex(c,withAlpha=true){ return `#${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}${withAlpha?hex2(c.a):''}`; }
  function parseAlpha(v){ const s=String(v??'').trim(); return s.endsWith('%')?clamp01(parseFloat(s)/100):clamp01(parseFloat(s)); }
  function parseRgbChannel(v){ const s=String(v??'').trim(); return s.endsWith('%')?clamp01(parseFloat(s)/100):clamp01(parseFloat(s)/255); }
  function parseAngle(v){ const s=String(v??'').trim().toLowerCase(); let x=parseFloat(s); if(!Number.isFinite(x))return 0; if(s.endsWith('turn'))x*=360; else if(s.endsWith('rad'))x=x*180/Math.PI; else if(s.endsWith('grad'))x*=.9; return ((x%360)+360)%360; }
  function hslToRgb(h,s,l,a=1){ h=((h%360)+360)%360/360;s=clamp01(s);l=clamp01(l); if(s===0)return rgba(l,l,l,a); const q=l<.5?l*(1+s):l+s-l*s,p=2*l-q; const f=t=>{if(t<0)t+=1;if(t>1)t-=1;if(t<1/6)return p+(q-p)*6*t;if(t<1/2)return q;if(t<2/3)return p+(q-p)*(2/3-t)*6;return p}; return rgba(f(h+1/3),f(h),f(h-1/3),a); }
  function rgbToHsl(c){ const r=c.r,g=c.g,b=c.b,max=Math.max(r,g,b),min=Math.min(r,g,b),l=(max+min)/2; let h=0,s=0; if(max!==min){const d=max-min;s=l>.5?d/(2-max-min):d/(max+min);switch(max){case r:h=(g-b)/d+(g<b?6:0);break;case g:h=(b-r)/d+2;break;default:h=(r-g)/d+4}h*=60}return{h,s,l}; }
  function hsvToRgb(h,s,v,a=1){ h=((h%360)+360)%360;s=clamp01(s);v=clamp01(v); const c=v*s,x=c*(1-Math.abs((h/60)%2-1)),m=v-c; let r=0,g=0,b=0; if(h<60)[r,g,b]=[c,x,0];else if(h<120)[r,g,b]=[x,c,0];else if(h<180)[r,g,b]=[0,c,x];else if(h<240)[r,g,b]=[0,x,c];else if(h<300)[r,g,b]=[x,0,c];else [r,g,b]=[c,0,x];return rgba(r+m,g+m,b+m,a); }
  function rgbToHsv(c){const max=Math.max(c.r,c.g,c.b),min=Math.min(c.r,c.g,c.b),d=max-min;let h=0;if(d){if(max===c.r)h=60*(((c.g-c.b)/d)%6);else if(max===c.g)h=60*((c.b-c.r)/d+2);else h=60*((c.r-c.g)/d+4)}if(h<0)h+=360;return{h,s:max===0?0:d/max,v:max};}
  function hwbToRgb(h,w,b,a=1){w=clamp01(w);b=clamp01(b);if(w+b>=1){const g=w/(w+b);return rgba(g,g,g,a)}const base=hsvToRgb(h,1,1,a),f=1-w-b;return rgba(base.r*f+w,base.g*f+w,base.b*f+w,a)}
  function rgbToHwb(c){const hsv=rgbToHsv(c);return{h:hsv.h,w:Math.min(c.r,c.g,c.b),b:1-Math.max(c.r,c.g,c.b)}}
  function cmykToRgb(c,m,y,k,a=1){return rgba((1-c)*(1-k),(1-m)*(1-k),(1-y)*(1-k),a)}
  function rgbToCmyk(c){const k=1-Math.max(c.r,c.g,c.b);if(k>=.999999)return{c:0,m:0,y:0,k:1};return{c:(1-c.r-k)/(1-k),m:(1-c.g-k)/(1-k),y:(1-c.b-k)/(1-k),k};}
  function rgbToXyz(c){const r=srgbToLinear(c.r),g=srgbToLinear(c.g),b=srgbToLinear(c.b);return{x:.4124564*r+.3575761*g+.1804375*b,y:.2126729*r+.7151522*g+.072175*b,z:.0193339*r+.119192*g+.9503041*b};}
  function xyzToRgb(x,y,z,a=1){const r=3.2404542*x-1.5371385*y-.4985314*z,g=-.969266*x+1.8760108*y+.041556*z,b=.0556434*x-.2040259*y+1.0572252*z;return rgba(linearToSrgb(r),linearToSrgb(g),linearToSrgb(b),a)}
  function xyzToLab(x,y,z){const Xn=.95047,Yn=1,Zn=1.08883,d=6/29, f=t=>t>d**3?Math.cbrt(t):t/(3*d*d)+4/29;const fx=f(x/Xn),fy=f(y/Yn),fz=f(z/Zn);return{L:116*fy-16,a:500*(fx-fy),b:200*(fy-fz)}}
  function labToXyz(L,a,b){const Xn=.95047,Yn=1,Zn=1.08883,d=6/29,fy=(L+16)/116,fx=fy+a/500,fz=fy-b/200,finv=t=>t>d?t**3:3*d*d*(t-4/29);return{x:Xn*finv(fx),y:Yn*finv(fy),z:Zn*finv(fz)}}
  function labToLch(lab){const C=Math.hypot(lab.a,lab.b),h=(Math.atan2(lab.b,lab.a)*180/Math.PI+360)%360;return{L:lab.L,C,h}}
  function lchToLab(L,C,h){const r=h*Math.PI/180;return{L,a:C*Math.cos(r),b:C*Math.sin(r)}}
  function rgbToOklab(c){const r=srgbToLinear(c.r),g=srgbToLinear(c.g),b=srgbToLinear(c.b);const l=.4122214708*r+.5363325363*g+.0514459929*b,m=.2119034982*r+.6806995451*g+.1073969566*b,s=.0883024619*r+.2817188376*g+.6299787005*b;const l_=Math.cbrt(l),m_=Math.cbrt(m),s_=Math.cbrt(s);return{L:.2104542553*l_+.793617785*m_-.0040720468*s_,a:1.9779984951*l_-2.428592205*m_+.4505937099*s_,b:.0259040371*l_+.7827717662*m_-.808675766*s_}}
  function oklabToRgb(L,a,b,alpha=1){const l_ = L+.3963377774*a+.2158037573*b,m_=L-.1055613458*a-.0638541728*b,s_=L-.0894841775*a-1.291485548*b,l=l_**3,m=m_**3,s=s_**3;const r=4.0767416621*l-3.3077115913*m+.2309699292*s,g=-1.2684380046*l+2.6097574011*m-.3413193965*s,bb=-.0041960863*l-.7034186147*m+1.707614701*s;return rgba(linearToSrgb(r),linearToSrgb(g),linearToSrgb(bb),alpha)}
  function oklabToOklch(v){return{L:v.L,C:Math.hypot(v.a,v.b),h:(Math.atan2(v.b,v.a)*180/Math.PI+360)%360}}
  function oklchToOklab(L,C,h){const r=h*Math.PI/180;return{L,a:C*Math.cos(r),b:C*Math.sin(r)}}
  function rgbToHsi(c){const r=c.r,g=c.g,b=c.b,I=(r+g+b)/3,min=Math.min(r,g,b),S=I===0?0:1-min/I;const num=.5*((r-g)+(r-b)),den=Math.sqrt((r-g)**2+(r-b)*(g-b));let theta=den===0?0:Math.acos(Math.min(1,Math.max(-1,num/den)))*180/Math.PI;const H=b<=g?theta:360-theta;return{h:H,s:S,i:I}}
  function hsiToRgb(h,s,i,a=1){h=((h%360)+360)%360;s=clamp01(s);i=clamp01(i);const rad=x=>x*Math.PI/180;let r,g,b;if(h<120){b=i*(1-s);r=i*(1+s*Math.cos(rad(h))/Math.max(1e-9,Math.cos(rad(60-h))));g=3*i-r-b}else if(h<240){const hh=h-120;r=i*(1-s);g=i*(1+s*Math.cos(rad(hh))/Math.max(1e-9,Math.cos(rad(60-hh))));b=3*i-r-g}else{const hh=h-240;g=i*(1-s);b=i*(1+s*Math.cos(rad(hh))/Math.max(1e-9,Math.cos(rad(60-hh))));r=3*i-g-b}return rgba(r,g,b,a)}
  function ycbcrToRgb(y,cb,cr,kr=.299,kb=.114,limited=false,a=1){let Y,Cb,Cr;if(limited){Y=(y-16)/219;Cb=(cb-128)/224;Cr=(cr-128)/224}else{Y=y/255;Cb=(cb-128)/255;Cr=(cr-128)/255}const kg=1-kr-kb,r=Y+2*(1-kr)*Cr,b=Y+2*(1-kb)*Cb,g=(Y-kr*r-kb*b)/kg;return rgba(r,g,b,a)}
  function yuvToRgb(y,u,v,a=1){return rgba(y+1.13983*v,y-.39465*u-.58060*v,y+2.03211*u,a)}
  function p3ToRgb(r,g,b,a=1){r=srgbToLinear(r);g=srgbToLinear(g);b=srgbToLinear(b);const x=.48657095*r+.26566769*g+.19821729*b,y=.22897456*r+.69173852*g+.07928691*b,z=.04511338*g+1.04394437*b;return xyzToRgb(x,y,z,a)}
  function ycbcr(c,kr,kb,limited=false){const kg=1-kr-kb,Y=kr*c.r+kg*c.g+kb*c.b,Cb=(c.b-Y)/(2*(1-kb)),Cr=(c.r-Y)/(2*(1-kr));return limited?{y:16+219*Y,cb:128+224*Cb,cr:128+224*Cr}:{y:255*Y,cb:128+255*Cb,cr:128+255*Cr}}
  function rgbToP3(c){const xyz=rgbToXyz(c),r=2.4934969*xyz.x-.9313836*xyz.y-.4027108*xyz.z,g=-.829489*xyz.x+1.762664*xyz.y+.0236247*xyz.z,b=.0358458*xyz.x-.0761724*xyz.y+.9568845*xyz.z;return{r:linearToSrgb(r),g:linearToSrgb(g),b:linearToSrgb(b)}}

  function parseFunctionColor(raw){
    const m=String(raw).trim().match(/^([a-z][a-z0-9-]*)\((.*)\)$/i);if(!m)return null;const fn=m[1].toLowerCase();let body=m[2].trim();const slash=body.split('/');let alpha=slash.length>1?parseAlpha(slash.pop()):1;body=slash.join('/').trim();const parts=body.replace(/,/g,' ').trim().split(/\s+/).filter(Boolean);
    if(fn==='color'&&parts[0]?.toLowerCase()==='display-p3'&&parts.length>=4)return p3ToRgb(Number(parts[1]),Number(parts[2]),Number(parts[3]),alpha);
    if((fn==='rgb'||fn==='rgba')&&parts.length>=3){if(fn==='rgba'&&parts.length>=4&&slash.length===1)alpha=parseAlpha(parts[3]);return rgba(parseRgbChannel(parts[0]),parseRgbChannel(parts[1]),parseRgbChannel(parts[2]),alpha)}
    if((fn==='hsl'||fn==='hsla')&&parts.length>=3){if(fn==='hsla'&&parts.length>=4&&slash.length===1)alpha=parseAlpha(parts[3]);return hslToRgb(parseAngle(parts[0]),parseFloat(parts[1])/100,parseFloat(parts[2])/100,alpha)}
    if((fn==='hsv'||fn==='hsb')&&parts.length>=3)return hsvToRgb(parseAngle(parts[0]),parseFloat(parts[1])/100,parseFloat(parts[2])/100,alpha);
    if(fn==='hwb'&&parts.length>=3)return hwbToRgb(parseAngle(parts[0]),parseFloat(parts[1])/100,parseFloat(parts[2])/100,alpha);
    if(fn==='hsi'&&parts.length>=3)return hsiToRgb(parseAngle(parts[0]),parseFloat(parts[1])/100,parseFloat(parts[2])/100,alpha);
    if(fn==='yuv'&&parts.length>=3)return yuvToRgb(Number(parts[0]),Number(parts[1]),Number(parts[2]),alpha);
    if((fn==='ycbcr'||fn==='ycbcr601'||fn==='ycbcr709'||fn==='ycbcr2020')&&parts.length>=3){const k=fn==='ycbcr709'?[.2126,.0722]:fn==='ycbcr2020'?[.2627,.0593]:[.299,.114];return ycbcrToRgb(Number(parts[0]),Number(parts[1]),Number(parts[2]),k[0],k[1],false,alpha)}
    if(fn==='cmyk'&&parts.length>=4)return cmykToRgb(parseFloat(parts[0])/100,parseFloat(parts[1])/100,parseFloat(parts[2])/100,parseFloat(parts[3])/100,alpha);
    if(fn==='lab'&&parts.length>=3){const L=parseFloat(parts[0]),aa=parseFloat(parts[1]),bb=parseFloat(parts[2]);return xyzToRgb(...Object.values(labToXyz(L,aa,bb)),alpha)}
    if(fn==='lch'&&parts.length>=3){const lab=lchToLab(parseFloat(parts[0]),parseFloat(parts[1]),parseAngle(parts[2]));const xyz=labToXyz(lab.L,lab.a,lab.b);return xyzToRgb(xyz.x,xyz.y,xyz.z,alpha)}
    if(fn==='oklab'&&parts.length>=3)return oklabToRgb(parseFloat(parts[0]),parseFloat(parts[1]),parseFloat(parts[2]),alpha);
    if(fn==='oklch'&&parts.length>=3){const o=oklchToOklab(parseFloat(parts[0]),parseFloat(parts[1]),parseAngle(parts[2]));return oklabToRgb(o.L,o.a,o.b,alpha)}
    if(fn==='xyz'&&parts.length>=3){let [x,y,z]=parts.map(Number);if(Math.max(Math.abs(x),Math.abs(y),Math.abs(z))>2){x/=100;y/=100;z/=100}return xyzToRgb(x,y,z,alpha)}
    return null;
  }

  function parseColorCandidates(raw,cssNames={}){
    const s=String(raw??'').trim(); if(!s)return[]; const out=[]; const push=(label,color,source=s)=>{if(color&&!out.some(x=>rgbaHex(x.color,true)===rgbaHex(color,true)))out.push({label,color,source})};
    if(/^transparent$/i.test(s))push('CSS transparent',rgba(0,0,0,0));
    const named=cssNames[s.toLowerCase()]; if(named){const h=named.replace('#','');push(`CSS ${s.toLowerCase()}`,rgba(parseInt(h.slice(0,2),16)/255,parseInt(h.slice(2,4),16)/255,parseInt(h.slice(4,6),16)/255,1))}
    const hm=s.match(/^#([0-9a-f]{1,8})$/i)||s.match(/^([0-9a-f]{6}|[0-9a-f]{8})$/i); if(hm){const h=hm[1];
      const from=(hh)=>rgba(parseInt(hh.slice(0,2),16)/255,parseInt(hh.slice(2,4),16)/255,parseInt(hh.slice(4,6),16)/255,hh.length>=8?parseInt(hh.slice(6,8),16)/255:1);
      if(h.length===1)push('QuickTool 短 HEX',from(h.repeat(6)));
      else if(h.length===2){push('QuickTool 灰度短 HEX',from(h.repeat(3)));push('灰度 + Alpha',from(h[0].repeat(6)+h[1].repeat(2)))}
      else if(h.length===3){push('CSS #RGB',from([...h].map(x=>x+x).join('')));push('QuickTool 重复模式',from(h+h))}
      else if(h.length===4)push('CSS #RGBA',from([...h].map(x=>x+x).join('')));
      else if(h.length===6)push('HEX',from(h));
      else if(h.length===8)push('HEXA #RRGGBBAA',from(h));
    }
    const f=parseFunctionColor(s);if(f)push('CSS / Color Function',f);
    return out;
  }

  function colorFormats(c){
    const rgb=[c.r,c.g,c.b].map(x=>Math.round(x*255)),hsl=rgbToHsl(c),hsv=rgbToHsv(c),hwb=rgbToHwb(c),cmyk=rgbToCmyk(c),xyz=rgbToXyz(c),lab=xyzToLab(xyz.x,xyz.y,xyz.z),lch=labToLch(lab),ok=rgbToOklab(c),okc=oklabToOklch(ok),hsi=rgbToHsi(c),p3=rgbToP3(c);
    const y601=ycbcr(c,.299,.114,false),y709=ycbcr(c,.2126,.0722,false),y2020=ycbcr(c,.2627,.0593,false),yl709=ycbcr(c,.2126,.0722,true);
    const a=round(c.a,3), alphaSuffix=c.a<.999?` / ${a}`:'';
    return [
      {title:'HEX',value:rgbaHex(c,false)},{title:'HEXA',value:rgbaHex(c,true)},
      {title:'RGB',value:`rgb(${rgb[0]}, ${rgb[1]}, ${rgb[2]})`},{title:'RGBA',value:`rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${a})`},
      {title:'HSL',value:`hsl(${round(hsl.h,2)} ${round(hsl.s*100,2)}% ${round(hsl.l*100,2)}%${alphaSuffix})`},
      {title:'HSV / HSB',value:`hsv(${round(hsv.h,2)} ${round(hsv.s*100,2)}% ${round(hsv.v*100,2)}%${alphaSuffix})`},
      {title:'HSI',value:`hsi(${round(hsi.h,2)} ${round(hsi.s*100,2)}% ${round(hsi.i*100,2)}%${alphaSuffix})`},
      {title:'HWB',value:`hwb(${round(hwb.h,2)} ${round(hwb.w*100,2)}% ${round(hwb.b*100,2)}%${alphaSuffix})`},
      {title:'CMYK',value:`cmyk(${round(cmyk.c*100,2)}% ${round(cmyk.m*100,2)}% ${round(cmyk.y*100,2)}% ${round(cmyk.k*100,2)}%${alphaSuffix})`},
      {title:'CIE XYZ D65',value:`xyz(${round(xyz.x*100,4)} ${round(xyz.y*100,4)} ${round(xyz.z*100,4)}${alphaSuffix})`},
      {title:'CIE Lab D65',value:`lab(${round(lab.L,3)} ${round(lab.a,3)} ${round(lab.b,3)}${alphaSuffix})`},
      {title:'CIE LCH D65',value:`lch(${round(lch.L,3)} ${round(lch.C,3)} ${round(lch.h,3)}${alphaSuffix})`},
      {title:'OKLab',value:`oklab(${round(ok.L,5)} ${round(ok.a,5)} ${round(ok.b,5)}${alphaSuffix})`},
      {title:'OKLCH',value:`oklch(${round(okc.L,5)} ${round(okc.C,5)} ${round(okc.h,3)}${alphaSuffix})`},
      {title:'Display-P3',value:`color(display-p3 ${round(p3.r,5)} ${round(p3.g,5)} ${round(p3.b,5)}${alphaSuffix})`},
      {title:'YCbCr BT.601 Full',value:`Y ${round(y601.y,2)} · Cb ${round(y601.cb,2)} · Cr ${round(y601.cr,2)}`},
      {title:'YCbCr BT.709 Full',value:`Y ${round(y709.y,2)} · Cb ${round(y709.cb,2)} · Cr ${round(y709.cr,2)}`},
      {title:'YCbCr BT.709 Limited',value:`Y ${round(yl709.y,2)} · Cb ${round(yl709.cb,2)} · Cr ${round(yl709.cr,2)}`},
      {title:'YCbCr BT.2020 Full',value:`Y ${round(y2020.y,2)} · Cb ${round(y2020.cb,2)} · Cr ${round(y2020.cr,2)}`},
      {title:'YUV BT.601',value:`Y ${round(.299*c.r+.587*c.g+.114*c.b,5)} · U ${round(-.14713*c.r-.28886*c.g+.436*c.b,5)} · V ${round(.615*c.r-.51499*c.g-.10001*c.b,5)}`}
    ].map(x=>({...x,swatch:rgbaHex(c,false),color:{...c}}));
  }

  global.QuickToolCore={
    CATEGORY_NAMES,normalizeSearch,fuzzySearch,parseCodePointSequence,describeUnicode,parseInteger,baseResults,
    toFullWidth,toHalfWidth,widthResults,unitLookup,parseUnitQuery,unitConvert,unitResults,percentEncode,percentDecode,unicodeEscapeEncode,unicodeEscapeDecode,
    calculatorLocaleSeparators,tokenizeCalculation,calculateExpression,formatCalculationResult,isoWeek,windowsFileTimeFromDate,dateFromWindowsFileTime,oleAutomationFromDate,dateFromOleAutomation,excel1900FromDate,dateFromExcel1900,excel1904FromDate,dateFromExcel1904,formatDatePattern,parseTimeSpecial,extendedTimeResults,
    htmlEncode,htmlDecode,jsonEscape,jsonUnescape,escapeResults,countryIndex,findCountries,countryResultItems,
    parseColorCandidates,colorFormats,rgbaHex,rgbToHsl,rgbToHsv,rgbToHwb,rgbToCmyk,rgbToXyz,xyzToLab,labToLch,rgbToOklab,oklabToOklch
  };
  if(typeof module!=='undefined'&&module.exports) module.exports=global.QuickToolCore;
})(typeof window!=='undefined'?window:globalThis);
