use chrono::Local;
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{audit::AuditUserSummary, production::{ProductionDb, ProductionSnapshot}};

const NETWORK_CLIENT_ENV: &str = "PRODUCTION_CYCLE_NETWORK_CLIENT";
const NETWORK_ROOT_ENV: &str = "PRODUCTION_CYCLE_SHARED_ROOT";
const DEFAULT_MAX_CLIENTS: usize = 3;

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

pub struct NetworkRuntime {
    enabled: bool,
    version: String,
    maximum: usize,
    shared_root: Option<PathBuf>,
    local_data: PathBuf,
    session: Mutex<Option<ActiveSession>>,
}

impl NetworkRuntime {
    pub fn from_environment(local_data: PathBuf) -> Result<Self, String> {
        let Some(shared) = env::var_os(NETWORK_ROOT_ENV) else {
            return Ok(Self { enabled:false, version:String::new(), maximum:DEFAULT_MAX_CLIENTS, shared_root:None, local_data, session:Mutex::new(None) });
        };
        let shared_root=PathBuf::from(shared).canonicalize().map_err(|e|format!("Сетевая папка недоступна: {e}"))?;
        let config=read_network_config(&shared_root.join("config/network.json"))?;
        if !config.enabled{return Err("Сетевой режим отключён в config/network.json".into())}
        if config.max_clients!=DEFAULT_MAX_CLIENTS{return Err("Тестовая сетевая версия поддерживает ровно 3 одновременных компьютера.".into())}
        for relative in ["shared/database","shared/sessions","shared/control","shared/drafts","shared/backups","shared/exports"]{
            fs::create_dir_all(shared_root.join(relative)).map_err(|e|format!("Не удалось подготовить {relative}: {e}"))?;
        }
        fs::create_dir_all(local_data.join("network-pending")).map_err(|e|e.to_string())?;
        Ok(Self {enabled:true,version:config.version,maximum:config.max_clients,shared_root:Some(shared_root),local_data,session:Mutex::new(None)})
    }

    pub fn enabled(&self)->bool{self.enabled}
    pub fn shared_backups(&self)->Option<PathBuf>{self.shared_root.as_ref().map(|root|root.join("shared/backups"))}
    fn root(&self)->Result<&Path,String>{self.shared_root.as_deref().ok_or_else(||"Сетевой режим не включён.".to_string())}
    fn shared_database(&self)->Result<PathBuf,String>{Ok(self.root()?.join("shared/database/production-cycle.db"))}
    fn store_lock_path(&self)->Result<PathBuf,String>{Ok(self.root()?.join("shared/database/store.lock"))}

    pub fn prepare_local_database(&self,local_db:&Path)->Result<(),String>{
        if !self.enabled{return Ok(())}
        let lock=self.acquire_store_lock()?;
        let shared=self.shared_database()?;
        if !shared.is_file(){
            let legacy=self.root()?.join("data/production-cycle.db");
            if legacy.is_file(){copy_checked_database(&legacy,&shared)?;}
        }
        if shared.is_file(){copy_checked_database(&shared,local_db)?;}
        FileExt::unlock(&lock).map_err(|e|e.to_string())?;
        Ok(())
    }

    pub fn ensure_shared_database(&self,db:&ProductionDb)->Result<(),String>{
        if !self.enabled{return Ok(())}
        let lock=self.acquire_store_lock()?;
        if !self.shared_database()?.is_file(){self.publish_locked(db)?;}
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
        self.refresh_locked(db)?;
        let result=operation();
        if result.is_ok(){self.publish_locked(db)?;}
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
        if !shared.is_file(){return self.publish_locked(db)}
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

    pub fn login(&self,user:Option<AuditUserSummary>)->Result<NetworkLoginResult,String>{
        if !self.enabled{return Ok(NetworkLoginResult{state:"standalone".into(),occupied:1,maximum:1,evicted_display_name:None})}
        if self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?.is_some(){
            return Ok(NetworkLoginResult{state:"active".into(),occupied:self.occupied_count(),maximum:self.maximum,evicted_display_name:None});
        }
        let (user_id,display_name,role)=match user{
            Some(value)=>(Some(value.id),value.display_name,value.role),
            None=>(None,"Первичная настройка".into(),"admin".into()),
        };
        if let Some(active)=self.try_acquire_slot(user_id,&display_name,&role)?{
            *self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?=Some(active);
            return Ok(NetworkLoginResult{state:"active".into(),occupied:self.occupied_count().max(1),maximum:self.maximum,evicted_display_name:None});
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
                *self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?=Some(active);
                return Ok(NetworkLoginResult{state:"active".into(),occupied:self.occupied_count().max(1),maximum:self.maximum,evicted_display_name:Some("последний вошедший пользователь".into())});
            }
            thread::sleep(Duration::from_millis(250));
        }
        Ok(NetworkLoginResult{state:"waitingForEviction".into(),occupied:self.maximum,maximum:self.maximum,evicted_display_name:Some("последний вошедший пользователь".into())})
    }

    fn try_acquire_slot(&self,user_id:Option<i64>,display_name:&str,role:&str)->Result<Option<ActiveSession>,String>{
        let sessions=self.root()?.join("shared/sessions");
        for slot in 1..=self.maximum{
            let path=sessions.join(format!("slot-{slot}.lock"));
            let file=OpenOptions::new().create(true).read(true).write(true).open(path).map_err(|e|e.to_string())?;
            if file.try_lock_exclusive().is_err(){continue}
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
        for (path,metadata) in self.session_metadata()?{if metadata.slot==slot{let _=fs::remove_file(path);}}
        Ok(())
    }

    fn session_metadata(&self)->Result<Vec<(PathBuf,SessionMetadata)>,String>{
        let dir=self.root()?.join("shared/sessions");
        let mut result=Vec::new();
        for entry in fs::read_dir(dir).map_err(|e|e.to_string())?{
            let path=entry.map_err(|e|e.to_string())?.path();
            if path.extension().and_then(|v|v.to_str())!=Some("json"){continue}
            if let Ok(bytes)=fs::read(&path){
                if let Ok(value)=serde_json::from_slice::<SessionMetadata>(&bytes){result.push((path,value));}
            }
        }
        Ok(result)
    }

    fn newest_non_admin_session(&self)->Result<Option<SessionMetadata>,String>{
        Ok(self.session_metadata()?.into_iter().map(|(_,value)|value).filter(|value|value.role!="admin").max_by(|a,b|a.started_at.cmp(&b.started_at)))
    }

    fn occupied_count(&self)->usize{self.session_metadata().map(|items|items.len().min(self.maximum)).unwrap_or(0)}

    pub fn status(&self)->Result<NetworkStatus,String>{
        if !self.enabled{return Ok(NetworkStatus{enabled:false,active:true,occupied:1,maximum:1,eviction_requested:false,role:None,display_name:None,session_id:None})}
        self.flush_pending_drafts();
        let mut guard=self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?;
        let Some(active)=guard.as_mut() else{return Ok(NetworkStatus{enabled:true,active:false,occupied:self.occupied_count(),maximum:self.maximum,eviction_requested:false,role:None,display_name:None,session_id:None})};
        active.metadata.heartbeat_at=Local::now().to_rfc3339();
        self.write_metadata(&active.metadata)?;
        let eviction=self.root()?.join("shared/control").join(format!("{}.evict",active.metadata.session_id)).is_file();
        Ok(NetworkStatus{enabled:true,active:true,occupied:self.occupied_count().max(1),maximum:self.maximum,eviction_requested:eviction,role:Some(active.metadata.role.clone()),display_name:Some(active.display_name.clone()),session_id:Some(active.metadata.session_id.clone())})
    }

    pub fn save_draft(&self,snapshot:&ProductionSnapshot)->Result<DraftSaveResult,String>{
        if !self.enabled{return Ok(DraftSaveResult{location:"standalone".into(),saved_at:Local::now().to_rfc3339()})}
        let guard=self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?;
        let active=guard.as_ref().ok_or_else(||"Сначала войдите в сетевую сессию.".to_string())?;
        let saved_at=Local::now().to_rfc3339();
        let snapshot_bytes=serde_json::to_vec(snapshot).map_err(|e|e.to_string())?;
        let content_sha256=format!("{:x}",Sha256::digest(&snapshot_bytes));
        let envelope=DraftEnvelope{format:"production-cycle-network-draft-v1",saved_at:saved_at.clone(),session_id:&active.metadata.session_id,user_id:active.metadata.user_id,content_sha256,snapshot};
        let bytes=serde_json::to_vec_pretty(&envelope).map_err(|e|e.to_string())?;
        let key=snapshot.database_id.map(|id|format!("project-{id}")).unwrap_or_else(||format!("new-{:x}",Sha256::digest(format!("{}|{}",snapshot.project.order_no,snapshot.project.name).as_bytes())));
        let user=active.metadata.user_id.map(|id|format!("user-{id}")).unwrap_or_else(||"bootstrap".into());
        let shared_dir=self.root()?.join("shared/drafts").join(user);
        let shared_path=shared_dir.join(format!("{key}.json"));
        match fs::create_dir_all(&shared_dir).and_then(|_|atomic_write_io(&shared_path,&bytes)){
            Ok(())=>Ok(DraftSaveResult{location:"shared".into(),saved_at}),
            Err(_)=>{
                let pending=self.local_data.join("network-pending").join(format!("{}-{key}.json",active.metadata.session_id));
                atomic_write(&pending,&bytes)?;
                Ok(DraftSaveResult{location:"localRecoveryQueue".into(),saved_at})
            }
        }
    }

    fn flush_pending_drafts(&self){
        if !self.enabled{return}
        let Ok(guard)=self.session.lock() else{return};let Some(active)=guard.as_ref() else{return};
        let user=active.metadata.user_id.map(|id|format!("user-{id}")).unwrap_or_else(||"bootstrap".into());
        let Ok(root)=self.root() else{return};let target=root.join("shared/drafts").join(user);let _=fs::create_dir_all(&target);
        let pending=self.local_data.join("network-pending");let Ok(entries)=fs::read_dir(pending) else{return};
        for entry in entries.flatten(){let path=entry.path();if !path.is_file(){continue}let Some(name)=path.file_name() else{continue};let destination=target.join(name);if fs::copy(&path,&destination).is_ok(){let _=fs::remove_file(path);}}
    }

    pub fn release(&self)->Result<(),String>{
        if !self.enabled{return Ok(())}
        let active=self.session.lock().map_err(|_|"Состояние сетевой сессии повреждено".to_string())?.take();
        if let Some(active)=active{
            let metadata=self.root()?.join("shared/sessions").join(format!("session-{}.json",active.metadata.session_id));
            let control=self.root()?.join("shared/control").join(format!("{}.evict",active.metadata.session_id));
            let _=fs::remove_file(metadata);let _=fs::remove_file(control);
            FileExt::unlock(&active.slot_file).map_err(|e|e.to_string())?;
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
    let local_base=env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(||env::temp_dir()).join("ProductionCycleNetwork/cache");
    let cache=local_base.join(&config.version);
    let marker=cache.join(".network-cache-ready");
    let ready=marker.is_file()&&fs::read_to_string(&marker).map(|value|value.trim()==config.version).unwrap_or(false)&&cache.join("production-cycle.exe").is_file();
    if !ready{
        fs::create_dir_all(&local_base).map_err(|e|e.to_string())?;
        let staging=local_base.join(format!(".staging-{}",Uuid::new_v4()));
        fs::create_dir_all(&staging).map_err(|e|e.to_string())?;
        for name in ["production-cycle.exe","frontend","runtime","config","app"]{
            let source=root.join(name);if !source.exists(){let _=fs::remove_dir_all(&staging);return Err(format!("В сетевой поставке отсутствует {name}."))}
            copy_entry(&source,&staging.join(name))?;
        }
        fs::write(staging.join(".network-cache-ready"),format!("{}\n",config.version)).map_err(|e|e.to_string())?;
        if cache.exists(){fs::remove_dir_all(&cache).map_err(|e|format!("Не удалось обновить локальный кэш: {e}"))?;}
        fs::rename(&staging,&cache).map_err(|e|format!("Не удалось активировать локальный кэш: {e}"))?;
    }
    let local_exe=cache.join("production-cycle.exe");
    let mut command=Command::new(local_exe);
    command.arg("--network-client");
    for argument in env::args().skip(1){if argument!="--network-client"{command.arg(argument);}}
    let smoke=env::args().any(|argument|argument=="--smoke-test");
    let mut child=command.env(NETWORK_CLIENT_ENV,"1").env(NETWORK_ROOT_ENV,root).current_dir(&cache).spawn().map_err(|e|format!("Не удалось запустить локальный клиент: {e}"))?;
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
pub fn network_finish_eviction(app:AppHandle,network:State<'_,NetworkRuntime>)->Result<(),String>{network.release()?;app.exit(0);Ok(())}

fn read_network_config(path:&Path)->Result<NetworkConfig,String>{
    let bytes=fs::read(path).map_err(|e|format!("Не удалось прочитать {}: {e}",path.display()))?;
    let config:NetworkConfig=serde_json::from_slice(&bytes).map_err(|e|format!("Некорректный network.json: {e}"))?;
    if config.version.trim().is_empty(){return Err("В network.json не задана версия клиента.".into())}
    Ok(config)
}

fn validate_database(path:&Path)->Result<(),String>{
    let conn=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e|format!("Не удалось открыть снимок базы: {e}"))?;
    let integrity:String=conn.query_row("PRAGMA integrity_check",[],|row|row.get(0)).map_err(|e|e.to_string())?;
    if integrity!="ok"{return Err(format!("Снимок общей базы повреждён: {integrity}"))}
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
    let mut file=File::create(&pending)?;file.write_all(bytes)?;file.sync_all()?;drop(file);
    atomic_replace_io(&pending,path)
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
        fs::copy(source,target).map_err(|e|e.to_string())?;
    }
    Ok(())
}

fn computer_fingerprint()->String{
    let mut source=String::new();
    for key in ["COMPUTERNAME","USERNAME","USERDOMAIN"]{if let Ok(value)=env::var(key){source.push_str(&value);source.push('|');}}
    format!("{:x}",Sha256::digest(source.as_bytes()))[..16].to_uppercase()
}

#[cfg(test)]
mod tests{
    use super::*;
    fn runtime(root:&Path,client:&str)->NetworkRuntime{
        for relative in ["shared/database","shared/sessions","shared/control","shared/drafts","shared/backups","shared/exports"]{fs::create_dir_all(root.join(relative)).unwrap();}
        let local_data=root.join(client);fs::create_dir_all(local_data.join("network-pending")).unwrap();
        NetworkRuntime{enabled:true,version:"test".into(),maximum:3,shared_root:Some(root.to_path_buf()),local_data,session:Mutex::new(None)}
    }
    fn user(id:i64,role:&str)->AuditUserSummary{AuditUserSummary{id,display_name:format!("Пользователь {id}"),role:role.into(),key_fingerprint:format!("KEY{id}"),active:true,created_at:"2026-10-05T12:00:00+03:00".into()}}
    #[test]fn network_config_requires_three_clients(){
        let root=env::temp_dir().join(format!("production-network-config-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
        fs::write(root.join("network.json"),br#"{"enabled":true,"version":"test","maxClients":3}"#).unwrap();
        let config=read_network_config(&root.join("network.json")).unwrap();assert!(config.enabled);assert_eq!(config.max_clients,3);let _=fs::remove_dir_all(root);
    }
    #[test]fn atomic_write_replaces_complete_document(){
        let root=env::temp_dir().join(format!("production-network-atomic-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();let path=root.join("value.json");
        atomic_write(&path,b"first").unwrap();atomic_write(&path,b"second").unwrap();assert_eq!(fs::read(path).unwrap(),b"second");let _=fs::remove_dir_all(root);
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
    #[test]fn shared_session_metadata_does_not_store_person_name(){
        let root=env::temp_dir().join(format!("production-network-private-session-{}",Uuid::new_v4()));let client=runtime(&root,"client");
        client.login(Some(user(7,"reviewer"))).unwrap();let files=client.session_metadata().unwrap();assert_eq!(files.len(),1);
        let json=fs::read_to_string(&files[0].0).unwrap();assert!(!json.contains("Пользователь 7"));assert!(!json.contains("displayName"));
        drop(client);let _=fs::remove_dir_all(root);
    }
}
