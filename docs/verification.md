# Проверка portable-макета №1 — 2026-09-08

Код: отдельный репозиторий Prapeller12/production-cycle. Репозиторий otchet не изменяется.

Проверенный native/offline-коммит: b2c078189b7a7d6da9444bbfbfd56dbfde19154d.
Источник: https://github.com/Prapeller12/production-cycle/actions/runs/34202615428

| Проверка | Результат |
|---|---|
| JavaScript domain/contracts | 19 passed, 0 failed |
| Браузерные сценарии Chromium | успешно: демо, редактирование, JSON/TXT, защита при ошибке импорта, PDF |
| HTTP(S)-запросы UI в браузерном сценарии | отсутствуют |
| Rust release unit tests | 2 passed, 0 failed |
| Windows EXE с внешним frontend | скомпилирован |
| Подпись, размер и версия Microsoft Fixed WebView2 | проверены |
| Полный ZIP с runtime | собран |
| Native WebView2 / IPC / JSON / TXT | PASS |
| Путь с кириллицей и пробелами | PASS |
| Данные и профиль в папке программы | подтверждено native self-test |
| ОС native-проверки | Microsoft Windows Server 2025, GitHub runner |
| Offline с запретом исходящей сети и без Python/Node в PATH | PASS: исходящая сеть EXE/WebView2 заблокирована; PATH содержит только системные папки Windows |
| Чистые Windows 10/11 x64, без установленного runtime | пока не проверены |
| Ручная визуальная приёмка скриншотов/PDF | пока не выполнена |

Проверка на сервере сборки не заменяет пользовательские Windows 10/11.
В программе пока JSON-хранение. SQLite и production-сервисы Rust остаются отдельным этапом.
Готовые предварительные выпуски: https://github.com/Prapeller12/production-cycle/releases

Выпуск: https://github.com/Prapeller12/production-cycle/releases/tag/portable-prototype-1-b2c0781
Cargo.lock взят из артефакта этой успешно проверенной сборки и закреплён в репозитории.
