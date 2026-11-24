'use strict';
(() => {
  const $ = id => document.getElementById('xdb' + id);
  const invoke = (name,args={})=>window.__TAURI__.core.invoke(name,args);
  const labels={applied:'Применено',already_accounted:'Уже учтено',needs_review:'Требуется решение',rejected:'Отклонено'};
  const fields={title:'Название',executor:'Исполнитель',start:'Начало',deadline:'Срок',comment:'Комментарий',addressees:'Адресаты',status:'Статус',parentUid:'Вышестоящий этап',sort:'Порядок','$create':'Новый этап','$delete':'Удаление',name:'Проект',orderNo:'Заказ',topic:'Тематика'};
  let mode='export', catalog=[], encoded='', plan=null, session=0, busy=false;
  function el(tag,text){const n=document.createElement(tag);if(text!=null)n.textContent=text;return n}
  function status(t){$('Status').textContent=t}
  function invalidate(){plan=null;$('PlanRows').replaceChildren();$('Go').disabled=mode==='import'}
  function reset(){session++;busy=false;catalog=[];encoded='';plan=null;
    for(const id of ['Pin','Pass','Comment','File']) $(id).value='';
    for(const id of ['Mapping','PlanRows','Reviews','Projects','Status']) $(id).replaceChildren();
    $('ReviewPane').open=false;
  }
  function tab(next){mode=next;$('ExportPane').hidden=next!=='export';$('ImportPane').hidden=next!=='import';$('ImportActions').hidden=next!=='import';$('CommentLabel').hidden=next!=='import'&&!$('ReviewPane').open;
    $('ExportTab').classList.toggle('active',next==='export');$('ImportTab').classList.toggle('active',next==='import');
    $('PassHint').textContent=next==='export'?'От 12 символов. Передайте фразу получателю отдельно.':'Введите фразу переданного файла. Для старой открытой базы оставьте поле пустым.';
    $('Go').textContent=next==='export'?'Сохранить файл базы':'Подписать и применить';$('Plan').hidden=true;$('Mapping').replaceChildren();invalidate();status('');}
  function guarded(fn){return async()=>{if(busy)return;const generation=session;busy=true;try{await fn(()=>generation===session)}catch(e){if(generation===session)status(String(e?.message||e))}finally{if(generation===session)busy=false}}}
  async function open(){reset();tab('export');$('Kind').value=window.ProductionShared?'assignment':'changes';const epoch=session;
    try{const items=await invoke('production_list_projects');if(epoch!==session)return;catalog=items;
      for(const p of items){const l=el('label');l.className='checkline';const ch=el('input');ch.type='checkbox';ch.value=p.projectId;l.append(ch,document.createTextNode(p.name+' · заказ '+p.orderNo));$('Projects').append(l)}
      if(!items.length)$('Projects').textContent='Сохранённых проектов пока нет. Для получения проекта выберите импорт.';
    }catch(e){if(epoch===session)status(String(e))}}
  function choices(){return Array.from($('Mapping').querySelectorAll('select'),n=>({guid:n.dataset.guid,target:Number(n.value)}))}
  const args=()=>({data:encoded,passphrase:$('Pass').value});
  const fmt=v=>v==null?'—':typeof v==='string'?v:JSON.stringify(v);
  function details(list,container){container.replaceChildren();for(const p of list){const h=el('h3',p.project);container.append(h);
    if(!p.rows.length&&!p.receipt?.length)container.append(el('p','Карточка проекта сверена.'));
    for(const r of p.rows){const d=el('details');d.append(el('summary',(r.repeat?'Повтор · ':'')+labels[r.state]+' · '+(fields[r.field]||r.field)),el('p',r.reason));
      if(!r.repeat)d.append(el('pre','Этап: '+fmt(r.stage)+'\nБыло: '+fmt(r.before)+'\nНа принимающем компьютере: '+fmt(r.server)+'\nПрислано: '+fmt(r.incoming)));container.append(d)}
    for(const r of p.receipt||[])container.append(el('p',(labels[r.state]||r.state)+' · '+r.reason));
  }}
  $('ExportTab').onclick=()=>{if(!busy)tab('export')};$('ImportTab').onclick=()=>{if(!busy)tab('import')};
  $('File').onchange=()=>{encoded='';$('Mapping').replaceChildren();$('Plan').hidden=true;invalidate()};
  $('Pass').oninput=invalidate;
  $('Inspect').onclick=guarded(async live=>{
    invalidate();$('Plan').hidden=true;$('Mapping').replaceChildren();const f=$('File').files[0];if(!f||f.size>48*1024*1024)throw Error('Выберите файл до 48 МиБ.');
    const buffer=await f.arrayBuffer();if(!live())return;
    const bytes=new Uint8Array(buffer),parts=[];for(let i=0;i<bytes.length;i+=8192)parts.push(String.fromCharCode(...bytes.subarray(i,i+8192)));encoded=btoa(parts.join(''));
    const res=await invoke('exchange_preview',{...args(),choices:null});if(!live())return;status('Выберите назначение каждого проекта.');
    const box=$('Mapping');box.replaceChildren(el('p','Источник: '+res.sender));
    if(res.legacy||res.keys.length){for(const k of res.keys)box.append(el('pre',k.name+' · SHA-256 открытого ключа\n'+k.fingerprint));
      const l=el('label');l.className='checkline';const ck=el('input');ck.type='checkbox';ck.dataset.trust='1';l.append(ck,document.createTextNode(res.legacy?'Происхождение старой базы проверено.':'Я сверил отпечатки новых ключей с отправителем.'));box.append(l)}
    for(const p of res.projects){const l=el('label',p.name+' · заказ '+p.order+' · этапов '+p.stages);const s=el('select');s.dataset.guid=p.guid;
      s.append(new Option('Пропустить',-1));if(p.target==null){s.append(new Option('Создать самостоятельный проект',0));for(const a of catalog)s.append(new Option(a.name+' · '+a.orderNo+' · № '+a.projectId,a.projectId));s.value='-1'}
      else{s.append(new Option('Обновить связанный проект № '+p.target,p.target));s.value=String(p.target)}
      s.onchange=invalidate;l.append(s);box.append(l)}
    $('Plan').hidden=false;
  });
  $('Plan').onclick=guarded(async live=>{invalidate();const res=await invoke('exchange_preview',{...args(),choices:choices()});if(!live())return;plan=res;details(res.plan.projects,$('PlanRows'));$('Go').disabled=false;status('Проверьте изменения. Для записи введите ПИН и комментарий.')});
  $('Go').onclick=guarded(async live=>{
    if(mode==='export'){const ids=Array.from($('Projects').querySelectorAll('input:checked'),x=>Number(x.value));const path=await invoke('exchange_export',{ids,kind:$('Kind').value,pin:$('Pin').value,passphrase:$('Pass').value});if(live()){status('Зашифрованный файл сохранён:\n'+path);$('Pin').value='';$('Pass').value=''}return}
    if(!plan)throw Error('Сначала проверьте изменения.');
    const result=await invoke('exchange_commit',{...args(),choices:choices(),token:plan.token,trust:Boolean($('Mapping').querySelector('[data-trust]')?.checked),pin:$('Pin').value,comment:$('Comment').value});if(!live())return;
    details(result.projects,$('PlanRows'));plan=null;$('Go').disabled=true;$('Pin').value='';status('Приём записан. Чтобы передать ответ, выберите экспорт и тип «Квитанция и актуальные проекты».'+(result.backupWarning?' Резервная копия: '+result.backupWarning:''));
  });
  $('ReviewPane').ontoggle=()=>{$('CommentLabel').hidden=mode!=='import'&&!$('ReviewPane').open};
  $('Refresh').onclick=guarded(async live=>{const list=await invoke('exchange_reviews');if(!live())return;const box=$('Reviews');box.replaceChildren();
    if(!list.length)box.append(el('p','Изменений на согласовании нет.'));
    for(const r of list){const d=el('details');d.append(el('summary',r.project+' · '+(fields[r.op.field]||r.op.field)),el('p',r.reason),el('pre','Было: '+fmt(r.op.before)+'\nСейчас: '+fmt(r.server)+'\nПрислано: '+fmt(r.op.after)));
      for(const [accept,t] of [[true,'Принять предложенное'],[false,'Оставить действующее']]){const b=el('button',t);b.className='btn';b.onclick=guarded(async valid=>{await invoke('exchange_resolve',{op:r.op.id,revision:r.revision,accept,pin:$('Pin').value,comment:$('Comment').value});if(valid()){d.remove();$('Pin').value='';status('Решение записано.')}});d.append(b)}box.append(d)}
  });
  window.ProductionDbExchange={open,reset};
})();
