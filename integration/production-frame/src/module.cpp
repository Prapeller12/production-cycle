#include "frame.hpp"
#include <filesystem>
namespace frame {
void Module::launch(bool fixture, int index) {
    if (alive()) return;
    release(); failed = false;
    std::wstring path = fixture ? executable() : portableFile(relative);
    std::wstring command = L"\"" + path + L"\"";
    if (fixture) command += L" --fixture " + std::to_wstring(index);
    STARTUPINFOW startup{}; startup.cb = sizeof(startup);
    auto directory = std::filesystem::path(path).parent_path().wstring();
    if (!CreateProcessW(path.c_str(), command.data(), nullptr, nullptr, FALSE,
            CREATE_UNICODE_ENVIRONMENT, nullptr, directory.c_str(), &startup, &process)) {
        auto code = GetLastError(); process = {};
        throw std::runtime_error("Cannot launch module, Windows error " + std::to_string(code));
    }
    CloseHandle(process.hThread); process.hThread = nullptr;
    started = GetTickCount64();
}
bool Module::alive() const {
    return process.hProcess && WaitForSingleObject(process.hProcess, 0) == WAIT_TIMEOUT;
}
bool Module::discover() {
    if (IsWindow(window)) return true;
    EnumWindows([](HWND candidate, LPARAM param) -> BOOL {
        auto& module = *reinterpret_cast<Module*>(param);
        DWORD pid{}; GetWindowThreadProcessId(candidate, &pid);
        if (pid != module.process.dwProcessId || GetWindow(candidate, GW_OWNER)) return TRUE;
        wchar_t text[512]{}; GetWindowTextW(candidate, text, 512);
        if (std::wstring(text).find(module.title) != 0) return TRUE;
        if (GetWindowLongPtrW(candidate, GWL_STYLE) & WS_CHILD) return TRUE;
        module.window = candidate;
        return FALSE;
    }, reinterpret_cast<LPARAM>(this));
    return window != nullptr;
}
void Module::attach(HWND parent) {
    auto context = GetWindowDpiAwarenessContext(window);
    auto previous = SetThreadDpiAwarenessContext(context);
    slot = CreateWindowExW(0, L"STATIC", L"", WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
        0, 0, 1, 1, parent, nullptr, GetModuleHandleW(nullptr), nullptr);
    SetThreadDpiAwarenessContext(previous);
    if (!slot || !AreDpiAwarenessContextsEqual(context, GetWindowDpiAwarenessContext(slot)))
        throw std::runtime_error("Cannot create a matching DPI container");
    originalStyle = GetWindowLongPtrW(window, GWL_STYLE);
    originalExStyle = GetWindowLongPtrW(window, GWL_EXSTYLE);
    SetLastError(0);
    auto style = (originalStyle & ~(WS_POPUP | WS_CAPTION | WS_THICKFRAME |
        WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU)) | WS_CHILD | WS_CLIPSIBLINGS;
    if (!SetWindowLongPtrW(window, GWL_STYLE, style) && GetLastError())
        throw std::runtime_error("Cannot change child window style");
    SetWindowLongPtrW(window, GWL_EXSTYLE, originalExStyle & ~WS_EX_APPWINDOW);
    SetLastError(0);
    auto oldParent = SetParent(window, slot);
    auto code = GetLastError();
    if (!oldParent && code) {
        SetWindowLongPtrW(window, GWL_STYLE, originalStyle);
        SetWindowLongPtrW(window, GWL_EXSTYLE, originalExStyle);
        DestroyWindow(slot); slot = nullptr;
        throw std::runtime_error("SetParent failed, Windows error " + std::to_string(code));
    }
    SetWindowPos(window, nullptr, 0, 0, 1, 1, SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    SendMessageTimeoutW(window, WM_CHANGEUISTATE, MAKEWPARAM(UIS_INITIALIZE, 0), 0,
        SMTO_ABORTIFHUNG, 500, nullptr);
}
void Module::detach() {
    if (IsWindow(window) && slot && GetParent(window) == slot) {
        SetParent(window, nullptr);
        SetWindowLongPtrW(window, GWL_STYLE, originalStyle);
        SetWindowLongPtrW(window, GWL_EXSTYLE, originalExStyle);
        SetWindowPos(window, nullptr, 40, 40, 1280, 800,
            SWP_NOZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
    }
    if (slot) DestroyWindow(slot);
    slot = nullptr;
}
void Module::release() {
    detach();
    if (process.hProcess) CloseHandle(process.hProcess);
    process = {}; window = nullptr;
}
}
