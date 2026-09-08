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
  for(let i=0;i<100&&!window.Production?.projectFile;i++)await new Promise(r=>setTimeout(r,100));
  if(!window.__TAURI__?.core?.invoke)throw Error('Native IPC is unavailable');
  const invoke=window.__TAURI__.core.invoke;
  const p={project:{orderNo:'SMOKE-1',name:'Проверка запуска',initiator:'Тест',executor:'Тест',addressees:'Тест',start:'2026-09-01',deadline:'2026-09-30'},stages:[],nextSeq:1,collapsed:{}};
  const json=Production.projectFile.stringify(p);
  const saved=await invoke('save_file',{name:'Проверка.json',text:json});
  if(!saved.endsWith('Проверка.json'))throw Error('Native save failed');
  const pair=Production.exchange.exportPair(p);
  await invoke('save_file',{name:'Исходящий.txt',text:pair.out});
  await invoke('save_file',{name:'Входящий.txt',text:pair.in});
  if(!document.getElementById('saveJsonBtn'))throw Error('UI did not load');
 }catch(e){error=String(e)}
 try{await window.__TAURI__.core.invoke('finish_smoke',{error})}catch(e){document.body.textContent='SELF-TEST IPC ERROR: '+e}
})();
"#;
