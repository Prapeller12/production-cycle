(function(root,factory){'use strict';const api=factory();if(typeof module==='object'&&module.exports)module.exports=api;else root.ProductionStatistics=api;})(typeof globalThis!=='undefined'?globalThis:this,function(){
  'use strict';
  function create(options){
    const {container,backend,getSession,openProject}=options;
    const mount=options.mount||globalThis.ProductionDashboards.mount;
    const providerFactory=options.providerFactory||globalThis.createTauriDashboardProvider;
    let panel=null,pending=null,generation=0;
    const identity=()=>{const s=getSession();return {key:String(s.key),ready:s.ready===true};};
    const fresh=(ticket,key)=>{const s=identity();return ticket===generation&&s.ready&&s.key===key;};
    const stale=()=>Object.assign(Error('Сеанс изменился. Откройте статистику повторно.'),{code:'STALE_SESSION'});
    function notice(text,error=false){container.replaceChildren();const box=container.ownerDocument.createElement('div');box.className='statistics-notice'+(error?' error':'');box.setAttribute('role','status');box.textContent=text;container.append(box);}
    function invalidate(){generation++;if(panel)panel.destroy();panel=null;pending=null;container.replaceChildren();}
    function show(){
      const session=identity();if(!session.ready){invalidate();notice('Войдите в программу для просмотра статистики.');return Promise.resolve();}
      if(pending)return pending;
      if(panel){const task=panel.refresh();pending=task;return Promise.resolve(task).finally(()=>{if(pending===task)pending=null;});}
      const ticket=generation,key=session.key;
      const guard=()=>{if(!fresh(ticket,key))throw stale();};
      const provider=providerFactory(async(command,args)=>{
        guard();let value;
        if(command==='production_list_projects')value=await backend.listProjects();
        else if(command==='production_load_snapshot_by_id')value=await backend.loadById(args.projectId);
        else if(command==='audit_list_events')value=await backend.listAuditEvents(args.projectId);
        else throw Error('Неизвестная команда статистики.');
        guard();return value;
      },{includeAudit:true});
      notice('Загрузка сохранённых проектов…');
      const task=(async()=>{
        try{
          const data=await provider();guard();container.replaceChildren();
          panel=mount(container,{
            embedded:true,data,provider,
            storageKey:'production-cycle-statistics:v1:'+key.replace(/^\d+:/,''),
            onOpen:async target=>{guard();await openProject(target.projectId,target.stageUid);},
            onPrint:async()=>{guard();await options.print();},
            onDownload:async(content,name,mime)=>{guard();const result=await options.download(content,name,mime);guard();return result;}
          });
        }catch(error){if(fresh(ticket,key)&&error.code!=='STALE_SESSION')notice('Не удалось загрузить статистику: '+error.message+'. Повторите открытие вкладки.',true);}
        finally{if(pending===task)pending=null;}
      })();
      pending=task;return task;
    }
    return Object.freeze({show,invalidate});
  }
  return Object.freeze({create,version:'1.0.0'});
});
