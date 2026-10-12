//! The phone page: one self-contained HTML document (no external resources, no scripts loaded
//! from the network). It pairs, shows the reconnect snapshot, sends commands with unique request
//! ids and the revision it last saw, and polls meters separately from actions.

pub const HTML: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1">
<title>SML Remote</title>
<style>
:root{color-scheme:dark}
body{margin:0;background:#1e1e1e;color:#ddd;font:16px system-ui,sans-serif}
header{padding:10px 12px;background:#111;display:flex;justify-content:space-between;align-items:center}
main{padding:10px 12px}
button{font:inherit;padding:12px 14px;margin:4px 4px 4px 0;border:1px solid #444;border-radius:6px;background:#333;color:#eee;min-width:64px}
button.on{background:#4084c4;color:#000}
button.rec{background:#5a1a18}button.rec.on{background:#d6342e;color:#fff}
.row{display:flex;flex-wrap:wrap;align-items:center;gap:4px}
.track{border:1px solid #333;border-radius:6px;padding:8px;margin:6px 0;background:#262626}
.track.sel{border-color:#4084c4}
.meter{height:8px;background:#111;border-radius:3px;overflow:hidden;margin-top:6px}
.meter i{display:block;height:100%;background:#4cc46c;width:0}
input[type=range]{width:100%}
small{color:#999}
#status{min-height:1.2em;color:#e8b020}
#pair{display:none}
</style></head><body>
<header><strong>SML Remote</strong><span id="conn"><small>not paired</small></span></header>
<main>
<section id="pair">
 <p>Type the pairing code shown on the desktop (it expires).</p>
 <div class="row"><input id="code" inputmode="numeric" pattern="[0-9]*" maxlength="6" placeholder="000000" style="font-size:24px;width:8em">
 <input id="name" placeholder="this phone" maxlength="32"><button id="pairbtn">Pair</button></div>
</section>
<section id="app" style="display:none">
 <div id="status"></div>
 <div class="row">
  <button id="rtz">|◀</button><button id="play">Play</button><button id="stop">Stop</button>
  <button id="rec" class="rec">● Rec</button><button id="loop">Loop</button>
 </div>
 <div class="row">
  <button id="preroll">Pre-roll</button><button id="countoff">Count-in</button><button id="marker">+ Marker</button>
  <button id="audition">Audition sel.</button>
 </div>
 <div id="pos"><small>position — receipt is not sample execution; the desktop clock is authoritative</small></div>
 <div class="meter"><i id="mainL"></i></div><div class="meter"><i id="mainR"></i></div>
 <h3 style="margin:12px 0 4px">Take</h3>
 <div class="row"><span id="take">none selected</span>
  <span id="rating"></span></div>
 <h3 style="margin:12px 0 4px">Tracks</h3>
 <div id="tracks"></div>
 <h3 style="margin:12px 0 4px">Ask</h3>
 <div class="row"><input id="ask" placeholder="ask, or say what to do (typed)" maxlength="200" style="flex:1"><button id="askbtn">Ask</button></div>
 <small>Answers here come from the grounded rules. The optional local model, spoken replies and push-to-talk run in the desktop Assistant window: a phone microphone needs a secure (HTTPS) page, and this local-network page is plain HTTP.</small>
 <small id="caps"></small>
 <pre id="answer" style="white-space:pre-wrap;font-size:12px;margin:4px 0"></pre>
 <button id="runplan" style="display:none">Run plan (one undo step)</button>
 <h3 style="margin:12px 0 4px">Monitor (speaker path only, never export)</h3>
 <div class="row"><button id="dim">Dim</button><button id="mono">Mono</button><input id="monvol" type="range" min="-60" max="0" step="1" value="0" style="width:50%"><span id="monv">0 dB</span></div>
 <div id="cue"><small>Headphone/cue: unavailable (no dedicated cue path in I/O setup)</small></div>
 <p><small>Plain HTTP on the local network, no installation, no microphone, no encryption. Refresh the page to reconnect; a stale Record is refused by design.</small></p>
 <button id="logout">Forget this phone</button>
</section>
</main>
<script>
(function(){
const $=id=>document.getElementById(id);
let state=null, revision=0, nonce=0, seq=0, metersTimer=null, stateTimer=null;
let rid=0; const newId=()=> (crypto.randomUUID? crypto.randomUUID() : (Date.now()+'-'+(rid++)));
function say(t){ $('status').textContent=t; }
async function api(method,path,body){
  const r=await fetch(path,{method,headers:{'Content-Type':'application/json'},body: body?JSON.stringify(body):undefined, credentials:'same-origin'});
  let j=null; try{ j=await r.json(); }catch(e){}
  return {ok:r.ok,status:r.status,json:j};
}
async function refresh(){
  const r=await api('GET','/state');
  if(r.status===401){ showPair(); return; }
  if(!r.ok){ say('state: '+(r.json&&r.json.error||r.status)); return; }
  revision=r.json.revision; nonce=r.json.nonce; state=r.json.state||{}; render();
}
async function cmd(command,params,extra){
  const body=Object.assign({request_id:newId(),command,params:params||{},expected_revision:revision},extra||{});
  const r=await api('POST','/cmd',body);
  if(r.status===401){ showPair(); return null; }
  if(!r.ok){ say(command+': '+(r.json&&r.json.error||r.status)); await refresh(); return null; }
  const rc=r.json;
  if(!rc.accepted){ say(command+' rejected: '+JSON.stringify(rc.rejection)); }
  else say('');
  await refresh();
  return rc;
}
function render(){
  const s=state||{}; const t=s.transport||{};
  $('play').classList.toggle('on',!!t.playing&&!t.recording);
  $('rec').classList.toggle('on',!!t.recording);
  $('loop').classList.toggle('on',!!s.loop);
  $('preroll').classList.toggle('on',!!s.pre_roll);
  $('countoff').classList.toggle('on',!!s.countoff);
  $('pos').innerHTML='<small>'+(t.position_text||'')+' · '+(s.session||'')+' · rev '+revision+'</small>';
  const take=s.selected_clip;
  $('take').textContent= take? (take.name+' ('+(take.rating||0)+'★)') : 'no clip selected';
  let stars=''; for(let i=1;i<=5;i++){ stars+='<button data-r="'+i+'"'+(take&&take.rating>=i?' class="on"':'')+'>'+i+'</button>'; }
  $('rating').innerHTML= take? stars : '';
  $('rating').querySelectorAll('button').forEach(b=>b.onclick=()=>cmd('clip.rating',{clips:[take.id],rating:+b.dataset.r}));
  $('dim').classList.toggle('on',!!(s.monitor&&s.monitor.dim));
  $('mono').classList.toggle('on',!!(s.monitor&&s.monitor.mono));
  if(s.monitor&&typeof s.monitor.volume_db==='number'&&document.activeElement!==$('monvol')){ $('monvol').value=s.monitor.volume_db; $('monv').textContent=s.monitor.volume_db+' dB'; }
  $('cue').innerHTML='<small>'+(s.monitor&&s.monitor.cue? 'Headphone/cue: '+s.monitor.cue : 'Headphone/cue: unavailable (no dedicated cue path in I/O setup)')+'</small>';
  const box=$('tracks'); box.innerHTML='';
  (s.tracks||[]).forEach(tr=>{
    const d=document.createElement('div'); d.className='track'+(tr.selected?' sel':'');
    d.innerHTML='<div class="row"><strong style="flex:1">'+esc(tr.name)+'</strong>'
      +'<button data-a="sel">Select</button>'
      +'<button data-a="arm" class="rec'+(tr.arm?' on':'')+'">Arm</button>'
      +'<button data-a="mon"'+(tr.monitor?' class="on"':'')+'>Mon</button>'
      +'<button data-a="mute"'+(tr.mute?' class="on"':'')+'>M</button>'
      +'<button data-a="solo"'+(tr.solo?' class="on"':'')+'>S</button></div>'
      +'<div class="row"><small>vol '+(tr.volume_db==null?'':tr.volume_db.toFixed(1))+' dB</small><input data-a="vol" type="range" min="-60" max="12" step="0.5" value="'+(tr.volume_db==null?0:tr.volume_db)+'"></div>'
      +'<div class="row"><small>pan</small><input data-a="pan" type="range" min="-1" max="1" step="0.02" value="'+(tr.pan==null?0:tr.pan)+'"></div>'
      +'<div class="meter"><i data-m="'+tr.id+'"></i></div>';
    d.querySelector('[data-a=sel]').onclick=()=>cmd('edit.select',{tracks:[tr.id]});
    d.querySelector('[data-a=arm]').onclick=()=>cmd('mix.record_arm',{track:tr.id});
    d.querySelector('[data-a=mon]').onclick=()=>cmd('mix.input_monitor',{track:tr.id});
    d.querySelector('[data-a=mute]').onclick=()=>cmd('mix.mute',{track:tr.id});
    d.querySelector('[data-a=solo]').onclick=()=>cmd('mix.solo',{track:tr.id});
    d.querySelector('[data-a=vol]').onchange=e=>cmd('mix.volume',{track:tr.id,db:+e.target.value});
    d.querySelector('[data-a=pan]').onchange=e=>cmd('mix.pan',{track:tr.id,pan:+e.target.value});
    box.appendChild(d);
  });
}
function esc(s){ return String(s).replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c])); }
async function meters(){
  try{
    const r=await fetch('/meters?since='+seq,{credentials:'same-origin'});
    if(r.status===401){ showPair(); return; }
    const j=await r.json(); seq=j.seq||seq;
    const m=j.meters||{};
    if(m.main){ $('mainL').style.width=Math.round(100*Math.min(1,m.main[0]||0))+'%'; $('mainR').style.width=Math.round(100*Math.min(1,m.main[1]||0))+'%'; }
    document.querySelectorAll('[data-m]').forEach(i=>{ const v=(m.tracks||{})[i.dataset.m]; i.style.width=Math.round(100*Math.min(1,v||0))+'%'; });
  }catch(e){}
  metersTimer=setTimeout(meters,50);
}
function showPair(){ $('pair').style.display='block'; $('app').style.display='none'; $('conn').innerHTML='<small>not paired</small>'; clearTimeout(metersTimer); clearInterval(stateTimer); }
function showApp(){ $('pair').style.display='none'; $('app').style.display='block'; $('conn').innerHTML='<small>paired</small>'; refresh(); meters(); stateTimer=setInterval(refresh,1500); }
$('pairbtn').onclick=async()=>{
  const r=await api('POST','/pair',{code:$('code').value,name:$('name').value||'phone'});
  if(r.ok){ showApp(); } else { say('pairing: '+(r.json&&r.json.error||r.status)); }
};
$('play').onclick=()=>cmd('transport.play'); $('stop').onclick=()=>cmd('transport.stop'); $('rtz').onclick=()=>cmd('transport.rtz');
$('rec').onclick=async()=>{ await refresh(); cmd('transport.record',{}, {nonce}); };
$('loop').onclick=()=>cmd('options.loop_playback'); $('preroll').onclick=()=>cmd('options.pre_roll'); $('countoff').onclick=()=>cmd('setup.click_countoff');
$('marker').onclick=()=>cmd('markers.add'); $('audition').onclick=()=>cmd('transport.play_selection');
$('dim').onclick=()=>cmd('monitor.dim'); $('mono').onclick=()=>cmd('monitor.mono');
$('monvol').onchange=e=>cmd('monitor.volume',{db:+e.target.value});
// RC08: what this browser itself reports (the server states the same in /state.capabilities).
$('caps').textContent='This page: '+(window.isSecureContext?'secure origin':'not a secure origin (plain HTTP)')+'; microphone '+((window.isSecureContext&&navigator.mediaDevices)?'available to the browser (push-to-talk still runs on the desktop)':'unavailable')+'. Closing this page never stops playback or recording on the desktop.';
let lastPlan=null;
$('askbtn').onclick=async()=>{
  const text=$('ask').value.trim(); if(!text) return;
  const rc=await cmd('assistant.ask',{text}); if(!rc||!rc.accepted) return;
  const a=rc.applied||{}; lastPlan=(a.plan&&a.plan.length)?{plan:a.plan,revision:a.revision}:null;
  $('answer').textContent='['+a.mode+'] '+a.answer+(a.citations&&a.citations.length?'\ncites: '+a.citations.join(', '):'');
  $('runplan').style.display=lastPlan?'':'none';
};
$('runplan').onclick=async()=>{
  if(!lastPlan) return; const rc=await cmd('assistant.execute',{plan:lastPlan.plan,revision:lastPlan.revision,request_id:newId()});
  if(rc&&rc.applied){ $('answer').textContent+='\n'+(rc.applied.accepted?'done':'not applied: '+JSON.stringify(rc.applied.reason)); }
  lastPlan=null; $('runplan').style.display='none';
};
$('logout').onclick=async()=>{ await api('POST','/logout'); showPair(); };
api('GET','/state').then(r=>{ if(r.ok){ showApp(); } else { showPair(); } });
})();
</script></body></html>
"#;
