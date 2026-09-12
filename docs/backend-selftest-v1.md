# Backend self-test v1

Первая реализация backend-контуров по утверждённым `architecture-v0.2.md` и DEV SPEC v0.1.

## Реализовано

- SQLite `production_cycle` + `production_stage`;
- транзакционное сохранение/загрузка снимка проекта;
- валидация обязательных полей, дат, TAB/CR/LF, родителей и циклов;
- стабильный ID этапа `<номер заказа>-NNN`;
- pre-order дерева;
- TXT 9/8 колонок, TAB + CRLF, поле `Ссылка` пустое;
- производный `overdue`;
- backend-модель управленческого отчёта;
- Tauri IPC:
  - `production_validate`;
  - `production_save_snapshot`;
  - `production_load_snapshot`;
  - `production_export_txt`;
  - `production_get_management_report`.
- frontend подключён к IPC без изменения утверждённой компоновки:
  - «Сохранить проект» записывает снимок в SQLite;
  - «Открыть проект» загружает заказ из SQLite по номеру;
  - «Проверить» использует backend-валидацию;
  - TXT и модель PDF-отчёта формируются backend-сервисами;
  - при открытии `frontend/index.html` в браузере остаётся JSON fallback для автономной проверки макета.

## Проверки разработчика

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Текущий frontend остаётся утверждённым UX/self-test эталоном и уже переключён на backend IPC в native-режиме без изменения компоновки.

## Инварианты

- production-иерархия не использует `LINK` Реестра;
- `Ссылка` не содержит служебных данных;
- исходящий TXT — 9 колонок;
- входящий TXT — 8 колонок;
- `overdue` не хранится как ручной статус;
- БД находится только в portable `data/production-cycle.db`.
