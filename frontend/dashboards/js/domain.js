(function(root,factory){'use strict';const api=factory();if(typeof module==='object'&&module.exports)module.exports=api;else root.PCDomain=api;})(typeof globalThis!=='undefined'?globalThis:this,function(){
  'use strict';
  const DAY=86400000, STATUSES=['new','work','hold','done'];
  const labels={new:'Не начато',work:'В работе',hold:'Приостановлено',done:'Выполнено'};
  function validDate(s){if(typeof s!=='string'||!/^\d{4}-\d{2}-\d{2}$/.test(s))return false;const n=Date.parse(s+'T00:00:00Z');return Number.isFinite(n)&&new Date(n).toISOString().slice(0,10)===s;}
  function today(){const d=new Date();return [d.getFullYear(),String(d.getMonth()+1).padStart(2,'0'),String(d.getDate()).padStart(2,'0')].join('-');}
  function days(a,b){if(!validDate(a)||!validDate(b))throw Error('Некорректная дата расчёта.');return Math.round((Date.parse(a+'T00:00:00Z')-Date.parse(b+'T00:00:00Z'))/DAY);}
  function addDays(date,n){return new Date(Date.parse(date+'T00:00:00Z')+n*DAY).toISOString().slice(0,10);}
  function text(v,max=2000){if(typeof v!=='string'||v.length>max)throw Error('Текстовое поле отсутствует или превышает допустимый размер.');return v.trim();}
  function required(v,label){const s=text(v);if(!s)throw Error('Не заполнено: '+label);return s;}
  function date(v,label){if(!validDate(v))throw Error('Неверная дата: '+label);return v;}
  function normalize(input){
    if(!input||typeof input!=='object'||Array.isArray(input))throw Error('Ожидается объект с проектами или снимок проекта.');
    if(input.schemaVersion!==undefined&&input.schemaVersion!==1)throw Error('Эта версия формата данных не поддерживается.');
    const raw=input.projects||(input.project?[input]:null);
    if(!Array.isArray(raw)||raw.length>5000)throw Error('Ожидается массив projects, не более 5000 проектов.');
    const ids=new Set();let total=0;
    const projects=raw.map((item)=>{
      if(!item||typeof item!=='object')throw Error('Некорректный проект.');
      const p=item.project||item,id=item.projectId??item.databaseId;
      if(!Number.isSafeInteger(id)||id<1||ids.has(id))throw Error('Нужен уникальный положительный projectId или databaseId.');ids.add(id);
      if(!Array.isArray(item.stages))throw Error('Отсутствует массив этапов проекта '+id);
      total+=item.stages.length;if(total>50000)throw Error('В одном наборе поддерживается не более 50000 этапов.');
      const project={projectId:id,orderNo:required(p.orderNo,'номер заказа'),name:required(p.name,'название проекта'),enterprise:required(p.enterprise??p.addressees,'предприятие проекта'),executor:text(p.executor??''),start:date(p.start,'начало проекта'),deadline:date(p.deadline,'срок проекта'),updatedAt:text(item.updatedAt??''),stages:[]};
      if(project.start>project.deadline)throw Error('Начало проекта позже срока: '+id);
      const uids=new Set();project.stages=item.stages.map(s=>{
        if(!s||typeof s!=='object')throw Error('Некорректный этап.');
        const uid=required(s.uid,'UID этапа');if(uids.has(uid))throw Error('Повтор UID в проекте '+id);uids.add(uid);
        if(!STATUSES.includes(s.status))throw Error('Неизвестный статус этапа '+uid);
        const stage={uid,seq:Number.isSafeInteger(s.seq)&&s.seq>0?s.seq:0,parentUid:s.parentUid==null?null:required(s.parentUid,'головной этап'),title:required(s.title,'название этапа'),executor:required(s.executor,'исполнитель этапа'),addressees:text(s.addressees??''),start:date(s.start,'начало этапа'),deadline:date(s.deadline,'срок этапа'),status:s.status,comment:text(s.comment??'',10000)};
        if(stage.start>stage.deadline)throw Error('Начало этапа позже срока: '+uid);return stage;
      });
      const map=new Map(project.stages.map(s=>[s.uid,s]));const complete=new Set();
      project.stages.forEach(s=>{let cur=s;const path=new Set();while(cur&&!complete.has(cur.uid)){if(path.has(cur.uid))throw Error('Цикл подчинённости в проекте '+id);path.add(cur.uid);if(cur.parentUid&&!map.has(cur.parentUid))throw Error('Не найден головной этап '+cur.parentUid);cur=cur.parentUid?map.get(cur.parentUid):null;}for(const uid of path)complete.add(uid);});
      return project;
    });
    const pids=new Set(projects.map(p=>p.projectId));const rawEvents=input.events??[];
    if(!Array.isArray(rawEvents)||rawEvents.length>100000)throw Error('Некорректный список событий или более 100000 событий.');
    const events=rawEvents.map(e=>{
      if(!e||!pids.has(e.projectId))throw Error('Событие ссылается на неизвестный проект.');
      if(!Array.isArray(e.changes??[]))throw Error('Неверный список изменений.');
      const timestamp=required(e.createdAt,'время события');if(!Number.isFinite(Date.parse(timestamp)))throw Error('Неверное время события.');
      return {eventId:text(e.eventId??''),projectId:e.projectId,stageUid:e.stageUid==null?null:text(e.stageUid),eventType:required(e.eventType,'тип события'),actorName:required(e.actorName,'автор события'),createdAt:timestamp,comment:text(e.comment??'',10000),evidenceType:text(e.evidenceType??''),evidenceReference:text(e.evidenceReference??''),changes:(e.changes??[]).map(c=>({entity:text(c.entity??''),entityId:c.entityId==null?'':text(c.entityId),field:text(c.field??''),before:text(c.before??'',10000),after:text(c.after??'',10000)}))};
    });
    const source=input.source&&typeof input.source==='object'?input.source:{};
    return {schemaVersion:1,source:{kind:['demo','host','import'].includes(source.kind)?source.kind:'import',label:text(source.label??'Файл JSON'),generatedAt:text(source.generatedAt??''),consistent:source.consistent===true,auditAvailable:source.auditAvailable===true,asOf:validDate(source.asOf)?source.asOf:today()},projects,events};
  }
  function isLeafStages(stages){const parents=new Set(stages.map(s=>s.parentUid).filter(Boolean));return stages.filter(s=>!parents.has(s.uid));}
  function analyse(data,filters={},settings={}){
    const asOf=date(filters.asOf||data.source.asOf||today(),'дата расчёта'),horizon=[7,14,30].includes(settings.horizon)?settings.horizon:7;
    const query=String(filters.search||'').trim().toLocaleLowerCase('ru');
    const selected=data.projects.filter(p=>(!filters.orderNo||p.orderNo===filters.orderNo)&&(!filters.enterprise||p.enterprise===filters.enterprise)&&(!filters.projectId||String(p.projectId)===String(filters.projectId))&&(!query||[p.name,p.orderNo,p.enterprise].some(x=>x.toLocaleLowerCase('ru').includes(query))));
    const rows=[],projects=[];
    for(const p of selected){
      const population=settings.countMode==='leaves'?isLeafStages(p.stages):p.stages;
      const stages=population.filter(s=>!filters.executor||s.executor===filters.executor);
      if(filters.executor&&!stages.length)continue;
      const stageMap=new Map(p.stages.map(s=>[s.uid,s]));
      const annotated=stages.map(s=>({...s,key:p.projectId+':'+s.uid,projectId:p.projectId,projectName:p.name,orderNo:p.orderNo,enterprise:p.enterprise,daysLeft:days(s.deadline,asOf),projectDeadline:p.deadline,parentDeadline:s.parentUid?stageMap.get(s.parentUid).deadline:null}));
      rows.push(...annotated);const done=annotated.filter(s=>s.status==='done').length;
      const projectDone=p.stages.length>0&&p.stages.every(s=>s.status==='done');
      projects.push({...p,rows:annotated,total:annotated.length,done,progress:annotated.length?Math.round(100*done/annotated.length):0,projectDone,projectOverdue:!projectDone&&p.deadline<asOf,overdue:annotated.filter(s=>s.status!=='done'&&s.daysLeft<0).length,held:annotated.filter(s=>s.status==='hold').length});
    }
    const active=rows.filter(s=>s.status!=='done'),overdue=active.filter(s=>s.daysLeft<0),done=rows.filter(s=>s.status==='done');
    const group=(key)=>{const map=new Map();for(const row of rows){const name=row[key];if(!map.has(name))map.set(name,{name,total:0,active:0,overdue:0,held:0,done:0});const g=map.get(name);g.total++;if(row.status==='done')g.done++;else{g.active++;if(row.daysLeft<0)g.overdue++;if(row.status==='hold')g.held++;}}return [...map.values()].sort((a,b)=>b.overdue-a.overdue||b.active-a.active||a.name.localeCompare(b.name,'ru'));};
    const projectIds=new Set(selected.map(p=>p.projectId));const events=data.events.filter(e=>projectIds.has(e.projectId)).sort((a,b)=>Date.parse(b.createdAt)-Date.parse(a.createdAt));
    return {asOf,horizon,projects,rows,active,overdue,done,events,enterprises:group('enterprise'),executors:group('executor'),counts:{projects:projects.length,activeProjects:projects.filter(p=>!p.projectDone).length,projectOverdue:projects.filter(p=>p.projectOverdue).length,total:rows.length,done:done.length,active:active.length,overdue:overdue.length,heldOverdue:overdue.filter(s=>s.status==='hold').length,hold:rows.filter(s=>s.status==='hold').length,today:active.filter(s=>s.daysLeft===0).length,dueSoon:active.filter(s=>s.daysLeft>=1&&s.daysLeft<=horizon).length,donePercent:rows.length?Math.round(done.length/rows.length*100):0},statuses:STATUSES.map(status=>({status,label:labels[status],count:rows.filter(s=>s.status===status).length})),calendar:Array.from({length:horizon+1},(_,i)=>({date:addDays(asOf,i),count:active.filter(s=>s.daysLeft===i).length})),quality:{emptyProjects:projects.filter(p=>!p.stages.length).length,afterProject:rows.filter(s=>s.deadline>s.projectDeadline).length,afterParent:rows.filter(s=>s.parentDeadline&&s.deadline>s.parentDeadline).length}};
  }
  function csvCell(value){let s=String(value??'');if(/^[\s]*[=+@-]/.test(s))s="'"+s;return '"'+s.replace(/"/g,'""')+'"';}
  function participation(data,filters={},settings={},requestedOrder=''){
    // Keep the whole order as denominator when one performer is highlighted.
    const all=analyse(data,{...filters,executor:''},settings);
    const orders=[...new Set(all.projects.map(p=>p.orderNo))].sort((a,b)=>a.localeCompare(b,'ru',{numeric:true}));
    const requested=requestedOrder||filters.orderNo||'';
    const orderNo=requested;
    const rows=all.rows.filter(s=>s.orderNo===orderNo),total=rows.length,doneTotal=rows.filter(s=>s.status==='done').length;
    const groups=new Map();
    for(const s of rows){if(!groups.has(s.executor))groups.set(s.executor,{executor:s.executor,assigned:0,done:0,overdue:0});const g=groups.get(s.executor);g.assigned++;if(s.status==='done')g.done++;else if(s.daysLeft<0)g.overdue++;}
    const percent=(n,d)=>d?Math.round(n/d*1000)/10:0;
    const members=[...groups.values()].map(g=>({...g,active:g.assigned-g.done,sharePercent:percent(g.assigned,total),donePercent:percent(g.done,g.assigned),doneOrderPercent:percent(g.done,total),highlighted:!!filters.executor&&g.executor===filters.executor})).sort((a,b)=>b.assigned-a.assigned||a.executor.localeCompare(b.executor,'ru'));
    return {orders,orderNo,total,doneTotal,donePercent:percent(doneTotal,total),projectCount:all.projects.filter(p=>p.orderNo===orderNo).length,members};
  }
  function toCSV(rows){const head=['ID проекта','Номер заказа','Проект','Предприятие проекта','UID этапа','Этап','Исполнитель','Статус','Начало','Срок исполнения','Дней до срока','Комментарий'];return '\uFEFF'+[head,...rows.map(s=>[s.projectId,s.orderNo,s.projectName,s.enterprise,s.uid,s.title,s.executor,labels[s.status],s.start,s.deadline,s.daysLeft,s.comment])].map(row=>row.map(csvCell).join(';')).join('\r\n');}
  return Object.freeze({normalize,analyse,participation,validDate,today,days,addDays,toCSV,labels});
});
