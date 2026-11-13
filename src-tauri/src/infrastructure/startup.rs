//! Native feedback remains usable even if WebView2 or the frontend never starts.
//! Diagnostic text stays in memory unless an explicit probe requests a report.
use std::sync::{atomic::{AtomicBool, Ordering}, Mutex, OnceLock};
use std::time::{Duration, Instant};

static COMPLETE: AtomicBool = AtomicBool::new(false);
static CANCELLED: AtomicBool = AtomicBool::new(false);
static STATUS: OnceLock<Mutex<Status>> = OnceLock::new();
struct Status {stage:String,started:Instant,phase_started:Instant,details:Vec<(String,String)>}
impl Default for Status {
    fn default()->Self {Self{stage:"Запуск программы…".into(),started:Instant::now(),phase_started:Instant::now(),details:Vec::new()}}
}
fn status()->&'static Mutex<Status>{STATUS.get_or_init(||Mutex::new(Status::default()))}
fn publish(){super::launch::signal_status(serde_json::json!({"state":"starting","processId":std::process::id(),"stage":current_message(),"diagnostics":diagnostics()}));}
pub fn update(message:&str){
    if let Ok(mut state)=status().lock(){if state.stage!=message{state.phase_started=Instant::now();}state.stage=message.to_string();}
    publish();
}
pub fn detail(label:&str,value:&str){
    if let Ok(mut state)=status().lock(){if let Some(entry)=state.details.iter_mut().find(|entry|entry.0==label){entry.1=value.to_string();}else{state.details.push((label.to_string(),value.to_string()));}}
    publish();
}
pub fn is_ready()->bool{COMPLETE.load(Ordering::Acquire)}
pub fn cancelled()->bool{CANCELLED.load(Ordering::Acquire)}
pub fn current_message()->String{status().lock().map(|state|state.stage.clone()).unwrap_or_else(|_|"Запуск программы".into())}
pub fn diagnostics()->String{
    status().lock().map(|state|{
        let mut text=format!("Этап: {}\nПрошло: {} сек. · текущий этап: {} сек.",state.stage,state.started.elapsed().as_secs(),state.phase_started.elapsed().as_secs());
        for(label,value)in &state.details{text.push_str(&format!("\n\n{label}:\n{value}"));}text
    }).unwrap_or_else(|_|"Не удалось получить сведения о запуске.".into())
}
pub fn stop(){COMPLETE.store(true,Ordering::Release);}

#[tauri::command]
pub fn frontend_ready(window:tauri::WebviewWindow)->Result<(),String>{
    if window.label()!="main"{return Err("Неверное окно интерфейса.".into());}
    window.show().map_err(|e|e.to_string())?;
    if !window.is_visible().map_err(|e|e.to_string())?{return Err("Основное окно не открыто.".into());}
    super::launch::signal_status(serde_json::json!({"state":"ready","processId":std::process::id(),"windowVisible":true,"frontendReady":true,"executable":std::env::current_exe().ok(),"diagnostics":diagnostics()}));
    stop();Ok(())
}
#[tauri::command]
pub fn frontend_failed(window:tauri::WebviewWindow,message:String,resource:Option<String>)->Result<(),String>{
    if window.label()!="main"{return Err("Неверное окно интерфейса.".into());}
    if is_ready(){return Ok(());}
    if let Some(resource)=resource{detail("Ресурс с ошибкой",&resource);}
    crate::startup_failure(&format!("Интерфейс не смог запуститься: {message}"))
}
pub const FRONTEND_GUARD:&str=r#"
(()=>{
 const report=(message,resource)=>{const invoke=window.__TAURI__?.core?.invoke;if(invoke)void invoke('frontend_failed',{message:String(message||'Ошибка загрузки интерфейса'),resource:resource?String(resource):null}).catch(()=>{});};
 window.addEventListener('error',e=>{
  if(e.target&&e.target!==window&&/^(SCRIPT|LINK)$/.test(e.target.tagName||'')){report('Не удалось загрузить обязательный файл',e.target.src||e.target.href);return;}
  if(e.message)report(e.message,e.filename?e.filename+':'+e.lineno+':'+e.colno:null);
 },true);
 window.addEventListener('unhandledrejection',e=>report(e.reason?.message||String(e.reason),null));
})();
"#;
fn window_phase(stage:&str)->bool{["Создание рабочего окна","Загрузка интерфейса","Ожидание готовности"].iter().any(|prefix|stage.starts_with(prefix))}
fn timeout(state:&Status)->bool{
    // File copying has a separate overall budget; a blank WebView has 45 seconds.
    let phase_budget=if std::env::args().any(|arg|arg=="--smoke-test"){
        std::env::var("PRODUCTION_CYCLE_TEST_STARTUP_TIMEOUT_MS").ok().and_then(|s|s.parse::<u64>().ok()).map(|ms|Duration::from_millis(ms.clamp(100,90_000))).unwrap_or(Duration::from_secs(45))
    }else{Duration::from_secs(45)};
    state.started.elapsed()>Duration::from_secs(300)||(window_phase(&state.stage)&&state.phase_started.elapsed()>phase_budget)
}
#[cfg(windows)]fn wide(text:&str)->Vec<u16>{text.encode_utf16().chain(Some(0)).collect()}
#[cfg(windows)]
mod native{
    use super::wide;
    use windows_sys::Win32::{Foundation::*,Graphics::Gdi::*,System::LibraryLoader::GetModuleHandleW,UI::WindowsAndMessaging::*};
    unsafe extern "system" fn procedure(hwnd:HWND,message:u32,wparam:WPARAM,lparam:LPARAM)->LRESULT{
        if message==WM_CLOSE||(message==WM_COMMAND&&wparam&0xffff==1){DestroyWindow(hwnd);return 0;}
        if message==WM_SIZE{
            let mut rect:RECT=std::mem::zeroed();GetClientRect(hwnd,&mut rect);
            let edit=GetDlgItem(hwnd,1001);if !edit.is_null(){MoveWindow(edit,12,12,(rect.right-24).max(100),(rect.bottom-68).max(60),1);}
            let button=GetDlgItem(hwnd,1);if !button.is_null(){MoveWindow(button,(rect.right-124).max(12),(rect.bottom-44).max(12),112,30,1);}return 0;
        }
        DefWindowProcW(hwnd,message,wparam,lparam)
    }
    pub unsafe fn create(title:&str,body:&str,button_text:&str)->(HWND,HWND){
        let instance=GetModuleHandleW(std::ptr::null());let class_name=wide("ProductionCycleStartupDiagnostic");
        let class=WNDCLASSW{lpfnWndProc:Some(procedure),hInstance:instance,lpszClassName:class_name.as_ptr(),hbrBackground:(COLOR_BTNFACE+1)as HBRUSH,..std::mem::zeroed()};RegisterClassW(&class);
        let width=780.min(GetSystemMetrics(SM_CXSCREEN)-40).max(500);let height=520.min(GetSystemMetrics(SM_CYSCREEN)-70).max(300);
        let hwnd=CreateWindowExW(WS_EX_TOPMOST,class_name.as_ptr(),wide(title).as_ptr(),WS_OVERLAPPEDWINDOW|WS_VISIBLE,(GetSystemMetrics(SM_CXSCREEN)-width)/2,(GetSystemMetrics(SM_CYSCREEN)-height)/2,width,height,std::ptr::null_mut(),std::ptr::null_mut(),instance,std::ptr::null());
        if hwnd.is_null(){return(hwnd,std::ptr::null_mut());}
        let edit=CreateWindowExW(WS_EX_CLIENTEDGE,wide("EDIT").as_ptr(),wide(body).as_ptr(),WS_CHILD|WS_VISIBLE|WS_VSCROLL|ES_MULTILINE as u32|ES_READONLY as u32|ES_AUTOVSCROLL as u32,12,12,width-42,height-108,hwnd,1001usize as HMENU,instance,std::ptr::null());
        let button=CreateWindowExW(0,wide("BUTTON").as_ptr(),wide(button_text).as_ptr(),WS_CHILD|WS_VISIBLE|WS_TABSTOP|BS_DEFPUSHBUTTON as u32,width-142,height-84,112,30,hwnd,1usize as HMENU,instance,std::ptr::null());
        let font=GetStockObject(DEFAULT_GUI_FONT)as usize;SendMessageW(edit,WM_SETFONT,font,1);SendMessageW(button,WM_SETFONT,font,1);SetForegroundWindow(hwnd);(hwnd,edit)
    }
    pub unsafe fn pump(){let mut msg=std::mem::zeroed();while PeekMessageW(&mut msg,std::ptr::null_mut(),0,0,PM_REMOVE)!=0{TranslateMessage(&msg);DispatchMessageW(&msg);}}
}
#[cfg(windows)]
pub fn show_error(message:&str){
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let body=format!("Программа не смогла открыться.\r\n\r\n{}\r\n\r\nМожно выделить и скопировать текст этого окна. Передайте сведения администратору.\r\nДля сетевого ярлыка требуются полный комплект программы, доступ к общей папке и доступная локальная папка клиента.",message.replace('\n',"\r\n"));
    unsafe{
        let(hwnd,_)=native::create("Производственный цикл — ошибка запуска",&body,"Закрыть");
        if hwnd.is_null(){MessageBoxW(std::ptr::null_mut(),wide(&body).as_ptr(),wide("Производственный цикл — ошибка запуска").as_ptr(),MB_OK|MB_ICONERROR|MB_SETFOREGROUND);return;}
        while IsWindow(hwnd)!=0{native::pump();std::thread::sleep(Duration::from_millis(50));}
    }
}
#[cfg(not(windows))]pub fn show_error(message:&str){eprintln!("{message}");}

pub fn start(){
    update("Запуск программы…");
    if let Ok(exe)=std::env::current_exe(){detail("EXE",&exe.display().to_string());}
    if let Ok(cwd)=std::env::current_dir(){detail("Рабочая папка ярлыка",&cwd.display().to_string());}
    // A staged child's launcher owns the visible native diagnostic window.
    if std::env::var_os(super::launch::STATUS_ENV).is_some(){return;}
    #[cfg(windows)]std::thread::spawn(||{
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        unsafe{
            let(hwnd,edit)=native::create("Производственный цикл — запуск",&diagnostics(),"Отмена");
            if hwnd.is_null(){crate::startup_failure(&format!("Не удалось показать окно запуска: {}",std::io::Error::last_os_error()));}
            let mut previous=String::new();
            loop{
                if COMPLETE.load(Ordering::Acquire){DestroyWindow(hwnd);break;}
                native::pump();if IsWindow(hwnd)==0{CANCELLED.store(true,Ordering::Release);break;}
                let text=diagnostics().replace('\n',"\r\n");if text!=previous{SetWindowTextW(edit,wide(&text).as_ptr());previous=text;}
                if status().lock().map(|state|timeout(&state)).unwrap_or(false){crate::startup_failure("Превышено время ожидания. Текущий этап не завершился; готовность интерфейса не подтверждена. Причина зависания WebView2 не установлена: проверьте права доступа и разрешение запуска указанных файлов в защите Windows.");}
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn window_wait_is_bounded_without_timing_out_each_copied_file(){
        assert!(window_phase("Создание рабочего окна WebView2…"));assert!(window_phase("Ожидание готовности интерфейса…"));assert!(!window_phase("Подготовка локального клиента: файл 5 из 50"));
        let state=Status{phase_started:Instant::now()-Duration::from_secs(46),stage:"Создание рабочего окна WebView2…".into(),..Status::default()};assert!(timeout(&state));
    }
    #[test]fn diagnostics_keep_actual_paths_and_stage_without_creating_files(){
        detail("Проверяемый путь",r"\\server\общая папка\программа");let text=diagnostics();assert!(text.contains("Этап:"));assert!(text.contains(r"\\server\общая папка\программа"));
    }
}
