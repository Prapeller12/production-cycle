//! Native startup feedback works before WebView2 exists; no technical log files.
use std::sync::{atomic::{AtomicBool, Ordering}, Mutex, OnceLock};
use std::time::{Duration, Instant};

static COMPLETE: AtomicBool = AtomicBool::new(false);
static MESSAGE: OnceLock<Mutex<String>> = OnceLock::new();

pub fn update(message: &str) {
    if let Ok(mut text) = MESSAGE.get_or_init(|| Mutex::new(String::new())).lock() {
        *text = message.to_string();
    }
}

pub fn is_ready() -> bool { COMPLETE.load(Ordering::Acquire) }

pub fn stop() { COMPLETE.store(true, Ordering::Release); }

#[tauri::command]
pub fn frontend_ready(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" || !window.is_visible().map_err(|e|e.to_string())? {
        return Err("Основное окно не открыто.".into());
    }
    stop();
    Ok(())
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }

#[cfg(windows)]
pub fn show_error(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let body = wide(&format!("Программа не смогла открыться.\n\n{message}\n\nЗапускайте EXE на своём компьютере из Проводника Windows в обычном сеансе пользователя. Общая папка должна быть доступна для чтения и записи.\nЕсли ошибка повторится, передайте администратору снимок этого окна."));
    unsafe { MessageBoxW(std::ptr::null_mut(), body.as_ptr(), wide("Производственный цикл — ошибка запуска").as_ptr(), MB_OK | MB_ICONERROR | MB_SETFOREGROUND); }
}
#[cfg(not(windows))]
pub fn show_error(message: &str) { eprintln!("{message}"); }

pub fn start() {
    update("Запуск программы…");
    #[cfg(windows)]
    std::thread::spawn(|| {
        use windows_sys::Win32::{UI::WindowsAndMessaging::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW};
        let deadline = if std::env::var_os("PRODUCTION_CYCLE_NETWORK_CLIENT").is_some(){90}else{300};
        let started=Instant::now();
        unsafe {
            let instance=GetModuleHandleW(std::ptr::null());
            let class=wide("STATIC");
            let hwnd=CreateWindowExW(WS_EX_TOPMOST, class.as_ptr(), wide("Производственный цикл — запуск").as_ptr(), WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE, (GetSystemMetrics(SM_CXSCREEN)-560)/2, (GetSystemMetrics(SM_CYSCREEN)-170)/2, 560, 170, std::ptr::null_mut(), std::ptr::null_mut(), instance, std::ptr::null());
            if hwnd.is_null(){crate::startup_failure(&format!("Не удалось показать окно запуска: {}",std::io::Error::last_os_error()));}
            let label=CreateWindowExW(0,class.as_ptr(),wide("Подготовка…").as_ptr(),WS_CHILD | WS_VISIBLE,15,20,520,80,hwnd,std::ptr::null_mut(),instance,std::ptr::null());
            SendMessageW(label,WM_SETFONT,GetStockObject(DEFAULT_GUI_FONT) as usize,1);
            let mut msg=std::mem::zeroed();
            loop {
                if COMPLETE.load(Ordering::Acquire){DestroyWindow(hwnd);break;}
                while PeekMessageW(&mut msg,std::ptr::null_mut(),0,0,PM_REMOVE)!=0 {TranslateMessage(&msg);DispatchMessageW(&msg);}
                if IsWindow(hwnd)==0 {std::process::exit(0);}
                if let Ok(text)=MESSAGE.get().unwrap().lock(){SetWindowTextW(label,wide(text.as_str()).as_ptr());}
                if started.elapsed()>Duration::from_secs(deadline){crate::startup_failure("Превышено время ожидания запуска. Проверьте доступность общей папки, свободное место на локальном диске и разрешение запуска программы в настройках защиты. Окно интерфейса не подтвердило готовность.");}
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
}
