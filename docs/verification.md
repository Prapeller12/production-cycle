# Проверка portable-макета №1 — 2026-09-08

Код: отдельный репозиторий Prapeller12/production-cycle. Репозиторий otchet не изменяется.

Проверенный native-коммит: 2f087e0f88769ba09bad93b7e93f46ff2071761f.
Источник: https://github.com/Prapeller12/production-cycle/actions/runs/34201808086

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
| Offline с запретом исходящей сети и без Python/Node в PATH | отдельный тест добавлен; смотреть результат текущего workflow |
| Чистые Windows 10/11 x64, без установленного runtime | пока не проверены |
| Ручная визуальная приёмка скриншотов/PDF | пока не выполнена |

Проверка на сервере сборки не заменяет пользовательские Windows 10/11.
В программе пока JSON-хранение. SQLite и production-сервисы Rust остаются отдельным этапом.
Готовые предварительные выпуски: https://github.com/Prapeller12/production-cycle/releases
