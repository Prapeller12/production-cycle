use tauri::State;
use crate::infrastructure::portable::Paths;
pub struct Smoke(pub bool);

#[tauri::command]
pub fn finish_smoke(app:tauri::AppHandle, enabled:State<'_,Smoke>, paths:State<'_,Paths>, error:Option<String>)->Result<(),String>{
    if !enabled.0{return Err("Self-test is not enabled".into());}
    let result=serde_json::json!({"ok":error.is_none(),"error":error,"runtime":paths.runtime,"profile":paths.webview});
    std::fs::write(paths.smoke_report.clone(),serde_json::to_vec_pretty(&result).unwrap()).map_err(|e|e.to_string())?;
    app.exit(if result["ok"]==true{0}else{1});Ok(())
}

pub const SCRIPT:&str=r#"
(async()=>{
 let error=null;
 try{
  for(let i=0;i<100&&!window.Production?.backend;i++)await new Promise(r=>setTimeout(r,100));
  if(!window.__TAURI__?.core?.invoke)throw Error('Native IPC is unavailable');
  const invoke=window.__TAURI__.core.invoke;
  const p={project:{orderNo:'SMOKE-1',name:'Проверка запуска',initiator:'Тест',executor:'Тест',addressees:'Тест',start:'2026-09-01',deadline:'2026-09-30'},stages:[{uid:'smoke-stage',seq:1,sort:1,parentUid:null,title:'Этап проверки',executor:'Тест',addressees:'Тест',start:'2026-09-01',deadline:'2026-09-15',status:'done',comment:'Проверка комментария'}],nextSeq:2,collapsed:{}};
  const validation=await Production.backend.validate(p);
  if(validation.errors.length)throw Error('Backend validation failed');
  const users=await Production.backend.listAuditUsers();
  const signer=users[0]||await Production.backend.createAuditUser({displayName:'Администратор проверки',pin:'739201',isAdmin:true,adminUserId:null,adminPin:null});
  const savedProject=await Production.backend.saveSigned(p,{userId:signer.id,pin:'739201',comment:'Автоматическая проверка подписанного сохранения',evidence:{documentType:'Акт приёмки',documentReference:'SMOKE-ACT-1 от 01.09.2026',comment:'Проверка обязательного подтверждения выполнения',stageUids:['smoke-stage']}});
  p.databaseId=savedProject.projectId;
  const loaded=await Production.backend.load('SMOKE-1');
  if(!loaded||loaded.stages.length!==1||loaded.project.addressees!=='Тест'||loaded.stages[0].comment!=='Проверка комментария')throw Error('SQLite roundtrip failed');
  const report=await Production.backend.report(p);
  if(report.total!==1||report.donePercent!==100||!report.rows[0].completionConfirmation?.signatureValid)throw Error('Backend signed report failed');
  const verification=await Production.backend.verifyAuditLog(signer.id,'739201',p.databaseId);
  if(verification.invalidEvents!==0||verification.checkedEvents<2)throw Error('Audit verification failed');
  const json=Production.projectFile.stringify(p);
  const saved=await invoke('save_file',{name:'Проверка.json',text:json});
  if(!saved.endsWith('Проверка.json'))throw Error('Native save failed');
  const pair=await Production.backend.exportPair(p);
  await invoke('save_file',{name:'Исходящий.txt',text:pair.out});
  await invoke('save_file',{name:'Входящий.txt',text:pair.in});
  if(!document.getElementById('saveJsonBtn'))throw Error('UI did not load');
 }catch(e){error=String(e)}
 try{await window.__TAURI__.core.invoke('finish_smoke',{error})}catch(e){document.body.textContent='SELF-TEST IPC ERROR: '+e}
})();
"#;
