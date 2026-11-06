use crate::{audit::{AuditUserSummary, CreateAuditUserInput}, config::{self, Config}, production::{self, ProductionDb, ProductionSnapshot}, sessions::Sessions};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, io::{Read, Write}, path::{Component, Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}, time::{Duration, Instant}};
use tiny_http::{Header, Method, Request, Response, Server, SslConfig, StatusCode};

const MAX_BODY: u64 = 32 * 1024 * 1024;
const COOKIE: &str = "production_cycle_session";

pub struct App {
    pub db: ProductionDb,
    sessions: Sessions,
    data: PathBuf,
    failed: HashMap<String, (Instant, usize)>,
}
struct Reply { value: Value, cookie: Option<String> }
fn value<T: serde::Serialize>(value: T) -> Result<Reply, String> { Ok(Reply { value: serde_json::to_value(value).map_err(|e|e.to_string())?, cookie: None }) }
fn arg<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, String> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(Value::Null)).map_err(|_|format!("Некорректное поле {name}"))
}

impl App {
    fn open(root: &Path, config: &Config) -> Result<(Self, fs::File), String> {
        let data = config::inside(root, &config.data_directory)?;
        let lock = config::lock_data(&data)?;
        let real = data.canonicalize().map_err(|e|e.to_string())?;
        if !config::local_path(&real) { return Err("База сервера должна находиться на локальном диске.".into()); }
        let db = ProductionDb::open_with_backup_dir(&data.join("production-cycle.db"), Some(data.join("backups"))).map_err(|e|e.to_string())?;
        Ok((Self { db, sessions: Sessions::new(config.max_clients, Duration::from_secs(config.session_timeout_seconds)), data, failed: HashMap::new() }, lock))
    }
    fn signed_user(&mut self, token: &str) -> Result<AuditUserSummary, String> {
        self.sessions.user(token)?.ok_or_else(||"Сначала создайте единственного администратора программы.".to_string()).and_then(|u| {
            if self.db.list_audit_users()?.iter().any(|a|a.id==u.id && a.active && a.key_fingerprint==u.key_fingerprint) { Ok(u) }
            else { Err("Профиль сеанса изменён или отключён. Войдите повторно.".into()) }
        })
    }
    fn invoke(&mut self, command: &str, args: Value, token: &str, local: bool) -> Result<Reply, String> {
        match command {
            "network_status" => {
                if let Ok(current)=self.sessions.user(token){
                    let users=self.db.list_audit_users()?;
                    let valid=match current{Some(current)=>users.iter().any(|u|u.id==current.id&&u.active&&u.key_fingerprint==current.key_fingerprint),None=>users.is_empty()};
                    if !valid{self.sessions.logout(token);}
                }
                return value(self.sessions.status(token));
            }
            "audit_list_users" => return value(self.db.list_audit_users()?),
            "network_login" => {
                let id: Option<i64> = arg(&args,"userId")?;
                let users = self.db.list_audit_users()?;
                let user = if users.is_empty() {
                    if !local { return Err("Первичная настройка выполняется на самом сервере: откройте https://localhost:8443 и создайте администратора.".into()); }
                    if id.is_some() { return Err("Сначала создайте администратора.".into()); } None
                } else { Some(self.db.authorize_user(id.ok_or("Выберите профиль")?, &arg::<String>(&args,"pin")?)?) };
                let (result, cookie) = self.sessions.login(token, user)?;
                return Ok(Reply { value: result, cookie });
            }
            "network_logout" | "network_finish_eviction" => {
                self.sessions.logout(token); return Ok(Reply { value: Value::Null, cookie: Some(String::new()) });
            }
            "audit_create_user" => {
                let current = self.sessions.user(token)?;
                let input: CreateAuditUserInput = arg(&args,"input")?;
                if current.is_none() && (!local || !self.db.list_audit_users()?.is_empty()) { return Err("Первичная настройка доступна только локально до создания администратора.".into()); }
                let user = self.db.create_audit_user(input)?;
                if current.is_none() { self.sessions.bind(token,user.clone())?; }
                return value(user);
            }
            _ => {}
        }
        let current = self.signed_user(token)?;
        match command {
            "production_validate" => value(production::validate_snapshot(&arg(&args,"snapshot")?).map_err(|e|e.to_string())?),
            "production_load_snapshot" => value(self.db.load_snapshot(&arg::<String>(&args,"orderNo")?).map_err(|e|e.to_string())?),
            "production_load_snapshot_by_id" => value(self.db.load_snapshot_by_id(arg(&args,"projectId")?).map_err(|e|e.to_string())?),
            "production_list_projects" => value(self.db.list_projects().map_err(|e|e.to_string())?),
            "production_list_deadline_control" => value(self.db.list_deadline_control(chrono::Local::now().date_naive()).map_err(|e|e.to_string())?),
            "production_list_dictionary" => value(self.db.list_dictionary(arg(&args,"kind")?).map_err(|e|e.to_string())?),
            "production_export_txt" => value(production::build_txt_export(&arg(&args,"snapshot")?).map_err(|e|e.to_string())?),
            "production_get_management_report" => {
                let snapshot: ProductionSnapshot = arg(&args,"snapshot")?;
                let mut report = production::build_report(&snapshot,chrono::Local::now().date_naive()).map_err(|e|e.to_string())?;
                if let Some(id)=snapshot.database_id { crate::audit::attach_completion_confirmations(&self.db,id,&mut report)?; }
                value(report)
            }
            "audit_authorize_admin" => { self.db.authorize_admin(arg(&args,"userId")?,&arg::<String>(&args,"pin")?)?;self.sessions.elevate(token);value(()) }
            "audit_save_snapshot" => value(self.db.save_signed_snapshot(&arg(&args,"snapshot")?,arg(&args,"userId")?,&arg::<String>(&args,"pin")?,&arg::<String>(&args,"comment")?,arg(&args,"evidence")?)?),
            "audit_list_events" => {
                if !self.sessions.admin(token) { return Err("Откройте административный интерфейс с PIN администратора.".into()); }
                value(self.db.list_audit_events(arg(&args,"projectId")?)?)
            }
            "audit_verify_log" => value(self.db.verify_audit_log(arg(&args,"adminUserId")?,&arg::<String>(&args,"adminPin")?,arg(&args,"projectId")?)?),
            "audit_replace_dictionary_value" => value(self.db.replace_dictionary_value_signed(arg(&args,"kind")?,&arg::<String>(&args,"fromValue")?,&arg::<String>(&args,"toValue")?,arg(&args,"userId")?,&arg::<String>(&args,"pin")?,&arg::<String>(&args,"comment")?)?),
            "backup_list" => {
                if !self.sessions.admin(token) { return Err("Список резервных копий доступен в административном интерфейсе.".into()); }
                value(self.db.list_backups()?)
            }
            "backup_create" => value(self.db.create_backup_as_admin(arg(&args,"adminUserId")?,&arg::<String>(&args,"adminPin")?)?),
            "backup_restore" => {
                let result=self.db.restore_backup_as_admin(&arg::<String>(&args,"fileName")?,arg(&args,"adminUserId")?,&arg::<String>(&args,"adminPin")?)?;
                self.sessions.retain_current(token);
                value(result)
            }
            "network_save_draft" => {
                let snapshot: ProductionSnapshot = arg(&args,"snapshot")?;
                let session = format!("{:x}",Sha256::digest(token.as_bytes()));
                let folder=self.data.join("drafts").join(current.id.to_string()).join(&session[..16]);
                fs::create_dir_all(&folder).map_err(|e|config::io_error("Не удалось сохранить черновик",&folder,e))?;
                let path=folder.join(snapshot.database_id.map(|id|id.to_string()).unwrap_or("new".into())+".json");
                atomic_write(&path,&serde_json::to_vec(&json!({"format":"production-cycle-browser-draft-v1","userId":current.id,"savedAt":chrono::Local::now().to_rfc3339(),"snapshot":snapshot})).map_err(|e|e.to_string())?)?;
                value(json!({"location":"shared","savedAt":chrono::Local::now().to_rfc3339()}))
            }
            _ => Err("Неизвестная или запрещённая операция. Изменения базы требуют подписанного сохранения.".into()),
        }
    }
}

fn atomic_write(path:&Path,bytes:&[u8])->Result<(),String>{
    let temp=path.with_extension(format!("{}.tmp",uuid::Uuid::new_v4()));
    let mut file=fs::OpenOptions::new().create_new(true).write(true).open(&temp).map_err(|e|e.to_string())?;
    file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
    #[cfg(not(windows))] let result=fs::rename(&temp,path).map_err(|e|e.to_string());
    #[cfg(windows)] let result={
        use std::os::windows::ffi::OsStrExt;
        #[link(name="kernel32")] extern "system" {fn MoveFileExW(from:*const u16,to:*const u16,flags:u32)->i32;}
        let from:Vec<_>=temp.as_os_str().encode_wide().chain(Some(0)).collect();let to:Vec<_>=path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe{MoveFileExW(from.as_ptr(),to.as_ptr(),9)}!=0{Ok(())}else{Err(std::io::Error::last_os_error().to_string())}
    };
    if result.is_err(){let _=fs::remove_file(temp);}result
}

fn header(request:&Request,name:&str)->Option<String>{request.headers().iter().find(|h|h.field.as_str().as_str().eq_ignore_ascii_case(name)).map(|h|h.value.as_str().to_string())}
fn token(request:&Request)->String{header(request,"Cookie").unwrap_or_default().split(';').find_map(|v|{let (k,v)=v.trim().split_once('=')?;if k==COOKIE{Some(v.to_string())}else{None}}).unwrap_or_default()}
fn response(body:Vec<u8>,status:u16,mime:&str)->Response<std::io::Cursor<Vec<u8>>>{
    let mut r=Response::from_data(body).with_status_code(StatusCode(status));
    for (key,value) in [("Content-Type",mime),("Cache-Control","no-store"),("X-Content-Type-Options","nosniff"),("X-Frame-Options","DENY"),("Referrer-Policy","same-origin"),("Content-Security-Policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")] {
        if let Ok(h)=Header::from_bytes(key,value){r.add_header(h);}
    }r
}
fn error(request:Request,status:u16,message:&str){let _=request.respond(response(serde_json::to_vec(&json!({"ok":false,"error":message})).unwrap_or_default(),status,"application/json; charset=utf-8"));}
fn allowed_host(request:&Request,config:&Config)->bool{
    let Some(host)=header(request,"Host") else{return false};
    let scheme=if config.tls{"https"}else{"http"};let Ok(url)=url::Url::parse(&format!("{scheme}://{host}"))else{return false};
    let Ok(public)=url::Url::parse(&config.public_url)else{return false};
    let port=public.port_or_known_default();
    url.origin()==public.origin() || (url.port_or_known_default()==port&&matches!(url.host_str(),Some("localhost"|"127.0.0.1"|"[::1]")))
}
fn same_origin(request:&Request,config:&Config)->bool{
    match header(request,"Origin") {
        None=>true,
        Some(origin)=>{
            let scheme=if config.tls{"https"}else{"http"};let host=header(request,"Host").unwrap_or_default();
            match (url::Url::parse(&origin),url::Url::parse(&format!("{scheme}://{host}"))){(Ok(a),Ok(b))=>a.origin()==b.origin(),_=>false}
        }
    }
}

#[derive(Deserialize)] struct Invocation {command:String, #[serde(default)] args:Value}
fn handle(mut request:Request,root:&Path,config:&Config,app:&Mutex<App>,runtime:&crate::launcher::Runtime,stop:&AtomicBool){
    if !allowed_host(&request,config){error(request,400,"Адрес запроса не соответствует настройке publicUrl сервера.");return;}
    let path=request.url().split('?').next().unwrap_or("/").to_string();
    if path=="/__local/stop" {
        if request.method()!=&Method::Post || !same_origin(&request,config)
            || !request.remote_addr().map(|a|a.ip().is_loopback()).unwrap_or(false)
            || header(&request,"X-Production-Control").as_deref()!=Some(runtime.token.as_str()) {
            error(request,403,"Локальное управление недоступно этому запросу.");return;
        }
        if runtime.mode=="service" { error(request,409,"Программа работает как служба. Для остановки используйте Службы Windows или Remove-Service.ps1.");return; }
        stop.store(true,Ordering::SeqCst);
        let _=request.respond(response(b"{\"stopping\":true}".to_vec(),200,"application/json"));return;
    }
    if path.starts_with("/api/"){
        if request.method()!=&Method::Post || !same_origin(&request,config){error(request,403,"Запрос отклонён: требуется обращение с самого приложения.");return;}
        let logout=path=="/api/logout";
        if !logout && (path!="/api/invoke" || header(&request,"X-Production-Request").as_deref()!=Some("1") || !header(&request,"Content-Type").unwrap_or_default().starts_with("application/json")) {error(request,403,"Некорректный запрос приложения.");return;}
        if request.body_length().map(|n|n as u64>MAX_BODY).unwrap_or(false){error(request,413,"Размер проекта превышает 32 МиБ.");return;}
        let mut bytes=Vec::new();if request.as_reader().take(MAX_BODY+1).read_to_end(&mut bytes).is_err()||bytes.len()as u64>MAX_BODY{error(request,400,"Не удалось прочитать запрос.");return;}
        let Ok(mut app)=app.lock()else{error(request,503,"Сервер временно недоступен.");return;};
        let token=token(&request);
        let result=if logout{app.sessions.logout(&token);Ok(Reply{value:Value::Null,cookie:Some(String::new())})}else{
            let input:Invocation=match serde_json::from_slice(&bytes){Ok(v)=>v,Err(_)=>{drop(app);error(request,400,"Некорректный JSON запроса.");return;}};
            let remote=request.remote_addr().map(|a|a.ip().to_string()).unwrap_or_default();
            app.failed.retain(|_,(time,_)|time.elapsed()<Duration::from_secs(60));
            if app.failed.get(&remote).map(|(_,n)|*n>=5).unwrap_or(false){drop(app);error(request,429,"Слишком много неверных PIN. Повторите через минуту.");return;}
            let local=request.remote_addr().map(|a|a.ip().is_loopback()).unwrap_or(false);
            let result=app.invoke(&input.command,input.args,&token,local);
            if result.as_ref().err().map(|e|e.contains("PIN")||e.contains("ключ подписи")).unwrap_or(false){let e=app.failed.entry(remote).or_insert((Instant::now(),0));e.1+=1;}
            result
        };
        drop(app);
        match result{
            Ok(reply)=>{
                let mut r=response(serde_json::to_vec(&json!({"ok":true,"result":reply.value})).unwrap_or_default(),200,"application/json; charset=utf-8");
                if let Some(cookie)=reply.cookie{let secure=if config.tls{"; Secure"}else{""};let expiry=if cookie.is_empty(){"; Max-Age=0"}else{""};let value=format!("{COOKIE}={cookie}; Path=/api; HttpOnly; SameSite=Strict{secure}{expiry}");if let Ok(h)=Header::from_bytes("Set-Cookie",value){r.add_header(h);}}
                let _=request.respond(r);
            }
            Err(message)=>error(request,400,&message),
        }
        return;
    }
    if request.method()!=&Method::Get && request.method()!=&Method::Head{error(request,405,"Метод не поддерживается.");return;}
    if path=="/health"{let _=request.respond(response(serde_json::to_vec(&runtime.health()).unwrap_or_default(),200,"application/json"));return;}
    if path=="/server-config.js"{let _=request.respond(response(b"window.__PRODUCTION_SERVER__={enabled:true};".to_vec(),200,"text/javascript; charset=utf-8"));return;}
    let mut relative=path.trim_start_matches('/').to_string();
    if relative.is_empty()||relative=="index.html"{
        let legacy=header(&request,"User-Agent").map(|a|a.contains("Trident/")||a.contains("MSIE ")).unwrap_or(false)||request.url().contains("legacy=1");
        relative=if legacy{"legacy/index.html"}else{"index.html"}.into();
    }
    if Path::new(&relative).components().any(|c|!matches!(c,Component::Normal(_)))||relative.contains('%')||relative.contains('\\'){error(request,404,"Файл не найден.");return;}
    let frontend=root.join("frontend");let path=frontend.join(&relative);
    let Some(real)=path.canonicalize().ok().filter(|p|p.starts_with(&frontend)&&p.is_file())else{error(request,404,"Файл не найден. Распакуйте серверный ZIP целиком.");return;};
    let extension=real.extension().and_then(|x|x.to_str()).unwrap_or("");
    let mime=match extension{"html"=>"text/html; charset=utf-8","js"=>"text/javascript; charset=utf-8","css"=>"text/css; charset=utf-8","json"=>"application/json; charset=utf-8","svg"=>"image/svg+xml","woff2"=>"font/woff2","woff"=>"font/woff","png"=>"image/png",_=>{error(request,404,"Файл не найден.");return;}};
    let Ok(mut bytes)=fs::read(&real)else{error(request,500,"Не удалось прочитать файл интерфейса.");return;};
    if extension=="html"{let html=String::from_utf8_lossy(&bytes).replace("</head>","<script src=\"/server-config.js\"></script></head>");bytes=html.into_bytes();}
    if request.method()==&Method::Head{bytes.clear();}
    let _=request.respond(response(bytes,200,mime));
}

pub fn run(root:&Path,config:Config,stop:Arc<AtomicBool>)->Result<(),String>{
    run_with_ready(root,config,stop,"console",None)
}
pub fn run_with_ready(root:&Path,config:Config,stop:Arc<AtomicBool>,mode:&str,ready:Option<std::sync::mpsc::Sender<()>>)->Result<(),String>{
    let root=root.canonicalize().map_err(|e|e.to_string())?;config.validate()?;config.prepare_tls(&root)?;
    let (state,_data_lock)=App::open(&root,&config)?;
    let legacy=root.join("frontend").join("legacy").join("app.js");
    if !legacy.is_file(){return Err(format!("В поставке отсутствует файл интерфейса IE11: {}. Распакуйте готовый серверный ZIP целиком.",legacy.display()));}
    let server=if config.tls{Server::https(&config.listen,SslConfig{certificate:fs::read(config::inside(&root,&config.certificate)?).map_err(|e|e.to_string())?,private_key:fs::read(config::inside(&root,&config.private_key)?).map_err(|e|e.to_string())?})}else{Server::http(&config.listen)}.map_err(|e|format!("Не удалось открыть {}: {e}. Проверьте, не занят ли порт.",config.listen))?;
    let runtime=Arc::new(crate::launcher::Runtime::create(&root,&config,mode)?);
    if let Some(ready)=ready { let _=ready.send(()); }
    let state=Arc::new(Mutex::new(state));let workers=Arc::new(AtomicUsize::new(0));let mut handles=Vec::new();
    while !stop.load(Ordering::SeqCst){
        if let Some(request)=server.recv_timeout(Duration::from_millis(200)).map_err(|e|e.to_string())?{
            if workers.load(Ordering::SeqCst)>=16{error(request,503,"Сервер занят. Повторите запрос.");continue;}
            workers.fetch_add(1,Ordering::SeqCst);let workers=workers.clone();let state=state.clone();let root=root.clone();let config=config.clone();let runtime=runtime.clone();let stop=stop.clone();
            handles.retain(|handle: &std::thread::JoinHandle<()>|!handle.is_finished());
            handles.push(std::thread::spawn(move||{handle(request,&root,&config,&state,&runtime,&stop);workers.fetch_sub(1,Ordering::SeqCst);}));
        }
    }
    // Finish in-flight saves before releasing the exclusive database ownership.
    for handle in handles { let _=handle.join(); }
    Ok(())
}
