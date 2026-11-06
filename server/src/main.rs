mod config;
mod sessions;
mod http;
mod service;
#[path = "../../src-tauri/src/production/mod.rs"]
mod production;
#[path = "../../src-tauri/src/audit.rs"]
mod audit;
#[path = "../../src-tauri/src/backup.rs"]
mod backup;

use std::{path::PathBuf, sync::{Arc, atomic::AtomicBool}};

fn main() {
    if let Err(error) = entry() {
        eprintln!("Производственный цикл — ошибка сервера\n{error}");
        std::process::exit(1);
    }
}

fn entry() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--service") { return service::dispatch(); }
    let root = args.windows(2).find(|a| a[0] == "--root")
        .map(|a| PathBuf::from(&a[1]))
        .unwrap_or(std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("Нет папки EXE")?.to_path_buf());
    let config = config::Config::load(&root)?;
    if args.iter().any(|a| a == "--init") {
        config.prepare_tls(&root)?;
        println!("Конфигурация проверена. Адрес: {}", config.public_url);
        return Ok(());
    }
    if let Some(pair) = args.windows(2).find(|a| a[0] == "--import-database") {
        return config.import_database(&root, &PathBuf::from(&pair[1]));
    }
    println!("Производственный цикл — сервер\nАдрес: {}\nДля остановки закройте это окно. Постоянный запуск настраивается скриптом службы.", config.public_url);
    http::run(&root, config, Arc::new(AtomicBool::new(false)))
}
