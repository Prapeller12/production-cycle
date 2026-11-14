#include "frame.hpp"
#include <commdlg.h>
#include <filesystem>
#include <fstream>
namespace frame {
void validateVersion(const std::wstring& relative, int index) {
    const auto exe = std::filesystem::path(portableFile(relative));
    const auto expected = index == 0 ? L"ReportingSystem.exe" : L"production-cycle.exe";
    if (_wcsicmp(exe.filename().c_str(), expected))
        throw std::runtime_error("Select the main EXE of the selected application");
    DWORD binary{};
    if (!GetBinaryTypeW(exe.c_str(), &binary) || binary != SCS_64BIT_BINARY)
        throw std::runtime_error("Expected a Windows x64 executable");
    const auto parent = std::filesystem::path(relative).parent_path();
    portableFile((parent / L"runtime/webview2/msedgewebview2.exe").wstring());
    const auto frontend = portableFile((parent / (index == 0 ? L"app/frontend" : L"frontend")).wstring());
    if (!std::filesystem::is_directory(frontend))
        throw std::runtime_error("Missing frontend folder; extract the complete portable ZIP");
    portableFile((parent / (index == 0 ? L"app/frontend/index.html" : L"frontend/index.html")).wstring());
}
std::wstring chooseVersion(HWND owner, int index) {
    wchar_t file[32768]{};
    const auto initial = root();
    OPENFILENAMEW dialog{}; dialog.lStructSize = sizeof(dialog); dialog.hwndOwner = owner;
    dialog.lpstrFile = file; dialog.nMaxFile = 32768;
    dialog.lpstrInitialDir = initial.c_str();
    dialog.lpstrTitle = L"Распакуйте полный ZIP в отдельную папку внутри ProductionFrame и выберите основной EXE";
    dialog.lpstrFilter = index == 0 ? L"Отчётность\0ReportingSystem.exe\0\0" : L"Производственный цикл\0production-cycle.exe\0\0";
    dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_DONTADDTORECENT;
    if (!GetOpenFileNameW(&dialog)) {
        if (CommDlgExtendedError()) throw std::runtime_error("Cannot open file selection dialog");
        return L"";
    }
    const auto relative = std::filesystem::path(file).lexically_relative(std::filesystem::path(root())).wstring();
    validateVersion(relative, index);
    return relative;
}
void saveVersion(const wchar_t* section, const std::wstring& next, const std::wstring& previous) {
    if (next == previous) return;
    const auto config = root() + L"\\config\\frame.ini";
    const auto staging = config + L".pending";
    // Only the frame configuration changes. No module files or databases are modified.
    if (!CopyFileW(config.c_str(), staging.c_str(), FALSE))
        throw std::runtime_error("Cannot prepare frame configuration");
    if (!WritePrivateProfileStringW(section, L"previous_executable", previous.c_str(), staging.c_str()) ||
        !WritePrivateProfileStringW(section, L"executable", next.c_str(), staging.c_str())) {
        DeleteFileW(staging.c_str()); throw std::runtime_error("Cannot write frame configuration");
    }
    WritePrivateProfileStringW(nullptr, nullptr, nullptr, staging.c_str());
    if (!MoveFileExW(staging.c_str(), config.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
        DeleteFileW(staging.c_str()); throw std::runtime_error("Cannot commit frame configuration");
    }
    WritePrivateProfileStringW(nullptr, nullptr, nullptr, config.c_str());
}
}
