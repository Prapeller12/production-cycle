use tauri::{State,WebviewWindow};
use crate::infrastructure::{files,portable::Paths};
#[tauri::command]
pub fn save_file(name:String, text:String, paths:State<'_,Paths>) -> Result<String,String> {
    let folder=if name.ends_with(".json") {
        let value:serde_json::Value=serde_json::from_str(&text).map_err(|e|e.to_string())?;
        if value.get("format").and_then(|v|v.as_str())!=Some("production-cycle-builder-v8") {return Err("Unknown project format".into());}
        &paths.data
    } else {&paths.exports};
    files::save_new(folder,&name,&text).map(|p|p.to_string_lossy().into_owned())
}
#[tauri::command]
pub fn print_report(window:WebviewWindow)->Result<(),String>{window.print().map_err(|e|e.to_string())}
