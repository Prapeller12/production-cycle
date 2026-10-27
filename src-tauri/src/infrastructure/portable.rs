use serde::Deserialize;
use std::{fs, path::{Path, PathBuf}};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub frontend: String,
    pub webview_runtime: String,
    pub webview_data: String,
    pub working_data: String,
    pub exports: String,
    pub technical_logging: bool,
}
pub struct Paths { pub frontend: PathBuf, pub runtime: PathBuf, pub webview: PathBuf, pub data: PathBuf, pub recovery:PathBuf, pub exports: PathBuf, pub smoke_report:PathBuf, process_temp:PathBuf }
fn inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let p = Path::new(relative);
    if p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) { return Err("Invalid portable path".into()); }
    let result = root.join(p);
    fs::create_dir_all(&result).map_err(|e| e.to_string())?;
    let real = result.canonicalize().map_err(|e| e.to_string())?;
    if !real.starts_with(root) { return Err("Portable path escapes program folder".into()); }
    Ok(real)
}
impl Paths {
    fn resolve(root:&Path,c:&Config,network_client:bool,recovery_override:Option<&Path>)->Result<Self,String>{
        // Recovery is supplied by the launcher outside its versioned cache. Each
        // network process has an independent SQLite connection target and
        // WebView profile, including concurrent launches of one Windows user.
        let recovery=if network_client{
            let path=recovery_override.ok_or_else(||"Launcher не передал локальную папку восстановления.".to_string())?;
            if !path.is_absolute(){return Err("Папка восстановления должна иметь абсолютный локальный путь.".into())}
            fs::create_dir_all(path).map_err(|e|e.to_string())?;
            let real=path.canonicalize().map_err(|e|e.to_string())?;
            if !super::network::is_local_disk(&real){return Err("Папка восстановления должна находиться на локальном диске.".into())}
            real
        }else{inside(root,&c.working_data)?};
        let working_root=if network_client{inside(root,&format!("temp/clients/{}-{}",std::process::id(),uuid::Uuid::new_v4()))?}else{root.to_path_buf()};
        Ok(Self{frontend:inside(root,&c.frontend)?,runtime:inside(root,&c.webview_runtime)?,webview:inside(&working_root,&c.webview_data)?,data:inside(&working_root,&c.working_data)?,recovery,exports:inside(root,&c.exports)?,smoke_report:inside(root,"temp")?.join("self-test.json"),process_temp:inside(&working_root,"temp/process")?})
    }
    pub fn load() -> Result<Self, String> {
        // Current ZIP layout: root/production-cycle.exe. The legacy app/backend layout
        // remains readable so an existing unpacked copy can still start after an update.
        // Never use the current working directory to resolve portable data.
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let direct = exe.parent().ok_or("Invalid application layout")?;
        let legacy = exe.parent().and_then(Path::parent).and_then(Path::parent);
        let root = if direct.join("config/portable.json").is_file() { direct } else { legacy.filter(|p| p.join("config/portable.json").is_file()).ok_or("Invalid application layout: config/portable.json was not found beside the program")? }.canonicalize().map_err(|e| e.to_string())?;
        let c: Config = serde_json::from_slice(&fs::read(root.join("config/portable.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if c.technical_logging { return Err("Technical logging is not implemented in this prototype".into()); }
        let network_client=std::env::var_os("PRODUCTION_CYCLE_NETWORK_CLIENT").is_some();
        let recovery=std::env::var_os("PRODUCTION_CYCLE_NETWORK_RECOVERY").map(PathBuf::from);
        let result=Self::resolve(&root,&c,network_client,recovery.as_deref())?;
        if !result.runtime.join("msedgewebview2.exe").is_file() { return Err("Missing bundled Fixed WebView2 Runtime. Extract the complete portable ZIP.".into()); }
        if network_client && [&result.runtime,&result.webview,&result.data,&result.recovery].iter().any(|path|!super::network::is_local_disk(path)) {return Err("Runtime, профиль или рабочие данные клиента оказались на сетевом диске. Запуск отменён: нужна локальная папка клиента.".into());}
        // Microsoft Fixed Runtime >=120 needs these runtime-folder rights on Windows 10.
        // No registry changes, elevation or sandbox disabling; only read/execute in this app folder.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let status=std::process::Command::new("icacls.exe").arg(&result.runtime)
                .args(["/grant", "*S-1-15-2-2:(OI)(CI)(RX)", "*S-1-15-2-1:(OI)(CI)(RX)","/Q"])
                .creation_flags(0x08000000).output().map_err(|e|e.to_string())?;
            if !status.status.success(){return Err("Cannot grant read/execute access to the bundled runtime. Use a local folder owned by your user.".into());}
        }
        std::env::set_var("TEMP",&result.process_temp); std::env::set_var("TMP",&result.process_temp);
        std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",&result.runtime);
        Ok(result)
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn config()->Config{Config{frontend:"frontend".into(),webview_runtime:"runtime/webview2".into(),webview_data:"temp/webview2".into(),working_data:"data".into(),exports:"exports".into(),technical_logging:false}}
    #[test]fn network_launches_share_assets_and_recovery_but_isolate_sqlite_and_profiles(){
        let root=std::env::temp_dir().join(format!("production-portable-paths-{}",uuid::Uuid::new_v4()));fs::create_dir_all(&root).unwrap();let root=root.canonicalize().unwrap();
        let stable=root.join("workspaces/share/data");
        let first=Paths::resolve(&root,&config(),true,Some(&stable)).unwrap();
        fs::create_dir_all(first.recovery.join("network-pending")).unwrap();fs::write(first.recovery.join("network-pending/draft.json"),b"saved draft").unwrap();
        let second=Paths::resolve(&root,&config(),true,Some(&stable)).unwrap();
        assert_ne!(first.data,second.data);assert_ne!(first.webview,second.webview);assert_ne!(first.process_temp,second.process_temp);
        assert_eq!(first.runtime,second.runtime);assert_eq!(first.frontend,second.frontend);assert_eq!(first.recovery,second.recovery);
        assert!(!first.recovery.starts_with(root.join("temp/clients")));
        assert_eq!(fs::read(second.recovery.join("network-pending/draft.json")).unwrap(),b"saved draft");
        let first_db=rusqlite::Connection::open(first.data.join("production-cycle.db")).unwrap();
        let second_db=rusqlite::Connection::open(second.data.join("production-cycle.db")).unwrap();
        first_db.execute_batch("CREATE TABLE probe(value TEXT);INSERT INTO probe VALUES('first');").unwrap();
        second_db.execute_batch("CREATE TABLE probe(value TEXT);INSERT INTO probe VALUES('second');").unwrap();
        assert_eq!(first_db.query_row("SELECT value FROM probe",[],|row|row.get::<_,String>(0)).unwrap(),"first");
        assert_eq!(second_db.query_row("SELECT value FROM probe",[],|row|row.get::<_,String>(0)).unwrap(),"second");
        drop((first_db,second_db));fs::remove_dir_all(root).unwrap();
    }
    #[test]fn standalone_paths_preserve_existing_data_and_webview_locations(){
        let root=std::env::temp_dir().join(format!("production-portable-standalone-{}",uuid::Uuid::new_v4()));fs::create_dir_all(&root).unwrap();let root=root.canonicalize().unwrap();
        let paths=Paths::resolve(&root,&config(),false,None).unwrap();
        assert_eq!(paths.data,root.join("data"));assert_eq!(paths.webview,root.join("temp/webview2"));assert_eq!(paths.recovery,paths.data);assert_eq!(paths.smoke_report,root.join("temp/self-test.json"));
        fs::remove_dir_all(root).unwrap();
    }
}
