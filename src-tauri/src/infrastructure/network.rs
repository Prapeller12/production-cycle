use chrono::Local;
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{audit::AuditUserSummary, production::{ProductionDb, ProductionSnapshot, SCHEMA_VERSION}};

const NETWORK_CLIENT_ENV: &str = "PRODUCTION_CYCLE_NETWORK_CLIENT";
const NETWORK_ROOT_ENV: &str = "PRODUCTION_CYCLE_SHARED_ROOT";
const DEFAULT_MAX_CLIENTS: usize = 3;
const MAX_SESSION_METADATA_BYTES: u64 = 16 * 1024;
const MAX_PENDING_DRAFT_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkConfig {
    #[serde(default)]
    enabled: bool,
    version: String,
    #[serde(default = "default_max_clients")]
    max_clients: usize,
}

fn default_max_clients() -> usize { DEFAULT_MAX_CLIENTS }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub enabled: bool,
    pub active: bool,
    pub occupied: usize,
    pub maximum: usize,
    pub eviction_requested: bool,
    pub role: Option<String>,
    pub display_name: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkLoginResult {
    pub state: String,
    pub occupied: usize,
    pub maximum: usize,
    pub evicted_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftSaveResult {
    pub location: String,
    pub saved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionMetadata {
    session_id: String,
    slot: usize,
    user_id: Option<i64>,
    role: String,
    started_at: String,
    heartbeat_at: String,
    computer_fingerprint: String,
    client_version: String,
}

struct ActiveSession {
    metadata: SessionMetadata,
    display_name: String,
    slot_file: File,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DraftEnvelope<'a> {
    format: &'static str,
    saved_at: String,
    session_id: &'a str,
    user_id: Option<i64>,
    content_sha256: String,
    snapshot: &'a ProductionSnapshot,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingDraftEnvelope {
    format: String,
    saved_at: String,
    session_id: String,
    user_id: Option<i64>,
    content_sha256: String,
    snapshot: ProductionSnapshot,
}

pub struct NetworkRuntime {
    enabled: bool,
    version: String,
    maximum: usize,
    shared_root: Option<PathBuf>,
    local_data: PathBuf,
    recovery_data: PathBuf,
    session: Mutex<Option<ActiveSession>>,
}

impl NetworkRuntime {
    pub fn from_environment(local_data: PathBuf,recovery_data: PathBuf) -> Result<Self, String> {
        let Some(shared) = env::var_os(NETWORK_ROOT_ENV) else {
            return Ok(Self { enabled:false, version:String::new(), maximum:DEFAULT_MAX_CLIENTS, shared_root:None, local_data, recovery_data, session:Mutex::new(None) });
        };
        let shared_root=PathBuf::from(shared).canonicalize().map_err(|e|format!("Сетевая папка недоступна: {e}"))?;
        let config=read_network_config(&shared_root.join("config/network.json"))?;
        if !config.enabled{return Err("Сетевой режим отключён в config/network.json".into())}
        if config.max_clients!=DEFAULT_MAX_CLIENTS{return Err("Тестовая сетевая версия поддерживает ровно 3 одновременных компьютера.".into())}
        for relative in ["shared/database","shared/sessions","shared/control","shared/drafts","shared/backups","shared/exports"]{
            fs::create_dir_all(shared_root.join(relative)).map_err(|e|format!("Не удалось подготовить {relative}: {e}"))?;
        }
        fs::create_dir_all(recovery_data.join("network-pending")).map_err(|e|e.to_string())?;
        Ok(Self {enabled:true,version:config.version,maximum:config.max_clients,shared_root:Some(shared_root),local_data,recovery_data,session:Mutex::new(None)})
    }

    pub fn enabled(&self)->bool{self.enabled}
    pub fn shared_backups(&self)->Option<PathBuf>{self.shared_root.as_ref().map(|root|root.join("shared/backups"))}
    fn root(&self)->Result<&Path,String>{self.shared_root.as_deref().ok_or_else(||"Сетевой режим не включён.".to_string())}
    fn shared_database(&self)->Result<PathBuf,String>{Ok(self.root()?.join("shared/database/production-cycle.db"))}
    fn store_lock_path(&self)->Result<PathBuf,String>{Ok(self.root()?.join("shared/database/store.lock"))}
    fn store_was_initialized(&self)->Result<bool,String>{
        self.root()?.join("shared/database/initialized.json").try_exists().map_err(|e|format!("Не удалось проверить метку общей базы: {e}"))
    }
    // Call only while holding store.lock and after validating/publishing the DB.
    fn mark_store_initialized(&self)->Result<(),String>{
        if self.store_was_initialized()?{return Ok(())}
        atomic_write(&self.root()?.join("shared/database/initialized.json"),
            &serde_json::to_vec_pretty(&serde_json::json!({"format":"production-cycle-shared-store-v1","initializedAt":Local::now().to_rfc3339()})).map_err(|e|e.to_string())?)
    }

    pub fn prepare_local_database(&self,local_db:&Path)->Result<(),String>{
        if !self.enabled{return Ok(())}
        let lock=self.acquire_store_lock()?;
        let shared=self.shared_database()?;
        if !shared.is_file(){
            if self.store_was_initialized()?{return Err("Ранее созданная общая база недоступна или удалена. Автоматическое создание пустой базы запрещено; восстановите общий снимок из резервной копии.".into())}
            let legacy=self.root()?.join("data/production-cycle.db");
            if legacy.is_file(){copy_checked_database(&legacy,&shared)?;}
        }
        if shared.is_file(){copy_checked_database(&shared,local_db)?;self.mark_store_initialized()?;}
        FileExt::unlock(&lock).map_err(|e|e.to_string())?;
        Ok(())
    }

    pub fn ensure_shared_database(&self,db:&ProductionDb)->Result<(),String>{
        if !self.enabled{return Ok(())}
        let lock=self.acquire_store_lock()?;
        if !self.shared_database()?.is_file(){
            if self.store_was_initialized()?{return Err("Ранее созданная общая база недоступна или удалена. Восстановите общий снимок из резервной копии.".into())}
            if self.occupied_count()?>0{return Err("Общий снимок базы отсутствует, хотя другие сетевые сеансы ещё активны. Восстановите общую базу из резервной копии.".into())}
            self.publish_locked(db)?;
        }else{validate_database(&self.shared_database()?)?;}
        self.mark_store_initialized()?;
        FileExt::unlock(&lock).map_err(|e|e.to_string())?;
        Ok(())
    }

    pub fn with_read<T>(&self,db:&ProductionDb,operation:impl FnOnce()->Result<T,String>)->Result<T,String>{
        if !self.enabled{return operation()}
        let lock=self.acquire_store_lock()?;
        self.refresh_locked(db)?;
        let result=operation();
        FileExt::unlock(&lock).map_err(|e|e.to_string())?;
        result
    }

    pub fn with_write<T>(&self,db:&ProductionDb,operation:impl FnOnce()->Result<T,String>)->Result<T,String>{
        if !self.enabled{return operation()}
        let lock=self.acquire_store_lock()?;
        // An admitted write completes before logout can release its slot.
        let _session_guard=self.active_session_guard()?;
        self.refresh_locked(db)?;
        let result=operation();
        if result.is_ok(){
            if let Err(error)=self.publish_locked(db){
                return Err(match self.preserve_unpublished_database(db){
                    Ok(path)=>format!("Общая запись не подтверждена: {error}. Изменения сохранены только в локальной копии для ручного восстановления: {}. Автоматическое повторное применение не выполняется.",path.display()),
                    Err(recovery_error)=>format!("Общая запись не подтверждена: {error}. Не удалось сохранить копию для восстановления: {recovery_error}. Локальная база остаётся в {}; не выполняйте обновление данных до её сохранения.",self.local_data.join("production-cycle.db").display()),
                })
            }
        }
        FileExt::unlock(&lock).map_err(|e|e.to_string())?;
        result
    }

    fn acquire_store_lock(&self)->Result<File,String>{
        let path=self.store_lock_path()?;
        let file=OpenOptions::new().create(true).read(true).write(true).open(path).map_err(|e|e.to_string())?;
        let started=Instant::now();
        loop{
            match file.try_lock_exclusive(){
                Ok(())=>return Ok(file),
                Err(error) if started.elapsed()<Duration::from_secs(15)=>{let _=error;thread::sleep(Duration::from_millis(75));}
                Err(_)=>return Err("Общая база занята другим компьютером более 15 секунд. Повторите действие.".into()),
            }
        }
    }

    fn refresh_locked(&self,db:&ProductionDb)->Result<(),String>{
        let shared=self.shared_database()?;
        if !shared.is_file(){return Err("Общий снимок базы недоступен или удалён. Сохранение остановлено: восстановите общую базу или соединение с сетевой папкой.".into())}
        let temp=self.local_data.join(format!("network-incoming-{}.db",Uuid::new_v4()));
        copy_checked_database(&shared,&temp)?;
        let result=db.restore_database(&temp).map_err(|e|e.to_string());
        let _=fs::remove_file(temp);
        result
    }

    fn publish_locked(&self,db:&ProductionDb)->Result<(),String>{
        let local=self.local_data.join(format!("network-outgoing-{}.db",Uuid::new_v4()));
        db.export_database(&local).map_err(|e|e.to_string())?;
        validate_database(&local)?;
        let target=self.shared_database()?;
        let pending=target.with_extension(format!("pending-{}",Uuid::new_v4().simple()));
        fs::copy(&local,&pending).map_err(|e|format!("Не удалось передать базу в общую папку: {e}"))?;
        OpenOptions::new().read(true).write(true).open(&pending).and_then(|file|file.sync_all()).map_err(|e|format!("Не удалось подтвердить запись общей базы: {e}"))?;
        let replace=atomic_replace(&pending,&target);
        let _=fs::remove_file(&local);
        if replace.is_err(){let _=fs::remove_file(&pending);}
        replace
    }

    fn preserve_unpublished_database(&self,db:&ProductionDb)->Result<PathBuf,String>{
        let directory=self.recovery_data.join("network-unsent");fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
        let path=directory.join(format!("unsent-{}.db",Uuid::new_v4()));
        db.export_database(&path).map_err(|e|e.to_string())?;
        validate_database(&path)?;
        OpenOptions::new().read(true).write(true).open(&path).and_then(|file|file.sync_all()).map_err(|e|e.to_string())?;
        Ok(path)
    }

    pub fn login(&self,user:Option<AuditUserSummary>)->Result<NetworkLoginResult,String>{
        if !self.enabled{return Ok(NetworkLoginResult{state:"standalone".into(),occupied:1,maximum:1,evicted_display_name:None})}
        // One runtime must own at most one slot, including concurrent IPC calls.
        // Keep the lifecycle mutex until admission or release is complete.
        let mut guard=self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?;
        if guard.as_ref().map(|active|self.session_is_current(active)).transpose()?==Some(false){
            self.release_session(&mut guard)?;
        }
        if let Some(active)=guard.as_mut(){
            if let (Some(current),Some(authenticated))=(active.metadata.user_id,user.as_ref()){
                if current!=authenticated.id{return Err("Сначала завершите текущий сетевой сеанс, затем выберите другого пользователя.".into())}
            }
            // Bind the initial setup slot to the newly authenticated administrator.
            if active.metadata.user_id.is_none(){
                if let Some(ref authenticated)=user{
                    active.metadata.user_id=Some(authenticated.id);
                    active.metadata.role=authenticated.role.clone();
                    active.display_name=authenticated.display_name.clone();
                    self.write_metadata(&active.metadata)?;
                }
            }
            return Ok(NetworkLoginResult{state:"active".into(),occupied:self.occupied_count()?,maximum:self.maximum,evicted_display_name:None});
        }
        let (user_id,display_name,role)=match user{
            Some(value)=>(Some(value.id),value.display_name,value.role),
            None=>(None,"Первичная настройка".into(),"admin".into()),
        };
        if let Some(active)=self.try_acquire_slot(user_id,&display_name,&role)?{
            *guard=Some(active);
            return Ok(NetworkLoginResult{state:"active".into(),occupied:self.occupied_count()?.max(1),maximum:self.maximum,evicted_display_name:None});
        }
        if role!="admin"{
            return Ok(NetworkLoginResult{state:"full".into(),occupied:self.maximum,maximum:self.maximum,evicted_display_name:None});
        }
        let victim=self.newest_non_admin_session()?;
        let Some(victim)=victim else{return Ok(NetworkLoginResult{state:"full".into(),occupied:self.maximum,maximum:self.maximum,evicted_display_name:None})};
        let request=self.root()?.join("shared/control").join(format!("{}.evict",victim.session_id));
        atomic_write(&request,serde_json::to_vec_pretty(&serde_json::json!({"requestedAt":Local::now().to_rfc3339(),"reason":"Вход администратора при занятых 3 из 3 слотах"})).map_err(|e|e.to_string())?.as_slice())?;
        let started=Instant::now();
        while started.elapsed()<Duration::from_secs(15){
            if let Some(active)=self.try_acquire_slot(user_id,&display_name,&role)?{
                *guard=Some(active);
                return Ok(NetworkLoginResult{state:"active".into(),occupied:self.occupied_count()?.max(1),maximum:self.maximum,evicted_display_name:Some("последний вошедший пользователь".into())});
            }
            thread::sleep(Duration::from_millis(250));
        }
        Ok(NetworkLoginResult{state:"waitingForEviction".into(),occupied:self.maximum,maximum:self.maximum,evicted_display_name:Some("последний вошедший пользователь".into())})
    }

    fn try_acquire_slot(&self,user_id:Option<i64>,display_name:&str,role:&str)->Result<Option<ActiveSession>,String>{
        let _presence_guard=self.acquire_presence_lock()?;
        let sessions=self.root()?.join("shared/sessions");
        for slot in 1..=self.maximum{
            let path=sessions.join(format!("slot-{slot}.lock"));
            let file=OpenOptions::new().create(true).read(true).write(true).open(path).map_err(|e|e.to_string())?;
            match file.try_lock_exclusive(){
                Ok(())=>{},
                Err(error) if error.raw_os_error()==fs2::lock_contended_error().raw_os_error()=>continue,
                Err(error)=>return Err(format!("Не удалось занять сетевое место: {error}")),
            }
            self.remove_metadata_for_slot(slot)?;
            let now=Local::now().to_rfc3339();
            let metadata=SessionMetadata{session_id:Uuid::new_v4().to_string(),slot,user_id,role:role.into(),started_at:now.clone(),heartbeat_at:now,computer_fingerprint:computer_fingerprint(),client_version:self.version.clone()};
            self.write_metadata(&metadata)?;
            return Ok(Some(ActiveSession{metadata,display_name:display_name.into(),slot_file:file}));
        }
        Ok(None)
    }

    fn write_metadata(&self,metadata:&SessionMetadata)->Result<(),String>{
        let path=self.root()?.join("shared/sessions").join(format!("session-{}.json",metadata.session_id));
        atomic_write(&path,&serde_json::to_vec_pretty(metadata).map_err(|e|e.to_string())?)
    }

    fn remove_metadata_for_slot(&self,slot:usize)->Result<(),String>{
        for (path,metadata) in self.session_metadata()?{if metadata.slot==slot{
            fs::remove_file(path).map_err(|e|e.to_string())?;
            let control=self.root()?.join("shared/control").join(format!("{}.evict",metadata.session_id));
            match fs::remove_file(control){Ok(())=>{},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(e.to_string())}
        }}
        Ok(())
    }

    fn session_metadata(&self)->Result<Vec<(PathBuf,SessionMetadata)>,String>{
        let dir=self.root()?.join("shared/sessions");
        let mut result=Vec::new();
        for entry in fs::read_dir(dir).map_err(|e|e.to_string())?{
            let path=entry.map_err(|e|e.to_string())?.path();
            if path.extension().and_then(|v|v.to_str())!=Some("json"){continue}
            // The shared directory is writable by clients. Do not trust IDs from
            // arbitrary JSON as file paths, or allocate unbounded metadata.
            let Ok(file)=File::open(&path) else{continue};
            let mut bytes=Vec::new();
            if file.take(MAX_SESSION_METADATA_BYTES+1).read_to_end(&mut bytes).is_err()
                || bytes.len() as u64>MAX_SESSION_METADATA_BYTES{continue}
            let Ok(value)=serde_json::from_slice::<SessionMetadata>(&bytes) else{continue};
            let Ok(id)=Uuid::parse_str(&value.session_id) else{continue};
            if value.slot==0 || value.slot>self.maximum
                || value.session_id!=id.hyphenated().to_string()
                || path.file_name().and_then(|name|name.to_str())!=Some(format!("session-{}.json",value.session_id).as_str()){continue}
            result.push((path,value));
        }
        Ok(result)
    }

    // Metadata can survive a crash. Only the OS/SMB lock proves that a slot
    // is occupied; an old heartbeat must never evict a still-running client.
    fn acquire_presence_lock(&self)->Result<File,String>{
        // Serialize probes and admission: a probe's short-lived slot lock
        // must never be mistaken for an actual client by another process.
        let file=OpenOptions::new().create(true).read(true).write(true)
            .open(self.root()?.join("shared/sessions/presence.lock")).map_err(|e|e.to_string())?;
        let started=Instant::now();
        loop{match file.try_lock_exclusive(){
            Ok(())=>return Ok(file),
            Err(error) if error.raw_os_error()==fs2::lock_contended_error().raw_os_error() && started.elapsed()<Duration::from_secs(5)=>thread::sleep(Duration::from_millis(10)),
            Err(error)=>return Err(format!("Не удалось проверить сетевые сеансы. Повторите попытку: {error}")),
        }}
    }

    fn occupied_slots(&self)->Result<(usize,Vec<SessionMetadata>),String>{
        let _presence_guard=self.acquire_presence_lock()?;
        let sessions=self.root()?.join("shared/sessions");
        let mut occupied=0;
        let mut active=Vec::new();
        for slot in 1..=self.maximum{
            let file=OpenOptions::new().create(true).read(true).write(true)
                .open(sessions.join(format!("slot-{slot}.lock"))).map_err(|e|e.to_string())?;
            match file.try_lock_exclusive(){
                Ok(())=>{
                    // Keep the slot locked throughout cleanup, so a new login
                    // cannot publish metadata that this probe would remove.
                    self.remove_metadata_for_slot(slot)?;
                    FileExt::unlock(&file).map_err(|e|e.to_string())?;
                }
                Err(error) if error.raw_os_error()==fs2::lock_contended_error().raw_os_error()=>{
                    occupied+=1;
                    if let Some((_,metadata))=self.session_metadata()?.into_iter()
                        .filter(|(_,metadata)|metadata.slot==slot)
                        .max_by(|(_,a),(_,b)|a.started_at.cmp(&b.started_at)){
                        active.push(metadata);
                    }
                }
                Err(error)=>return Err(format!("Не удалось проверить занятые сетевые места: {error}")),
            }
        }
        Ok((occupied,active))
    }

    fn newest_non_admin_session(&self)->Result<Option<SessionMetadata>,String>{
        Ok(self.occupied_slots()?.1.into_iter().filter(|value|value.role!="admin").max_by(|a,b|a.started_at.cmp(&b.started_at)))
    }

    fn occupied_count(&self)->Result<usize,String>{Ok(self.occupied_slots()?.0)}

    // A local handle alone is not enough after an SMB reconnect. Before writing
    // or renewing metadata, verify that the slot is still locked and its identity
    // has not been removed/replaced by another client. Lock order is store (when
    // used) -> session mutex -> presence; presence never acquires the store lock.
    fn session_is_current(&self,active:&ActiveSession)->Result<bool,String>{
        let _presence_guard=self.acquire_presence_lock()?;
        let file=OpenOptions::new().create(true).read(true).write(true)
            .open(self.root()?.join("shared/sessions").join(format!("slot-{}.lock",active.metadata.slot)))
            .map_err(|e|e.to_string())?;
        match file.try_lock_exclusive(){
            Ok(())=>{FileExt::unlock(&file).map_err(|e|e.to_string())?;return Ok(false)},
            Err(error) if error.raw_os_error()==fs2::lock_contended_error().raw_os_error()=>{},
            Err(error)=>return Err(format!("Не удалось подтвердить сетевой сеанс: {error}")),
        }
        let metadata=self.session_metadata()?.into_iter().filter(|(_,value)|value.slot==active.metadata.slot)
            .map(|(_,value)|value).collect::<Vec<_>>();
        Ok(metadata.len()==1 && metadata[0].session_id==active.metadata.session_id
            && metadata[0].user_id==active.metadata.user_id && metadata[0].role==active.metadata.role)
    }

    pub fn require_active_session(&self)->Result<(),String>{
        if !self.enabled{return Ok(())}
        self.active_session_guard().map(|_|())
    }

    fn active_session_guard(&self)->Result<MutexGuard<'_,Option<ActiveSession>>,String>{
        let mut guard=self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?;
        if let Some(active)=guard.as_ref(){
            if self.session_is_current(active)?{
                if self.root()?.join("shared/control").join(format!("{}.evict",active.metadata.session_id)).is_file(){
                    return Err("Получен запрос завершения сетевого сеанса. Сохраните черновик и войдите повторно.".into())
                }
                return Ok(guard)
            }
            self.release_session(&mut guard)?;
        }
        Err("Сетевой сеанс завершён или его место потеряно. Войдите повторно перед сохранением.".into())
    }

    pub fn status(&self)->Result<NetworkStatus,String>{
        if !self.enabled{return Ok(NetworkStatus{enabled:false,active:true,occupied:1,maximum:1,eviction_requested:false,role:None,display_name:None,session_id:None})}
        self.flush_pending_drafts();
        let mut guard=self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?;
        if guard.as_ref().map(|active|self.session_is_current(active)).transpose()?==Some(false){
            self.release_session(&mut guard)?;
        }
        let Some(active)=guard.as_mut() else{return Ok(NetworkStatus{enabled:true,active:false,occupied:self.occupied_count()?,maximum:self.maximum,eviction_requested:false,role:None,display_name:None,session_id:None})};
        active.metadata.heartbeat_at=Local::now().to_rfc3339();
        self.write_metadata(&active.metadata)?;
        let eviction=self.root()?.join("shared/control").join(format!("{}.evict",active.metadata.session_id)).is_file();
        Ok(NetworkStatus{enabled:true,active:true,occupied:self.occupied_count()?.max(1),maximum:self.maximum,eviction_requested:eviction,role:Some(active.metadata.role.clone()),display_name:Some(active.display_name.clone()),session_id:Some(active.metadata.session_id.clone())})
    }

    pub fn save_draft(&self,snapshot:&ProductionSnapshot)->Result<DraftSaveResult,String>{
        if !self.enabled{return Ok(DraftSaveResult{location:"standalone".into(),saved_at:Local::now().to_rfc3339()})}
        let guard=self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?;
        let active=guard.as_ref().ok_or_else(||"Сначала войдите в сетевую сессию.".to_string())?;
        let verified=match self.session_is_current(active){
            Ok(true)=>true,
            Ok(false)=>return Err("Место сетевого сеанса потеряно. Войдите повторно перед сохранением черновика.".into()),
            // A temporary SMB failure must still allow a local recovery draft,
            // but must not write to the share with an unverified session.
            Err(_)=>false,
        };
        let saved_at=Local::now().to_rfc3339();
        let snapshot_bytes=serde_json::to_vec(snapshot).map_err(|e|e.to_string())?;
        let content_sha256=format!("{:x}",Sha256::digest(&snapshot_bytes));
        let envelope=DraftEnvelope{format:"production-cycle-network-draft-v1",saved_at:saved_at.clone(),session_id:&active.metadata.session_id,user_id:active.metadata.user_id,content_sha256,snapshot};
        let bytes=serde_json::to_vec_pretty(&envelope).map_err(|e|e.to_string())?;
        let key=draft_key(snapshot);
        let user=active.metadata.user_id.map(|id|format!("user-{id}")).unwrap_or_else(||"bootstrap".into());
        let shared_dir=self.root()?.join("shared/drafts").join(user);
        let shared_path=shared_dir.join(format!("{key}.json"));
        let delivered=verified && fs::create_dir_all(&shared_dir).and_then(|_|atomic_write_io(&shared_path,&bytes)).is_ok();
        if delivered{
            Ok(DraftSaveResult{location:"shared".into(),saved_at})
        }else{
            let pending=self.recovery_data.join("network-pending").join(format!("{}-{key}.json",active.metadata.session_id));
            atomic_write(&pending,&bytes)?;
            Ok(DraftSaveResult{location:"localRecoveryQueue".into(),saved_at})
        }
    }

    fn flush_pending_drafts(&self){
        if !self.enabled{return}
        let Ok(guard)=self.session.lock() else{return};let Some(active)=guard.as_ref() else{return};
        if !matches!(self.session_is_current(active),Ok(true)){return}
        let user=active.metadata.user_id.map(|id|format!("user-{id}")).unwrap_or_else(||"bootstrap".into());
        let Ok(root)=self.root() else{return};let target=root.join("shared/drafts").join(user);let _=fs::create_dir_all(&target);
        let pending=self.recovery_data.join("network-pending");let Ok(entries)=fs::read_dir(pending) else{return};
        for entry in entries.flatten(){
            let path=entry.path();if !path.is_file(){continue}
            let Ok(file)=File::open(&path) else{continue};let mut bytes=Vec::new();
            if file.take(MAX_PENDING_DRAFT_BYTES+1).read_to_end(&mut bytes).is_err()
                || bytes.len() as u64>MAX_PENDING_DRAFT_BYTES{continue}
            let Ok(envelope)=serde_json::from_slice::<PendingDraftEnvelope>(&bytes) else{continue};
            let Ok(id)=Uuid::parse_str(&envelope.session_id) else{continue};
            if envelope.format!="production-cycle-network-draft-v1"
                || envelope.user_id!=active.metadata.user_id
                || envelope.session_id!=id.hyphenated().to_string()
                || chrono::DateTime::parse_from_rfc3339(&envelope.saved_at).is_err(){continue}
            let expected_name=format!("{}-{}.json",envelope.session_id,draft_key(&envelope.snapshot));
            if path.file_name().and_then(|name|name.to_str())!=Some(expected_name.as_str()){continue}
            let Ok(snapshot_bytes)=serde_json::to_vec(&envelope.snapshot) else{continue};
            if envelope.content_sha256!=format!("{:x}",Sha256::digest(&snapshot_bytes)){continue}
            // Only the matching user's queue is delivered. Older sessions of
            // that user retain their UUID in the name; other users stay local.
            if atomic_write_io(&target.join(expected_name),&bytes).is_ok(){let _=fs::remove_file(path);}
        }
    }

    pub fn release(&self)->Result<(),String>{
        if !self.enabled{return Ok(())}
        let mut guard=self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?;
        self.release_session(&mut guard)
    }

    fn release_session(&self,session:&mut Option<ActiveSession>)->Result<(),String>{
        if let Some(active)=session.take(){
            let metadata=self.root()?.join("shared/sessions").join(format!("session-{}.json",active.metadata.session_id));
            let control=self.root()?.join("shared/control").join(format!("{}.evict",active.metadata.session_id));
            let _=fs::remove_file(metadata);let _=fs::remove_file(control);
            // Close the handle even when SMB already discarded its lock. A
            // failed explicit unlock must not prevent local logout/re-login.
            let _=FileExt::unlock(&active.slot_file);
            drop(active);
        }
        Ok(())
    }
}

impl Drop for NetworkRuntime{fn drop(&mut self){let _=self.release();}}

pub fn maybe_relaunch_network_client()->Result<bool,String>{
    if env::var_os(NETWORK_CLIENT_ENV).is_some(){return Ok(false)}
    let exe=env::current_exe().map_err(|e|e.to_string())?;
    let root=exe.parent().ok_or_else(||"Не удалось определить папку программы.".to_string())?;
    let config_path=root.join("config/network.json");
    if !config_path.is_file(){return Ok(false)}
    let config=read_network_config(&config_path)?;
    if !config.enabled{return Ok(false)}
    let local_base=local_cache_base()?;
    // A cache may be used by several interactive sessions of the same Windows user.
    // Serialize preparation so one launcher cannot delete another one's staging/cache.
    crate::infrastructure::startup::update("Подготовка локальной копии. Первый запуск может занять несколько минут…");
    let cache_lock=OpenOptions::new().create(true).read(true).write(true).open(local_base.join("prepare.lock")).map_err(|e|e.to_string())?;
    let waiting=Instant::now();
    while let Err(error)=cache_lock.try_lock_exclusive(){
        if waiting.elapsed()>Duration::from_secs(240){return Err(format!("Другой запуск не завершил подготовку локальной копии: {error}"));}
        thread::sleep(Duration::from_millis(100));
    }
    let cache=local_base.join(&config.version);
    let marker=cache.join(".network-cache-ready");
    let ready=marker.is_file()&&fs::read_to_string(&marker).map(|value|value.trim()==config.version).unwrap_or(false)&&["production-cycle.exe","frontend/index.html","runtime/webview2/msedgewebview2.exe","config/portable.json"].iter().all(|name|cache.join(name).is_file());
    if !ready{
        fs::create_dir_all(&local_base).map_err(|e|e.to_string())?;
        let staging=local_base.join(format!(".staging-{}",Uuid::new_v4()));
        fs::create_dir_all(&staging).map_err(|e|e.to_string())?;
        for name in ["production-cycle.exe","frontend","runtime","config","app"]{
            let source=root.join(name);if !source.exists(){let _=fs::remove_dir_all(&staging);return Err(format!("В сетевой поставке отсутствует {name}."))}
            if let Err(error)=copy_entry(&source,&staging.join(name)){let _=fs::remove_dir_all(&staging);return Err(format!("Не удалось получить {name} из общей папки: {error}"));}
        }
        fs::write(staging.join(".network-cache-ready"),format!("{}\n",config.version)).map_err(|e|e.to_string())?;
        if cache.exists(){fs::remove_dir_all(&cache).map_err(|e|format!("Не удалось обновить локальный кэш: {e}"))?;}
        fs::rename(&staging,&cache).map_err(|e|format!("Не удалось активировать локальный кэш: {e}"))?;
    }
    FileExt::unlock(&cache_lock).map_err(|e|e.to_string())?;
    let local_exe=cache.join("production-cycle.exe");
    let mut command=Command::new(local_exe);
    command.arg("--network-client");
    for argument in env::args().skip(1){if argument!="--network-client"{command.arg(argument);}}
    let smoke=env::args().any(|argument|argument=="--smoke-test");
    let mut child=command.env(NETWORK_CLIENT_ENV,"1").env(NETWORK_ROOT_ENV,root).current_dir(&cache).spawn().map_err(|e|format!("Не удалось запустить локальный клиент: {e}"))?;
    crate::infrastructure::startup::stop();
    if smoke{
        let status=child.wait().map_err(|e|format!("Не удалось дождаться сетевого self-test: {e}"))?;
        if !status.success(){return Err(format!("Сетевой self-test завершился с кодом {:?}.",status.code()))}
    }
    Ok(true)
}

#[tauri::command]
pub fn network_status(network:State<'_,NetworkRuntime>)->Result<NetworkStatus,String>{network.status()}

#[tauri::command]
pub fn network_login(user_id:Option<i64>,pin:String,db:State<'_,ProductionDb>,network:State<'_,NetworkRuntime>)->Result<NetworkLoginResult,String>{
    if !network.enabled(){return network.login(None)}
    let users=network.with_read(&db,||db.list_audit_users())?;
    let user=if users.is_empty(){
        if user_id.is_some(){return Err("В общей базе ещё нет пользователей. Выполните первичную настройку администратора.".into())}None
    }else{
        let id=user_id.ok_or_else(||"Выберите пользователя.".to_string())?;
        Some(network.with_read(&db,||db.authorize_user(id,&pin))?)
    };
    network.login(user)
}

#[tauri::command]
pub fn network_save_draft(snapshot:ProductionSnapshot,network:State<'_,NetworkRuntime>)->Result<DraftSaveResult,String>{network.save_draft(&snapshot)}

#[tauri::command]
pub fn network_logout(network:State<'_,NetworkRuntime>)->Result<(),String>{network.release()}

#[tauri::command]
pub fn network_finish_eviction(app:AppHandle,network:State<'_,NetworkRuntime>)->Result<(),String>{network.release()?;app.exit(0);Ok(())}

// Resolve junctions and mapped drives before accepting a cache. LOCALAPPDATA may be
// redirected to SMB on domain PCs; Fixed WebView2 must NEVER execute there.
fn local_cache_base()->Result<PathBuf,String>{
    let mut candidates=Vec::new();
    if let Some(path)=env::var_os("LOCALAPPDATA"){candidates.push(PathBuf::from(path));}
    if let Some(path)=env::var_os("USERPROFILE"){candidates.push(PathBuf::from(path).join("AppData/Local"));}
    candidates.push(env::temp_dir());
    for candidate in candidates{
        if fs::create_dir_all(&candidate).is_err(){continue;}
        let Ok(real)=candidate.canonicalize() else {continue;};
        if !is_local_disk(&real){continue;}
        let base=real.join("ProductionCycleNetwork/cache");
        if fs::create_dir_all(&base).is_err(){continue;}
        let probe=base.join(format!(".write-test-{}",Uuid::new_v4()));
        if fs::write(&probe,b"ok").is_ok(){let _=fs::remove_file(probe);return Ok(base);}
    }
    Err("Нет доступной локальной папки для клиента. Локальный профиль Windows или TEMP должен находиться на диске этого компьютера и разрешать запись. WebView2 нельзя запускать из сетевой папки.".into())
}

#[cfg(windows)]
pub(crate) fn is_local_disk(path:&Path)->bool{
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetVolumePathNameW,GetDriveTypeW};
    let input:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut volume=vec![0u16;32768];
    unsafe {GetVolumePathNameW(input.as_ptr(),volume.as_mut_ptr(),volume.len() as u32)!=0 && matches!(GetDriveTypeW(volume.as_ptr()),2|3)}
}
#[cfg(not(windows))]
pub(crate) fn is_local_disk(_path:&Path)->bool{true}

fn read_network_config(path:&Path)->Result<NetworkConfig,String>{
    let bytes=fs::read(path).map_err(|e|format!("Не удалось прочитать {}: {e}",path.display()))?;
    let config:NetworkConfig=serde_json::from_slice(&bytes).map_err(|e|format!("Некорректный network.json: {e}"))?;
    if config.version.trim().is_empty() || !config.version.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'-'||c==b'_'||c==b'.') || config.version=="." || config.version==".." {return Err("Некорректная версия клиента в network.json.".into())}
    Ok(config)
}

fn validate_database(path:&Path)->Result<(),String>{
    let conn=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|format!("Не удалось открыть снимок базы: {e}"))?;
    let integrity:String=conn.query_row("PRAGMA integrity_check",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    if integrity!="ok"{return Err(format!("Снимок общей базы повреждён: {integrity}"))}
    let tables:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('production_cycle','production_stage')",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    if tables!=2{return Err("Снимок не является базой приложения «Производственный цикл»: отсутствуют обязательные таблицы.".into())}
    let version:i64=conn.query_row("PRAGMA user_version",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    if version>SCHEMA_VERSION{return Err(format!("Снимок создан более новой версией программы: схема {version}, поддерживается {SCHEMA_VERSION}."))}
    Ok(())
}

fn copy_checked_database(source:&Path,target:&Path)->Result<(),String>{
    if let Some(parent)=target.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
    let pending=target.with_extension(format!("copy-{}",Uuid::new_v4().simple()));
    fs::copy(source,&pending).map_err(|e|format!("Не удалось скопировать базу {}: {e}",source.display()))?;
    validate_database(&pending)?;
    atomic_replace(&pending,target)
}

fn atomic_write(path:&Path,bytes:&[u8])->Result<(),String>{atomic_write_io(path,bytes).map_err(|e|e.to_string())}
fn atomic_write_io(path:&Path,bytes:&[u8])->std::io::Result<()>{
    if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
    let pending=path.with_extension(format!("pending-{}",Uuid::new_v4().simple()));
    let result=(||{
        let mut file=File::create(&pending)?;file.write_all(bytes)?;file.sync_all()?;drop(file);
        atomic_replace_io(&pending,path)
    })();
    if result.is_err(){let _=fs::remove_file(&pending);}
    result
}

fn atomic_replace(pending:&Path,target:&Path)->Result<(),String>{atomic_replace_io(pending,target).map_err(|e|e.to_string())}

#[cfg(windows)]
fn atomic_replace_io(pending:&Path,target:&Path)->std::io::Result<()>{
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH};
    let from:Vec<u16>=pending.as_os_str().encode_wide().chain(Some(0)).collect();
    let to:Vec<u16>=target.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok=unsafe{MoveFileExW(from.as_ptr(),to.as_ptr(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)};
    if ok==0{Err(std::io::Error::last_os_error())}else{Ok(())}
}

#[cfg(not(windows))]
fn atomic_replace_io(pending:&Path,target:&Path)->std::io::Result<()>{fs::rename(pending,target)}

fn copy_entry(source:&Path,target:&Path)->Result<(),String>{
    if source.is_dir(){
        fs::create_dir_all(target).map_err(|e|e.to_string())?;
        for entry in fs::read_dir(source).map_err(|e|e.to_string())?{let entry=entry.map_err(|e|e.to_string())?;copy_entry(&entry.path(),&target.join(entry.file_name()))?;}
    }else{
        if let Some(parent)=target.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
        crate::infrastructure::startup::update(&format!("Получение локальной копии: {}\nПервый запуск может занять несколько минут. Не запускайте программу повторно.",source.file_name().unwrap_or_default().to_string_lossy()));
        fs::copy(source,target).map_err(|e|e.to_string())?;
    }
    Ok(())
}

fn computer_fingerprint()->String{
    let mut source=String::new();
    for key in ["COMPUTERNAME","USERNAME","USERDOMAIN"]{if let Ok(value)=env::var(key){source.push_str(&value);source.push('|');}}
    format!("{:x}",Sha256::digest(source.as_bytes()))[..16].to_uppercase()
}

fn draft_key(snapshot:&ProductionSnapshot)->String{
    snapshot.database_id.map(|id|format!("project-{id}")).unwrap_or_else(||format!("new-{:x}",Sha256::digest(format!("{}|{}",snapshot.project.order_no,snapshot.project.name).as_bytes())))
}

#[cfg(test)]
mod tests{
    use super::*;
    fn runtime(root:&Path,client:&str)->NetworkRuntime{
        for relative in ["shared/database","shared/sessions","shared/control","shared/drafts","shared/backups","shared/exports"]{fs::create_dir_all(root.join(relative)).unwrap();}
        let local_data=root.join(client);fs::create_dir_all(local_data.join("network-pending")).unwrap();
        NetworkRuntime{enabled:true,version:"test".into(),maximum:3,shared_root:Some(root.to_path_buf()),recovery_data:local_data.clone(),local_data,session:Mutex::new(None)}
    }
    fn user(id:i64,role:&str)->AuditUserSummary{AuditUserSummary{id,display_name:format!("Пользователь {id}"),role:role.into(),key_fingerprint:format!("KEY{id}"),active:true,created_at:"2026-10-05T12:00:00+03:00".into()}}
    fn draft_snapshot()->ProductionSnapshot{
        serde_json::from_value(serde_json::json!({"databaseId":null,"project":{"name":"Локальный черновик","orderNo":"DRAFT","initiator":"И","executor":"И","enterprise":"П","start":"2026-09-01","deadline":"2026-11-30"},"stages":[],"nextSeq":1})).unwrap()
    }
    #[test]fn network_config_requires_three_clients(){
        let root=env::temp_dir().join(format!("production-network-config-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
        fs::write(root.join("network.json"),br#"{"enabled":true,"version":"test","maxClients":3}"#).unwrap();
        let config=read_network_config(&root.join("network.json")).unwrap();assert!(config.enabled);assert_eq!(config.max_clients,3);let _=fs::remove_dir_all(root);
    }
    #[test]fn network_version_cannot_escape_cache_directory(){
        let root=env::temp_dir().join(format!("production-network-config-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
        for version in ["..","../escape","a/b","C:\\bad"]{
            fs::write(root.join("network.json"),serde_json::to_vec(&serde_json::json!({"enabled":true,"version":version,"maxClients":3})).unwrap()).unwrap();
            assert!(read_network_config(&root.join("network.json")).is_err());
        }
        let _=fs::remove_dir_all(root);
    }
    #[test]fn atomic_write_replaces_complete_document(){
        let root=env::temp_dir().join(format!("production-network-atomic-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();let path=root.join("value.json");
        atomic_write(&path,b"first").unwrap();atomic_write(&path,b"second").unwrap();assert_eq!(fs::read(path).unwrap(),b"second");let _=fs::remove_dir_all(root);
    }
    #[test]fn initialized_store_is_never_recreated_after_snapshot_deletion(){
        let root=env::temp_dir().join(format!("production-store-marker-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");let db=ProductionDb::open(&client.local_data.join("production-cycle.db")).unwrap();
        client.ensure_shared_database(&db).unwrap();assert!(client.store_was_initialized().unwrap());
        fs::remove_file(client.shared_database().unwrap()).unwrap();
        // Even an old valid legacy copy cannot replace a deleted authoritative DB.
        let legacy=root.join("data/production-cycle.db");fs::create_dir_all(legacy.parent().unwrap()).unwrap();db.export_database(&legacy).unwrap();
        assert!(client.prepare_local_database(&root.join("fresh/production-cycle.db")).is_err());
        assert!(client.ensure_shared_database(&db).is_err());
        assert!(client.with_read(&db,||Ok(())).is_err());
        assert!(!client.shared_database().unwrap().exists());
        drop((client,db));let _=fs::remove_dir_all(root);
    }
    #[test]fn existing_valid_shared_store_gets_a_marker_but_foreign_databases_are_rejected(){
        let root=env::temp_dir().join(format!("production-store-validation-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");let db=ProductionDb::open(&client.local_data.join("production-cycle.db")).unwrap();
        db.export_database(&client.shared_database().unwrap()).unwrap();
        assert!(!client.store_was_initialized().unwrap());
        client.prepare_local_database(&root.join("copy/production-cycle.db")).unwrap();
        assert!(client.store_was_initialized().unwrap());
        let foreign=root.join("foreign.db");let conn=Connection::open(&foreign).unwrap();conn.execute_batch("CREATE TABLE unrelated(id INTEGER);").unwrap();drop(conn);
        assert!(validate_database(&foreign).unwrap_err().contains("обязательные таблицы"));
        let shared=client.shared_database().unwrap();let conn=Connection::open(&shared).unwrap();conn.pragma_update(None,"user_version",SCHEMA_VERSION+1).unwrap();drop(conn);
        assert!(validate_database(&shared).unwrap_err().contains("более новой"));
        drop((client,db));let _=fs::remove_dir_all(root);
    }
    #[test]fn shared_mutations_require_an_admitted_session_including_bootstrap(){
        let root=env::temp_dir().join(format!("production-store-session-gate-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");let db=ProductionDb::open(&client.local_data.join("production-cycle.db")).unwrap();
        client.ensure_shared_database(&db).unwrap();let mut called=false;
        assert!(client.with_write(&db,||{called=true;Ok(())}).is_err());assert!(!called);
        client.login(None).unwrap();client.with_write(&db,||{called=true;Ok(())}).unwrap();assert!(called);
        client.release().unwrap();called=false;
        assert!(client.with_write(&db,||{called=true;Ok(())}).is_err());assert!(!called);
        drop((client,db));let _=fs::remove_dir_all(root);
    }
    #[test]fn logout_waits_for_an_admitted_write_to_finish_publishing(){
        let root=env::temp_dir().join(format!("production-store-logout-{}",Uuid::new_v4()));
        let client=std::sync::Arc::new(runtime(&root,"client"));let db=ProductionDb::open(&client.local_data.join("production-cycle.db")).unwrap();
        client.ensure_shared_database(&db).unwrap();client.login(Some(user(1,"reviewer"))).unwrap();
        let (started_tx,started_rx)=std::sync::mpsc::channel();let (continue_tx,continue_rx)=std::sync::mpsc::channel();
        let writer_client=client.clone();let writer=thread::spawn(move||writer_client.with_write(&db,||{started_tx.send(()).unwrap();continue_rx.recv_timeout(Duration::from_secs(10)).unwrap();Ok(())}).unwrap());
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let (logout_started_tx,logout_started_rx)=std::sync::mpsc::channel();let (logout_done_tx,logout_done_rx)=std::sync::mpsc::channel();
        let logout_client=client.clone();let logout=thread::spawn(move||{logout_started_tx.send(()).unwrap();logout_client.release().unwrap();logout_done_tx.send(()).unwrap();});
        logout_started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(matches!(logout_done_rx.recv_timeout(Duration::from_millis(50)),Err(std::sync::mpsc::RecvTimeoutError::Timeout)));
        continue_tx.send(()).unwrap();writer.join().unwrap();logout.join().unwrap();
        logout_done_rx.recv_timeout(Duration::from_secs(10)).unwrap();assert_eq!(client.status().unwrap().occupied,0);
        drop(client);let _=fs::remove_dir_all(root);
    }
    #[test]fn failed_publish_preserves_committed_database_before_any_refresh(){
        let root=env::temp_dir().join(format!("production-store-unpublished-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");let db=ProductionDb::open(&client.local_data.join("production-cycle.db")).unwrap();
        client.ensure_shared_database(&db).unwrap();client.login(Some(user(1,"reviewer"))).unwrap();
        let shared=client.shared_database().unwrap();let original=fs::read(&shared).unwrap();
        let error=client.with_write(&db,||{
            db.save_snapshot(&draft_snapshot()).map_err(|e|e.to_string())?;
            // Fail final publication after the local transaction has committed.
            fs::remove_file(&shared).unwrap();fs::create_dir(&shared).unwrap();Ok(())
        }).unwrap_err();
        assert!(error.contains("Общая запись не подтверждена"));
        let recovery=fs::read_dir(client.recovery_data.join("network-unsent")).unwrap().next().unwrap().unwrap().path();
        assert!(error.contains(recovery.to_str().unwrap()));validate_database(&recovery).unwrap();
        let recovered=ProductionDb::open(&recovery).unwrap();assert_eq!(recovered.list_projects().unwrap().len(),1);drop(recovered);
        fs::remove_dir(&shared).unwrap();fs::write(&shared,original).unwrap();
        assert!(client.with_read(&db,||db.list_projects().map_err(|e|e.to_string())).unwrap().is_empty());
        assert!(recovery.is_file());let recovered=ProductionDb::open(&recovery).unwrap();
        assert_eq!(recovered.list_projects().unwrap()[0].name,"Локальный черновик");
        drop((client,db,recovered));let _=fs::remove_dir_all(root);
    }
    #[test]fn exactly_three_sessions_are_allowed(){
        let root=env::temp_dir().join(format!("production-network-slots-{}",Uuid::new_v4()));
        let first=runtime(&root,"client-1");
        let second=runtime(&root,"client-2");
        let third=runtime(&root,"client-3");
        let fourth=runtime(&root,"client-4");
        assert_eq!(first.login(Some(user(1,"project_manager"))).unwrap().state,"active");
        assert_eq!(second.login(Some(user(2,"reviewer"))).unwrap().state,"active");
        assert_eq!(third.login(Some(user(3,"project_manager"))).unwrap().state,"active");
        let denied=fourth.login(Some(user(4,"reviewer"))).unwrap();assert_eq!(denied.state,"full");assert_eq!(denied.occupied,3);
        first.release().unwrap();
        assert_eq!(fourth.login(Some(user(4,"reviewer"))).unwrap().state,"active");
        drop((second,third,fourth,first));let _=fs::remove_dir_all(root);
    }
    #[test]fn bootstrap_session_is_bound_after_profile_creation(){
        let root=env::temp_dir().join(format!("production-bootstrap-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");
        client.login(None).unwrap();
        let session_id=client.status().unwrap().session_id;
        client.login(Some(user(1,"admin"))).unwrap();
        let status=client.status().unwrap();
        assert_eq!(status.session_id,session_id);
        assert_eq!(status.display_name.as_deref(),Some("Пользователь 1"));
        assert_eq!(client.session_metadata().unwrap()[0].1.user_id,Some(1));
        assert_eq!(status.occupied,1);
        drop(client);let _=fs::remove_dir_all(root);
    }
    #[test]fn session_count_tracks_close_reopen_and_removes_abandoned_metadata(){
        let root=env::temp_dir().join(format!("production-session-count-{}",Uuid::new_v4()));
        let observer=runtime(&root,"observer");let client=runtime(&root,"client");
        observer.login(Some(user(1,"reviewer"))).unwrap();
        client.login(Some(user(2,"project_manager"))).unwrap();
        assert_eq!(observer.status().unwrap().occupied,2);
        client.release().unwrap();assert_eq!(observer.status().unwrap().occupied,1);
        client.login(Some(user(2,"project_manager"))).unwrap();
        assert_eq!(observer.status().unwrap().occupied,2);
        // Simulate process termination: OS releases the handle, JSON survives.
        let abandoned=client.session.lock().unwrap().take().unwrap();
        let id=abandoned.metadata.session_id.clone();
        let metadata=root.join(format!("shared/sessions/session-{id}.json"));
        let control=root.join(format!("shared/control/{id}.evict"));
        fs::write(&control,b"{}").unwrap();drop(abandoned);
        assert!(metadata.is_file());
        assert_eq!(observer.status().unwrap().occupied,1);
        assert!(!metadata.exists());assert!(!control.exists());
        assert_eq!(observer.newest_non_admin_session().unwrap().unwrap().user_id,Some(1));
        // A live slot is counted even if its descriptive JSON is missing.
        client.login(Some(user(2,"project_manager"))).unwrap();
        let metadata=client.session_metadata().unwrap().into_iter().find(|(_,m)|m.user_id==Some(2)).unwrap().0;
        fs::remove_file(metadata).unwrap();assert_eq!(observer.status().unwrap().occupied,2);
        drop(client);assert_eq!(observer.status().unwrap().occupied,1);
        observer.release().unwrap();assert_eq!(observer.status().unwrap().occupied,0);
        drop(observer);let _=fs::remove_dir_all(root);
    }
    #[test]fn simultaneous_logins_and_status_probes_admit_exactly_three_clients(){
        let root=env::temp_dir().join(format!("production-session-concurrent-{}",Uuid::new_v4()));
        let observer=runtime(&root,"observer");
        let barrier=std::sync::Arc::new(std::sync::Barrier::new(5));
        let mut handles=Vec::new();
        for index in 1..=4{
            let client=runtime(&root,&format!("client-{index}"));let barrier=barrier.clone();
            handles.push(thread::spawn(move||{barrier.wait();let result=client.login(Some(user(index,"project_manager"))).unwrap();(client,result)}));
        }
        barrier.wait();
        for _ in 0..10{assert!(observer.status().unwrap().occupied<=3);}
        let clients:Vec<_>=handles.into_iter().map(|handle|handle.join().unwrap()).collect();
        assert_eq!(clients.iter().filter(|(_,result)|result.state=="active").count(),3);
        assert_eq!(clients.iter().filter(|(_,result)|result.state=="full").count(),1);
        assert_eq!(observer.status().unwrap().occupied,3);
        drop(clients);assert_eq!(observer.status().unwrap().occupied,0);
        drop(observer);let _=fs::remove_dir_all(root);
    }
    #[test]fn concurrent_login_calls_on_one_runtime_keep_one_session(){
        let root=env::temp_dir().join(format!("production-session-one-runtime-{}",Uuid::new_v4()));
        let client=std::sync::Arc::new(runtime(&root,"client"));let observer=runtime(&root,"observer");
        let barrier=std::sync::Arc::new(std::sync::Barrier::new(9));let mut handles=Vec::new();
        for _ in 0..8{
            let client=client.clone();let barrier=barrier.clone();
            handles.push(thread::spawn(move||{barrier.wait();client.login(Some(user(1,"reviewer"))).unwrap()}));
        }
        barrier.wait();
        for handle in handles{assert_eq!(handle.join().unwrap().state,"active");}
        assert_eq!(client.session_metadata().unwrap().len(),1);
        assert_eq!(observer.status().unwrap().occupied,1);
        client.release().unwrap();assert_eq!(observer.status().unwrap().occupied,0);
        assert!(client.require_active_session().is_err());
        client.login(Some(user(2,"project_manager"))).unwrap();
        assert_eq!(client.status().unwrap().display_name.as_deref(),Some("Пользователь 2"));
        assert!(client.login(Some(user(1,"reviewer"))).is_err());
        assert_eq!(observer.status().unwrap().occupied,1);
        drop((client,observer));let _=fs::remove_dir_all(root);
    }
    #[test]fn lost_slot_cannot_renew_metadata_or_authorize_shared_writes(){
        let root=env::temp_dir().join(format!("production-session-lost-slot-{}",Uuid::new_v4()));
        let old=runtime(&root,"old");let replacement=runtime(&root,"replacement");
        old.login(Some(user(1,"reviewer"))).unwrap();
        let old_id=old.status().unwrap().session_id.unwrap();
        // Model the old client's SMB lock disappearing while its local state survives.
        FileExt::unlock(&old.session.lock().unwrap().as_ref().unwrap().slot_file).unwrap();
        replacement.login(Some(user(2,"project_manager"))).unwrap();
        let replacement_id=replacement.status().unwrap().session_id.unwrap();
        assert!(old.require_active_session().is_err());
        let status=old.status().unwrap();assert!(!status.active);assert_eq!(status.occupied,1);
        assert!(!root.join(format!("shared/sessions/session-{old_id}.json")).exists());
        assert!(root.join(format!("shared/sessions/session-{replacement_id}.json")).exists());
        assert!(old.save_draft(&draft_snapshot()).is_err());
        replacement.require_active_session().unwrap();
        drop((old,replacement));let _=fs::remove_dir_all(root);
    }
    #[test]fn missing_session_identity_requires_reauthentication(){
        let root=env::temp_dir().join(format!("production-session-missing-identity-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");client.login(Some(user(1,"reviewer"))).unwrap();
        fs::remove_file(&client.session_metadata().unwrap()[0].0).unwrap();
        let status=client.status().unwrap();assert!(!status.active);assert_eq!(status.occupied,0);
        assert!(client.require_active_session().is_err());
        client.login(Some(user(1,"reviewer"))).unwrap();client.require_active_session().unwrap();
        drop(client);let _=fs::remove_dir_all(root);
    }
    #[test]fn session_metadata_cannot_redirect_cleanup_outside_sessions(){
        let root=env::temp_dir().join(format!("production-session-invalid-metadata-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");client.login(Some(user(1,"reviewer"))).unwrap();
        let mut metadata=client.session_metadata().unwrap()[0].1.clone();client.release().unwrap();
        let sentinel=root.join("shared/sentinel.evict");fs::write(&sentinel,b"keep").unwrap();
        metadata.session_id="../sentinel".into();
        fs::write(root.join("shared/sessions/session-invalid.json"),serde_json::to_vec(&metadata).unwrap()).unwrap();
        assert_eq!(client.status().unwrap().occupied,0);assert_eq!(fs::read(&sentinel).unwrap(),b"keep");
        assert!(client.session_metadata().unwrap().is_empty());
        metadata.session_id=Uuid::new_v4().to_string();metadata.slot=1;
        let mut oversized=serde_json::to_vec(&metadata).unwrap();oversized.extend(vec![b' ';MAX_SESSION_METADATA_BYTES as usize]);
        fs::write(root.join(format!("shared/sessions/session-{}.json",metadata.session_id)),oversized).unwrap();
        assert!(client.session_metadata().unwrap().is_empty());
        drop(client);let _=fs::remove_dir_all(root);
    }
    #[test]fn eviction_blocks_shared_mutations_but_allows_a_recovery_draft(){
        let root=env::temp_dir().join(format!("production-session-evict-guard-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");client.login(Some(user(1,"reviewer"))).unwrap();
        let id=client.status().unwrap().session_id.unwrap();
        fs::write(root.join(format!("shared/control/{id}.evict")),b"{}").unwrap();
        assert!(client.require_active_session().is_err());
        assert_eq!(client.save_draft(&draft_snapshot()).unwrap().location,"shared");
        client.release().unwrap();assert_eq!(client.status().unwrap().occupied,0);
        drop(client);let _=fs::remove_dir_all(root);
    }
    #[test]fn queued_drafts_keep_their_owner_across_logout_and_retry_atomically(){
        let root=env::temp_dir().join(format!("production-draft-owner-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");client.login(Some(user(1,"reviewer"))).unwrap();
        let blocked=root.join("shared/drafts/user-1");fs::write(&blocked,b"not a directory").unwrap();
        assert_eq!(client.save_draft(&draft_snapshot()).unwrap().location,"localRecoveryQueue");
        let pending=fs::read_dir(client.local_data.join("network-pending")).unwrap().next().unwrap().unwrap().path();
        let bytes=fs::read(&pending).unwrap();
        // A failed delivery preserves the complete local recovery copy.
        client.status().unwrap();assert_eq!(fs::read(&pending).unwrap(),bytes);
        client.release().unwrap();fs::remove_file(&blocked).unwrap();
        client.login(Some(user(2,"project_manager"))).unwrap();client.status().unwrap();
        assert!(pending.is_file());
        assert_eq!(fs::read_dir(root.join("shared/drafts/user-2")).unwrap().count(),0);
        client.release().unwrap();client.login(Some(user(1,"reviewer"))).unwrap();client.status().unwrap();
        assert!(!pending.exists());
        let recovered=root.join("shared/drafts/user-1").join(pending.file_name().unwrap());
        assert_eq!(fs::read(recovered).unwrap(),bytes);
        drop(client);let _=fs::remove_dir_all(root);
    }
    #[test]fn corrupt_queued_draft_is_preserved_without_publishing(){
        let root=env::temp_dir().join(format!("production-draft-corrupt-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");client.login(Some(user(1,"reviewer"))).unwrap();
        let blocked=root.join("shared/drafts/user-1");fs::write(&blocked,b"not a directory").unwrap();
        client.save_draft(&draft_snapshot()).unwrap();
        let pending=fs::read_dir(client.local_data.join("network-pending")).unwrap().next().unwrap().unwrap().path();
        let mut envelope:serde_json::Value=serde_json::from_slice(&fs::read(&pending).unwrap()).unwrap();
        envelope["snapshot"]["project"]["initiator"]=serde_json::json!("повреждённое содержимое");
        fs::write(&pending,serde_json::to_vec(&envelope).unwrap()).unwrap();fs::remove_file(&blocked).unwrap();
        client.status().unwrap();assert!(pending.is_file());assert_eq!(fs::read_dir(blocked).unwrap().count(),0);
        drop(client);let _=fs::remove_dir_all(root);
    }
    #[test]fn unavailable_presence_coordination_keeps_a_local_recovery_draft(){
        let root=env::temp_dir().join(format!("production-draft-offline-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");client.login(Some(user(1,"reviewer"))).unwrap();
        let presence=root.join("shared/sessions/presence.lock");let saved=presence.with_extension("saved");
        fs::rename(&presence,&saved).unwrap();fs::create_dir(&presence).unwrap();
        assert_eq!(client.save_draft(&draft_snapshot()).unwrap().location,"localRecoveryQueue");
        assert!(!root.join("shared/drafts/user-1").exists());
        let pending=fs::read_dir(client.local_data.join("network-pending")).unwrap().next().unwrap().unwrap().path();
        fs::remove_dir(&presence).unwrap();fs::rename(saved,&presence).unwrap();
        client.status().unwrap();assert!(!pending.exists());
        assert_eq!(fs::read_dir(root.join("shared/drafts/user-1")).unwrap().count(),1);
        drop(client);let _=fs::remove_dir_all(root);
    }
    #[test]fn old_heartbeat_does_not_release_a_live_slot(){
        let root=env::temp_dir().join(format!("production-session-heartbeat-{}",Uuid::new_v4()));
        let client=runtime(&root,"client");let observer=runtime(&root,"observer");
        client.login(Some(user(1,"project_manager"))).unwrap();
        {let mut guard=client.session.lock().unwrap();let active=guard.as_mut().unwrap();
            active.metadata.heartbeat_at="2000-01-01T00:00:00Z".into();client.write_metadata(&active.metadata).unwrap();}
        assert_eq!(observer.status().unwrap().occupied,1);
        assert_eq!(observer.newest_non_admin_session().unwrap().unwrap().user_id,Some(1));
        drop((client,observer));let _=fs::remove_dir_all(root);
    }
    #[test]fn administrator_evicts_newest_user_only_after_draft_is_saved(){
        let root=env::temp_dir().join(format!("production-eviction-{}",Uuid::new_v4()));
        let first=runtime(&root,"first");let second=runtime(&root,"second");let third=runtime(&root,"third");let admin=runtime(&root,"admin");
        first.login(Some(user(1,"project_manager"))).unwrap();
        thread::sleep(Duration::from_millis(5));second.login(Some(user(2,"reviewer"))).unwrap();
        thread::sleep(Duration::from_millis(5));third.login(Some(user(3,"project_manager"))).unwrap();
        let handle=thread::spawn(move||{
            let started=Instant::now();
            while !third.status().unwrap().eviction_requested{
                assert!(started.elapsed()<Duration::from_secs(10),"Eviction request not delivered");
                thread::sleep(Duration::from_millis(30));
            }
            let snapshot:ProductionSnapshot=serde_json::from_value(serde_json::json!({"databaseId":null,"project":{"name":"Черновик до вытеснения","orderNo":"DRAFT","initiator":"И","executor":"И","enterprise":"П","start":"2026-09-01","deadline":"2026-11-30"},"stages":[],"nextSeq":1})).unwrap();
            assert_eq!(third.save_draft(&snapshot).unwrap().location,"shared");
            third.release().unwrap();
        });
        assert_eq!(admin.login(Some(user(4,"admin"))).unwrap().state,"active");
        handle.join().unwrap();
        assert!(!first.status().unwrap().eviction_requested);assert!(!second.status().unwrap().eviction_requested);
        let drafts=fs::read_dir(root.join("shared/drafts/user-3")).unwrap().collect::<Vec<_>>();assert_eq!(drafts.len(),1);
        let draft:serde_json::Value=serde_json::from_slice(&fs::read(drafts[0].as_ref().unwrap().path()).unwrap()).unwrap();
        assert_eq!(draft["snapshot"]["project"]["name"],"Черновик до вытеснения");
        assert_eq!(admin.status().unwrap().occupied,3);
        drop((first,second,admin));let _=fs::remove_dir_all(root);
    }
    #[test]fn shared_session_metadata_does_not_store_person_name(){
        let root=env::temp_dir().join(format!("production-network-private-session-{}",Uuid::new_v4()));let client=runtime(&root,"client");
        client.login(Some(user(7,"reviewer"))).unwrap();let files=client.session_metadata().unwrap();assert_eq!(files.len(),1);
        let json=fs::read_to_string(&files[0].0).unwrap();assert!(!json.contains("Пользователь 7"));assert!(!json.contains("displayName"));
        drop(client);let _=fs::remove_dir_all(root);
    }
}
