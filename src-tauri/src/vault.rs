//! All on-disk business connections in the native application pass through this module.
use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use rusqlite::{backup::Backup, Connection, OpenFlags};
use std::{fs, io::Read, path::Path, sync::OnceLock, time::Duration};
use uuid::Uuid;
use zeroize::Zeroizing;

static SECRET: OnceLock<Zeroizing<String>> = OnceLock::new();
type Result<T> = std::result::Result<T, String>;
pub fn validate_phrase(s: &str) -> Result<()> {
    if s.chars().count() < 12 || s.len() > 1024 || s.contains('\0') {
        return Err(
            "Введите отдельную парольную фразу длиной от 12 символов (до 1024 байт).".into(),
        );
    }
    Ok(())
}
fn secret() -> Result<Option<&'static str>> {
    match SECRET.get() {
        Some(v) => Ok(Some(v.as_str())),
        None if cfg!(test) => Ok(None),
        None => Err("Хранилище не разблокировано.".into()),
    }
}
#[cfg(windows)]
pub fn unlock() -> Result<()> {
    // The static UI script receives no secret on the command line or filesystem.
    use std::os::windows::process::CommandExt;
    let sys = std::env::var_os("SystemRoot").ok_or("Windows недоступна.")?;
    let exe = Path::new(&sys).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let out = std::process::Command::new(exe)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-STA",
            "-Command",
            include_str!("unlock.ps1"),
        ])
        .creation_flags(0x08000000)
        .output()
        .map_err(|_| "Не удалось открыть окно разблокировки хранилища.")?;
    if !out.status.success() {
        return Err("Открытие хранилища отменено.".into());
    }
    let phrase = Zeroizing::new(
        String::from_utf8(out.stdout).map_err(|_| "Не удалось прочитать парольную фразу.")?,
    );
    validate_phrase(&phrase)?;
    SECRET
        .set(phrase)
        .map_err(|_| "Хранилище уже открыто.".into())
}
#[cfg(not(windows))]
pub fn unlock() -> Result<()> {
    Err("Нативный комплект предназначен для Windows.".into())
}

pub fn keyed(path: &Path, flags: OpenFlags, pass: Option<&str>) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, flags).map_err(|e| e.to_string())?;
    if let Some(s) = pass {
        let version: String = conn
            .query_row("PRAGMA cipher_version", [], |r| r.get(0))
            .map_err(|_| "В сборке отсутствует SQLCipher.")?;
        if version.is_empty() {
            return Err("В сборке отсутствует SQLCipher.".into());
        }
        conn.pragma_update(None, "key", s)
            .map_err(|_| "Не удалось открыть ключ базы.")?;
    }
    conn.execute_batch("PRAGMA temp_store=MEMORY; PRAGMA trusted_schema=OFF;")
        .map_err(|e| e.to_string())?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| {
        "Неверная парольная фраза, неподдерживаемый формат или повреждённая база.".to_string()
    })?;
    Ok(conn)
}
pub fn open(path: &Path, flags: OpenFlags) -> Result<Connection> {
    keyed(path, flags, secret()?)
}
pub fn read(path: &Path) -> Result<Connection> {
    open(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}
pub fn copy(source: &Connection, dest: &mut Connection) -> Result<()> {
    Backup::new(source, dest)
        .map_err(|e| e.to_string())?
        .run_to_completion(128, Duration::from_millis(1), None)
        .map_err(|e| e.to_string())
}
pub fn backup(source: &Connection, path: &Path) -> Result<()> {
    let mut dest = open(path, OpenFlags::default())?;
    copy(source, &mut dest)
}
pub fn restore(dest: &mut Connection, path: &Path) -> Result<()> {
    copy(&read(path)?, dest)
}
pub fn plaintext(path: &Path) -> bool {
    let mut bytes = [0u8; 16];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_ok()
        && &bytes == b"SQLite format 3\0"
}
// Convert only a verified, quiescent old production file on local disk; replace it after verification.
pub fn migrate_plain(path: &Path) -> Result<()> {
    let Some(pass) = secret()? else { return Ok(()) };
    if !plaintext(path) {
        return Ok(());
    }
    for suffix in ["-wal", "-journal", "-shm"] {
        let mut s = path.as_os_str().to_owned();
        s.push(suffix);
        if Path::new(&s).exists() {
            return Err("У старой базы есть незавершённый журнал. Закройте прежнее приложение и подготовьте согласованную резервную копию.".into());
        }
    }
    let temp = path.with_extension(format!("encrypt-{}", Uuid::new_v4()));
    let result = (|| {
        let source = keyed(path, OpenFlags::SQLITE_OPEN_READ_ONLY, None)?;
        let n:i64=source.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('production_cycle','production_stage')",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        let ver: i64 = source
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let check: String = source
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if n != 2 || ver > crate::production::SCHEMA_VERSION || check != "ok" {
            return Err("Исходная база не прошла проверку совместимости.".into());
        }
        // A read-only main connection also makes attached databases read-only on some SQLite versions.
        drop(source);
        let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA temp_store=MEMORY;")
            .map_err(|e| e.to_string())?;
        c.execute(
            "ATTACH DATABASE ?1 AS secured KEY ?2",
            rusqlite::params![temp.to_str().ok_or("Путь файла не поддерживается.")?, pass],
        )
        .map_err(|_| "Не удалось подготовить зашифрованную копию.")?;
        c.query_row("SELECT sqlcipher_export('secured')", [], |_| Ok(()))
            .map_err(|e| e.to_string())?;
        c.pragma_update(
            Some(rusqlite::DatabaseName::Attached("secured")),
            "user_version",
            ver,
        )
        .map_err(|e| e.to_string())?;
        c.execute_batch("DETACH DATABASE secured")
            .map_err(|e| e.to_string())?;
        drop(c);
        let protected = read(&temp)?;
        let check: String = protected
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if check != "ok" || plaintext(&temp) {
            return Err("Не прошла проверка шифрования копии.".into());
        }
        drop(protected);
        fs::File::open(&temp)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        crate::infrastructure::network::atomic_replace(&temp, path)
    })();
    if temp.exists() {
        let _ = fs::remove_file(temp);
    }
    result
}
const DRAFT_HEADER: &[u8] = b"PCDRAFT1";
fn draft_key(pass: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let mut key = Zeroizing::new([0u8; 32]);
    argon2::Argon2::default()
        .hash_password_into(pass.as_bytes(), salt, &mut *key)
        .map_err(|_| "Ошибка защиты черновика.")?;
    Ok(key)
}
pub fn seal_draft(data: &[u8]) -> Result<Vec<u8>> {
    let Some(pass) = secret()? else {
        return Ok(data.to_vec());
    };
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new((&*draft_key(pass, &salt)?).into());
    let sealed = cipher
        .encrypt(XNonce::from_slice(&nonce), data)
        .map_err(|_| "Не удалось защитить черновик.")?;
    Ok([DRAFT_HEADER, &salt, &nonce, &sealed].concat())
}
pub fn read_draft(data: &[u8]) -> Result<Vec<u8>> {
    let Some(pass) = secret()? else {
        return Ok(data.to_vec());
    };
    if !data.starts_with(DRAFT_HEADER) || data.len() < 64 {
        return Err("Черновик прежнего формата требует отдельного переноса.".into());
    }
    let key = draft_key(pass, &data[8..24])?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    cipher
        .decrypt(XNonce::from_slice(&data[24..48]), &data[48..])
        .map_err(|_| "Черновик не прошёл проверку.".into())
}
