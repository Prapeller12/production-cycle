//! Local launcher and authenticated local stop. No downloads or shell commands.
use crate::config::{self, Config};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::{BufRead, BufReader, Read, Write}, net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::Arc, time::{Duration, Instant}};

#[derive(Serialize, Deserialize)]
pub struct Runtime {
    pub pid: u32,
    pub token: String,
    pub mode: String,
    pub instance: String,
    #[serde(skip)]
    path: PathBuf,
}

pub fn identity(root: &Path, config: &Config) -> String {
    let name = format!("{}|{}", root.to_string_lossy(), config.data_directory);
    #[cfg(windows)] let name = name.to_lowercase();
    format!("{:x}", Sha256::digest(name.as_bytes()))
}

impl Runtime {
    pub fn create(root: &Path, config: &Config, mode: &str) -> Result<Self, String> {
        let path = config::inside(root, &config.data_directory)?.join("running.json");
        let value = Self { pid: std::process::id(), token: uuid::Uuid::new_v4().to_string(), mode: mode.into(), instance: identity(root, config), path };
        let temporary = value.path.with_extension(format!("{}.tmp", value.token));
        let result = (|| {
            let mut options = fs::OpenOptions::new(); options.create_new(true).write(true);
            #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
            let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
            file.write_all(&serde_json::to_vec(&value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            drop(file);
            fs::rename(&temporary, &value.path).map_err(|e| e.to_string())
        })();
        if result.is_err() { let _ = fs::remove_file(temporary); }
        result.map_err(|e| config::io_error("Не удалось подготовить управление локальным запуском", &value.path, e))?;
        Ok(value)
    }
    pub fn health(&self) -> Value { json!({"ready":true,"application":"production-cycle","instance":self.instance,"mode":self.mode}) }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // A delayed shutdown must never remove another process's control file.
        if let Ok(bytes) = fs::read(&self.path) {
            if serde_json::from_slice::<Value>(&bytes).ok().and_then(|v| v.get("token").cloned()) == Some(json!(self.token)) { let _ = fs::remove_file(&self.path); }
        }
    }
}

fn connection_address(config: &Config) -> Result<SocketAddr, String> {
    let mut address: SocketAddr = config.listen.parse().map_err(|e: std::net::AddrParseError| e.to_string())?;
    if address.ip().is_unspecified() { address.set_ip(if address.is_ipv4() { IpAddr::V4(Ipv4Addr::LOCALHOST) } else { IpAddr::V6(Ipv6Addr::LOCALHOST) }); }
    Ok(address)
}

pub fn browser_url(config: &Config) -> Result<String, String> {
    let address = connection_address(config)?;
    if !address.ip().is_loopback() { return Ok(config.public_url.clone()); }
    let host = if config.tls { "localhost" } else if address.is_ipv4() { "127.0.0.1" } else { "[::1]" };
    Ok(format!("{}://{}:{}", if config.tls { "https" } else { "http" }, host, address.port()))
}

pub fn request(root: &Path, config: &Config, resource: &str, stop_token: Option<&str>) -> Result<Value, String> {
    let address = connection_address(config)?;
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).map_err(|e| e.to_string())?;
    stream.set_write_timeout(Some(Duration::from_secs(3))).map_err(|e| e.to_string())?;
    let host = if address.is_ipv4() { format!("127.0.0.1:{}", address.port()) } else { format!("[::1]:{}", address.port()) };
    let message = if let Some(token) = stop_token { format!("POST {resource} HTTP/1.1\r\nHost: {host}\r\nX-Production-Control: {token}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n") } else { format!("GET {resource} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n") };
    if config.tls {
        let mut roots = rustls::RootCertStore::empty();
        let certificates = match fs::read(config::inside(root, &config.certificate)?) {
            Ok(bytes) => rustls_pemfile::certs(&mut BufReader::new(bytes.as_slice())).map_err(|e| e.to_string())?,
            Err(_) => vec![fs::read(root.join("server.cer")).map_err(|e| e.to_string())?],
        };
        for bytes in certificates { roots.add(&rustls::Certificate(bytes)).map_err(|e| e.to_string())?; }
        let tls = rustls::ClientConfig::builder().with_safe_defaults().with_root_certificates(roots).with_no_client_auth();
        let public = url::Url::parse(&config.public_url).map_err(|e| e.to_string())?;
        // rustls 0.20 validates DNS SANs; generated certificates always include localhost.
        let name = rustls::ServerName::try_from(if address.ip().is_loopback() { "localhost" } else { public.host_str().ok_or("Нет имени сервера")? }).map_err(|e| e.to_string())?;
        let session = rustls::ClientConnection::new(Arc::new(tls), name).map_err(|e| e.to_string())?;
        exchange(rustls::StreamOwned::new(session, stream), &message)
    } else { exchange(stream, &message) }
}

fn exchange(mut stream: impl Read + Write, message: &str) -> Result<Value, String> {
    stream.write_all(message.as_bytes()).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut status = String::new(); reader.read_line(&mut status).map_err(|e| e.to_string())?;
    let mut size = None; let mut total = status.len();
    loop {
        let mut line = String::new(); let count = reader.read_line(&mut line).map_err(|e| e.to_string())?; total += count;
        if count == 0 || total > 8192 { return Err("Некорректный ответ локального сервера".into()); }
        if line == "\r\n" { break; }
        if let Some((key, value)) = line.split_once(':') { if key.eq_ignore_ascii_case("Content-Length") { size = value.trim().parse::<usize>().ok(); } }
    }
    let size = size.filter(|n| *n <= 8192).ok_or("Некорректный размер ответа сервера")?;
    let mut bytes = vec![0; size]; reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if status.split_whitespace().nth(1) != Some("200") { return Err(value.get("error").and_then(Value::as_str).unwrap_or("Сервер отклонил запрос").into()); }
    Ok(value)
}

pub fn running(root: &Path, config: &Config) -> bool {
    check(root, config).is_ok()
}
pub fn check(root: &Path, config: &Config) -> Result<(), String> {
    let value = request(root, config, "/health", None)?;
    if value["ready"] == true && value["application"] == "production-cycle" && value["instance"] == identity(root, config) { Ok(()) }
    else { Err("Этот адрес занят другой копией программы или другим приложением.".into()) }
}

pub fn stop(root: &Path, config: &Config) -> Result<(), String> {
    if !running(root, config) { return Err("Эта копия программы сейчас не запущена. Проверьте папку программы и её адрес.".into()); }
    let path = config::inside(root, &config.data_directory)?.join("running.json");
    let bytes = fs::read(&path).map_err(|e| config::io_error("Нет доступа к управлению сервером. Для службы используйте раздел Службы Windows", &path, e))?;
    let runtime: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let token = runtime["token"].as_str().ok_or("Нет ключа локального управления")?;
    request(root, config, "/__local/stop", Some(token))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let same = fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).map(|v| v["token"] == token).unwrap_or(false);
        if !same && !running(root, config) { return Ok(()); }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Сервер завершает операции дольше 30 секунд. Дождитесь окончания сохранения.".into())
}

pub fn open_browser(config: &Config) -> Result<(), String> {
    let target = browser_url(config)?;
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        for variable in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(variable) {
                let exe = PathBuf::from(base).join("Microsoft/Edge/Application/msedge.exe");
                if exe.is_file() { Command::new(exe).args(["--no-first-run", "--new-window", &target]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn().map_err(|e| e.to_string())?; return Ok(()); }
            }
        }
        if let Some(base) = std::env::var_os("ProgramFiles") {
            let exe = PathBuf::from(base).join("Internet Explorer/iexplore.exe");
            if exe.is_file() { Command::new(exe).arg(&target).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn().map_err(|e| e.to_string())?; return Ok(()); }
        }
        return Err(format!("Не найден установленный Edge или Internet Explorer 11. Откройте адрес вручную в установленном браузере: {target}"));
    }
    #[cfg(not(windows))] { Command::new("xdg-open").arg(target).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?; Ok(()) }
}

#[cfg(windows)]
pub fn notice(message: &str, error: bool) {
    #[link(name = "user32")] extern "system" { fn MessageBoxW(window: isize, text: *const u16, title: *const u16, flags: u32) -> i32; }
    let message: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "Производственный цикл".encode_utf16().chain(Some(0)).collect();
    unsafe { MessageBoxW(0, message.as_ptr(), title.as_ptr(), if error { 0x10 } else { 0x40 }); }
}
#[cfg(not(windows))] pub fn notice(message: &str, _error: bool) { eprintln!("{message}"); }

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn local_http_url_uses_ipv4_and_keeps_port() {
        let config: Config = serde_json::from_str(include_str!("../../config/server.json")).unwrap();
        assert_eq!(browser_url(&config).unwrap(), "http://127.0.0.1:8080");
        assert_ne!(identity(Path::new("/one"), &config), identity(Path::new("/two"), &config));
    }
}
