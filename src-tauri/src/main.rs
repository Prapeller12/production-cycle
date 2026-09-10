#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod smoke;
mod infrastructure;
mod production;
use infrastructure::portable::Paths;
use production::ProductionDb;
use tauri::{Manager,WebviewUrl,WebviewWindowBuilder,http::Response};

fn main(){
    let paths=match Paths::load(){Ok(p)=>p,Err(e)=>{eprintln!("{}",e);std::process::exit(1)}};
    let db=match ProductionDb::open(&paths.data.join("production-cycle.db")){Ok(db)=>db,Err(e)=>{eprintln!("{}",e);std::process::exit(1)}};
    let frontend=paths.frontend.clone();
    let smoke=std::env::args().any(|arg|arg=="--smoke-test");
    tauri::Builder::default()
        .manage(paths)
        .manage(db)
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
            production::production_validate,production::production_save_snapshot,production::production_load_snapshot,production::production_load_snapshot_by_id,
            production::production_list_projects,production::production_list_dictionary,production::production_replace_dictionary_value,
            production::production_export_txt,production::production_get_management_report
        ])
        .setup(move |app|{
            let paths=app.state::<Paths>();
            WebviewWindowBuilder::new(app,"main",WebviewUrl::CustomProtocol("production://localhost/index.html".parse()?))
                .title("Производственный цикл — self-test v1")
                .inner_size(1440.0,900.0).min_inner_size(700.0,500.0)
                .data_directory(paths.webview.clone())
                .on_page_load(move |window,payload|{if smoke && payload.event()==tauri::webview::PageLoadEvent::Finished {let _=window.eval(smoke::SCRIPT);}})
                .on_navigation(|url|matches!(url.scheme(),"production") || url.host_str()==Some("production.localhost"))
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!()).expect("Application startup failed");
}
