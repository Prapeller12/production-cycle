//! Local launch preparation. Never execute Fixed WebView2 from a share.
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env,fs::{self,File,OpenOptions},io::{Read,Write},path::{Component,Path,PathBuf},process::Child,time::{Duration,Instant}};
use uuid::Uuid;

pub const STATUS_ENV:&str="PRODUCTION_CYCLE_LAUNCH_STATUS";
pub const MANIFEST:&str="launch-manifest.json";
const FINGERPRINT:&str=".launch-fingerprint";

pub fn io_error(operation:&str,path:&Path,error:std::io::Error)->String{
    format!("{operation}\nПуть: {}\nПричина: {error}\nКод ОС: {}",path.display(),error.raw_os_error().map(|v|v.to_string()).unwrap_or_else(||"не указан".into()))
}
pub fn create_dir(path:&Path)->Result<(),String>{fs::create_dir_all(path).map_err(|e|io_error("Не удалось создать папку",path,e))}

/// Check the operations used by this application, not just directory existence.
pub fn probe_directory(path:&Path)->Result<(),String>{
    create_dir(path)?;
    let source=path.join(format!(".launch-probe-{}",Uuid::new_v4()));let target=source.with_extension("renamed");
    let result=(||{
        let mut file=OpenOptions::new().create_new(true).read(true).write(true).open(&source).map_err(|e|io_error("Нет доступа к записи в рабочую папку",&source,e))?;
        file.try_lock_exclusive().map_err(|e|io_error("Недоступна файловая блокировка",&source,e))?;
        file.write_all(b"launch-check").and_then(|_|file.sync_all()).map_err(|e|io_error("Не удалось записать пробный файл",&source,e))?;
        FileExt::unlock(&file).map_err(|e|io_error("Не удалось снять пробную блокировку",&source,e))?;drop(file);
        fs::rename(&source,&target).map_err(|e|io_error("Нет доступа к переименованию в рабочей папке",&source,e))?;
        fs::remove_file(&target).map_err(|e|io_error("Нет доступа к удалению в рабочей папке",&target,e))
    })();
    let _=fs::remove_file(&source);let _=fs::remove_file(&target);result
}

pub fn cache_bases()->Result<Vec<PathBuf>,String>{
    let mut candidates=Vec::new();
    if let Some(path)=env::var_os("LOCALAPPDATA"){candidates.push(PathBuf::from(path));}
    if let Some(path)=env::var_os("USERPROFILE"){candidates.push(PathBuf::from(path).join("AppData/Local"));}
    candidates.push(env::temp_dir());
    let mut bases=Vec::new();let mut errors=Vec::new();
    for candidate in candidates{
        let result=(||{
            create_dir(&candidate)?;
            let real=candidate.canonicalize().map_err(|e|io_error("Не удалось определить локальный путь",&candidate,e))?;
            if !super::network::is_local_disk(&real){return Err(format!("Локальный профиль перенаправлен в сеть: {}",real.display()))}
            let path=real.join("ProductionCycleNetwork/cache");probe_directory(&path)?;
            let base=path.canonicalize().map_err(|e|io_error("Не удалось определить папку локального кэша",&path,e))?;
            if !super::network::is_local_disk(&base){return Err(format!("Локальный кэш перенаправлен в сеть: {}",base.display()))}
            Ok(base)
        })();
        match result{Ok(base)=>if !bases.contains(&base){bases.push(base)},Err(e)=>errors.push(e)}
    }
    if bases.is_empty(){return Err(format!("Нет доступной локальной папки клиента.\n{}",errors.join("\n\n")))}
    Ok(bases)
}

#[derive(Debug,Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase")]
struct Entry{path:String,bytes:u64,sha256:String}
#[derive(Debug,Deserialize,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Manifest{format:String,version:String,files:Vec<Entry>,#[serde(skip)]fingerprint:String}

fn payload_path(path:&str)->bool{
    !path.contains('\\') && !path.contains(':') && Path::new(path).components().all(|p|matches!(p,Component::Normal(_))) &&
    (path=="production-cycle.exe" || ["frontend/","runtime/","config/","app/"].iter().any(|p|path.starts_with(p)))
}
pub fn load_manifest(root:&Path,version:&str)->Result<Manifest,String>{
    let path=root.join(MANIFEST);let metadata=fs::metadata(&path).map_err(|e|io_error("Не удалось прочитать состав сетевой поставки",&path,e))?;
    if metadata.len()>2*1024*1024{return Err(format!("Слишком большой манифест поставки: {}",path.display()))}
    let bytes=fs::read(&path).map_err(|e|io_error("Не удалось прочитать манифест поставки",&path,e))?;
    let mut value:Manifest=serde_json::from_slice(&bytes).map_err(|e|format!("Некорректный манифест {}: {e}",path.display()))?;
    if value.format!="production-cycle-launch-v1"||value.version!=version||value.files.is_empty()||value.files.len()>20000{return Err("Манифест не соответствует версии сетевой поставки. Распакуйте новый ZIP целиком.".into())}
    let mut names=std::collections::HashSet::new();
    for item in &value.files{
        if !payload_path(&item.path)||!names.insert(item.path.to_lowercase())||item.sha256.len()!=64||!item.sha256.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err(format!("Некорректный путь или хеш в манифесте: {}",item.path))
        }
    }
    for required in ["production-cycle.exe","frontend/index.html","runtime/webview2/msedgewebview2.exe","config/portable.json","config/network.json"]{
        if !names.contains(required){return Err(format!("В манифесте нет обязательного файла {required}"))}
    }
    value.fingerprint=format!("{:x}",Sha256::digest(&bytes));Ok(value)
}
fn file_hash(path:&Path)->Result<String,String>{
    let mut file=File::open(path).map_err(|e|io_error("Не удалось открыть файл поставки",path,e))?;
    let mut hash=Sha256::new();let mut buffer=[0u8;131072];
    loop{let count=file.read(&mut buffer).map_err(|e|io_error("Не удалось прочитать файл поставки",path,e))?;if count==0{break}hash.update(&buffer[..count]);}
    Ok(format!("{:x}",hash.finalize()))
}
fn verify(root:&Path,manifest:&Manifest)->Result<(),String>{
    let real=root.canonicalize().map_err(|e|io_error("Не удалось открыть локальную копию",root,e))?;
    for item in &manifest.files{
        if super::startup::cancelled(){return Err("Запуск отменён пользователем.".into())}
        let path=root.join(&item.path);let resolved=path.canonicalize().map_err(|e|io_error("Не найден файл локального клиента",&path,e))?;
        if !resolved.starts_with(&real){return Err(format!("Файл клиента находится за пределами его папки: {}",path.display()))}
        let metadata=fs::metadata(&resolved).map_err(|e|io_error("Не удалось проверить файл клиента",&resolved,e))?;
        if metadata.len()!=item.bytes||file_hash(&resolved)?!=item.sha256.to_lowercase(){return Err(format!("Повреждён файл клиента: {}",path.display()))}
    }Ok(())
}

/// Existing clients are immutable. Rebuild beside them; never remove a live EXE,
/// an old user's cache, a database or recovery queue to get a new window open.
pub fn prepare_cache(root:&Path,base:&Path,origin:&str,_version:&str,manifest:&Manifest)->Result<PathBuf,String>{
    let prefix=format!("{}-{}",&origin[..12],&manifest.fingerprint[..12]);
    for entry in fs::read_dir(base).map_err(|e|io_error("Не удалось прочитать список локальных копий",base,e))?{
        let entry=entry.map_err(|e|io_error("Не удалось прочитать локальную копию",base,e))?;
        let name=entry.file_name().to_string_lossy().to_string();if !name.starts_with(&prefix){continue}
        let cache=entry.path();
        let Ok(real)=cache.canonicalize() else {continue};
        if !real.starts_with(base)||!super::network::is_local_disk(&real){continue}
        if fs::read_to_string(cache.join(FINGERPRINT)).ok().as_deref()==Some(manifest.fingerprint.as_str()) &&
            fs::read_to_string(cache.join(".network-shared-root")).ok().as_deref()==Some(origin) && verify(&cache,manifest).is_ok() && probe_directory(&cache.join("temp")).is_ok() && probe_directory(&cache.join("exports")).is_ok(){return Ok(cache)}
    }
    let cache=base.join(format!("{prefix}-{}",&Uuid::new_v4().simple().to_string()[..8]));create_dir(&cache)?;
    for (index,item) in manifest.files.iter().enumerate(){
        if super::startup::cancelled(){return Err("Запуск отменён пользователем.".into())}
        super::startup::update(&format!("Подготовка локального клиента: файл {} из {}\n{}",index+1,manifest.files.len(),item.path));
        let source=root.join(&item.path);let target=cache.join(&item.path);
        let resolved=source.canonicalize().map_err(|e|io_error("Нет доступа к исходному файлу в общей папке",&source,e))?;
        if !resolved.starts_with(root){return Err(format!("Исходный файл находится за пределами поставки: {}",source.display()))}
        if let Some(parent)=target.parent(){create_dir(parent)?;}
        fs::copy(&resolved,&target).map_err(|e|format!("Копирование {} → {}\n{}",source.display(),target.display(),io_error("Не удалось скопировать файл клиента",&target,e)))?;
        #[cfg(windows)]{let mut permissions=fs::metadata(&target).map_err(|e|io_error("Не удалось проверить права локального файла",&target,e))?.permissions();if permissions.readonly(){permissions.set_readonly(false);fs::set_permissions(&target,permissions).map_err(|e|io_error("Не удалось снять атрибут только для чтения с локальной копии",&target,e))?;}}
    }
    verify(&cache,manifest)?;probe_directory(&cache.join("temp"))?;probe_directory(&cache.join("exports"))?;
    fs::write(cache.join(".network-shared-root"),origin).map_err(|e|io_error("Не удалось записать привязку общей папки",&cache,e))?;
    fs::write(cache.join(FINGERPRINT),&manifest.fingerprint).map_err(|e|io_error("Не удалось подтвердить целостность клиента",&cache,e))?;
    Ok(cache)
}

pub fn signal_status(value:serde_json::Value){
    if let Some(path)=env::var_os(STATUS_ENV).map(PathBuf::from){let _=write_status(&path,&value);}
}
fn write_status(path:&Path,value:&serde_json::Value)->Result<(),String>{
    if let Some(parent)=path.parent(){create_dir(parent)?;}
    let temp=path.with_extension(format!("{}.tmp",Uuid::new_v4()));
    fs::write(&temp,serde_json::to_vec_pretty(value).map_err(|e|e.to_string())?).map_err(|e|io_error("Не удалось записать подтверждение запуска",&temp,e))?;
    fs::rename(&temp,path).map_err(|e|io_error("Не удалось опубликовать подтверждение запуска",path,e))
}
pub fn diagnostic_report(value:&serde_json::Value){
    if let Some(path)=env::var_os("PRODUCTION_CYCLE_STARTUP_REPORT").map(PathBuf::from){let _=write_status(&path,value);}
}
pub fn wait_for_window(child:&mut Child,status:&Path,client:&Path)->Result<serde_json::Value,String>{
    let started=Instant::now();
    loop{
        if let Ok(bytes)=fs::read(status){
            if let Ok(value)=serde_json::from_slice::<serde_json::Value>(&bytes){
                if value["state"]=="ready"&&value["processId"].as_u64()==Some(child.id() as u64){let _=fs::remove_file(status);return Ok(value)}
                if value["state"]=="failed"{let _=fs::remove_file(status);return Err(value["error"].as_str().unwrap_or("Клиент не открыл окно.").into())}
            }
        }
        if super::startup::cancelled(){let _=child.kill();let _=child.wait();return Err("Запуск отменён пользователем.".into())}
        if let Some(exit)=child.try_wait().map_err(|e|io_error("Не удалось проверить процесс клиента",client,e))?{return Err(format!("Локальный клиент завершился до открытия окна.\nПуть: {}\nКод завершения: {:?}\nПроверьте разрешение запуска EXE и WebView2 в настройках защиты Windows.",client.display(),exit.code()))}
        if started.elapsed()>Duration::from_secs(120){let _=child.kill();let _=child.wait();return Err(format!("Клиент не подтвердил открытие окна за 120 секунд.\nПуть: {}\nПроверьте разрешения EXE, WebView2 и локального профиля.",client.display()))}
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]mod tests{
    use super::*;
    fn folder()->PathBuf{let p=env::temp_dir().join(format!("production-launch-test-{}",Uuid::new_v4()));fs::create_dir_all(&p).unwrap();p.canonicalize().unwrap()}
    fn fixture(root:&Path)->Manifest{
        let mut files=Vec::new();for path in ["production-cycle.exe","frontend/index.html","runtime/webview2/msedgewebview2.exe","config/portable.json","config/network.json"]{let file=root.join(path);fs::create_dir_all(file.parent().unwrap()).unwrap();fs::write(&file,path.as_bytes()).unwrap();files.push(Entry{path:path.into(),bytes:path.len() as u64,sha256:file_hash(&file).unwrap()});}
        let m=Manifest{format:"production-cycle-launch-v1".into(),version:"test".into(),files,fingerprint:String::new()};fs::write(root.join(MANIFEST),serde_json::to_vec(&m).unwrap()).unwrap();load_manifest(root,"test").unwrap()
    }
    #[test]fn manifest_rejects_escape_absolute_and_duplicate_payloads(){
        for path in ["../production-cycle.exe","frontend/../../evil","C:/evil","/frontend/index.html","frontend\\escape","data/production-cycle.db"]{assert!(!payload_path(path),"{path}")}
        let root=folder();let mut m=fixture(&root);m.files.push(m.files[0].clone());fs::write(root.join(MANIFEST),serde_json::to_vec(&m).unwrap()).unwrap();assert!(load_manifest(&root,"test").unwrap_err().contains("Некорректный"));fs::remove_dir_all(root).unwrap();
    }
    #[test]fn corrupt_cache_is_rebuilt_without_deleting_old_data_or_running_assets(){
        let root=folder();let source=root.join("source");let base=root.join("cache");fs::create_dir_all(&source).unwrap();fs::create_dir_all(&base).unwrap();let m=fixture(&source);let origin="a".repeat(64);
        let first=prepare_cache(&source,&base,&origin,"test",&m).unwrap();assert_eq!(prepare_cache(&source,&base,&origin,"test",&m).unwrap(),first);
        fs::create_dir_all(first.join("data/network-pending")).unwrap();fs::write(first.join("data/network-pending/unsent.json"),b"keep").unwrap();fs::write(first.join("frontend/index.html"),b"damaged").unwrap();
        let second=prepare_cache(&source,&base,&origin,"test",&m).unwrap();assert_ne!(first,second);assert_eq!(fs::read(first.join("data/network-pending/unsent.json")).unwrap(),b"keep");verify(&second,&m).unwrap();fs::remove_dir_all(root).unwrap();
    }
    #[test]fn failed_file_operations_include_operation_path_and_os_code(){
        let root=folder();let blocked=root.join("blocked");fs::write(&blocked,b"file").unwrap();let error=probe_directory(&blocked).unwrap_err();assert!(error.contains("создать папку"));assert!(error.contains(&blocked.display().to_string()));assert!(error.contains("Код ОС"));fs::remove_dir_all(root).unwrap();
    }
    #[test]fn source_corruption_cannot_become_a_ready_client(){
        let root=folder();let source=root.join("source");let base=root.join("cache");fs::create_dir_all(&source).unwrap();fs::create_dir_all(&base).unwrap();let m=fixture(&source);fs::write(source.join("frontend/index.html"),b"broken").unwrap();assert!(prepare_cache(&source,&base,&"a".repeat(64),"test",&m).unwrap_err().contains("Повреждён"));for entry in fs::read_dir(&base).unwrap(){assert!(!entry.unwrap().path().join(FINGERPRINT).exists())}fs::remove_dir_all(root).unwrap();
    }
}
