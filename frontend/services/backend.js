'use strict';
(() => {
  const D=Production.domain,X=Production.exchange;
  const native=()=>Boolean(window.__TAURI__&&window.__TAURI__.core&&typeof window.__TAURI__.core.invoke==='function');
  const invoke=(command,args={})=>window.__TAURI__.core.invoke(command,args);
  function toSnapshot(state){
    return {
      project:{
        name:state.project.name,
        orderNo:state.project.orderNo,
        initiator:state.project.initiator,
        executor:state.project.executor,
        enterprise:state.project.addressees,
        start:state.project.start,
        deadline:state.project.deadline
      },
      stages:state.stages.map(s=>({...s})),
      nextSeq:state.nextSeq
    };
  }
  function fromSnapshot(snapshot){
    if(!snapshot)return null;
    return {
      project:{
        name:snapshot.project.name,
        orderNo:snapshot.project.orderNo,
        initiator:snapshot.project.initiator,
        executor:snapshot.project.executor,
        addressees:snapshot.project.enterprise,
        start:snapshot.project.start,
        deadline:snapshot.project.deadline
      },
      stages:snapshot.stages.map(s=>({...s})),
      nextSeq:snapshot.nextSeq,
      collapsed:{}
    };
  }
  const issueMessages=items=>(items||[]).map(item=>typeof item==='string'?item:item.message||String(item));
  async function validate(state){
    if(!native())return D.validate(state);
    const result=await invoke('production_validate',{snapshot:toSnapshot(state)});
    return {errors:issueMessages(result.errors),warnings:issueMessages(result.warnings)};
  }
  async function save(state){
    if(!native())return false;
    await invoke('production_save_snapshot',{snapshot:toSnapshot(state)});
    return true;
  }
  async function load(orderNo){
    if(!native())return null;
    return fromSnapshot(await invoke('production_load_snapshot',{orderNo:String(orderNo||'').trim()}));
  }
  async function listProjects(){
    if(!native())return [];
    return invoke('production_list_projects');
  }
  function localDictionary(kind,state){
    const values=[];
    if(kind==='projectName')values.push(state.project.name);
    if(kind==='initiator')values.push(state.project.initiator);
    if(kind==='executor')values.push(state.project.executor,...state.stages.map(s=>s.executor));
    if(kind==='enterprise')values.push(state.project.addressees,...state.stages.map(s=>s.addressees));
    if(kind==='stageTitle')values.push(...state.stages.map(s=>s.title));
    const counts=new Map();for(const raw of values){const value=String(raw||'').trim();if(value)counts.set(value,(counts.get(value)||0)+1)}
    return [...counts].sort((a,b)=>a[0].localeCompare(b[0],'ru')).map(([value,usageCount])=>({value,usageCount,projectCount:1}));
  }
  async function listDictionary(kind,state){return native()?invoke('production_list_dictionary',{kind}):localDictionary(kind,state)}
  async function replaceDictionaryValue(kind,fromValue,toValue){
    if(!native())return {affectedProjectRows:0,affectedStageRows:0};
    return invoke('production_replace_dictionary_value',{kind,fromValue,toValue});
  }
  async function exportPair(state){
    if(!native())return X.exportPair(state);
    const bundle=await invoke('production_export_txt',{snapshot:toSnapshot(state)});
    return {out:bundle.outgoingText,in:bundle.incomingText,outFileName:bundle.outgoingFileName,inFileName:bundle.incomingFileName};
  }
  async function report(state){
    if(!native())return D.report(state);
    const value=await invoke('production_get_management_report',{snapshot:toSnapshot(state)});
    return {
      rows:value.stages.map(row=>({stage:row.stage,depth:row.depth,visualStatus:row.visualStatus,daysRemaining:row.daysRemaining})),
      counts:{new:value.counts.newCount,work:value.counts.work,hold:value.counts.hold,done:value.counts.done,overdue:value.counts.overdue},
      total:value.counts.total,
      donePercent:value.counts.donePercent,
      dueSoon:value.counts.dueWithin7Days,
      overdueHeld:value.overdueHeld,
      status:value.status,
      projectDays:value.projectDays,
      projectOverdue:value.projectOverdue
    };
  }
  Production.backend=Object.freeze({native,toSnapshot,fromSnapshot,validate,save,load,listProjects,listDictionary,replaceDictionaryValue,exportPair,report});
})();
