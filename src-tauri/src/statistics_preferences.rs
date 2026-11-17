//! Personal Statistics files. No SQLite connection, business-write lock or PIN
//! participates in this store. Shared JSON is authoritative; recovery files are local.
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, fs::{self, File, OpenOptions}, io::Read, path::{Path, PathBuf}, sync::Mutex};
use uuid::Uuid;

const MAX_BYTES: u64 = 256 * 1024;
const FORMAT: &str = "production-cycle-statistics-preferences-v2";
const WIDGETS: [&str; 9] = ["participation", "enterprises", "portfolio", "calendar", "executors", "risks", "statuses", "events", "quality"];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Workspace { format: String, workspace_id: String }

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document { format: String, workspace_id: String, key_fingerprint: String, revision: u64, settings: Value, saved_at: String }

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Draft { format: String, workspace_id: String, key_fingerprint: String, draft_id: String, settings: Value, expected_revision: u64, saved_at: String }

pub struct StatisticsPreferences {
    store: Mutex<Result<PreferenceStore, String>>,
    authoritative: PathBuf,
    local_base: Result<PathBuf, String>,
    shared: bool,
    session_secret: String,
    // The observed draft id prevents an old clear from deleting a newer draft
    // produced by another process. The file itself survives process restarts.
    observed_drafts: Mutex<HashMap<String, Option<String>>>,
}

#[derive(Clone)]
struct PreferenceStore { authoritative: PathBuf, local: PathBuf, workspace_id: String, shared: bool }

fn lock(directory: &Path) -> Result<File, String> {
    fs::create_dir_all(directory).map_err(|e| format!("Не удалось открыть папку настроек: {e}"))?;
    let file = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(directory.join("preferences.lock")).map_err(|e|e.to_string())?;
    file.try_lock_exclusive().map_err(|e|format!("Настройки заняты другим процессом. Повторите: {e}"))?;
    Ok(file)
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    let mut file = match File::open(path) { Ok(v)=>v, Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None), Err(e)=>return Err(e.to_string()) };
    let mut bytes = Vec::new(); file.by_ref().take(MAX_BYTES+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Файл настроек превышает 256 КиБ.".into()); }
    serde_json::from_slice(&bytes).map(Some).map_err(|e|format!("Файл настроек повреждён: {e}"))
}
fn write_json(path:&Path, value:&impl Serialize)->Result<(),String>{
    let bytes=serde_json::to_vec_pretty(value).map_err(|e|e.to_string())?;
    // Pretty output includes envelope metadata; settings are separately limited.
    if bytes.len() as u64>MAX_BYTES{return Err("Файл настроек превышает 256 КиБ.".into())}
    crate::infrastructure::network::atomic_write(path,&bytes)
}
fn fingerprint(value:&str)->Result<String,String>{
    if value.len()!=16 || !value.bytes().all(|b|b.is_ascii_hexdigit()){return Err("У профиля отсутствует корректный отпечаток ключа.".into())}
    Ok(value.to_ascii_uppercase())
}

/// Strictly validate the persistent screen schema; owner ids and session/filter
/// state are intentionally absent. A raw v1 archive remains intact for export.
pub fn validate_settings(value:&Value)->Result<(),String>{
    let object=value.as_object().ok_or("Некорректные настройки статистики.")?;
    let allowed=["version","theme","font","horizon","countMode","columns","widgets","legacyArchive"];
    if object.keys().any(|key|!allowed.contains(&key.as_str())) || value["version"]!=2
        || !matches!(value["theme"].as_str(),Some("light"|"dark"|"system"))
        || !matches!(value["font"].as_str(),Some("normal"|"large"))
        || !matches!(value["horizon"].as_u64(),Some(7|14|30))
        || !matches!(value["countMode"].as_str(),Some("all"|"leaves"))
        || !matches!(value["columns"].as_u64(),Some(1|2)) {return Err("Неподдерживаемый формат настроек статистики v2.".into())}
    let widgets=value["widgets"].as_array().ok_or("Не задан порядок блоков.")?;
    let mut seen=HashSet::new();
    if widgets.len()!=WIDGETS.len(){return Err("В настройках должен присутствовать каждый блок статистики.".into())}
    for widget in widgets {
        let w=widget.as_object().ok_or("Некорректный блок статистики.")?;
        let id=widget["id"].as_str().ok_or("Не задан блок статистики.")?;
        if w.keys().any(|k|!["id","visible","width"].contains(&k.as_str())) || !WIDGETS.contains(&id) || !seen.insert(id)
            || !widget["visible"].is_boolean() || !matches!(widget["width"].as_str(),Some("half"|"full")) {return Err("Некорректный порядок или видимость блоков.".into())}
    }
    if let Some(archive)=object.get("legacyArchive") {validate_archive(archive)?;}
    if serde_json::to_vec(value).map_err(|e|e.to_string())?.len()>240*1024{return Err("Настройки и архив превышают 240 КиБ.".into())}
    Ok(())
}
pub(crate) fn validate_archive(value:&Value)->Result<(),String>{
    // The archive may contain the original v1 object, an array of v1 snapshots,
    // or migration metadata containing raw v1/savedViews. Never rewrite it.
    if !value.is_null() && !value.is_object() && !value.is_array(){return Err("Некорректный архив прежних настроек.".into())}
    if serde_json::to_vec(value).map_err(|e|e.to_string())?.len()>240*1024{return Err("Архив прежних настроек превышает 240 КиБ.".into())}
    Ok(())
}

impl PreferenceStore {
    fn open(authoritative:PathBuf,local_base:PathBuf,shared:bool)->Result<Self,String>{
        let _lock=lock(&authoritative)?;
        let marker=authoritative.join("workspace.json");
        let workspace=match read_json::<Workspace>(&marker)? {
            Some(value)=>{
                if value.format!="production-cycle-preferences-workspace-v1" || Uuid::parse_str(&value.workspace_id).map(|id|id.to_string()!=value.workspace_id).unwrap_or(true){return Err("Повреждён идентификатор рабочего пространства настроек.".into())}
                value
            },
            None=>{
                let value=Workspace{format:"production-cycle-preferences-workspace-v1".into(),workspace_id:Uuid::new_v4().to_string()};
                write_json(&marker,&value)?;value
            }
        };
        let local=local_base.join(&workspace.workspace_id);
        fs::create_dir_all(&local).map_err(|e|format!("Не удалось открыть локальную копию настроек: {e}"))?;
        let local_base=local_base.canonicalize().map_err(|e|format!("Не удалось проверить папку локальных настроек: {e}"))?;
        let local=local.canonicalize().map_err(|e|format!("Не удалось проверить локальную копию настроек: {e}"))?;
        if !local.starts_with(&local_base) || !crate::infrastructure::network::is_local_disk(&local){return Err("Копия настроек и черновик должны находиться внутри локальной папки программы, а не по сетевому перенаправлению.".into())}
        Ok(Self{authoritative,local,workspace_id:workspace.workspace_id,shared})
    }
    fn verify_workspace(&self)->Result<(),String>{
        let marker=read_json::<Workspace>(&self.authoritative.join("workspace.json"))?.ok_or("Идентификатор рабочего пространства настроек недоступен.")?;
        if marker.format!="production-cycle-preferences-workspace-v1" || marker.workspace_id!=self.workspace_id{return Err("Рабочее пространство настроек изменилось. Перезапустите программу.".into())}
        Ok(())
    }
    fn directory(&self,fp:&str,local:bool)->PathBuf {if local{self.local.join(fp)}else{self.authoritative.join(fp)}}
    fn read_document(&self,fp:&str,local:bool)->Result<Option<Document>,String>{
        if !local{self.verify_workspace()?;}
        let value=read_json::<Document>(&self.directory(fp,local).join("statistics.json"))?;
        if let Some(document)=&value{
            if document.format!=FORMAT || document.workspace_id!=self.workspace_id || document.key_fingerprint!=fp || document.revision==0{return Err("Файл настроек принадлежит другому профилю или рабочему пространству.".into())}
            validate_settings(&document.settings)?;
        }
        Ok(value)
    }
    fn read_draft(&self,fp:&str)->Result<Option<Draft>,String>{
        let draft=read_json::<Draft>(&self.directory(fp,true).join("draft.json"))?;
        if let Some(value)=&draft{
            if value.format!="production-cycle-statistics-draft-v2" || value.workspace_id!=self.workspace_id || value.key_fingerprint!=fp || Uuid::parse_str(&value.draft_id).is_err(){return Err("Черновик принадлежит другому профилю или рабочему пространству.".into())}
            validate_settings(&value.settings)?;
        }
        Ok(draft)
    }
    fn cache(&self,fp:&str,document:&Document)->Result<(),String>{
        let directory=self.directory(fp,true);let _lock=lock(&directory)?;
        // A delayed read from another process cannot roll the local cache back.
        if self.read_document(fp,true)?.is_some_and(|v|v.revision>document.revision){return Ok(())}
        write_json(&directory.join("statistics.json"),document)
    }
    fn load(&self,fp:&str,offline:Option<String>)->Result<(Value,Option<String>),String>{
        let mut warning=offline;
        let mut location=if self.shared{"shared"}else{"local"};
        let document=if warning.is_none(){match self.read_document(fp,false){
            Ok(value)=>{
                if let Some(ref document)=value{if let Err(error)=self.cache(fp,document){warning=Some(format!("Общие настройки прочитаны, но локальная копия не обновлена: {error}"));}}
                // A previously confirmed cache must not be represented as a new
                // empty authoritative file after accidental deletion on SMB.
                if value.is_none() && self.read_document(fp,true)?.is_some(){location="local";warning=Some("Подтверждённый общий файл настроек недоступен. Показана локальная копия.".into());self.read_document(fp,true)?}else{value}
            },
            Err(error)=>{warning=Some(format!("Общие настройки не прочитаны: {error}. Показана локальная копия."));location="local";self.read_document(fp,true)?}
        }}else{location="local";self.read_document(fp,true)?};
        let draft=match self.read_draft(fp){Ok(v)=>v,Err(error)=>{warning=Some(match warning{Some(w)=>format!("{w} Локальный черновик не прочитан: {error}"),None=>format!("Локальный черновик не прочитан: {error}")});None}};
        let draft_id=draft.as_ref().map(|v|v.draft_id.clone());
        Ok((json!({"state":"loaded","revision":document.as_ref().map_or(0,|v|v.revision),"settings":document.as_ref().map(|v|&v.settings),"draft":draft.map(|v|json!({"settings":v.settings,"expectedRevision":v.expected_revision,"savedAt":v.saved_at})),"location":location,"warning":warning}),draft_id))
    }
    fn save(&self,fp:&str,mut settings:Value,expected_revision:u64)->Result<Value,String>{
        validate_settings(&settings)?;
        self.verify_workspace()?;
        let directory=self.directory(fp,false);let _lock=lock(&directory)?;
        // Re-read under the per-profile OS lock on EVERY attempt, including an
        // explicit overwrite following a previous conflict. No automatic merge.
        let actual=self.read_document(fp,false)?;
        let revision=actual.as_ref().map_or(0,|v|v.revision);
        let location=if self.shared{"shared"}else{"local"};
        if revision!=expected_revision{return Ok(json!({"state":"conflict","revision":revision,"settings":actual.map(|v|v.settings),"location":location}))}
        // A v1 migration archive is retained even if an older frontend omits
        // it on a later edit. Additionally retain exact archive JSON as files;
        // importing a different archive must not destroy an earlier one.
        if settings.get("legacyArchive").map_or(true,|v|v.is_null()) {
            if let Some(archive)=actual.as_ref().and_then(|v|v.settings.get("legacyArchive")).filter(|v|!v.is_null()){settings["legacyArchive"]=archive.clone();}
        }
        if let Some(archive)=settings.get("legacyArchive").filter(|v|!v.is_null()) {
            use sha2::{Digest,Sha256};
            let bytes=serde_json::to_vec(archive).map_err(|e|e.to_string())?;
            let archive_directory=directory.join("legacy-archives");fs::create_dir_all(&archive_directory).map_err(|e|e.to_string())?;
            let target=archive_directory.join(format!("v1-{:x}.json",Sha256::digest(&bytes)));
            if !target.try_exists().map_err(|e|e.to_string())?{write_json(&target,archive)?;}
        }
        let revision=revision.checked_add(1).ok_or("Исчерпан счётчик ревизий настроек.")?;
        let saved_at=chrono::Utc::now().to_rfc3339();
        let document=Document{format:FORMAT.into(),workspace_id:self.workspace_id.clone(),key_fingerprint:fp.into(),revision,settings,saved_at};
        write_json(&directory.join("statistics.json"),&document)?;
        let warning=self.cache(fp,&document).err().map(|e|format!("Настройки сохранены, но локальная копия не обновлена: {e}"));
        Ok(json!({"state":"saved","revision":document.revision,"settings":document.settings,"savedAt":document.saved_at,"location":location,"warning":warning}))
    }
}

impl StatisticsPreferences {
    pub fn new(paths:&crate::infrastructure::portable::Paths,network:&crate::infrastructure::network::NetworkRuntime)->Self{
        let shared=network.preferences_root();
        let authoritative=shared.clone().unwrap_or_else(||paths.data.join("preferences"));
        let local_base=if shared.is_some(){
            // Cache bases are outside both versioned application assets and
            // per-process WebView/SQLite folders; the UUID survives an SMB move.
            crate::infrastructure::launch::cache_bases().and_then(|bases|bases.into_iter().find_map(Result::ok).ok_or("Нет локальной папки настроек.".into())).and_then(|base|base.parent().map(|v|v.join("statistics-preferences")).ok_or("Нет локальной папки настроек.".into()))
        }else{Ok(paths.recovery.join("statistics-preferences"))};
        Self::from_folders(authoritative,local_base,shared.is_some())
    }
    pub(crate) fn from_folders(authoritative:PathBuf,local_base:Result<PathBuf,String>,shared:bool)->Self{
        let store=local_base.clone().and_then(|local|PreferenceStore::open(authoritative.clone(),local,shared));
        Self{store:Mutex::new(store),authoritative,local_base,shared,session_secret:Uuid::new_v4().to_string(),observed_drafts:Mutex::new(HashMap::new())}
    }
    fn store(&self)->Result<PreferenceStore,String>{
        let mut guard=self.store.lock().map_err(|_|"Состояние настроек занято")?;
        if guard.is_err(){*guard=self.local_base.clone().and_then(|local|PreferenceStore::open(self.authoritative.clone(),local,self.shared));}
        guard.as_ref().cloned().map_err(Clone::clone)
    }
    pub fn owner_token(&self,key_fingerprint:&str,generation:u64)->Result<String,String>{
        let fp=fingerprint(key_fingerprint)?;
        Ok(format!("{}:{}:{}:{}",self.session_secret,generation,self.store()?.workspace_id,fp))
    }
    pub fn load(&self,key_fingerprint:&str,owner_token:&str,offline:Option<String>)->Result<Value,String>{
        let fp=fingerprint(key_fingerprint)?;let (mut result,draft_id)=self.store()?.load(&fp,offline)?;
        self.observed_drafts.lock().map_err(|_|"Состояние черновика занято")?.insert(owner_token.into(),draft_id);
        result["ownerToken"]=json!(owner_token);Ok(result)
    }
    pub fn save(&self,key_fingerprint:&str,settings:Value,expected_revision:u64)->Result<Value,String>{self.store()?.save(&fingerprint(key_fingerprint)?,settings,expected_revision)}
    pub fn save_draft(&self,key_fingerprint:&str,owner_token:&str,settings:Value,expected_revision:u64)->Result<Value,String>{
        validate_settings(&settings)?;let store=self.store()?;let fp=fingerprint(key_fingerprint)?;
        let directory=store.directory(&fp,true);let _lock=lock(&directory)?;
        let mut observed=self.observed_drafts.lock().map_err(|_|"Состояние черновика занято")?;
        let expected=observed.get(owner_token).ok_or("Сначала прочитайте текущий черновик.")?;
        let actual=store.read_draft(&fp)?.map(|v|v.draft_id);
        if expected!=&actual{return Err("Локальный черновик изменён другим процессом. Откройте настройки повторно либо экспортируйте свой вариант.".into())}
        let draft=Draft{format:"production-cycle-statistics-draft-v2".into(),workspace_id:store.workspace_id.clone(),key_fingerprint:fp,draft_id:Uuid::new_v4().to_string(),settings,expected_revision,saved_at:chrono::Utc::now().to_rfc3339()};
        write_json(&directory.join("draft.json"),&draft)?;
        observed.insert(owner_token.into(),Some(draft.draft_id));
        Ok(json!({"state":"draft_saved","savedAt":draft.saved_at}))
    }
    pub fn clear_draft(&self,key_fingerprint:&str,owner_token:&str)->Result<(),String>{
        let store=self.store()?;let fp=fingerprint(key_fingerprint)?;let directory=store.directory(&fp,true);let _lock=lock(&directory)?;
        let mut observed=self.observed_drafts.lock().map_err(|_|"Состояние черновика занято")?;
        let expected=observed.get(owner_token).ok_or("Сначала прочитайте текущий черновик.")?;
        let actual=store.read_draft(&fp)?.map(|v|v.draft_id);
        if expected!=&actual{return Err("Черновик изменён другим процессом. Прочитайте его перед удалением.".into())}
        match fs::remove_file(directory.join("draft.json")){Ok(())=>{},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(e.to_string())}
        observed.insert(owner_token.into(),None);Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ALICE:&str="0123456789ABCDEF";
    const BOB:&str="FEDCBA9876543210";
    struct Fixture(PathBuf);
    impl Fixture {
        fn new()->Self{let root=std::env::temp_dir().join(format!("statistics-preferences-test-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();Self(root)}
        fn runtime(&self,share:&str,pc:&str)->StatisticsPreferences{StatisticsPreferences::from_folders(self.0.join(share),Ok(self.0.join(pc)),true)}
    }
    impl Drop for Fixture {fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
    fn settings(columns:u64)->Value{json!({"version":2,"theme":"light","font":"normal","horizon":14,"countMode":"leaves","columns":columns,"widgets":WIDGETS.iter().map(|id|json!({"id":id,"visible":true,"width":"half"})).collect::<Vec<_>>(),"legacyArchive":null})}

    #[test]
    fn confirmed_settings_cross_pcs_but_not_profiles_or_workspaces(){
        let f=Fixture::new();let first=f.runtime("share-a","pc-a");
        assert_eq!(first.save(ALICE,settings(1),0).unwrap()["state"],"saved");
        let another_pc=f.runtime("share-a","pc-b");
        assert_eq!(another_pc.load(ALICE,"a",None).unwrap()["settings"],settings(1));
        assert!(another_pc.load(BOB,"b",None).unwrap()["settings"].is_null());
        let other_workspace=f.runtime("share-b","pc-a");
        assert!(other_workspace.load(ALICE,"c",None).unwrap()["settings"].is_null());
        assert_ne!(first.store().unwrap().workspace_id,other_workspace.store().unwrap().workspace_id);
        assert_ne!(first.owner_token(ALICE,1).unwrap(),first.owner_token(BOB,1).unwrap());
        assert!(first.owner_token("../../other",1).is_err());
    }
    #[test]
    fn workspace_identity_and_cache_survive_folder_move_and_runtime_restart(){
        let f=Fixture::new();let first=f.runtime("share","stable-local");
        first.save(ALICE,settings(1),0).unwrap();let id=first.store().unwrap().workspace_id;drop(first);
        fs::rename(f.0.join("share"),f.0.join("moved-share")).unwrap();
        let restarted=f.runtime("moved-share","stable-local");assert_eq!(restarted.store().unwrap().workspace_id,id);
        assert_eq!(restarted.load(ALICE,"restarted",None).unwrap()["revision"],1);
        fs::rename(f.0.join("moved-share"),f.0.join("temporarily-offline")).unwrap();
        let loaded=restarted.load(ALICE,"restarted",Some("SMB unavailable".into())).unwrap();
        assert_eq!(loaded["location"],"local");assert_eq!(loaded["settings"],settings(1));assert_eq!(loaded["warning"],"SMB unavailable");
    }
    #[test]
    fn every_attempt_compares_actual_revision_under_lock(){
        let f=Fixture::new();let a=f.runtime("share","pc-a");let b=f.runtime("share","pc-b");
        a.save(ALICE,settings(1),0).unwrap();
        let conflict=b.save(ALICE,settings(2),0).unwrap();
        assert_eq!(conflict["state"],"conflict");assert_eq!(conflict["revision"],1);assert_eq!(conflict["settings"],settings(1));
        a.save(ALICE,settings(2),1).unwrap();
        let repeated=b.save(ALICE,settings(1),1).unwrap();
        assert_eq!(repeated["state"],"conflict");assert_eq!(repeated["revision"],2);assert_eq!(repeated["settings"],settings(2));
        assert_eq!(b.save(ALICE,settings(1),2).unwrap()["revision"],3);
        let store=a.store().unwrap();let held=lock(&store.directory(ALICE,false)).unwrap();
        assert!(b.save(ALICE,settings(2),3).is_err());drop(held);
        assert_eq!(a.load(ALICE,"a",None).unwrap()["settings"],settings(1));
    }
    #[test]
    fn local_draft_survives_restart_and_stale_clear_cannot_remove_newer_draft(){
        let f=Fixture::new();let a=f.runtime("share","same-pc");
        a.save(ALICE,settings(1),0).unwrap();a.load(ALICE,"old-process",None).unwrap();
        a.save_draft(ALICE,"old-process",settings(2),1).unwrap();
        let restarted=f.runtime("share","same-pc");let loaded=restarted.load(ALICE,"new-process",None).unwrap();
        assert_eq!(loaded["draft"]["settings"],settings(2));assert_eq!(loaded["draft"]["expectedRevision"],1);
        assert!(restarted.load(BOB,"bob",None).unwrap()["draft"].is_null());
        // An edit on one PC is not represented as a confirmed save on another.
        assert!(f.runtime("share","other-pc").load(ALICE,"remote",None).unwrap()["draft"].is_null());
        restarted.save_draft(ALICE,"new-process",settings(1),1).unwrap();
        assert!(a.save_draft(ALICE,"old-process",settings(2),1).is_err());
        assert!(a.clear_draft(ALICE,"old-process").is_err());
        assert_eq!(restarted.load(ALICE,"new-process",None).unwrap()["draft"]["settings"],settings(1));
        restarted.clear_draft(ALICE,"new-process").unwrap();
        assert!(restarted.load(ALICE,"new-process",None).unwrap()["draft"].is_null());
    }
    #[test]
    fn inaccessible_shared_store_never_reports_saved_and_preserves_local_draft(){
        let f=Fixture::new();let a=f.runtime("share","pc");a.save(ALICE,settings(1),0).unwrap();a.load(ALICE,"owner",None).unwrap();
        a.save_draft(ALICE,"owner",settings(2),1).unwrap();
        fs::rename(f.0.join("share"),f.0.join("offline")).unwrap();fs::write(f.0.join("share"),b"unavailable").unwrap();
        assert!(a.save(ALICE,settings(2),1).is_err());
        let load=a.load(ALICE,"owner",None).unwrap();
        assert_eq!(load["location"],"local");assert!(load["warning"].is_string());
        assert_eq!(load["revision"],1);assert_eq!(load["draft"]["settings"],settings(2));
        a.save_draft(ALICE,"owner",settings(1),1).unwrap(); // No SMB access required.
    }
    #[test]
    fn changing_workspace_marker_blocks_writes_from_old_runtime(){
        let f=Fixture::new();let a=f.runtime("share","pc");
        let replacement=Workspace{format:"production-cycle-preferences-workspace-v1".into(),workspace_id:Uuid::new_v4().to_string()};
        write_json(&f.0.join("share/workspace.json"),&replacement).unwrap();
        assert!(a.save(ALICE,settings(1),0).is_err());
        assert!(!f.0.join("share").join(ALICE).join("statistics.json").exists());
    }
    #[test]
    fn v1_archive_presets_and_named_views_roundtrip_without_loss(){
        let f=Fixture::new();let a=f.runtime("share","pc");let mut migrated=settings(1);
        let legacy=json!({"version":1,"preset":"performer","theme":"dark","font":"large","horizon":30,"countMode":"all","widgets":[{"id":"participation","width":"full","visible":true}],"savedViews":[{"name":"Старое представление","settings":{"version":1,"preset":"enterprise","widgets":[]}}]});
        migrated["legacyArchive"]=legacy.clone();a.save(ALICE,migrated,0).unwrap();
        let saved=a.save(ALICE,settings(2),1).unwrap();assert_eq!(saved["settings"]["legacyArchive"],legacy);
        let loaded=f.runtime("share","other-pc").load(ALICE,"other",None).unwrap();
        assert_eq!(loaded["settings"]["legacyArchive"],legacy);
        let archives=f.0.join("share").join(ALICE).join("legacy-archives");
        let files=fs::read_dir(archives).unwrap().collect::<Result<Vec<_>,_>>().unwrap();assert_eq!(files.len(),1);
        assert_eq!(read_json::<Value>(&files[0].path()).unwrap().unwrap(),legacy);
    }
    #[test]
    fn persistent_schema_rejects_filters_owner_ids_duplicate_blocks_and_oversize(){
        let mut value=settings(2);value["userId"]=json!(1);assert!(validate_settings(&value).is_err());
        let mut value=settings(2);value["filters"]=json!({"search":"secret"});assert!(validate_settings(&value).is_err());
        let mut value=settings(2);value["widgets"][1]=value["widgets"][0].clone();assert!(validate_settings(&value).is_err());
        let mut value=settings(2);value["columns"]=json!(3);assert!(validate_settings(&value).is_err());
        let mut value=settings(2);value["legacyArchive"]=json!({"large":"x".repeat(256*1024)});assert!(validate_settings(&value).is_err());
    }
    #[test]
    fn preferences_and_drafts_do_not_change_the_production_database(){
        let f=Fixture::new();let db_path=f.0.join("business.db");
        let db=crate::production::ProductionDb::open(&db_path).unwrap();let before=fs::read(&db_path).unwrap();
        let changes_before=db.conn.lock().unwrap().total_changes();
        let a=f.runtime("share","pc");a.save(ALICE,settings(1),0).unwrap();a.load(ALICE,"owner",None).unwrap();
        a.save_draft(ALICE,"owner",settings(2),1).unwrap();a.clear_draft(ALICE,"owner").unwrap();
        assert_eq!(db.conn.lock().unwrap().total_changes(),changes_before);
        assert_eq!(fs::read(db_path).unwrap(),before);
    }
}
