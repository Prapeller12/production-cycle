use chrono::{DateTime, Local};
use rusqlite::{backup::Progress, Connection, DatabaseName, OpenFlags};
use serde::Serialize;
use std::{fs, path::{Path, PathBuf}};
use tauri::State;
use uuid::Uuid;

use crate::production::{initialize_schema, ProductionDb, SCHEMA_VERSION};

const MAX_BACKUPS: usize = 10;
const BACKUP_PREFIX: &str = "production-cycle_";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub file_name: String,
    pub kind: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub project_count: usize,
    pub audit_event_count: usize,
    pub schema_version: i64,
    pub valid: bool,
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub restored_file_name: String,
    pub safety_backup_file_name: String,
    pub project_count: usize,
    pub audit_event_count: usize,
    pub integrity_check: String,
    pub schema_version: i64,
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

fn database_counts(conn: &Connection) -> Result<(usize, usize), String> {
    let projects = if table_exists(conn, "production_cycle")? {
        conn.query_row("SELECT COUNT(*) FROM production_cycle", [], |row| row.get::<_, i64>(0))
            .map_err(|error| error.to_string())? as usize
    } else {
        0
    };
    let events = if table_exists(conn, "audit_event")? {
        conn.query_row("SELECT COUNT(*) FROM audit_event", [], |row| row.get::<_, i64>(0))
            .map_err(|error| error.to_string())? as usize
    } else {
        0
    };
    Ok((projects, events))
}

fn validate_connection(conn: &Connection) -> Result<(usize, usize, i64, String), String> {
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if integrity != "ok" {
        return Err(format!("Проверка целостности SQLite: {integrity}"));
    }
    if !table_exists(conn, "production_cycle")? || !table_exists(conn, "production_stage")? {
        return Err("В копии отсутствуют таблицы производственного цикла.".into());
    }
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if version > SCHEMA_VERSION {
        return Err(format!("Копия создана более новой версией программы: схема {version}, поддерживается {SCHEMA_VERSION}."));
    }
    let (projects, events) = database_counts(conn)?;
    Ok((projects, events, version, integrity))
}

fn backup_kind(file_name: &str) -> String {
    file_name
        .strip_prefix(BACKUP_PREFIX)
        .and_then(|rest| rest.split('_').next())
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn inspect_backup(path: &Path) -> BackupSummary {
    let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default().to_string();
    let metadata = fs::metadata(path).ok();
    let created_at = metadata
        .as_ref()
        .and_then(|value| value.modified().ok())
        .map(|value| DateTime::<Local>::from(value).to_rfc3339())
        .unwrap_or_default();
    let mut summary = BackupSummary {
        kind: backup_kind(&file_name),
        file_name,
        created_at,
        size_bytes: metadata.as_ref().map(|value| value.len()).unwrap_or(0),
        project_count: 0,
        audit_event_count: 0,
        schema_version: 0,
        valid: false,
        validation_error: None,
    };
    let result = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())
        .and_then(|conn| validate_connection(&conn));
    match result {
        Ok((projects, events, version, _)) => {
            summary.project_count = projects;
            summary.audit_event_count = events;
            summary.schema_version = version;
            summary.valid = true;
        }
        Err(error) => summary.validation_error = Some(error),
    }
    summary
}

fn backup_paths(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(dir)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| {
            path.is_file()
                && path.extension().and_then(|value| value.to_str()) == Some("db")
                && path.file_name().and_then(|value| value.to_str()).map(|value| value.starts_with(BACKUP_PREFIX)).unwrap_or(false)
        })
        .collect::<Vec<_>>();
    paths.sort_by(|a, b| {
        let modified = |path: &PathBuf| fs::metadata(path).and_then(|value| value.modified()).ok();
        modified(b).cmp(&modified(a)).then_with(|| b.file_name().cmp(&a.file_name()))
    });
    Ok(paths)
}

fn prune_backups(dir: &Path) -> Result<(), String> {
    for path in backup_paths(dir)?.into_iter().skip(MAX_BACKUPS) {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn create_backup_from_connection(
    conn: &Connection,
    dir: &Path,
    kind: &str,
    prune: bool,
) -> Result<BackupSummary, String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let safe_kind = kind.chars().filter(|value| value.is_ascii_alphanumeric() || *value == '-').collect::<String>();
    let stamp = Local::now().format("%Y%m%dT%H%M%S%3f");
    let suffix = &Uuid::new_v4().simple().to_string()[..8];
    let file_name = format!("{BACKUP_PREFIX}{safe_kind}_{stamp}_{suffix}.db");
    let path = dir.join(&file_name);
    conn.backup(DatabaseName::Main, &path, None).map_err(|error| error.to_string())?;
    let summary = inspect_backup(&path);
    if !summary.valid {
        let _ = fs::remove_file(&path);
        return Err(summary.validation_error.unwrap_or_else(|| "Созданная копия не прошла проверку.".into()));
    }
    if prune {
        prune_backups(dir)?;
    }
    Ok(summary)
}

fn checked_backup_path(dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(file_name);
    if candidate.components().count() != 1
        || candidate.file_name().and_then(|value| value.to_str()) != Some(file_name)
        || !file_name.starts_with(BACKUP_PREFIX)
        || !file_name.ends_with(".db")
    {
        return Err("Недопустимое имя резервной копии.".into());
    }
    let path = dir.join(candidate);
    if !path.is_file() {
        return Err("Резервная копия не найдена.".into());
    }
    Ok(path)
}

impl ProductionDb {
    pub fn create_backup(&self, kind: &str) -> Result<BackupSummary, String> {
        let dir = self.backup_dir.as_deref().ok_or_else(|| "Резервные копии недоступны для временной базы.".to_string())?;
        let conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        create_backup_from_connection(&conn, dir, kind, true)
    }

    pub fn create_backup_as_admin(&self, admin_user_id: i64, admin_pin: &str) -> Result<BackupSummary, String> {
        let dir = self.backup_dir.as_deref().ok_or_else(|| "Папка резервных копий недоступна.".to_string())?;
        let conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        crate::audit::authenticate_admin(&conn, admin_user_id, admin_pin)?;
        create_backup_from_connection(&conn, dir, "manual", true)
    }

    pub fn list_backups(&self) -> Result<Vec<BackupSummary>, String> {
        let dir = self.backup_dir.as_deref().ok_or_else(|| "Папка резервных копий недоступна.".to_string())?;
        backup_paths(dir).map(|paths| paths.into_iter().map(|path| inspect_backup(&path)).collect())
    }

    pub fn restore_backup_as_admin(
        &self,
        file_name: &str,
        admin_user_id: i64,
        admin_pin: &str,
    ) -> Result<RestoreReport, String> {
        let dir = self.backup_dir.as_deref().ok_or_else(|| "Папка резервных копий недоступна.".to_string())?;
        let source = checked_backup_path(dir, file_name)?;
        let source_summary = inspect_backup(&source);
        if !source_summary.valid {
            return Err(source_summary.validation_error.unwrap_or_else(|| "Копия не прошла проверку.".into()));
        }
        let mut conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        crate::audit::authenticate_admin(&conn, admin_user_id, admin_pin)?;
        let safety = create_backup_from_connection(&conn, dir, "before-restore", false)?;
        let restore_result = (|| -> Result<(usize, usize, i64, String), String> {
            conn.restore(DatabaseName::Main, &source, None::<fn(Progress)>).map_err(|error| error.to_string())?;
            initialize_schema(&conn).map_err(|error| error.to_string())?;
            validate_connection(&conn)
        })();
        let (projects, events, version, integrity) = match restore_result {
            Ok(value) => value,
            Err(error) => {
                let rollback = conn.restore(DatabaseName::Main, dir.join(&safety.file_name), None::<fn(Progress)>)
                    .map_err(|value| value.to_string())
                    .and_then(|_| initialize_schema(&conn).map_err(|value| value.to_string()));
                return Err(match rollback {
                    Ok(_) => format!("Восстановление отменено, рабочая база возвращена в исходное состояние: {error}"),
                    Err(rollback_error) => format!("Критическая ошибка восстановления: {error}. Не удалось автоматически вернуть рабочую базу: {rollback_error}"),
                });
            }
        };
        prune_backups(dir)?;
        Ok(RestoreReport {
            restored_file_name: file_name.to_string(),
            safety_backup_file_name: safety.file_name,
            project_count: projects,
            audit_event_count: events,
            integrity_check: integrity,
            schema_version: version,
        })
    }
}

#[tauri::command]
pub fn backup_list(db: State<'_, ProductionDb>) -> Result<Vec<BackupSummary>, String> {
    db.list_backups()
}

#[tauri::command]
pub fn backup_create(admin_user_id: i64, admin_pin: String, db: State<'_, ProductionDb>) -> Result<BackupSummary, String> {
    db.create_backup_as_admin(admin_user_id, &admin_pin)
}

#[tauri::command]
pub fn backup_restore(file_name: String, admin_user_id: i64, admin_pin: String, db: State<'_, ProductionDb>) -> Result<RestoreReport, String> {
    db.restore_backup_as_admin(&file_name, admin_user_id, &admin_pin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::CreateAuditUserInput;
    use crate::production::{ProductionProjectInput, ProductionSnapshot, ProductionStageInput, StageStatus};

    fn temporary_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("production-cycle-{label}-{}", Uuid::new_v4()))
    }

    fn snapshot(name: &str) -> ProductionSnapshot {
        ProductionSnapshot {
            database_id: None,
            project: ProductionProjectInput { name:name.into(),order_no:"BACKUP-1".into(),initiator:"Инициатор".into(),executor:"Исполнитель".into(),enterprise:"Предприятие".into(),start:"2026-09-01".into(),deadline:"2026-09-30".into() },
            stages: vec![ProductionStageInput { uid:"stage-1".into(),seq:1,sort:1,parent_uid:None,title:"Контроль".into(),executor:"Исполнитель".into(),addressees:"Предприятие".into(),start:"2026-09-01".into(),deadline:"2026-09-20".into(),status:StageStatus::Work,comment:String::new() }],
            next_seq: 2,
        }
    }

    #[test]
    fn verified_backup_can_restore_previous_signed_state() {
        let dir=temporary_directory("restore");fs::create_dir_all(&dir).unwrap();
        let db=ProductionDb::open(&dir.join("production-cycle.db")).unwrap();
        let owner=db.create_audit_user(CreateAuditUserInput{display_name:"Владелец".into(),pin:"739201".into(),is_admin:false,admin_user_id:None,admin_pin:None}).unwrap();
        let mut initial=snapshot("До изменения");
        let first=db.save_signed_snapshot(&initial,owner.id,"739201","Первое сохранение",None).unwrap();
        let first_backup=first.backup.unwrap();assert!(first_backup.valid);
        initial.database_id=Some(first.project_id);initial.project.name="После изменения".into();
        db.save_signed_snapshot(&initial,owner.id,"739201","Изменение названия",None).unwrap();
        assert_eq!(db.load_snapshot_by_id(first.project_id).unwrap().unwrap().project.name,"После изменения");
        let restored=db.restore_backup_as_admin(&first_backup.file_name,owner.id,"739201").unwrap();
        assert_eq!(restored.integrity_check,"ok");assert_eq!(restored.project_count,1);assert_eq!(restored.audit_event_count,1);
        assert_eq!(db.load_snapshot_by_id(first.project_id).unwrap().unwrap().project.name,"До изменения");
        assert!(db.list_backups().unwrap().iter().any(|item|item.kind=="before-restore"&&item.valid));
        for _ in 0..12 { db.create_backup_as_admin(owner.id,"739201").unwrap(); }
        assert_eq!(db.list_backups().unwrap().len(),MAX_BACKUPS);
        drop(db);fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn existing_database_is_backed_up_and_migrated_without_data_loss() {
        let dir=temporary_directory("migration");fs::create_dir_all(&dir).unwrap();let path=dir.join("production-cycle.db");
        {
            let conn=Connection::open(&path).unwrap();initialize_schema(&conn).unwrap();
            conn.execute("INSERT INTO production_cycle(order_no,root_reg_number,name,initiator,executor,enterprise,start_date,deadline,created_at,updated_at) VALUES('OLD-7','OLD-7','Сохранённый проект','И','И','П','2026-09-01','2026-09-30','2026-09-01','2026-09-01')",[]).unwrap();
            conn.execute_batch("ALTER TABLE audit_event DROP COLUMN changes_json; PRAGMA user_version=2;").unwrap();
        }
        let db=ProductionDb::open(&path).unwrap();
        assert_eq!(db.load_snapshot("OLD-7").unwrap().unwrap().project.name,"Сохранённый проект");
        let conn=db.conn.lock().unwrap();let columns=conn.prepare("PRAGMA table_info(audit_event)").unwrap().query_map([],|row|row.get::<_,String>(1)).unwrap().collect::<std::result::Result<Vec<_>,_>>().unwrap();drop(conn);
        assert!(columns.iter().any(|name|name=="changes_json"));
        let copies=db.list_backups().unwrap();assert!(copies.iter().any(|item|item.kind=="before-migration"&&item.valid&&item.project_count==1));
        drop(db);fs::remove_dir_all(dir).unwrap();
    }
}
