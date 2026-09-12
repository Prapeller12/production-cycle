#include "frame.hpp"
#include <filesystem>
#include <fstream>

using namespace frame;
namespace {
HWND host{}, buttons[2]{}, status{}, retry{};
HFONT font{};
std::array<Module, 2> modules;
int selected = 0, closing = -1, testStep = 0, exitCode = 0;
bool fixtureTest = false, realTest = false, closeSent = false;
ULONGLONG closeStarted{}, testStarted{}, stepStarted{};
std::wstring notice;
const wchar_t* kClass = L"ProductionFrame.Main.v1";

void showError(const std::exception& e) {
    auto s = std::string(e.what());
    std::wstring message(s.begin(), s.end());
    MessageBoxW(host, (L"Не удалось выполнить действие.\n\n" + message +
        L"\n\nПроверьте полную распаковку ZIP в локальную папку.").c_str(),
        L"Производственный контур", MB_OK | MB_ICONERROR);
}
int scale(int value) { return MulDiv(value, static_cast<int>(GetDpiForWindow(host)), 96); }
void layout() {
    if (!host) return;
    RECT rect{}; GetClientRect(host, &rect);
    const int top = scale(58), bottom = scale(34), gap = scale(10);
    int contentHeight = std::max(1, static_cast<int>(rect.bottom) - top - bottom);
    MoveWindow(buttons[0], gap, gap, scale(255), scale(38), TRUE);
    MoveWindow(buttons[1], scale(275), gap, scale(265), scale(38), TRUE);
    MoveWindow(retry, std::max(scale(550), static_cast<int>(rect.right) - scale(200)),
        gap, scale(185), scale(38), TRUE);
    MoveWindow(status, gap, rect.bottom - bottom + scale(6), rect.right - 2 * gap, scale(24), TRUE);
    for (int i = 0; i < 2; ++i) {
        auto& m = modules[i];
        if (!m.slot) continue;
        MoveWindow(m.slot, 0, top, rect.right, contentHeight, TRUE);
        RECT inner{}; GetClientRect(m.slot, &inner);
        SetWindowPos(m.window, nullptr, 0, 0, inner.right, inner.bottom,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
        ShowWindow(m.slot, i == selected ? SW_SHOW : SW_HIDE);
        ShowWindowAsync(m.window, SW_SHOW);
    }
    InvalidateRect(host, nullptr, TRUE);
}
void selectModule(int index) {
    // Do not hide a disabled main window underneath its native modal dialog.
    auto& old = modules[selected];
    if (index != selected && IsWindow(old.window) && !IsWindowEnabled(old.window)) {
        auto popup = GetLastActivePopup(old.window);
        if (popup != old.window) SetForegroundWindow(popup);
        return;
    }
    selected = index;
    try {
        auto& m = modules[selected];
        if (!m.alive()) { m.launch(fixtureTest, selected); notice = L"Запуск: " + m.name; }
        else notice = m.name;
    } catch (const std::exception& e) {
        modules[selected].failed = true; notice = L"Не удалось запустить раздел";
        if (!fixtureTest && !realTest) showError(e); else exitCode = 1;
    }
    SetWindowTextW(status, notice.c_str());
    layout();
    if (IsWindow(modules[selected].window)) {
        // WM_MOUSEACTIVATE and real mouse/keyboard focus continue to be handled by the module.
        PostMessageW(modules[selected].window, WM_ACTIVATE, WA_ACTIVE, 0);
    }
}
void writeResult(bool ok) {
    CreateDirectoryW((root() + L"\\temp").c_str(), nullptr);
    std::ofstream f(std::filesystem::path(root()) / L"temp" / L"frame-test.json");
    f << "{\"ok\":" << (ok ? "true" : "false") << ",\"mode\":\""
      << (fixtureTest ? "fixtures" : "real-windows") << "\",\"step\":" << testStep << "}";
}
void beginClose(bool ask) {
    if (closing >= 0) return;
    if (ask && MessageBoxW(host,
        L"Сохраните изменения штатными кнопками в обоих разделах.\n\n"
        L"Закрыть обе программы и общее окно?",
        L"Завершение работы", MB_YESNO | MB_DEFBUTTON2 | MB_ICONQUESTION) != IDYES) return;
    closing = 0; closeSent = false;
}
void closeTick() {
    if (closing < 0) return;
    while (closing < 2 && !modules[closing].alive()) { modules[closing].release(); ++closing; closeSent = false; }
    if (closing == 2) { DestroyWindow(host); return; }
    auto& m = modules[closing];
    if (!m.window) m.discover();
    if (!closeSent && IsWindow(m.window)) {
        selected = closing; layout();
        ShowWindowAsync(m.window, SW_SHOW);
        PostMessageW(m.window, WM_CLOSE, 0, 0);
        closeStarted = GetTickCount64(); closeSent = true;
        notice = L"Ожидание закрытия: " + m.name;
        SetWindowTextW(status, notice.c_str());
    }
    if ((!closeSent && GetTickCount64() - m.started > 120000) ||
        (closeSent && GetTickCount64() - closeStarted > 15000)) {
        closing = -1; closeSent = false;
        notice = L"Раздел остаётся открытым. Завершите диалог, сохраните работу и повторите закрытие.";
        SetWindowTextW(status, notice.c_str());
        // Never kill a process that may contain unsaved data.
        if (fixtureTest || realTest) { exitCode = 1; writeResult(false); }
    }
}
void testTick() {
    if (!(fixtureTest || realTest) || closing >= 0) return;
    if (exitCode || GetTickCount64() - testStarted > 120000) {
        exitCode = 1; writeResult(false); beginClose(false); return;
    }
    auto& current = modules[selected];
    if (!current.slot || GetParent(current.window) != current.slot) return;
    RECT content{}, child{}; GetClientRect(current.slot, &content); GetClientRect(current.window, &child);
    if (abs(content.right - child.right) > 4 || abs(content.bottom - child.bottom) > 4) return;
    // Let the actual WebViews render before capturing, without blocking the event loop.
    if (!stepStarted) { stepStarted = GetTickCount64(); return; }
    if (GetTickCount64() - stepStarted < (realTest ? 10000 : 500)) return;
    switch (testStep) {
    case 0:
        evidence(host, L"frame-reporting.bmp"); ++testStep; stepStarted = 0; selectModule(1); break;
    case 1:
        if (IsWindowVisible(modules[0].window)) { exitCode = 1; break; }
        evidence(host, L"frame-cycle.bmp");
        SetWindowPos(host, nullptr, 20, 20, 1200, 820, SWP_NOZORDER);
        ++testStep; stepStarted = 0; break;
    case 2:
        ++testStep; stepStarted = 0; selectModule(0); break;
    case 3:
        if (!modules[0].alive() || !modules[1].alive()) { exitCode = 1; break; }
        ++testStep; writeResult(true); beginClose(false); break;
    default: break;
    }
}
LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM w, LPARAM l) {
    switch (message) {
    case WM_CREATE:
        host = window;
        font = CreateFontW(-scale(16), 0, 0, 0, FW_SEMIBOLD, FALSE, FALSE, FALSE,
            DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
            DEFAULT_PITCH, L"Segoe UI");
        for (int i = 0; i < 2; ++i)
            buttons[i] = CreateWindowW(L"BUTTON", modules[i].name.c_str(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON, 0, 0, 1, 1,
                window, reinterpret_cast<HMENU>(static_cast<INT_PTR>(101 + i)), nullptr, nullptr);
        retry = CreateWindowW(L"BUTTON", L"Открыть раздел", WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            0, 0, 1, 1, window, reinterpret_cast<HMENU>(103), nullptr, nullptr);
        status = CreateWindowW(L"STATIC", L"Готов к запуску", WS_CHILD | WS_VISIBLE,
            0, 0, 1, 1, window, nullptr, nullptr, nullptr);
        for (HWND c : {buttons[0], buttons[1], retry, status}) SendMessageW(c, WM_SETFONT, reinterpret_cast<WPARAM>(font), TRUE);
        SetTimer(window, 1, 100, nullptr); return 0;
    case WM_COMMAND:
        if (closing < 0 && HIWORD(w) == BN_CLICKED) {
            if (LOWORD(w) == 101) selectModule(0);
            if (LOWORD(w) == 102) selectModule(1);
            if (LOWORD(w) == 103) selectModule(selected);
        }
        return 0;
    case WM_TIMER:
        for (int i = 0; i < 2; ++i) {
            auto& m = modules[i];
            if (m.process.hProcess && !m.alive()) {
                DWORD code{}; GetExitCodeProcess(m.process.hProcess, &code); m.release();
                if (closing < 0 && i == selected) {
                    notice = L"Раздел закрыт. Нажмите «Открыть раздел» для запуска.";
                    if (code) notice = L"Раздел завершился с ошибкой " + std::to_wstring(code) + L". Возможен повторный запуск.";
                    SetWindowTextW(status, notice.c_str());
                }
                if ((fixtureTest || realTest) && closing < 0) exitCode = 1;
            }
            if (m.alive() && !m.slot && !m.failed) {
                if (m.discover()) {
                    try { m.attach(window); layout(); if (i == selected) SetWindowTextW(status, m.name.c_str()); }
                    catch (const std::exception& e) {
                        m.detach(); m.failed = true;
                        if (!fixtureTest && !realTest) showError(e); else exitCode = 1;
                        SetWindowTextW(status, L"Встраивание не выполнено. Сохраните работу в родном окне приложения.");
                    }
                } else if (GetTickCount64() - m.started > 120000) {
                    m.failed = true;
                    SetWindowTextW(status, L"Окно не появилось. Проверьте сообщение запуска приложения.");
                }
            }
        }
        closeTick(); if (IsWindow(window)) testTick(); return 0;
    case WM_SIZE: layout(); return 0;
    case WM_DPICHANGED: {
        const RECT* r = reinterpret_cast<const RECT*>(l);
        SetWindowPos(window, nullptr, r->left, r->top, r->right - r->left, r->bottom - r->top, SWP_NOZORDER);
        layout(); return 0;
    }
    case WM_GETMINMAXINFO: {
        auto* m = reinterpret_cast<MINMAXINFO*>(l);
        m->ptMinTrackSize.x = 1100; m->ptMinTrackSize.y = 800; return 0;
    }
    case WM_CLOSE: beginClose(!(fixtureTest || realTest)); return 0;
    case WM_DESTROY:
        KillTimer(window, 1);
        for (auto& m : modules) m.release();
        if (font) DeleteObject(font);
        PostQuitMessage(exitCode); return 0;
    }
    return DefWindowProcW(window, message, w, l);
}
LRESULT CALLBACK fixtureProcedure(HWND w, UINT m, WPARAM a, LPARAM b) {
    if (m == WM_DESTROY) { PostQuitMessage(0); return 0; }
    return DefWindowProcW(w, m, a, b);
}
int fixtureMain(int index) {
    SetProcessDpiAwarenessContext(index ? DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 : DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
    WNDCLASSW c{}; c.lpfnWndProc = fixtureProcedure; c.hInstance = GetModuleHandleW(nullptr);
    c.lpszClassName = L"ProductionFrame.Fixture"; c.hbrBackground = reinterpret_cast<HBRUSH>(COLOR_WINDOW + 1);
    RegisterClassW(&c);
    auto title = index ? L"Fixture Cycle" : L"Fixture Reporting";
    HWND w = CreateWindowW(c.lpszClassName, title, WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        0, 0, 900, 650, nullptr, nullptr, c.hInstance, nullptr);
    CreateWindowW(L"EDIT", L"Editable fixture — сохранение фокуса", WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP,
        30, 30, 600, 50, w, nullptr, c.hInstance, nullptr);
    MSG msg{}; while (GetMessageW(&msg, nullptr, 0, 0) > 0) { TranslateMessage(&msg); DispatchMessageW(&msg); }
    return 0;
}
}
int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR, int show) {
    int argc{}; auto args = CommandLineToArgvW(GetCommandLineW(), &argc);
    std::vector<std::wstring> arguments;
    for (int i = 1; i < argc; ++i) arguments.emplace_back(args[i]);
    LocalFree(args);
    if (arguments.size() == 2 && arguments[0] == L"--fixture") return fixtureMain(arguments[1] == L"1");
    fixtureTest = arguments.size() == 1 && arguments[0] == L"--self-test";
    realTest = arguments.size() == 1 && arguments[0] == L"--verify-windows";
    HANDLE mutex{};
    try {
        if (!fixtureTest) checkLayout();
        // Same frame folder has one host. Different complete portable copies are independent.
        auto name = root(); CharLowerBuffW(name.data(), static_cast<DWORD>(name.size()));
        unsigned long long hash = 14695981039346656037ULL;
        for (wchar_t c : name) { hash ^= static_cast<unsigned long long>(c); hash *= 1099511628211ULL; }
        auto mutexName = L"Local\\ProductionFrame-" + std::to_wstring(hash);
        mutex = CreateMutexW(nullptr, FALSE, mutexName.c_str());
        if (!mutex) throw std::runtime_error("Cannot create frame instance mutex");
        if (GetLastError() == ERROR_ALREADY_EXISTS) {
            MessageBoxW(nullptr, L"Общее окно из этой папки уже запущено.", L"Производственный контур", MB_OK);
            CloseHandle(mutex); return 2;
        }
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        if (SetThreadDpiHostingBehavior(DPI_HOSTING_BEHAVIOR_MIXED) == DPI_HOSTING_BEHAVIOR_INVALID)
            throw std::runtime_error("Mixed DPI hosting requires Windows 10 1803 or newer");
        modules[0].name = L"Производственная отчётность";
        modules[1].name = L"Производственный цикл";
        modules[0].title = fixtureTest ? L"Fixture Reporting" : readSetting(L"reporting", L"window_title");
        modules[1].title = fixtureTest ? L"Fixture Cycle" : readSetting(L"cycle", L"window_title");
        if (!fixtureTest) {
            modules[0].relative = readSetting(L"reporting", L"executable");
            modules[1].relative = readSetting(L"cycle", L"executable");
        }
        WNDCLASSW c{}; c.lpfnWndProc = procedure; c.hInstance = instance; c.lpszClassName = kClass;
        c.hCursor = LoadCursorW(nullptr, IDC_ARROW); c.hbrBackground = reinterpret_cast<HBRUSH>(COLOR_BTNFACE + 1);
        RegisterClassW(&c);
        HWND w = CreateWindowExW(WS_EX_CONTROLPARENT, kClass, L"Производственный контур — отчётность и цикл",
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN, CW_USEDEFAULT, CW_USEDEFAULT, 1500, 980,
            nullptr, nullptr, instance, nullptr);
        if (!w) throw std::runtime_error("Cannot create frame window");
        ShowWindow(w, show); UpdateWindow(w); testStarted = GetTickCount64(); selectModule(0);
        MSG msg{};
        while (GetMessageW(&msg, nullptr, 0, 0) > 0) { TranslateMessage(&msg); DispatchMessageW(&msg); }
        CloseHandle(mutex); return static_cast<int>(msg.wParam);
    } catch (const std::exception& e) {
        if (mutex) CloseHandle(mutex);
        showError(e); return 1;
    }
}
