# Проверка первого макета — 2026-09-08

Код опубликован в отдельном репозитории [Prapeller12/production-cycle](https://github.com/Prapeller12/production-cycle).
Коммит реализации: 960b96d7e8082263b7237d98070d734a0bc7199f.
История и файлы отчётности не включены; запись в otchet не выполнялась.

| Проверка | Результат |
|---|---|
| `node --test tests/domain.test.cjs` | 19 passed, 0 failed; локально и в GitHub Actions |
| `node --check frontend/ui/app.js` | успешно |
| `git diff --check` | успешно |
| Браузерные сценарии Chromium в Actions | успешно: демо, валидация, JSON/TXT, отказ повреждённого импорта без потери данных, редактирование и PDF |
| Сетевые запросы UI в браузерном сценарии | внешних HTTP(S)-запросов не обнаружено |
| Скриншоты и PDF | созданы workflow как артефакт production-cycle-visual-check; ручной визуальный просмотр ещё не выполнен |
| Rust/Windows build | выполняется в GitHub Actions; результат ещё не подтверждён |
| Windows 10/11 offline | не проведена |
| Полный portable ZIP | не выпущен |
| GitHub чтение и запись | подтверждены фактической публикацией в production-cycle |

Источник CI: [запуск 34200636096](https://github.com/Prapeller12/production-cycle/actions/runs/34200636096).
Следующие проверки: результат Rust-сборки, визуальные артефакты, закрепление Cargo.lock,
полный ZIP с проверенным Fixed Runtime и матрица Windows.
Успешные браузерные тесты не подтверждают работу native WebView2-host или совместимость Windows.
