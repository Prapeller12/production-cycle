#include "../src/frame.hpp"
#include <filesystem>
#include <fstream>
#include <iostream>
using namespace frame;
namespace fs = std::filesystem;
void require(bool value) { if (!value) throw std::runtime_error("Version test assertion failed"); }
int main() {
    try {
        const auto base = fs::path(root());
        for (auto name : {L"alpha", L"beta"}) {
            auto dir = base / name;
            fs::create_directories(dir / L"runtime/webview2");
            fs::create_directories(dir / L"app/frontend");
            fs::create_directories(dir / L"frontend");
            fs::create_directories(dir / L"data");
            fs::copy_file(executable(), dir / L"ReportingSystem.exe");
            fs::copy_file(executable(), dir / L"production-cycle.exe");
            std::ofstream(dir / L"runtime/webview2/msedgewebview2.exe") << "runtime fixture";
            std::ofstream(dir / L"app/frontend/index.html") << "reporting fixture";
            std::ofstream(dir / L"frontend/index.html") << "cycle fixture";
            std::ofstream(dir / L"data/sentinel.txt") << "unchanged";
        }
        validateVersion(L"alpha/ReportingSystem.exe", 0);
        validateVersion(L"beta/production-cycle.exe", 1);
        auto reject = [](const std::wstring& path, int index) {
            bool failed = false; try { validateVersion(path, index); } catch (...) { failed = true; }
            require(failed);
        };
        reject(L"../ReportingSystem.exe", 0);
        reject(L"alpha/production-cycle.exe", 0);
        reject(L"missing/ReportingSystem.exe", 0);
        fs::remove(base / L"beta/frontend/index.html");
        reject(L"beta/production-cycle.exe", 1);
        saveVersion(L"reporting", L"alpha/ReportingSystem.exe", L"modules/ReportingSystem/ReportingSystem.exe");
        saveVersion(L"reporting", L"beta/ReportingSystem.exe", L"alpha/ReportingSystem.exe");
        require(readSetting(L"reporting", L"executable") == L"beta/ReportingSystem.exe");
        require(readSetting(L"reporting", L"previous_executable") == L"alpha/ReportingSystem.exe");
        saveVersion(L"reporting", readSetting(L"reporting", L"previous_executable"), readSetting(L"reporting", L"executable"));
        require(readSetting(L"reporting", L"executable") == L"alpha/ReportingSystem.exe");
        require(readSetting(L"cycle", L"executable") == L"modules\\ProductionCycle\\production-cycle.exe");
        for (auto name : {L"alpha", L"beta"}) {
            std::string value; std::ifstream(base / name / L"data/sentinel.txt") >> value;
            require(value == "unchanged");
        }
        require(!fs::exists(base / L"config/frame.ini.pending"));
        std::cout << "Version validation, settings persistence, return and data isolation passed\n";
        return 0;
    } catch (const std::exception& e) { std::cerr << e.what(); return 1; }
}
