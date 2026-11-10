#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod config;
mod sessions;
mod http;
mod service;
mod launcher;
mod excel_import;
mod ui_preferences;
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
        if !std::env::args().skip(1).any(|a| a.starts_with("--")) {
            launcher::notice(&format!("Не удалось открыть программу.\n\n{error}\n\nПроверьте, что ZIP распакован целиком на локальный диск."), true);
        }
        std::process::exit(1);
    }
}

fn entry() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--service") { return service::dispatch(); }
    let root = args.windows(2).find(|a| a[0] == "--root")
        .map(|a| PathBuf::from(&a[1]))
        .unwrap_or(std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("Нет папки EXE")?.to_path_buf());
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let config = config::Config::load(&root)?;
    if args.iter().any(|a| a == "--init") {
        config.prepare_tls(&root)?;
        println!("Конфигурация проверена. Адрес: {}", config.public_url);
        return Ok(());
    }
    if let Some(pair) = args.windows(2).find(|a| a[0] == "--import-database") {
        return config.import_database(&root, &PathBuf::from(&pair[1]));
    }
    if args.iter().any(|a| a == "--check") {
        return launcher::check(&root, &config);
    }
    if args.iter().any(|a| a == "--stop") { return launcher::stop(&root, &config); }
    if args.iter().any(|a| a == "--serve") {
        println!("Производственный цикл — сервер\nАдрес: {}", config.public_url);
        return http::run(&root, config, Arc::new(AtomicBool::new(false)));
    }
    if launcher::running(&root, &config) { return launcher::open_browser(&config); }
    let (sender, receiver) = std::sync::mpsc::channel();
    let browser_config = config.clone();
    let opener = std::thread::spawn(move || {
        if receiver.recv().is_ok() {
            if let Err(error) = launcher::open_browser(&browser_config) { launcher::notice(&error, true); }
        }
    });
    let result = http::run_with_ready(&root, config.clone(), Arc::new(AtomicBool::new(false)), "manual", Some(sender));
    let _ = opener.join();
    if result.as_ref().err().map(|e| e.contains("уже используется другим процессом")).unwrap_or(false) {
        // Two simultaneous clicks may race before the first process binds its port.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if launcher::running(&root, &config) { return launcher::open_browser(&config); }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    result
}
