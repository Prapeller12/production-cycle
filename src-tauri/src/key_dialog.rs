//! Native password input. Secrets never enter a subprocess, environment variable or command line.
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::EM_LIMITTEXT, WindowsAndMessaging::*},
};
use zeroize::Zeroizing;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe extern "system" fn procedure(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // Read fields on the calling thread before destroying them.
    if m == WM_COMMAND && (w & 0xffff == 1 || w & 0xffff == 2) {
        PostMessageW(h, WM_APP + (w & 0xffff) as u32, 0, 0);
        return 0;
    }
    if m == WM_CLOSE {
        PostMessageW(h, WM_APP + 2, 0, 0);
        return 0;
    }
    DefWindowProcW(h, m, w, l)
}
unsafe fn add(
    h: HWND,
    class: &str,
    label: &str,
    id: usize,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    height: i32,
) -> Result<HWND, String> {
    let edit = class == "EDIT";
    let child = CreateWindowExW(
        if edit { WS_EX_CLIENTEDGE } else { 0 },
        wide(class).as_ptr(),
        wide(label).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        height,
        h,
        id as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    if child.is_null() {
        return Err(format!(
            "Windows не создала поле окна: {}",
            std::io::Error::last_os_error()
        ));
    }
    SendMessageW(
        child,
        WM_SETFONT,
        GetStockObject(DEFAULT_GUI_FONT) as usize,
        1,
    );
    if edit {
        SendMessageW(child, EM_LIMITTEXT, 1024, 0);
    }
    Ok(child)
}
unsafe fn value(h: HWND) -> Result<Zeroizing<String>, String> {
    let mut buf = Zeroizing::new(vec![0u16; GetWindowTextLengthW(h) as usize + 1]);
    let len = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
    Ok(Zeroizing::new(
        String::from_utf16(&buf[..len as usize])
            .map_err(|_| "Некорректный символ в парольной фразе.")?,
    ))
}
pub fn prompt(create: bool) -> Result<Zeroizing<String>, String> {
    unsafe {
        let class = wide("ProductionCycleVaultUnlock");
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(format!(
                "Не удалось подготовить окно ключа: {}",
                std::io::Error::last_os_error()
            ));
        }
        let w = 545;
        let ht = if create { 295 } else { 235 };
        let h = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide("Производственный цикл — ключ базы").as_ptr(),
            WS_CAPTION | WS_SYSMENU,
            (GetSystemMetrics(SM_CXSCREEN) - w) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - ht) / 2,
            w,
            ht,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if h.is_null() {
            return Err(format!(
                "Не удалось открыть окно ключа: {}",
                std::io::Error::last_os_error()
            ));
        }
        let result = (|| {
            let intro = if create {
                "Задайте фразу для шифрования базы и запишите её в защищённом месте. Сотрудникам общей базы потребуется эта же фраза."
            } else {
                "Введите парольную фразу этого хранилища. После открытия базы вы сможете войти под своим именем и ПИН."
            };
            add(h, "STATIC", intro, 100, 0, 16, 12, 501, 42)?;
            add(
                h,
                "STATIC",
                "Парольная фраза базы (от 12 символов)",
                101,
                0,
                16,
                57,
                501,
                18,
            )?;
            let style = ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32 | WS_TABSTOP;
            let first = add(h, "EDIT", "", 1101, style, 16, 79, 501, 26)?;
            let second = if create {
                add(
                    h,
                    "STATIC",
                    "Повторите новую фразу",
                    102,
                    0,
                    16,
                    113,
                    501,
                    18,
                )?;
                Some(add(h, "EDIT", "", 1102, style, 16, 135, 501, 26)?)
            } else {
                None
            };
            let bottom = if create { 164 } else { 108 };
            let hint = add(h, "STATIC", "", 103, 0, 16, bottom, 501, 32)?;
            add(
                h,
                "BUTTON",
                "Открыть",
                1,
                BS_DEFPUSHBUTTON as u32 | WS_TABSTOP,
                397,
                bottom + 36,
                120,
                32,
            )?;
            add(
                h,
                "BUTTON",
                "Отмена",
                2,
                WS_TABSTOP,
                272,
                bottom + 36,
                112,
                32,
            )?;
            ShowWindow(h, SW_SHOW);
            SetForegroundWindow(h);
            windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(first);
            loop {
                let mut msg: MSG = std::mem::zeroed();
                if GetMessageW(&mut msg, null_mut(), 0, 0) <= 0
                    || (msg.hwnd == h && msg.message == WM_APP + 2)
                {
                    return Err("Открытие хранилища отменено.".into());
                }
                if (msg.hwnd == h && msg.message == WM_APP + 1)
                    || (msg.message == WM_KEYDOWN
                        && msg.wParam == 13
                        && (msg.hwnd == first || Some(msg.hwnd) == second))
                {
                    let phrase = value(first)?;
                    let checked = crate::vault::validate_phrase(&phrase).and_then(|()| {
                        if let Some(field) = second {
                            if value(field)?.as_str() != phrase.as_str() {
                                return Err("Введённые фразы различаются. Повторите ввод.".into());
                            }
                        }
                        Ok(())
                    });
                    match checked {
                        Ok(()) => return Ok(phrase),
                        Err(reason) => {
                            SetWindowTextW(hint, wide(&reason).as_ptr());
                        }
                    }
                } else if IsDialogMessageW(h, &msg) == 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        })();
        DestroyWindow(h);
        result
    }
}
