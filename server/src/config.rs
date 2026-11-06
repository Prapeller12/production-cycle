use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{fs::{self, File, OpenOptions}, path::{Component, Path, PathBuf}, time::Duration};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub listen: String,
    pub public_url: String,
    pub tls: bool,
    pub certificate: String,
    pub private_key: String,
    pub data_directory: String,
    pub max_clients: usize,
    pub session_timeout_seconds: u64,
    #[serde(default)]
    pub allow_insecure_lan: bool,
}

pub fn io_error(operation: &str, path: &Path, error: impl std::fmt::Display) -> String {
    format!("{operation}\nПуть: {}\nПричина: {error}", path.display())
}

pub fn local_path(path: &Path) -> bool {
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        use std::path::Prefix;
        let Some(Component::Prefix(p)) = path.components().next() else { return false };
        let letter = match p.kind() { Prefix::Disk(l) | Prefix::VerbatimDisk(l) => l, _ => return false };
        let drive: Vec<u16> = std::ffi::OsStr::new(&format!("{}:\\", letter as char)).encode_wide().chain(Some(0)).collect();
        #[link(name = "kernel32")]
        extern "system" { fn GetDriveTypeW(root: *const u16) -> u32; }
        return unsafe { GetDriveTypeW(drive.as_ptr()) } == 3;
    }
    #[cfg(not(windows))] { path.is_absolute() }
}

pub fn inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty() || Path::new(relative).components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("Путь должен быть относительным и находиться внутри серверной папки: {relative}"));
    }
    Ok(root.join(relative))
}

pub fn lock_data(directory: &Path) -> Result<File, String> {
    fs::create_dir_all(directory).map_err(|e| io_error("Не удалось создать папку данных сервера", directory, e))?;
    let path = directory.join("server.lock");
    let lock = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&path)
        .map_err(|e| io_error("Нет доступа к блокировке сервера", &path, e))?;
    lock.try_lock_exclusive().map_err(|e| io_error("Эта папка данных уже используется другим процессом сервера", &path, e))?;
    Ok(lock)
}

impl Config {
    pub fn load(root: &Path) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|e| io_error("Не удалось открыть серверную папку", root, e))?;
        if !local_path(&root) { return Err("Сервер нужно запускать с локального диска сервера, а не по UNC или с подключённого сетевого диска.".into()); }
        let path = root.join("config/server.json");
        let bytes=fs::read(&path).map_err(|e| io_error("Не удалось прочитать настройки", &path, e))?;
        let value: Self = serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes))
            .map_err(|e| io_error("Некорректный config/server.json", &path, e))?;
        value.validate()?;
        for relative in [&value.data_directory, &value.certificate, &value.private_key] { inside(&root, relative)?; }
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), String> {
        let listen: std::net::SocketAddr = self.listen.parse().map_err(|_| "listen должен содержать IP и порт, например 0.0.0.0:8443")?;
        let public = url::Url::parse(&self.public_url).map_err(|_| "Некорректный publicUrl")?;
        if public.scheme() != if self.tls { "https" } else { "http" } || public.host_str().is_none()
            || !public.username().is_empty() || public.password().is_some() || public.path() != "/"
            || public.query().is_some() || public.fragment().is_some() || public.port_or_known_default() != Some(listen.port()) {
            return Err("publicUrl должен задавать адрес сервера с тем же портом и протоколом, что listen/tls, без пути и пароля.".into());
        }
        if !self.tls && !listen.ip().is_loopback() && !self.allow_insecure_lan {
            return Err("HTTP разрешён только на loopback. Для сетевого доступа настройте HTTPS; allowInsecureLan включается отдельно только для изолированной доверенной сети.".into());
        }
        if self.max_clients != 3 || !(30..=3600).contains(&self.session_timeout_seconds) {
            return Err("maxClients должен быть 3, sessionTimeoutSeconds — от 30 до 3600.".into());
        }
        Ok(())
    }
    pub fn prepare_tls(&self, root: &Path) -> Result<(), String> {
        if !self.tls { return Ok(()); }
        let cert = inside(root, &self.certificate)?;
        let key = inside(root, &self.private_key)?;
        if cert.try_exists().map_err(|e| io_error("Не удалось проверить сертификат", &cert, e))?
            && key.try_exists().map_err(|e| io_error("Не удалось проверить ключ TLS", &key, e))? {
            let bytes = fs::read(&cert).map_err(|e| io_error("Не удалось прочитать сертификат", &cert, e))?;
            let certificates = rustls_pemfile::certs(&mut std::io::BufReader::new(bytes.as_slice())).map_err(|e| e.to_string())?;
            let public = certificates.first().ok_or("Файл certificate не содержит сертификат PEM")?;
            let exported = root.join("server.cer");
            // Service has read-only access to application files; --init exports before installation.
            if fs::read(&exported).ok().as_deref() != Some(public.as_slice()) {
                fs::write(exported, public).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        if cert.exists() || key.exists() { return Err("Найден только один из файлов сертификата/ключа TLS. Исправьте комплект; существующий ключ автоматически не заменяется.".into()); }
        let host = url::Url::parse(&self.public_url).map_err(|e| e.to_string())?.host_str().ok_or("Нет адреса сервера")?.to_string();
        let mut hosts = vec![host, "localhost".into(), "127.0.0.1".into()]; hosts.sort(); hosts.dedup();
        let mut params=rcgen::CertificateParams::new(hosts).map_err(|e|e.to_string())?;
        params.not_before=time::OffsetDateTime::now_utc()-time::Duration::days(1);
        params.not_after=time::OffsetDateTime::now_utc()+time::Duration::days(365);
        params.distinguished_name.push(rcgen::DnType::CommonName,"ProductionCycle Server");
        params.extended_key_usages=vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        let key_pair=rcgen::KeyPair::generate().map_err(|e|e.to_string())?;
        let generated=params.self_signed(&key_pair).map_err(|e|e.to_string())?;
        for p in [&cert, &key] { fs::create_dir_all(p.parent().ok_or("Нет папки TLS")?).map_err(|e| io_error("Не удалось создать папку TLS", p, e))?; }
        let mut options = OpenOptions::new(); options.create_new(true).write(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        use std::io::Write;
        options.open(&key).and_then(|mut f| f.write_all(key_pair.serialize_pem().as_bytes())).map_err(|e| io_error("Не удалось создать ключ TLS", &key, e))?;
        fs::write(&cert, generated.pem()).map_err(|e| io_error("Не удалось сохранить сертификат", &cert, e))?;
        fs::write(root.join("server.cer"), generated.der()).map_err(|e| e.to_string())?;
        println!("Создан сертификат server.cer. Для штатного HTTPS установите доверие к нему на клиентских ПК либо используйте сертификат вашей организации.");
        Ok(())
    }
    pub fn import_database(&self, root: &Path, source: &Path) -> Result<(), String> {
        let dir = inside(root, &self.data_directory)?;
        let _lock = lock_data(&dir)?;
        let target = dir.join("production-cycle.db");
        if target.try_exists().map_err(|e| e.to_string())? { return Err("Серверная база уже существует. Импорт не перезаписывает данные; используйте новую серверную папку.".into()); }
        let source = source.canonicalize().map_err(|e| io_error("Не удалось открыть исходную базу", source, e))?;
        let source_db = rusqlite::Connection::open_with_flags(&source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
        let check: String = source_db.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if check != "ok" { return Err(format!("Исходная база не прошла проверку: {check}")); }
        let temp = dir.join(format!("import-{}.db", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut copy = rusqlite::Connection::open(&temp).map_err(|e| e.to_string())?;
            rusqlite::backup::Backup::new(&source_db, &mut copy).map_err(|e| e.to_string())?
                .run_to_completion(50, Duration::from_millis(10), None).map_err(|e| e.to_string())?;
            drop(copy);
            let db = crate::production::ProductionDb::open(&temp).map_err(|e| e.to_string())?;
            let projects = db.list_projects().map_err(|e| e.to_string())?.len();
            let users = db.list_audit_users()?.len(); drop(db);
            fs::rename(&temp, &target).map_err(|e| io_error("Не удалось разместить серверную базу", &target, e))?;
            println!("База перенесена. Проектов: {projects}; профилей: {users}. Исходная база сохранена без изменений.");
            Ok(())
        })();
        if result.is_err() { let _ = fs::remove_file(&temp); }
        result
    }
}
