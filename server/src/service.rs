#[cfg(not(windows))]
pub fn dispatch()->Result<(),String>{Err("Служба доступна только в Windows.".into())}

#[cfg(windows)]
mod windows {
    use std::{ffi::OsString, sync::{Arc, atomic::{AtomicBool,Ordering}}, time::Duration};
    use windows_service::{define_windows_service, service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType}, service_control_handler::{self, ServiceControlHandlerResult}, service_dispatcher};
    define_windows_service!(ffi_service_main, service_main);
    pub fn dispatch()->Result<(),String>{service_dispatcher::start("ProductionCycleServer",ffi_service_main).map_err(|e|e.to_string())}
    fn service_main(_args:Vec<OsString>){let _=run();}
    fn run()->Result<(),String>{
        let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
        let handle=service_control_handler::register("ProductionCycleServer",move|event|match event{
            ServiceControl::Stop|ServiceControl::Shutdown=>{flag.store(true,Ordering::SeqCst);ServiceControlHandlerResult::NoError},
            ServiceControl::Interrogate=>ServiceControlHandlerResult::NoError,_=>ServiceControlHandlerResult::NotImplemented
        }).map_err(|e|e.to_string())?;
        let status=|state,code|ServiceStatus{service_type:ServiceType::OWN_PROCESS,current_state:state,controls_accepted:ServiceControlAccept::STOP|ServiceControlAccept::SHUTDOWN,exit_code:ServiceExitCode::Win32(code),checkpoint:0,wait_hint:Duration::from_secs(15),process_id:None};
        handle.set_service_status(status(ServiceState::StartPending,0)).map_err(|e|e.to_string())?;
        let root=std::env::current_exe().map_err(|e|e.to_string())?.parent().ok_or("Нет папки сервера")?.to_path_buf();
        let result=(||{let config=crate::config::Config::load(&root)?;handle.set_service_status(status(ServiceState::Running,0)).map_err(|e|e.to_string())?;crate::http::run_with_ready(&root,config,stop,"service",None)})();
        handle.set_service_status(status(ServiceState::Stopped,if result.is_ok(){0}else{1})).map_err(|e|e.to_string())?;
        result
    }
}
#[cfg(windows)] pub use windows::dispatch;
