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

/// Read-only statistics exports go to exports; this never changes the project DB.
#[tauri::command]
pub fn save_statistics_file(name:String, text:String, paths:State<'_,Paths>)->Result<String,String>{
    let body=text.trim_start_matches('\u{feff}');
    if name.ends_with(".json") {
        let value:serde_json::Value=serde_json::from_str(body).map_err(|e|e.to_string())?;
        let settings_type=value.get("type").and_then(|v|v.as_str())==Some("production-cycle-dashboard-settings");
        let settings=settings_type && match value.get("version").and_then(|v|v.as_u64()) {
            Some(1)=>true,
            Some(2)=>{
                let mut settings=value.get("settings").unwrap_or(&value).clone();
                if let Some(object)=settings.as_object_mut(){object.remove("type");}
                crate::statistics_preferences::validate_settings(&settings).is_ok()
            },
            _=>false,
        };
        let dataset=value.get("schemaVersion").and_then(|v|v.as_u64())==Some(1)
            && value.get("projects").is_some_and(|v|v.is_array());
        let archive=value.get("type").and_then(|v|v.as_str())==Some("production-cycle-dashboard-settings-archive")
            && value.get("version").and_then(|v|v.as_u64())==Some(1)
            && value.get("archive").is_some_and(|v|!v.is_null() && crate::statistics_preferences::validate_archive(v).is_ok());
        if !settings&&!dataset&&!archive{return Err("Неизвестный формат экспорта статистики".into());}
    }else if !name.ends_with(".csv") {return Err("Статистика экспортируется только в CSV или JSON".into());}
    files::save_new(&paths.exports,&name,body).map(|p|p.to_string_lossy().into_owned())
}
