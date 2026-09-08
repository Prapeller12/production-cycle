'use strict';
(() => {
  const {STATUS,dueSoonDays}=Production.config;
  const text=v=>typeof v==='string'&&v.trim().length>0;
  function orderToken(value){return String(value??'').trim().replace(/\s+/g,'-').replace(/[;\[\]]/g,'').replace(/[^0-9A-Za-zА-Яа-яЁё._\/-]/g,'-')||'—'}
  function stageId(state,stage){return orderToken(state.project.orderNo)+'-'+String(stage.seq).padStart(3,'0')}
  function validDate(value){if(typeof value!=='string'||!/^\d{4}-\d{2}-\d{2}$/.test(value))return false;const d=new Date(value+'T00:00:00Z');return !isNaN(d)&&d.toISOString().slice(0,10)===value}
  function today(){const d=new Date();return [d.getFullYear(),String(d.getMonth()+1).padStart(2,'0'),String(d.getDate()).padStart(2,'0')].join('-')}
  function daysLeft(value,now=today()){return validDate(value)&&validDate(now)?Math.round((Date.parse(value+'T00:00:00Z')-Date.parse(now+'T00:00:00Z'))/86400000):null}
  function fullDate(v){return validDate(v)?v.split('-').reverse().join('.'):''}
  function shortDate(v){return fullDate(v).replace(/\.(\d{2})(\d{2})$/,'.$2')}
  function children(state,parent=null){return state.stages.filter(s=>(s.parentUid||null)===(parent||null)).sort((a,b)=>a.sort-b.sort||a.seq-b.seq)}
  function structureErrors(state){
    const errors=[],seen=new Set(),seqs=new Set();
    if(!state||typeof state.project!=='object'||!state.project||!Array.isArray(state.stages))return ['Неверная структура проекта.'];
    const map=new Map();
    for(const s of state.stages){
      if(!s||typeof s!=='object'){errors.push('Этап должен быть объектом.');continue}
      if(!text(s.uid)||seen.has(s.uid))errors.push('Пустой или дублирующийся UID этапа.');seen.add(s.uid);map.set(s.uid,s);
      if(!Number.isSafeInteger(s.seq)||s.seq<1||seqs.has(s.seq))errors.push('Некорректный или повторяющийся seq этапа.');seqs.add(s.seq);
      if(!Number.isSafeInteger(s.sort)||s.sort<0)errors.push('Некорректный порядок этапа.');
      if(s.parentUid!==null&&s.parentUid!==undefined&&!text(s.parentUid))errors.push('Некорректный родитель.');
    }
    if(errors.length)return errors;
    const done=new Set();
    for(const stage of state.stages){
      const path=new Set();let s=stage;
      while(s&&!done.has(s.uid)){
        if(path.has(s.uid)){errors.push('Циклическая связь этапов.');break}
        path.add(s.uid);
        if(s.parentUid&&!map.has(s.parentUid)){errors.push('У этапа отсутствует родитель: '+s.uid);break}
        s=s.parentUid?map.get(s.parentUid):null;
      }
      for(const id of path)done.add(id);
    }
    return [...new Set(errors)];
  }
  function preorder(state){
    const errors=structureErrors(state);if(errors.length)throw Error(errors.join(' '));
    const groups=new Map();for(const s of state.stages){const parent=s.parentUid||null;if(!groups.has(parent))groups.set(parent,[]);groups.get(parent).push(s)}
    for(const group of groups.values())group.sort((a,b)=>a.sort-b.sort||a.seq-b.seq);
    const stack=(groups.get(null)||[]).slice().reverse().map(stage=>({stage,depth:0})),out=[];
    while(stack.length){const item=stack.pop();out.push(item);for(const s of (groups.get(item.stage.uid)||[]).slice().reverse())stack.push({stage:s,depth:item.depth+1})}
    return out;
  }
  function descendants(state,id){const out=new Set(),stack=[id];while(stack.length){for(const s of children(state,stack.pop())){if(s.uid===id)throw Error('Цикл.');if(!out.has(s.uid)){out.add(s.uid);stack.push(s.uid)}}}return out}
  function validate(state,{draft=false}={}){
    const errors=structureErrors(state),warnings=[];if(errors.length)return{errors,warnings};
    const fields=(obj,keys,label)=>{for(const key of keys){if(typeof obj[key]!=='string')errors.push(label+': неверный тип '+key);else if(!draft&&!text(obj[key]))errors.push(label+': не заполнено '+key);else if(/[\t\r\n]/.test(obj[key]))errors.push(label+': TAB/CR/LF недопустимы в '+key)}};
    fields(state.project,['name','orderNo','initiator','executor','addressees','start','deadline'],'Проект');
    function dates(obj,label){for(const key of ['start','deadline'])if(obj[key]&&!validDate(obj[key]))errors.push(label+': неверная дата '+key);if(validDate(obj.start)&&validDate(obj.deadline)&&obj.start>obj.deadline)errors.push(label+': начало позже дедлайна')}
    dates(state.project,'Проект');
    for(const s of state.stages){fields(s,['title','executor','addressees','start','deadline'],'Этап '+s.seq);dates(s,'Этап '+s.seq);if(!Object.hasOwn(STATUS,s.status))errors.push('Некорректный статус этапа '+s.seq)}
    if(!state.stages.length)warnings.push('Проект не содержит этапов: входящий TXT будет содержать только заголовок.');
    if(!draft&&orderToken(state.project.orderNo)==='—')errors.push('Некорректный номер заказа.');
    return{errors,warnings};
  }
  function visualStatus(s,now=today()){if(s.status==='done'||s.status==='hold')return s.status;if(s.start&&now<s.start)return'new';if(s.deadline&&now>s.deadline)return'overdue';return s.status==='new'?'new':'work'}
  function projectStatus(state,now=today()){if(state.stages.length&&state.stages.every(s=>s.status==='done'))return'done';if(state.stages.some(s=>visualStatus(s,now)==='overdue'))return'overdue';if(state.project.start&&now<state.project.start)return'new';return'work'}
  function report(state,now=today()){
    const rows=preorder(state),counts={new:0,work:0,hold:0,done:0,overdue:0};let dueSoon=0,overdueHeld=0;
    for(const {stage:s} of rows){const st=visualStatus(s,now),d=daysLeft(s.deadline,now);counts[st]++;if(st!=='done'&&st!=='overdue'&&d!==null&&d>=0&&d<=dueSoonDays)dueSoon++;if(st==='hold'&&d!==null&&d<0)overdueHeld++}
    const status=projectStatus(state,now),projectDays=daysLeft(state.project.deadline,now);
    return{rows,counts,total:rows.length,donePercent:rows.length?Math.round(counts.done/rows.length*100):0,dueSoon,overdueHeld,status,projectDays,projectOverdue:status!=='done'&&projectDays!==null&&projectDays<0};
  }
  Production.domain=Object.freeze({orderToken,stageId,validDate,today,daysLeft,fullDate,shortDate,children,structureErrors,preorder,descendants,validate,visualStatus,projectStatus,report});
})();
