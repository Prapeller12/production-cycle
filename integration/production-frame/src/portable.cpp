#include "frame.hpp"
#include <fstream>
#include <filesystem>

namespace frame {
std::wstring executable() {
    std::vector<wchar_t> buffer(32768);
    DWORD n = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
    if (!n || n == buffer.size()) throw std::runtime_error("Cannot resolve executable path");
    return std::wstring(buffer.data(), n);
}
std::wstring root() {
    const auto path = executable();
    return path.substr(0, path.find_last_of(L"\\/"));
}
std::wstring errorText(DWORD code) {
    wchar_t* message = nullptr;
    FormatMessageW(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM |
        FORMAT_MESSAGE_IGNORE_INSERTS, nullptr, code, 0,
        reinterpret_cast<wchar_t*>(&message), 0, nullptr);
    std::wstring result = message ? message : L"Ошибка Windows";
    if (message) LocalFree(message);
    return result + L" (" + std::to_wstring(code) + L")";
}
std::wstring readSetting(const wchar_t* section, const wchar_t* key) {
    wchar_t buffer[2048]{};
    GetPrivateProfileStringW(section, key, L"", buffer, 2048,
        (root() + L"\\config\\frame.ini").c_str());
    if (!buffer[0]) throw std::runtime_error("Missing config/frame.ini setting");
    return buffer;
}
static std::wstring finalPath(const std::wstring& path) {
    HANDLE file = CreateFileW(path.c_str(), FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr, OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS, nullptr);
    if (file == INVALID_HANDLE_VALUE) throw std::runtime_error("Portable file is missing");
    wchar_t buffer[32768];
    DWORD n = GetFinalPathNameByHandleW(file, buffer, 32768, FILE_NAME_NORMALIZED);
    CloseHandle(file);
    if (!n || n >= 32768) throw std::runtime_error("Cannot resolve portable file");
    return std::wstring(buffer, n);
}
std::wstring portableFile(const std::wstring& relative) {
    if (relative.empty() || relative.find(L':') != std::wstring::npos ||
        relative.front() == L'\\' || relative.front() == L'/' ||
        relative.find(L'"') != std::wstring::npos)
        throw std::runtime_error("Expected a local relative path");
    std::filesystem::path r(relative);
    for (auto& part : r) if (part == L".." || part == L".")
        throw std::runtime_error("Portable path traversal is not allowed");
    auto base = finalPath(root()) + L"\\";
    auto candidate = finalPath(root() + L"\\" + relative);
    if (candidate.size() <= base.size() ||
        _wcsnicmp(base.c_str(), candidate.c_str(), base.size()) != 0)
        throw std::runtime_error("Portable path escapes program folder");
    return root() + L"\\" + relative;
}
void checkLayout() {
    auto base = root();
    if (base.rfind(L"\\\\", 0) == 0 || base.size() < 3 ||
        GetDriveTypeW(base.substr(0, 3).c_str()) == DRIVE_REMOTE)
        throw std::runtime_error("Use a local writable folder, not a network drive");
    portableFile(L"config\\frame.ini");
    // Module validation happens on launch/selection so a missing module can be repaired in the UI.

}
void evidence(HWND window, const wchar_t* name) {
    // Only explicitly requested test mode creates evidence; ordinary launch writes nothing.
    CreateDirectoryW((root() + L"\\temp").c_str(), nullptr);
    RECT r{}; GetWindowRect(window, &r);
    int w = r.right - r.left, h = r.bottom - r.top;
    HDC screen = GetDC(nullptr), dc = CreateCompatibleDC(screen);
    HBITMAP bitmap = CreateCompatibleBitmap(screen, w, h);
    auto old = SelectObject(dc, bitmap);
    BitBlt(dc, 0, 0, w, h, screen, r.left, r.top, SRCCOPY);
    SelectObject(dc, old);
    BITMAPINFO info{}; info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = w; info.bmiHeader.biHeight = -h;
    info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 32;
    std::vector<char> pixels(static_cast<size_t>(w) * h * 4);
    GetDIBits(dc, bitmap, 0, h, pixels.data(), &info, DIB_RGB_COLORS);
    BITMAPFILEHEADER header{}; header.bfType = 0x4d42;
    header.bfOffBits = sizeof(header) + sizeof(BITMAPINFOHEADER);
    header.bfSize = header.bfOffBits + static_cast<DWORD>(pixels.size());
    std::ofstream out(std::filesystem::path(root()) / L"temp" / name, std::ios::binary);
    out.write(reinterpret_cast<char*>(&header), sizeof(header));
    out.write(reinterpret_cast<char*>(&info.bmiHeader), sizeof(info.bmiHeader));
    out.write(pixels.data(), pixels.size());
    DeleteObject(bitmap); DeleteDC(dc); ReleaseDC(nullptr, screen);
}
}
