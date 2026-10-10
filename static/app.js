const $ = (selector, root = document) => root.querySelector(selector);
const escape = (value = '') => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const paths = {
  mountain:'<path d="m2 19 8-14 6 10 3-5 4 9H2Z"/><path d="m7 10 3 3 3-3"/>',
  arrow:'<path d="M4 12h15m-6-6 6 6-6 6"/>',
  back:'<path d="M20 12H5m6-6-6 6 6 6"/>',
  chevron:'<path d="m9 5 7 7-7 7"/>',
  calendar:'<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M16 3v4M8 3v4M3 11h18m-13 4h2m4 0h2m-8 3h2"/>',
  users:'<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2m20 0v-2a4 4 0 0 0-3-3.87"/><circle cx="9" cy="7" r="4"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>',
  user:'<circle cx="12" cy="7" r="4"/><path d="M4 21v-2a8 8 0 0 1 16 0v2"/>',
  check:'<path d="m5 12 4 4L19 6"/>',
  link:'<path d="M10 13a5 5 0 0 0 7 .5l3-3a5 5 0 0 0-7-7l-2 2m3 6a5 5 0 0 0-7-.5l-3 3a5 5 0 0 0 7 7l2-2"/>',
  cloud:'<path d="M20 16a4 4 0 0 0-.9-7.9A7 7 0 0 0 5 10a3 3 0 0 0 0 6h15Z"/>',
  snow:'<path d="M12 2v20M4 7l16 10M4 17 20 7m-12-3 4 2 4-2m-8 16 4-2 4 2M4 3v4L1 9m19-6v4l3 2M4 21v-4l-3-2m19 6v-4l3-2"/>',
  sun:'<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"/>',
  search:'<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
  x:'<path d="m6 6 12 12M6 18 18 6"/>',
  wind:'<path d="M3 8h12a3 3 0 1 0-3-3m-9 7h15a3 3 0 1 1-3 3M3 16h5a3 3 0 1 1-3 3"/>',
  edit:'<path d="m16 3 5 5L9 20H4v-5L16 3Zm-3 3 5 5"/>',
  copy:'<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M16 8V3H3v13h5"/>',
  pin:'<path d="M20 10c0 6-8 12-8 12S4 16 4 10a8 8 0 0 1 16 0Z"/><circle cx="12" cy="10" r="2.5"/>',
  clock:'<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  shield:'<path d="m12 2 8 4v6c0 5-8 10-8 10S4 17 4 12V6l8-4Z"/><path d="m8 11 3 3 5-5"/>',
};
const icon = (name) => `<svg viewBox="0 0 24 24" aria-hidden="true">${paths[name] || paths.mountain}</svg>`;
const brand = () => `<a href="/" class="brand" data-nav="/" aria-label="Weekend home">${icon('mountain')}<span><span class="brand-word">WEEKEND</span><span class="brand-club">SKI PLANNER</span></span></a>`;
const state = {
  mountains:[], today:'', route:'/', plan:null, weather:new Map(), weatherLoading:false,
  wizard:null, username:'', selected:new Set(), dirty:false, busy:false, tab:'trips',
  mountainFilter:'all', sort:'soonest', error:'', loadId:0, weatherLoadId:0,
};
const date = s => new Date(`${s}T12:00:00`);
const iso = d => `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
const addDays = (s, n) => { const d=date(s); d.setDate(d.getDate()+n); return iso(d); };
const monthShort = s => date(s).toLocaleDateString('en-US',{month:'short'});
const weekendLabel = (s, year=false) => {
  const saturday=date(s), sunday=date(addDays(s,1));
  return `${monthShort(s)} ${saturday.getDate()}–${saturday.getMonth()===sunday.getMonth() ? '' : monthShort(iso(sunday))+' '}${sunday.getDate()}${year ? ', '+sunday.getFullYear() : ''}`;
};
const mountain = id => state.mountains.find(m=>m.id===id);
const mountainName = id => mountain(id)?.name || id;
const tripKey = (id,weekend) => `${id}@${weekend}`;
const allTrips = plan => plan.trips || plan.mountains.flatMap(m=>plan.weekends.map(w=>tripKey(m,w)));
const weekendMountains = (plan,w) => plan.mountains.filter(m=>allTrips(plan).includes(tripKey(m,w)));
function stored(key, fallback) { try { return JSON.parse(localStorage.getItem(key)) ?? fallback; } catch { return fallback; } }
function save(key,value) { localStorage.setItem(key,JSON.stringify(value)); }
function access(id) { return stored(`wc.access.${id}`,{}); }
function rememberPlan(plan,credentials={}) {
  save(`wc.access.${plan.id}`,{...access(plan.id),...credentials});
  const plans=stored('wc.plans',[]).filter(p=>p.id!==plan.id);
  plans.unshift({id:plan.id,title:plan.title,owner:plan.owner,mountains:plan.mountains.length,weekends:plan.weekends.length});
  save('wc.plans',plans);
}
function storageAvailable() { try { localStorage.setItem('wc.probe','1');localStorage.removeItem('wc.probe');return true; } catch { return false; } }
function owner() { return Boolean(state.plan && access(state.plan.id).owner_token); }
function currentMember() { const credentials=access(state.plan?.id);return state.plan?.members.find(m=>m.id===credentials.member_id); }
async function api(path, options={}) {
  const {token,...rest}=options;
  const response=await fetch(path,{...rest,headers:{'Content-Type':'application/json',...(token?{Authorization:`Bearer ${token}`}:{})},signal:AbortSignal.timeout(180000)});
  const data=await response.json().catch(()=>({error:'The server returned an unexpected response.'}));
  if(!response.ok) throw new Error(data.error || 'Could not complete this request. Please try again.');
  return data;
}
let toastTimer;
function toast(message) { const el=$('#toast');el.textContent=message;el.classList.add('visible');clearTimeout(toastTimer);toastTimer=setTimeout(()=>el.classList.remove('visible'),4500); }
function footer() {
  return `<footer class="footer">${brand()}<p class="smallprint">Independent trip planner. Not affiliated with Ikon Pass. Check resort access and opening dates.</p><div class="footer-links"><a href="https://www.ikonpass.com/en/destinations" target="_blank" rel="noopener">Ikon destinations ↗</a><a href="https://open-meteo.com/" target="_blank" rel="noopener">Weather by Open-Meteo ↗</a></div></footer>`;
}
function shell(content,{home=false}={}) {
  $('#app').innerHTML=`<div class="container"><header class="nav">${brand()}<nav class="nav-links" aria-label="Main navigation">${home?'<a href="#how-it-works">How it works</a><button class="btn-link" data-action="explore">The mountains</button>':''}<button class="btn btn-outline btn-small" data-nav="/my-trips">${icon('calendar')} My trips</button></nav></header><main class="content-fade">${content}</main>${footer()}</div>`;
}
function landing() {
  const y=date(state.today).getFullYear(), season=date(state.today).getMonth()>=7?y:y-1;
  shell(`<section class="hero"><div class="hero-copy"><p class="eyebrow"><span class="dot"></span> US Ikon destinations · Weekend forecasts</p><h1>Pick a weekend.<br><em>Find a mountain.</em></h1><p class="hero-description">Choose your free weekends and their mountains.<br>Check the forecast, share your plan, and see<br class="desktop-break"> which weekends work for your friends.</p><div class="hero-actions"><button class="btn btn-primary" data-action="start">Create a trip plan ${icon('arrow')}</button><a class="btn btn-link" href="#how-it-works">How it works ${icon('chevron')}</a></div><p class="hero-note">${icon('check')} No account required. Share one link with your group.</p></div><div class="hero-art"><img class="hero-photo" src="/assets/alpine.jpg" alt="Snow-covered alpine peaks beneath a blue winter sky" fetchpriority="high"><div class="season-stamp"><span>SKI SEASON</span>${icon('snow')}<b>${season} / ${String(season+1).slice(2)} WINTER</b></div><div class="photo-caption"><div><p class="eyebrow">Choose your dates. Check the conditions.</p><p class="serif">Plan your next ski weekend.</p></div>${icon('arrow')}</div></div></section><div class="benefits"><div class="benefit">${icon('mountain')}<div><p>Your favorite Ikon mountains</p><small>${state.mountains.length} destinations across the US</small></div></div><div class="benefit">${icon('cloud')}<div><p>Weather that keeps up</p><small>Weekend forecasts, refreshed automatically</small></div></div><div class="benefit">${icon('users')}<div><p>One link for your friends.</p><small>Find the weekends that work for everyone</small></div></div></div><section class="section"><div class="section-head"><div><p class="eyebrow">Mountains across the US</p><h2>Choose your destinations.</h2><p class="muted">Browse mountains included in the Ikon Pass.</p></div><button class="btn btn-link" data-action="explore">Explore all mountains ${icon('arrow')}</button></div><div class="destination-grid">${['mammoth','palisades','steamboat'].map(id=>{const m=mountain(id);return `<button class="destination-card" data-action="start-mountain" data-id="${id}" aria-label="Plan a trip to ${escape(m.name)}"><div class="destination-photo"><img src="/assets/${m.image}" alt="Winter mountain scenery" loading="lazy"><span class="pill">${icon('mountain')} Ikon destination</span></div><div class="destination-info"><div><h3>${escape(m.name)}</h3><p>${icon('pin')} ${m.state==='CO'?'Colorado':'California'} <span>·</span> ${m.region}</p></div>${icon('arrow')}</div></button>`;}).join('')}</div></section><section class="how" id="how-it-works"><div class="how-heading"><h2>How it works</h2><p>Three steps to a shared weekend plan.</p></div><div class="steps"><div class="step-description"><span class="step-number">01</span><div><h3>Pick your free weekends</h3><p>Start with the Saturdays and Sundays you have free, then add mountains for each weekend.</p></div></div><div class="step-description"><span class="step-number">02</span><div><h3>Keep an eye on the forecast</h3><p>See snowfall, temperatures, and wind for every weekend, with current forecasts or labeled seasonal estimates.</p></div></div><div class="step-description"><span class="step-number">03</span><div><h3>Share your plan</h3><p>Your friends pick from your trips. You see where everyone’s calendars line up.</p></div></div></div></section>`,{home:true});
}
function makeWizard(selectedMountain,edit=false) {
  const p=edit?state.plan:null;
  const weekends=new Set(p?.weekends||[]);
  const choices=new Map([...weekends].map(w=>[w,new Set(weekendMountains(p,w))]));
  return {step:1,username:p?.owner||stored('wc.username',''),title:p?.title||'Winter weekends',mountains:new Set(p?.mountains||[]),weekends,choices,initialMountain:selectedMountain,activeWeekend:[...weekends].sort()[0]||'',region:'All',search:'',month:0,editing:edit};
}
function updateWizardMountains() {
  state.wizard.mountains=new Set([...state.wizard.choices.values()].flatMap(m=>[...m]));
}
function startWizard(selectedMountain,edit=false) {
  state.wizard=makeWizard(selectedMountain,edit);
  state.error='';state.busy=false;
  navigate(edit?`/p/${state.plan.id}/edit`:'/new',{retainWizard:true});
}
function wizardPreview() {
  const w=state.wizard;
  return `<aside class="plan-preview"><div class="preview-photo"><img src="/assets/alpine.jpg" alt="Snowy mountain landscape"><span>YOUR TRIP PLAN</span></div><div class="preview-body"><h3 id="preview-title">${escape(w.title||'Your trip plan')}</h3><p class="preview-owner">${icon('user')} <span id="preview-username">${w.username?`Planned by ${escape(w.username)}`:'Planner owner'}</span></p><div class="preview-selections"><p class="preview-label">${icon('mountain')} Mountains <span class="muted">· ${w.mountains.size}</span></p>${w.mountains.size?`<div class="preview-list">${[...w.mountains].map(m=>`<span class="mini-chip">${escape(mountainName(m))}</span>`).join('')}</div>`:'<p class="preview-placeholder">Pick your places to ski</p>'}<p class="preview-label">${icon('calendar')} Weekends <span class="muted">· ${w.weekends.size}</span></p>${w.weekends.size?`<div class="preview-list">${[...w.weekends].sort().map(d=>`<span class="mini-chip">${weekendLabel(d)} · ${w.choices.get(d)?.size||0} mountains</span>`).join('')}</div>`:'<p class="preview-placeholder">Select your available weekends</p>'}</div></div><div class="preview-foot">${icon('link')} Your invite link is created when you save.</div></aside>`;
}
function mountainOptions() {
  const w=state.wizard, selected=w.choices.get(w.activeWeekend)||new Set();
  const filtered=state.mountains.filter(m=>(w.region==='All'||m.region===w.region)&&`${m.name} ${m.state}`.toLowerCase().includes(w.search.toLowerCase()));
  return `<p class="results-count">${filtered.length} destinations · ${selected.size} selected for ${weekendLabel(w.activeWeekend)} · ${w.mountains.size} of 12 unique mountains</p><div class="mountain-grid">${filtered.map(m=>`<button class="mountain-option ${selected.has(m.id)?'selected':''}" data-action="toggle-mountain" data-id="${m.id}" aria-pressed="${selected.has(m.id)}"><img src="/assets/${m.image}" alt="" loading="lazy"><span class="details"><b>${escape(m.name)}</b><small>${m.state} · ${m.region}</small></span><span class="check-box">${selected.has(m.id)?icon('check'):''}</span></button>`).join('')}</div>${!filtered.length?'<p class="page-subtitle">No mountains match your search. Try a resort name or state.</p>':''}`;
}
function calendar() {
  const w=state.wizard, first=date(state.today);first.setDate(1);first.setMonth(first.getMonth()+w.month);
  const month=first.getMonth(), max=addDays(state.today,364), weeks=[];
  first.setDate(1+(6-first.getDay()+7)%7);
  while(first.getMonth()===month) { const s=iso(first);if(addDays(s,1)>=state.today&&s<=max)weeks.push(s);first.setDate(first.getDate()+7); }
  const viewDate=date(state.today);viewDate.setDate(1);viewDate.setMonth(viewDate.getMonth()+w.month);
  return `<div class="month-nav"><span class="serif">${viewDate.toLocaleDateString('en-US',{month:'long',year:'numeric'})}</span><div class="month-buttons"><button class="icon-btn" data-action="month-prev" aria-label="Previous month" ${w.month===0?'disabled':''}>${icon('back')}</button><button class="icon-btn" data-action="month-next" aria-label="Next month" ${w.month>=12?'disabled':''}>${icon('arrow')}</button></div></div><div class="weekend-grid">${weeks.map(s=>`<button class="weekend-option ${w.weekends.has(s)?'selected':''}" data-action="toggle-weekend" data-date="${s}" aria-pressed="${w.weekends.has(s)}"><span class="date-tile"><span>${monthShort(s).toUpperCase()}</span><b>${date(s).getDate()}</b></span><span class="date-info"><b>${weekendLabel(s)}</b><small>Saturday & Sunday</small></span><span class="check-box">${w.weekends.has(s)?icon('check'):''}</span></button>`).join('')}</div>${!weeks.length?'<p class="page-subtitle">No more available weekends in this month.</p>':''}<div class="selected-weekends"><p>${w.weekends.size} of 26 weekends selected${w.weekends.size?' · across all months':''}</p><div class="preview-list">${[...w.weekends].sort().map(s=>`<button class="mini-chip" data-action="toggle-weekend" data-date="${s}" aria-label="Remove ${weekendLabel(s,true)}">${weekendLabel(s,true)} ${icon('x')}</button>`).join('')}</div></div><div class="forecast-info">${icon('cloud')}<p>Choose dates first, then select mountains for each weekend. Every future trip gets a seasonal weather estimate; current forecasts replace those estimates as the dates approach.</p></div>`;
}
function renderWizard() {
  const w=state.wizard;if(!w)return;
  const headings=['Create a trip plan.','Select available weekends.','Choose mountains for each weekend.'];
  let content='';
  if(w.step===1) content=`<h2>Start with your details.</h2><p>Choose a username and a name for this planner.</p><div class="identity-form"><label class="field">Your username<input id="owner-username" autocomplete="nickname" maxlength="24" placeholder="e.g. aryan" value="${escape(w.username)}" ${w.editing?'readonly':''}><small>Your friends will see who’s organizing the trip.</small></label><label class="field">Give your planner a name<input id="plan-title" maxlength="70" placeholder="Winter weekends" value="${escape(w.title)}"><small>This name appears on your shared planner.</small></label><div class="owner-explainer">${icon('shield')}<div><b>You control the plan.</b><p>Pick your free weekends, then choose mountains for each date. Your friends choose which trips they can join. Your owner access is saved in this browser.</p></div></div></div>`;
  if(w.step===2) content=`<h2>When are you free?</h2><p>Select your available Saturday–Sunday weekends. You’ll choose destinations for each weekend next.</p>${calendar()}`;
  if(w.step===3) {
    if(!w.weekends.has(w.activeWeekend))w.activeWeekend=[...w.weekends].sort()[0];
    content=`<h2>Where do you want to ski that weekend?</h2><p>Choose a weekend below, then add its mountains. Different weekends can have different destinations.</p><div class="weekend-picker" aria-label="Choose a weekend to assign mountains">${[...w.weekends].sort().map(d=>`<button class="weekend-tab ${w.activeWeekend===d?'active':''}" data-action="active-weekend" data-date="${d}" aria-pressed="${w.activeWeekend===d}"><b>${weekendLabel(d)}</b><small>${w.choices.get(d)?.size||0} mountains ${w.choices.get(d)?.size?icon('check'):''}</small></button>`).join('')}</div><div class="mountain-weekend-head"><p>Mountains for <b>${weekendLabel(w.activeWeekend,true)}</b></p><button class="btn btn-link" data-action="copy-weekend-mountains" ${(w.choices.get(w.activeWeekend)?.size||0)?'':'disabled'}>Use for every weekend ${icon('copy')}</button></div><div class="search-wrap">${icon('search')}<input id="mountain-search" placeholder="Search mountains or a state…" value="${escape(w.search)}" aria-label="Search mountains"></div><div class="filters" aria-label="Filter mountains by region">${['All','West','Rockies','East & Midwest'].map(r=>`<button class="filter-chip ${w.region===r?'active':''}" data-action="region" data-region="${r}" aria-pressed="${w.region===r}">${r}</button>`).join('')}</div><div id="mountain-options">${mountainOptions()}</div>`;
  }
  shell(`<section class="wizard-top"><button class="back-link" data-action="wizard-cancel">${icon('back')} ${w.editing?'Back to your planner':'Back to home'}</button><h1 class="page-title">${headings[w.step-1]}</h1><p class="page-subtitle">${w.editing?'Update your weekends and their destinations.':'Choose your dates first, add mountains, then share your plan.'}</p><div class="progress" aria-label="Planner setup progress">${['Your plan','Weekends','Mountains'].map((label,i)=>`${i?'<span class="progress-line"></span>':''}<button class="progress-step ${w.step===i+1?'active':''} ${w.step>i+1?'done':''}" data-action="wizard-step" data-step="${i+1}" ${i+1>w.step?'disabled':''} ${w.step===i+1?'aria-current="step"':''}><span class="number">${w.step>i+1?icon('check'):String(i+1).padStart(2,'0')}</span>${label}</button>`).join('')}</div></section><div class="wizard-grid"><section class="wizard-content">${content}<div id="wizard-error" class="error-message ${state.error?'':'hidden'}" role="alert">${escape(state.error)}</div><div class="wizard-actions"><div>${w.step>1?`<button class="btn btn-outline" data-action="wizard-back">${icon('back')} Back</button>`:'<small>Your access is saved in this browser.</small>'}</div><button class="btn btn-primary" data-action="wizard-next" ${state.busy?'disabled':''}>${state.busy?'<span class="spinner"></span> Saving your plan…':w.step===3?(w.editing?'Save your changes':'Create & get my link'):'Continue'} ${!state.busy?icon('arrow'):''}</button></div></section>${wizardPreview()}</div>`);
}
function wizardError(message) {state.error=message;const el=$('#wizard-error');if(el){el.textContent=message;el.classList.remove('hidden');el.scrollIntoView({block:'nearest',behavior:'smooth'});} }
async function wizardNext() {
  if(state.busy)return;
  const w=state.wizard;state.error='';
  if(w.step===1) {
    if(!/^[a-zA-Z0-9 _-]{2,24}$/.test(w.username.trim()))return wizardError('Choose a username of 2–24 letters, numbers, spaces, hyphens, or underscores.');
    if(!w.title.trim())return wizardError('Give your planner a name.');
  }
  if(w.step===2&&!w.weekends.size)return wizardError('Pick at least one available weekend.');
  if(w.step<3) {w.step++;renderWizard();window.scrollTo({top:0,behavior:'smooth'});return;}
  const incomplete=[...w.weekends].sort().find(d=>!w.choices.get(d)?.size);
  if(incomplete){w.activeWeekend=incomplete;renderWizard();return wizardError(`Choose at least one mountain for ${weekendLabel(incomplete,true)}.`);}
  if(!storageAvailable())return wizardError('This browser can’t save your owner access. Enable local storage before creating a planner.');
  state.busy=true;renderWizard();
  try {
    const input={username:w.username.trim(),title:w.title.trim(),mountains:[...w.mountains],weekends:[...w.weekends].sort(),trips:[...w.choices].flatMap(([d,m])=>[...m].map(id=>tripKey(id,d)))};
    const result=await api(w.editing?`/api/plans/${state.plan.id}`:'/api/plans',{method:w.editing?'PATCH':'POST',body:JSON.stringify(input),token:w.editing?access(state.plan.id).owner_token:undefined});
    const plan=w.editing?result:result.plan;
    rememberPlan(plan,w.editing?{}:{owner_token:result.owner_token});save('wc.username',input.username);
    const edited=w.editing;
    state.busy=false;state.wizard=null;
    await navigate(`/p/${plan.id}`);
    if(!edited)showShare(true);else toast('Planner updated.');
  } catch(error) {state.busy=false;state.error=error.message;renderWizard();}
}
function avatar(username,index=0,isOwner=false) {return `<span class="avatar ${isOwner?'owner':`color-${index%4}`}" title="${escape(username)}">${escape(username.slice(0,2).toUpperCase())}</span>`;}
function attendees(key) {return [{username:state.plan.owner,isOwner:true},...state.plan.members.filter(m=>m.trips.includes(key))];}
function weatherHtml(key) {
  const f=state.weather.get(key);
  if(!f)return `<div class="weather-pending">${state.weatherLoading?'<span class="spinner"></span>':icon('cloud')} ${state.weatherLoading?'Loading forecasts and seasonal estimates…':'Weather unavailable. Try refreshing.'}</div>`;
  if(f.status==='past')return `<div class="weather-pending">${icon('calendar')} This weekend has passed</div>`;
  if(!f.days.length)return `<div class="weather-pending">${icon('cloud')} Estimates couldn’t load · try refreshing</div>`;
  const estimated=f.status==='estimate'||f.status==='mixed';
  const snow=f.days.reduce((sum,d)=>sum+d.snowfall_inches,0),low=Math.min(...f.days.map(d=>d.low_f)),high=Math.max(...f.days.map(d=>d.high_f)),wind=Math.max(...f.days.map(d=>d.wind_mph));
  return `${estimated?`<button class="estimate-label" data-action="estimate-details" data-key="${key}">${icon('clock')} ${f.status==='mixed'?'Forecast + seasonal estimate':'Seasonal estimate'} · about this estimate ${icon('chevron')}</button>`:''}<div class="weather-metrics"><div class="weather-metric"><b>${icon('snow')} ${snow.toFixed(1)}″</b><small>${estimated?'Typical new snow':'New snow'}${f.status==='partial'?' · 1 day':''}</small></div><div class="weather-metric"><b>${icon('cloud')} ${Math.round(low)}–${Math.round(high)}°</b><small>${estimated?'Typical low / high °F':'Low / high °F'}</small></div><div class="weather-metric"><b>${icon('wind')} ${Math.round(wind)}</b><small>${estimated?'Typical max wind mph':'Max wind mph'}</small></div></div>${estimated?`<div class="estimate-detail">Based on ${f.estimate_basis?.start_year}–${f.estimate_basis?.end_year} historical weather at this resort.</div>`:''}${f.stale?'<div class="weather-updated">Saved weather data · latest refresh unavailable</div>':''}${f.status==='partial'?'<div class="weather-updated">One day available · full weekend forecast coming soon</div>':''}`;
}
function tripCard(id,weekend) {
  const m=mountain(id),key=tripKey(id,weekend),people=attendees(key),isOwner=owner(),selected=isOwner||state.selected.has(key);
  return `<article class="trip-card ${!isOwner&&selected?'chosen':''}"><img class="trip-image" src="/assets/${m.image}" alt="" loading="lazy"><div class="trip-content"><div class="trip-top"><div><h3>${escape(m.name)}</h3><p class="trip-location">${icon('pin')} ${m.state} · ${m.region}</p></div>${isOwner?`<span class="trip-select owner">${icon('check')} You’re free</span>`:`<button class="trip-select ${selected?'selected':''}" data-action="toggle-trip" data-key="${key}" aria-pressed="${selected}" aria-label="${selected?'Remove':'Select'} ${escape(m.name)}, ${weekendLabel(weekend,true)}">${icon(selected?'check':'calendar')}${selected?'I’m free':'I can go'}</button>`}</div><div data-weather-key="${key}">${weatherHtml(key)}</div><div class="trip-bottom"><button class="attendees" data-action="attendees" data-key="${key}" aria-label="See who is free for ${escape(m.name)} on ${weekendLabel(weekend)}"><span class="avatars">${people.slice(0,4).map((p,i)=>avatar(p.username,i,p.isOwner)).join('')}</span><span class="count">${people.length} ${people.length===1?'person':'people'} free</span></button><span>${state.weather.get(key)?.status==='estimate'?'Seasonal estimate':state.weather.get(key)?.status==='mixed'?'Forecast + estimate':state.weather.get(key)?.status==='available'?'Weekend forecast':'Saturday + Sunday'}</span></div></div></article>`;
}
function renderTrips() {
  let weekends=[...state.plan.weekends];
  if(state.sort==='popular')weekends.sort((a,b)=>state.plan.members.reduce((sum,m)=>sum+m.trips.filter(k=>k.endsWith(`@${b}`)).length-m.trips.filter(k=>k.endsWith(`@${a}`)).length,0));
  return weekends.map(w=>{
    const mountains=weekendMountains(state.plan,w).filter(m=>state.mountainFilter==='all'||m===state.mountainFilter);
    if(!mountains.length)return '';
    return `<section class="weekend-group"><div class="weekend-heading"><div><h2>${icon('calendar')} ${weekendLabel(w,true)}</h2><small>${mountains.length} ${mountains.length===1?'mountain':'mountains'} to choose from</small></div>${owner()?'':`<button class="btn btn-link" data-action="select-weekend" data-date="${w}">${mountains.every(m=>state.selected.has(tripKey(m,w)))?'Clear weekend':'Select weekend'}</button>`}</div>${mountains.map(m=>tripCard(m,w)).join('')}</section>`;
  }).join('');
}
function crewRows(full=false) {
  const p=state.plan;
  const people=[{username:p.owner,trips:allTrips(p),isOwner:true},...p.members];
  if(full)return `<table class="crew-table"><thead><tr><th>Person</th><th>Available for</th><th>Role</th></tr></thead><tbody>${people.map((m,i)=>`<tr><td><div class="crew-row">${avatar(m.username,i,m.isOwner)}<div><b>${escape(m.username)}</b><small>${m.isOwner?'Planner owner':m.trips.length?'Availability submitted':'No availability selected'}</small></div></div></td><td>${m.trips.length} ${m.trips.length===1?'trip':'trips'}</td><td>${m.isOwner?'<span class="owner-pill">Owner</span>':'Participant'}</td></tr>`).join('')}</tbody></table>`;
  return people.slice(0,7).map((m,i)=>`<div class="crew-row">${avatar(m.username,i,m.isOwner)}<div><b>${escape(m.username)}</b><small>${m.isOwner?'Planner owner':`${m.trips.length} trips selected`}</small></div><span class="crew-label">${m.isOwner?'Owner':'Participant'}</span></div>`).join('')+(people.length>7?`<p class="smallprint" style="margin-top:16px">+ ${people.length-7} more people</p>`:'');
}
function renderPlan() {
  const p=state.plan;if(!p)return;
  const isOwner=owner(),member=currentMember();
  shell(`<section class="planner-header"><p class="eyebrow"><span class="dot"></span> Shared weekend planner</p><div class="planner-heading"><div><h1>${escape(p.title)}</h1><p>${icon('user')} Planned by ${escape(p.owner)} <span class="owner-badge">${isOwner?'You’re the owner':'Planner owner'}</span></p></div><div class="planner-header-actions">${isOwner?`<button class="btn btn-link" data-action="edit-plan">${icon('edit')} Edit plan</button>`:''}<button class="btn ${isOwner?'btn-orange':'btn-white'}" data-action="share">${icon('link')} Share planner</button></div></div></section><div class="plan-stats"><span class="plan-stat">${icon('mountain')}<b>${p.mountains.length}</b> ${p.mountains.length===1?'mountain':'mountains'}</span><span class="plan-stat">${icon('calendar')}<b>${p.weekends.length}</b> ${p.weekends.length===1?'weekend':'weekends'}</span><span class="plan-stat">${icon('users')}<b>${p.members.length+1}</b> people</span></div><div class="planner-layout"><div><div class="planner-toolbar"><div class="tabs" role="tablist" aria-label="Planner views"><button class="tab-btn ${state.tab==='trips'?'active':''}" data-action="tab" data-tab="trips" role="tab" aria-selected="${state.tab==='trips'}">${icon('calendar')} Trips</button><button class="tab-btn ${state.tab==='crew'?'active':''}" data-action="tab" data-tab="crew" role="tab" aria-selected="${state.tab==='crew'}">${icon('users')} People</button></div>${state.tab==='trips'?`<div class="toolbar-filters"><select id="plan-mountain-filter" aria-label="Filter trips by mountain"><option value="all">All mountains</option>${p.mountains.map(m=>`<option value="${m}" ${state.mountainFilter===m?'selected':''}>${escape(mountainName(m))}</option>`).join('')}</select><select id="plan-sort" aria-label="Sort weekends"><option value="soonest" ${state.sort==='soonest'?'selected':''}>Soonest first</option><option value="popular" ${state.sort==='popular'?'selected':''}>Most people free</option></select></div>`:''}</div><div id="plan-list">${state.tab==='trips'?renderTrips():crewRows(true)}</div></div><aside class="crew-sidebar"><div class="side-card"><p class="eyebrow">${isOwner?'Invite your friends':'You’re invited'}</p><h2>${isOwner?'Invite your group.':'Choose your trips.'}</h2><p>${isOwner?'Send your link and let friends pick the trips they’re free for. Your mountains and dates stay yours.':`Choose from ${escape(p.owner)}’s mountains and weekends, then save your availability.`}</p><button class="btn btn-outline" data-action="${isOwner?'share':'focus-join'}">${icon(isOwner?'link':'calendar')} ${isOwner?'Get the invite link':'Add my availability'}</button></div><div class="side-card"><p class="eyebrow">Group availability</p><h2>Your group <span class="muted">(${p.members.length+1})</span></h2>${crewRows()}</div><div class="source-card">${icon('cloud')}<p>Weather by <a href="https://open-meteo.com/" target="_blank" rel="noopener">Open-Meteo</a>. Updated hourly while the server runs. Current forecasts within 16 days; seasonal estimates beyond that.<br><button class="btn-link" data-action="refresh-weather" style="font-size:10px;text-decoration:underline" ${state.weatherLoading?'disabled':''}>${state.weatherLoading?'Checking forecasts…':'Refresh forecasts'}</button></p></div></aside></div>${isOwner?`<p class="owner-footnote">${icon('shield')} Your owner access is saved in this browser. Your invite link lets friends join the trips you’ve chosen.</p>`:`<section class="join-bar" aria-label="Save your availability"><div class="join-identity">${icon('user')}<div><label for="join-username">Your username</label><input id="join-username" placeholder="e.g. snowseeker" value="${escape(state.username)}" maxlength="24" autocomplete="nickname" ${member?'readonly':''}></div></div><div class="join-actions"><small>${state.selected.size} ${state.selected.size===1?'trip':'trips'} selected</small><button class="btn btn-orange" data-action="save-availability" ${state.busy||(!state.selected.size&&!member)?'disabled':''}>${state.busy?'<span class="spinner"></span> Saving…':member?'Update availability':'Save availability'} ${!state.busy?icon('arrow'):''}</button></div></section>`}<div id="plan-error" class="error-message ${state.error?'':'hidden'}" role="alert" style="margin-bottom:28px">${escape(state.error)}</div>`);
}
async function loadWeather() {
  const p=state.plan;if(!p||state.weatherLoading)return;
  const weatherLoadId=++state.weatherLoadId;
  state.weatherLoading=true;updateWeatherElements();
  try {
    const result=await api('/api/weather',{method:'POST',body:JSON.stringify({mountains:p.mountains,weekends:p.weekends,trips:allTrips(p)})});
    if(state.plan?.id!==p.id||weatherLoadId!==state.weatherLoadId)return;
    state.weather=new Map(result.forecasts.map(f=>[tripKey(f.mountain_id,f.weekend),f]));
  } catch(error) {if(state.plan?.id===p.id)toast('Forecasts couldn’t refresh. You can still plan and join trips.');}
  finally {if(weatherLoadId===state.weatherLoadId){state.weatherLoading=false;if(state.plan?.id===p.id)updateWeatherElements();}}
}
function updateWeatherElements() {
  document.querySelectorAll('[data-weather-key]').forEach(el=>el.innerHTML=weatherHtml(el.dataset.weatherKey));
  const button=$('[data-action="refresh-weather"]');if(button){button.disabled=state.weatherLoading;button.textContent=state.weatherLoading?'Checking forecasts…':'Refresh forecasts';}
}
async function join() {
  if(state.busy)return;
  if(!/^[a-zA-Z0-9 _-]{2,24}$/.test(state.username.trim())) {state.error='Choose a username of 2–24 letters, numbers, spaces, hyphens, or underscores.';renderPlan();$('#join-username')?.focus();return;}
  if(!storageAvailable()) {state.error='Enable local storage so you can update your availability later.';renderPlan();return;}
  state.busy=true;state.error='';renderPlan();
  try {
    const result=await api(`/api/plans/${state.plan.id}/availability`,{method:'PUT',body:JSON.stringify({username:state.username.trim(),trips:[...state.selected]}),token:access(state.plan.id).member_token});
    state.plan=result.plan;rememberPlan(result.plan,{member_id:result.member_id,member_token:result.member_token});save('wc.username',state.username.trim());
    state.username=state.username.trim();state.dirty=false;state.busy=false;renderPlan();toast('Your availability is saved.');
  } catch(error) {state.busy=false;state.error=error.message;renderPlan();$('#plan-error')?.scrollIntoView({block:'nearest',behavior:'smooth'});}
}
function showModal(content) { const modal=$('#modal');modal.innerHTML=content;if(!modal.open)modal.showModal(); }
function modalHead(title) {return `<div class="modal-head"><h2>${title}</h2><button class="icon-btn" data-action="close-modal" aria-label="Close dialog">${icon('x')}</button></div>`;}
function showShare(created=false) {
  const shareUrl=`${location.origin}/p/${state.plan.id}`;
  showModal(`${created?`<div class="success-icon">${icon('check')}</div>`:''}${modalHead(created?'Your planner is ready.':'Share this planner.')}<p class="modal-description">${created?'Your planner is ready. ':''}Send this link to your friends. They can add themselves to your mountains and weekends.</p><div class="share-link-box"><input id="share-url" value="${escape(shareUrl)}" readonly aria-label="Planner invite link"><button class="btn btn-primary" data-action="copy-link">${icon('copy')} Copy</button></div><div class="share-note">${icon('shield')}<p>${owner()?'Only you can edit this planner. Your owner access stays in this browser. This link gives your friends access to join.':'This link lets friends add their availability. Only the planner owner can change mountains and weekends.'}</p></div><div class="modal-actions"><button class="btn btn-outline" data-action="close-modal">${created?'See my weekends':'Done'} ${icon('arrow')}</button></div>`);
}
function explore() {
  showModal(`${modalHead('Find your next mountain.')}<p class="modal-description">${state.mountains.length} US Ikon destinations. Pick one to start your plan. Access depends on your pass; check the resort before you travel.</p><div class="catalog-list">${[...state.mountains].sort((a,b)=>a.name.localeCompare(b.name)).map(m=>`<button data-action="start-mountain" data-id="${m.id}">${escape(m.name)}<small>${m.state} · ${m.region}</small></button>`).join('')}</div>`);
}
function myTrips() {
  const plans=stored('wc.plans',[]);
  shell(`<section class="my-trips"><p class="eyebrow" style="margin-bottom:15px">Your trip plans</p><h1 class="page-title">Your planners.</h1><p class="page-subtitle">Planners you’ve created or joined in this browser.</p>${plans.length?`${plans.map(p=>`<a class="saved-plan" href="/p/${p.id}" data-nav="/p/${p.id}"><div><h2>${escape(p.title)}</h2><p>${p.mountains} mountains · ${p.weekends} weekends · ${access(p.id).owner_token?'You’re the owner':`Planned by ${escape(p.owner)}`}</p></div>${icon('arrow')}</a>`).join('')}<button class="btn btn-primary" data-action="start" style="margin-top:20px">Create another planner ${icon('arrow')}</button>`:`<div class="empty-state">${icon('mountain')}<h2>No planners yet.</h2><p>You haven’t started a planner yet. Pick your mountains and weekends, then invite your favorite people.</p><button class="btn btn-primary" data-action="start">Create a trip plan ${icon('arrow')}</button></div>`}</section>`);
}
async function navigate(path,{retainWizard=false,replace=false}={}) {
  if($('#modal').open)$('#modal').close();
  if(location.pathname!==path)history[replace?'replaceState':'pushState']({},'',path);
  state.route=path;state.error='';
  const loadId=++state.loadId;
  window.scrollTo(0,0);
  if(path==='/') {state.plan=null;state.wizard=null;landing();return;}
  if(path==='/my-trips') {state.plan=null;myTrips();return;}
  if(path==='/new') {if(!retainWizard||!state.wizard) {state.wizard=makeWizard();}state.plan=null;renderWizard();return;}
  const match=path.match(/^\/p\/([a-f0-9]{32})(\/edit)?$/);
  if(!match) {shell(`<div class="empty-state" style="margin:50px 0">${icon('mountain')}<h2>A little off the trail.</h2><p>This page doesn’t exist. Let’s get you back to the mountains.</p><button class="btn btn-primary" data-nav="/">Back to home ${icon('arrow')}</button></div>`);return;}
  if(match[2]&&retainWizard&&state.wizard){renderWizard();return;}
  shell(`<div class="page-loading"><span class="spinner"></span><p class="muted">Loading planner…</p></div>`);
  try {
    const p=await api(`/api/plans/${match[1]}`);if(loadId!==state.loadId)return;
    state.plan=p;state.weather=new Map();state.weatherLoading=false;state.tab='trips';state.mountainFilter='all';state.sort='soonest';state.dirty=false;state.busy=false;
    const m=currentMember();state.username=m?.username||stored('wc.username','');if(!m&&state.username.toLowerCase()===p.owner.toLowerCase())state.username='';
    state.selected=new Set(m?.trips||[]);
    if(match[2]) {if(owner()) {startWizard(null,true);return;}history.replaceState({},'',`/p/${p.id}`);state.route=`/p/${p.id}`;}
    renderPlan();loadWeather();
  } catch(error) {if(loadId!==state.loadId)return;shell(`<div class="empty-state" style="margin:50px 0">${icon('mountain')}<h2>Planner unavailable.</h2><p>${escape(error.message)}</p><button class="btn btn-primary" data-nav="/">Back to home ${icon('arrow')}</button></div>`);}
}
document.addEventListener('input',event=>{
  const el=event.target;
  if(el.id==='owner-username') {state.wizard.username=el.value;const preview=$('#preview-username');if(preview)preview.textContent=el.value?`Planned by ${el.value}`:'Planner owner';}
  if(el.id==='plan-title') {state.wizard.title=el.value;const preview=$('#preview-title');if(preview)preview.textContent=el.value||'Your trip plan';}
  if(el.id==='mountain-search') {state.wizard.search=el.value;$('#mountain-options').innerHTML=mountainOptions();}
  if(el.id==='join-username') {state.username=el.value;state.dirty=true;}
});
document.addEventListener('change',event=>{
  if(event.target.id==='plan-mountain-filter'){state.mountainFilter=event.target.value;renderPlan();}
  if(event.target.id==='plan-sort'){state.sort=event.target.value;renderPlan();}
});
document.addEventListener('click',async event=>{
  const el=event.target.closest('[data-action],[data-nav]');if(!el||el.disabled)return;
  event.preventDefault();
  if(el.dataset.nav){navigate(el.dataset.nav);return;}
  const w=state.wizard,key=el.dataset.key;
  switch(el.dataset.action){
    case 'start':startWizard();break;
    case 'start-mountain':startWizard(el.dataset.id);break;
    case 'explore':explore();break;
    case 'close-modal':$('#modal').close();break;
    case 'wizard-next':await wizardNext();break;
    case 'wizard-back':w.step--;state.error='';renderWizard();window.scrollTo(0,0);break;
    case 'wizard-step':w.step=Number(el.dataset.step);state.error='';renderWizard();break;
    case 'wizard-cancel':navigate(w.editing?`/p/${state.plan.id}`:'/');break;
    case 'region':w.region=el.dataset.region;renderWizard();break;
    case 'toggle-mountain':{const selected=w.choices.get(w.activeWeekend);if(selected.has(el.dataset.id))selected.delete(el.dataset.id);else if(w.mountains.has(el.dataset.id)||w.mountains.size<12)selected.add(el.dataset.id);else toast('Choose up to 12 unique mountains for one planner.');updateWizardMountains();renderWizard();break;}
    case 'active-weekend':w.activeWeekend=el.dataset.date;state.error='';renderWizard();break;
    case 'copy-weekend-mountains':{const selected=w.choices.get(w.activeWeekend);w.weekends.forEach(d=>w.choices.set(d,new Set(selected)));updateWizardMountains();renderWizard();toast('These mountains are selected for every weekend.');break;}
    case 'toggle-weekend':if(w.weekends.has(el.dataset.date)){w.weekends.delete(el.dataset.date);w.choices.delete(el.dataset.date);}else if(w.weekends.size<26){w.weekends.add(el.dataset.date);w.choices.set(el.dataset.date,new Set(w.initialMountain&&(w.mountains.has(w.initialMountain)||w.mountains.size<12)?[w.initialMountain]:[]));}else toast('Choose up to 26 weekends for one planner.');updateWizardMountains();renderWizard();break;
    case 'month-prev':w.month--;renderWizard();break;
    case 'month-next':w.month++;renderWizard();break;
    case 'edit-plan':startWizard(null,true);break;
    case 'share':showShare();break;
    case 'copy-link':try{await navigator.clipboard.writeText($('#share-url').value);toast('Invite link copied.');el.innerHTML=`${icon('check')} Copied`;}catch{$('#share-url').select();toast('Select and copy the invite link to share it.');}break;
    case 'toggle-trip':if(state.selected.has(key))state.selected.delete(key);else state.selected.add(key);state.dirty=true;renderPlan();break;
    case 'select-weekend':{const keys=weekendMountains(state.plan,el.dataset.date).filter(m=>state.mountainFilter==='all'||state.mountainFilter===m).map(m=>tripKey(m,el.dataset.date));const clear=keys.every(k=>state.selected.has(k));keys.forEach(k=>clear?state.selected.delete(k):state.selected.add(k));state.dirty=true;renderPlan();break;}
    case 'save-availability':await join();break;
    case 'focus-join':$('#join-username')?.scrollIntoView({behavior:'smooth',block:'center'});$('#join-username')?.focus({preventScroll:true});break;
    case 'tab':state.tab=el.dataset.tab;renderPlan();break;
    case 'refresh-weather':await loadWeather();break;
    case 'estimate-details':{const f=state.weather.get(key),basis=f?.estimate_basis;showModal(`${modalHead('Seasonal weather estimate')}<p class="modal-description">These numbers are planning estimates based on ${basis?.start_year}–${basis?.end_year} historical weather at ${escape(mountainName(f.mountain_id))}. They average daily temperatures, snowfall, and wind within seven calendar days of each trip day across those years.</p><p class="modal-description">This describes typical conditions for that time of year, rather than predicting an individual storm. Current forecasts replace the estimates when your dates enter the 16-day forecast range. ${f.status==='mixed'?'One day currently uses a forecast and the other uses a seasonal estimate.':''}</p><a class="btn btn-outline" href="https://open-meteo.com/en/docs/historical-weather-api" target="_blank" rel="noopener">Historical data source ↗</a><div class="modal-actions"><button class="btn btn-primary" data-action="close-modal">Done</button></div>`);break;}
    case 'attendees':{const [id,weekend]=key.split('@');showModal(`${modalHead('Who is available?')}<p class="modal-description">${escape(mountainName(id))} · ${weekendLabel(weekend,true)}</p>${attendees(key).map((m,i)=>`<div class="crew-row">${avatar(m.username,i,m.isOwner)}<div><b>${escape(m.username)}</b><small>${m.isOwner?'Planner owner':'Available for this trip'}</small></div></div>`).join('')}<div class="modal-actions"><button class="btn btn-outline" data-action="close-modal">Done ${icon('check')}</button></div>`);break;}
  }
});
document.addEventListener('keydown',event=>{if(event.key==='Enter'&&event.target.id==='join-username')join();if(event.key==='Enter'&&['owner-username','plan-title'].includes(event.target.id))wizardNext();});
$('#modal').addEventListener('click',event=>{if(event.target===$('#modal')){const r=$('#modal').getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)$('#modal').close();}});
window.addEventListener('popstate',()=>navigate(location.pathname,{replace:true}));
setInterval(async()=>{
  if(!state.plan||state.wizard||state.dirty||state.busy||$('#modal').open||document.hidden||document.activeElement?.tagName==='INPUT')return;
  try {const p=await api(`/api/plans/${state.plan.id}`);if(state.plan?.id===p.id&&!state.dirty){state.plan=p;state.selected=new Set(currentMember()?.trips||[]);renderPlan();updateWeatherElements();}}catch{/* Retry next poll without interrupting the planner. */}
},30000);
setInterval(()=>{if(state.plan&&!state.wizard&&!document.hidden)loadWeather();},15*60*1000);
async function boot() {
  try {const data=await api('/api/mountains');state.mountains=data.mountains;state.today=data.today;await navigate(location.pathname,{replace:true});}
  catch(error){shell(`<div class="empty-state" style="margin:50px 0">${icon('mountain')}<h2>Unable to connect.</h2><p>${escape(error.message)}</p><button class="btn btn-primary" data-action="reload">Try again ${icon('arrow')}</button></div>`);}
}
document.addEventListener('click',event=>{if(event.target.closest('[data-action="reload"]'))boot();});
boot();
