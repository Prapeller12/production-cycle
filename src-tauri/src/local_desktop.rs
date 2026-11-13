//! Native EXE adapter. A configured shared-folder package uses the existing
//! admitted-session/snapshot transport; a standalone package keeps local data.
use std::{fs::OpenOptions, io::Write, sync::{Mutex, atomic::{AtomicBool, Ordering}}};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use tauri::State;
use crate::{audit::AuditUserSummary, infrastructure::{portable::Paths,network::NetworkRuntime}, production::ProductionDb};

#[derive(Default)]
pub struct LocalSession(Mutex<Option<AuditUserSummary>>, AtomicBool);

impl LocalSession {
    fn status(&self) -> Result<Value, String> {
        let user = self.0.lock().map_err(|_| "Сеанс занят")?;
        Ok(json!({"enabled":true,"mode":"local","active":user.is_some() || self.1.load(Ordering::SeqCst),"occupied":usize::from(user.is_some() || self.1.load(Ordering::SeqCst)),"maximum":1,"evictionRequested":false,"userId":user.as_ref().map(|u|u.id),"displayName":user.as_ref().map(|u|&u.display_name),"role":user.as_ref().map(|u|&u.role)}))
    }
    fn login(&self, db: &ProductionDb, user_id: Option<i64>, pin: &str) -> Result<Value, String> {
        if db.list_audit_users()?.is_empty() {
            if user_id.is_some() { return Err("Сначала создайте администратора.".into()); }
            self.1.store(true, Ordering::SeqCst);
            return Ok(json!({"state":"active","occupied":1,"maximum":1}));
        }
        let user = db.authorize_user(user_id.ok_or("Выберите пользователя.")?, pin)?;
        *self.0.lock().map_err(|_| "Сеанс занят")? = Some(user);
        self.1.store(false, Ordering::SeqCst);
        let mut result = self.status()?;
        result["state"] = json!("active");
        Ok(result)
    }
    fn user(&self) -> Result<AuditUserSummary, String> {
        self.0.lock().map_err(|_| "Сеанс занят")?.clone().ok_or_else(|| "Войдите под своим профилем.".into())
    }
}

#[tauri::command]
pub fn local_status(session: State<'_, LocalSession>, network:State<'_,NetworkRuntime>) -> Result<Value, String> {
    if !network.enabled(){return session.status();}
    let mut value=serde_json::to_value(network.status()?).map_err(|e|e.to_string())?;
    value["mode"]=json!("shared-folder");
    value["userId"]=if value["active"]==true{json!(session.0.lock().map_err(|_|"Сеанс занят")?.as_ref().map(|user|user.id))}else{Value::Null};
    Ok(value)
}

#[tauri::command]
pub fn local_login(user_id: Option<i64>, pin: String, db: State<'_, ProductionDb>, session: State<'_, LocalSession>,network:State<'_,NetworkRuntime>) -> Result<Value, String> {
    if !network.enabled(){return session.login(&db,user_id,&pin);}
    let user=network.with_read(&db,||{
        if db.list_audit_users()?.is_empty(){if user_id.is_some(){return Err("Сначала создайте администратора.".into());}Ok(None)}
        else{Ok(Some(db.authorize_user(user_id.ok_or("Выберите пользователя.")?,&pin)?))}
    })?;
    let admitted=network.login(user.clone())?;
    if admitted.state=="active"{*session.0.lock().map_err(|_|"Сеанс занят")?=user.clone();session.1.store(user.is_none(),Ordering::SeqCst);}
    let mut value=serde_json::to_value(admitted).map_err(|e|e.to_string())?;
    value["userId"]=json!(user.as_ref().map(|u|u.id));value["role"]=json!(user.as_ref().map(|u|&u.role));
    Ok(value)
}

#[tauri::command]
pub fn local_logout(session: State<'_, LocalSession>,network:State<'_,NetworkRuntime>) -> Result<(), String> {
    network.release()?;
    *session.0.lock().map_err(|_| "Сеанс занят")? = None;
    session.1.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn local_finish_eviction(app:tauri::AppHandle,session:State<'_,LocalSession>,network:State<'_,NetworkRuntime>)->Result<(),String>{
    network.release()?;*session.0.lock().map_err(|_|"Сеанс занят")?=None;session.1.store(false,Ordering::SeqCst);app.exit(0);Ok(())
}

#[tauri::command]
pub fn ui_claim_onboarding(db: State<'_, ProductionDb>, session: State<'_, LocalSession>,network:State<'_,NetworkRuntime>) -> Result<Value, String> {
    let user=session.user()?;
    network.with_write(&db,||Ok(json!({"show":crate::ui_preferences::claim_onboarding(&db,&user.key_fingerprint)?})))
}

#[tauri::command]
pub fn production_import_excel(file_name: String, base64: String, session: State<'_, LocalSession>,network:State<'_,NetworkRuntime>) -> Result<crate::excel_import::ImportResult, String> {
    network.require_active_session()?;session.user()?;
    std::panic::catch_unwind(|| crate::excel_import::parse(&base64, &file_name))
        .map_err(|_| "Не удалось прочитать Excel. Используйте образец программы.".to_string())?
}

#[tauri::command]
pub fn local_save_draft(snapshot: crate::production::ProductionSnapshot, paths: State<'_, Paths>, session: State<'_, LocalSession>,network:State<'_,NetworkRuntime>) -> Result<Value, String> {
    let user = session.user()?;
    if network.enabled(){return serde_json::to_value(network.save_draft(&snapshot)?).map_err(|e|e.to_string());}
    let folder = paths.data.join("drafts");
    std::fs::create_dir_all(&folder).map_err(|e|e.to_string())?;
    let saved_at = chrono::Utc::now().to_rfc3339();
    let value = json!({"format":"production-cycle-local-draft-v1","savedAt":saved_at,"userId":user.id,"snapshot":snapshot});
    crate::infrastructure::network::atomic_write(&folder.join(format!("user-{}.json",user.id)), &serde_json::to_vec_pretty(&value).map_err(|e|e.to_string())?)?;
    Ok(json!({"location":"local","savedAt":saved_at}))
}

#[tauri::command]
pub fn save_excel_template(paths: State<'_, Paths>) -> Result<String, String> {
    let template = crate::excel_import::template()?;
    let name = template["fileName"].as_str().ok_or("В комплекте нет образца Excel.")?;
    if name.contains(['/', '\\']) { return Err("Некорректное имя образца.".into()); }
    let bytes = STANDARD.decode(template["base64"].as_str().ok_or("Нет данных образца.")?).map_err(|e|e.to_string())?;
    for suffix in 0..10_000 {
        let name = if suffix == 0 { name.to_string() } else { format!("{} ({suffix}).xlsx",name.trim_end_matches(".xlsx")) };
        let path = paths.exports.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => { file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?; return Ok(path.to_string_lossy().into_owned()); }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Не удалось сохранить образец: {e}")),
        }
    }
    Err("Слишком много копий образца в папке exports.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_does_not_grant_an_identity_or_onboarding() {
        let db = ProductionDb::memory().unwrap();
        let session = LocalSession::default();
        assert_eq!(session.login(&db,None,"").unwrap()["state"], "active");
        assert!(session.user().is_err());
        assert_eq!(session.status().unwrap()["active"], true);
        assert!(session.login(&db,Some(1),"").is_err());
    }
}
