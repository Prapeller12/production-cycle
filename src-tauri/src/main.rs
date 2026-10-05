#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod audit;
mod backup;
mod smoke;
mod infrastructure;
mod production;
use infrastructure::portable::Paths;
use infrastructure::network::NetworkRuntime;
use production::ProductionDb;
use tauri::{Manager,WebviewUrl,WebviewWindowBuilder,http::Response};

fn startup_failure(message:&str)->!{
    if let Some(path)=std::env::var_os("PRODUCTION_CYCLE_SMOKE_REPORT").map(std::path::PathBuf::from){
        if !path.is_file(){
            if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
            let body=serde_json::json!({"ok":false,"error":message,"phase":"startup"});
            let _=std::fs::write(path,serde_json::to_vec_pretty(&body).unwrap_or_default());
        }
    }
    eprintln!("{message}");std::process::exit(1)
}

fn main(){
    match infrastructure::network::maybe_relaunch_network_client(){Ok(true)=>return,Ok(false)=>{},Err(e)=>startup_failure(&e)}
    let paths=match Paths::load(){Ok(p)=>p,Err(e)=>startup_failure(&e)};
    let network=match NetworkRuntime::from_environment(paths.data.clone()){Ok(value)=>value,Err(e)=>startup_failure(&e)};
    let local_db=paths.data.join("production-cycle.db");
    if let Err(e)=network.prepare_local_database(&local_db){startup_failure(&e)}
    let db=match ProductionDb::open_with_backup_dir(&local_db,network.shared_backups()){Ok(db)=>db,Err(e)=>startup_failure(&e.to_string())};
    if let Err(e)=network.ensure_shared_database(&db){startup_failure(&e)}
    let network_enabled=network.enabled();
    let frontend=paths.frontend.clone();
    let smoke=std::env::args().any(|arg|arg=="--smoke-test");
    tauri::Builder::default()
        .manage(paths)
        .manage(db)
        .manage(network)
        .manage(smoke::Smoke(smoke))
        .register_uri_scheme_protocol("production",move |_context,request|{
            let relative=request.uri().path().trim_start_matches('/');
            let relative=if relative.is_empty(){"index.html"}else{relative};
            let path=frontend.join(relative);
            let body=path.canonicalize().ok().filter(|p|p.starts_with(&frontend)&&p.is_file()).and_then(|p|std::fs::read(p).ok());
            let mime=match path.extension().and_then(|s|s.to_str()){Some("js")=>"text/javascript; charset=utf-8",Some("css")=>"text/css; charset=utf-8",Some("html")=>"text/html; charset=utf-8",_=>"application/octet-stream"};
            Response::builder().status(if body.is_some(){200}else{404}).header("Content-Type",mime).header("Cache-Control","no-store").header("Content-Security-Policy", "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src ipc: http://ipc.localhost; object-src 'none'; frame-src 'none'").body(body.unwrap_or_default()).unwrap()
        })
        .invoke_handler(tauri::generate_handler![
            commands::save_file,commands::print_report,smoke::finish_smoke,
            production::production_validate,production::production_load_snapshot,production::production_load_snapshot_by_id,
            production::production_list_projects,production::production_list_deadline_control,production::production_list_dictionary,
            production::production_export_txt,production::production_get_management_report,
            audit::audit_list_users,audit::audit_authorize_admin,audit::audit_create_user,audit::audit_save_snapshot,
            audit::audit_list_events,audit::audit_verify_log,audit::audit_replace_dictionary_value,
            backup::backup_list,backup::backup_create,backup::backup_restore,
            infrastructure::network::network_status,infrastructure::network::network_login,
            infrastructure::network::network_save_draft,infrastructure::network::network_finish_eviction
        ])
        .setup(move |app|{
            let paths=app.state::<Paths>();
            WebviewWindowBuilder::new(app,"main",WebviewUrl::CustomProtocol("production://localhost/index.html".parse()?))
                .title("Производственный цикл — self-test v1")
                .inner_size(1440.0,900.0).min_inner_size(700.0,500.0)
                .data_directory(paths.webview.clone())
                .on_page_load(move |window,payload|{if smoke && payload.event()==tauri::webview::PageLoadEvent::Finished {let _=window.eval(if network_enabled{smoke::NETWORK_SCRIPT}else{smoke::SCRIPT});}})
                .on_navigation(|url|matches!(url.scheme(),"production") || url.host_str()==Some("production.localhost"))
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!()).expect("Application startup failed");
}
