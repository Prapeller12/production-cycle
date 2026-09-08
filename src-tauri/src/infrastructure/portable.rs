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
pub struct Paths { pub frontend: PathBuf, pub runtime: PathBuf, pub webview: PathBuf, pub data: PathBuf, pub exports: PathBuf, pub smoke_report:PathBuf }
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
    pub fn load() -> Result<Self, String> {
        // ZIP layout: root/app/backend/production-cycle.exe; never use current working directory.
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let root = exe.parent().and_then(Path::parent).and_then(Path::parent).ok_or("Invalid application layout")?.canonicalize().map_err(|e| e.to_string())?;
        let c: Config = serde_json::from_slice(&fs::read(root.join("config/portable.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if c.technical_logging { return Err("Technical logging is not implemented in this prototype".into()); }
        let result = Self {frontend: inside(&root,&c.frontend)?, runtime: inside(&root,&c.webview_runtime)?,webview:inside(&root,&c.webview_data)?,data:inside(&root,&c.working_data)?,exports:inside(&root,&c.exports)?,smoke_report:inside(&root,"temp")?.join("self-test.json")};
        if !result.runtime.join("msedgewebview2.exe").is_file() { return Err("Missing bundled Fixed WebView2 Runtime. Extract the complete portable ZIP.".into()); }
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
        let temp = inside(&root,"temp/process")?;
        std::env::set_var("TEMP",&temp); std::env::set_var("TMP",&temp);
        std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",&result.runtime);
        Ok(result)
    }
}
