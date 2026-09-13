#pragma once
#define UNICODE
#define _UNICODE
#define NOMINMAX
#define _WIN32_WINNT 0x0A00
#include <windows.h>
#include <shellapi.h>
#include <string>
#include <vector>
#include <array>
#include <stdexcept>
#include <algorithm>

namespace frame {
std::wstring executable();
std::wstring root();
std::wstring errorText(DWORD code);
std::wstring readSetting(const wchar_t* section, const wchar_t* key);
std::wstring portableFile(const std::wstring& relative);
void checkLayout();
std::wstring chooseVersion(HWND owner, int index);
void validateVersion(const std::wstring& relative, int index);
void saveVersion(const wchar_t* section, const std::wstring& next, const std::wstring& previous);
void evidence(HWND window, const wchar_t* name);

struct Module {
    std::wstring name, relative, title;
    PROCESS_INFORMATION process{};
    HWND window{}, slot{};
    LONG_PTR originalStyle{}, originalExStyle{};
    bool failed = false;
    ULONGLONG started{};
    void launch(bool fixture, int index);
    bool alive() const;
    bool discover();
    void attach(HWND parent);
    void detach();
    void release();
};
}
