(function(root){'use strict';
  // Existing read commands, verified against d12e437. No database writes.
  function createTauriDashboardProvider(invoke,options={}){
    if(typeof invoke!=='function')throw Error('Нужна функция Tauri invoke.');
    const stamp=items=>JSON.stringify(items.map(p=>[p.projectId,p.updatedAt,p.stageCount,p.doneCount]).sort((a,b)=>a[0]-b[0]));
    return async function(){
      const before=await invoke('production_list_projects',{}),projects=[];
      if(!Array.isArray(before))throw Error('Команда списка проектов вернула неверный формат.');
      for(const summary of before){const snapshot=await invoke('production_load_snapshot_by_id',{projectId:summary.projectId});if(!snapshot)throw Error('Проект изменился во время чтения. Обновите панель.');projects.push({...snapshot,updatedAt:summary.updatedAt});}
      const events=options.includeAudit===true?await invoke('audit_list_events',{projectId:null}):[];
      const after=await invoke('production_list_projects',{});
      if(stamp(before)!==stamp(after))throw Error('Данные менялись во время обновления. Повторите обновление.');
      return {schemaVersion:1,source:{kind:'host',label:'Производственный цикл · Tauri',asOf:root.PCDomain.today(),generatedAt:new Date().toISOString(),consistent:false,auditAvailable:options.includeAudit===true},projects,events};
    };
  }
  root.createTauriDashboardProvider=createTauriDashboardProvider;
})(globalThis);
