//! Signed administrative changes. PINs and private keys never enter this log or reports.
use super::*;
use serde_json::{json, Value};

const PAGE_SIZE: u32 = 50;

pub(super) fn initialize(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS audit_user_change (
      seq INTEGER PRIMARY KEY AUTOINCREMENT,
      target_user_id INTEGER NOT NULL REFERENCES audit_user(id) ON DELETE RESTRICT,
      actor_user_id INTEGER NOT NULL REFERENCES audit_user(id) ON DELETE RESTRICT,
      payload_json TEXT NOT NULL, previous_hash TEXT NOT NULL,
      event_hash TEXT NOT NULL UNIQUE, signature TEXT NOT NULL,
      public_key TEXT NOT NULL, created_at TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_audit_user_change_target ON audit_user_change(target_user_id,seq);
    CREATE INDEX IF NOT EXISTS idx_audit_event_actor ON audit_event(actor_user_id);")
        .map_err(|e| e.to_string())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserState {
    pub display_name: String,
    pub role: String,
    pub active: bool,
    pub key_fingerprint: String,
}
impl From<&AuditUserSummary> for UserState {
    fn from(user: &AuditUserSummary) -> Self {
        Self { display_name: user.display_name.clone(), role: user.role.clone(), active: user.active, key_fingerprint: user.key_fingerprint.clone() }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateUserInput {
    pub target_user_id: i64,
    pub display_name: String,
    pub role: String,
    pub active: bool,
    pub expected: UserState,
    pub admin_user_id: i64,
    pub admin_pin: String,
    pub comment: String,
}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct UserUpdateResult {
    pub user: AuditUserSummary,
    pub backup_warning: Option<String>,
}

fn read_user(conn: &Connection, id: i64) -> Result<AuditUserSummary, String> {
    conn.query_row("SELECT id,display_name,role,key_fingerprint,active,created_at FROM audit_user WHERE id=?1", [id], |row| {
        Ok(AuditUserSummary { id: row.get(0)?, display_name: row.get(1)?, role: row.get(2)?, key_fingerprint: row.get(3)?, active: row.get::<_, i64>(4)? == 1, created_at: row.get(5)? })
    }).optional().map_err(|e| e.to_string())?.ok_or_else(|| "Пользователь не найден. Обновите список.".into())
}

pub(super) struct UserLogVerification {
    pub checked: usize,
    pub valid: usize,
    pub errors: Vec<String>,
}

pub(super) fn verify_registered_changes(conn: &Connection) -> Result<UserLogVerification, String> {
    let mut statement=conn.prepare("SELECT seq,target_user_id,actor_user_id,payload_json,previous_hash,event_hash,signature,public_key,created_at FROM audit_user_change ORDER BY seq").map_err(|e| e.to_string())?;
    let rows=statement.query_map([], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?,row.get::<_,i64>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,String>(5)?,row.get::<_,String>(6)?,row.get::<_,String>(7)?,row.get::<_,String>(8)?)))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    let mut previous=String::new();
    let mut latest=HashMap::<i64,UserState>::new();
    let mut result=UserLogVerification { checked: rows.len(), valid: 0, errors: Vec::new() };
    for (seq,target,actor,payload,stored_previous,hash,signature,public_key,created_at) in rows {
        let value=serde_json::from_str::<Value>(&payload).ok();
        let registered=conn.query_row("SELECT public_key,key_fingerprint,role FROM audit_user WHERE id=?1", [actor], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?))).optional().map_err(|e| e.to_string())?;
        let mut input=stored_previous.as_bytes().to_vec();input.extend_from_slice(payload.as_bytes());
        let after=value.as_ref().and_then(|v|serde_json::from_value::<UserState>(v["after"].clone()).ok());
        let before=value.as_ref().and_then(|v|serde_json::from_value::<UserState>(v["before"].clone()).ok());
        let valid_payload=value.as_ref().is_some_and(|v|v["version"]==1 && v["eventType"]=="user_updated" && v["targetUserId"]==target && v["actorUserId"]==actor && v["createdAt"]==created_at && v["previousHash"]==stored_previous && v["comment"].as_str().is_some_and(|s|!s.trim().is_empty()) && before.is_some() && after.is_some());
        let valid_actor=registered.as_ref().is_some_and(|(key,fp,role)|key==&public_key && role=="admin" && verify_event_signer(conn,actor,fp,&public_key).is_ok() && value.as_ref().is_some_and(|v|v["actorKeyFingerprint"]==*fp));
        let valid_transition=before.as_ref().is_some_and(|b|latest.get(&target).is_none_or(|old|old==b)) && after.as_ref().is_some_and(|a|before.as_ref().is_some_and(|b|a.key_fingerprint==b.key_fingerprint));
        if previous==stored_previous && sha256_hex(&input)==hash && verify_signature(&hash,&signature,&public_key).is_ok() && valid_payload && valid_actor && valid_transition {
            result.valid+=1;
        } else {
            result.errors.push(format!("Изменение профиля #{seq}: нарушена подпись, цепочка или состав записи."));
        }
        if let Some(after)=after { latest.insert(target,after); }
        previous=hash;
    }
    for (id,expected) in latest {
        if read_user(conn,id).map(|user|UserState::from(&user)!=expected).unwrap_or(true) {
            result.errors.push(format!("Профиль #{id}: текущее состояние не совпадает с последним подписанным изменением."));
        }
    }
    Ok(result)
}

impl ProductionDb {
    pub fn update_audit_user(&self, input: UpdateUserInput) -> Result<UserUpdateResult, String> {
        let name=clean(&input.display_name);
        let comment=clean(&input.comment);
        if name.is_empty() || name.chars().count()>160 { return Err("Укажите ФИО длиной от 1 до 160 символов.".into()); }
        if comment.is_empty() || comment.chars().count()>2000 { return Err("Укажите причину изменения профиля: от 1 до 2000 символов.".into()); }
        if !matches!(input.role.as_str(),"admin"|"reviewer"|"project_manager") { return Err("Неизвестная роль пользователя.".into()); }
        let mut conn=self.conn.lock().map_err(|_|"База данных занята")?;
        let (admin,key)=authenticate(&conn,input.admin_user_id,&input.admin_pin,RoleRequirement::Admin)?;
        let tx=conn.transaction().map_err(|e|e.to_string())?;
        let integrity=verify_registered_changes(&tx)?;
        if !integrity.errors.is_empty() { return Err("Журнал управления пользователями повреждён. Выполните проверку целостности и восстановление копии.".into()); }
        let current=read_user(&tx,input.target_user_id)?;
        let before=UserState::from(&current);
        if input.expected!=before { return Err("Этот профиль уже изменён в другом окне. Обновите список и проверьте изменения заново.".into()); }
        if current.role=="admin" && (input.role!="admin" || !input.active) { return Err("Единственного администратора нельзя заблокировать или перевести в другую роль.".into()); }
        if current.role!="admin" && input.role=="admin" { return Err("В программе может быть только один администратор.".into()); }
        let after=UserState {display_name:name,role:input.role,active:input.active,key_fingerprint:current.key_fingerprint.clone()};
        if after==before { return Err("В профиле нет изменений.".into()); }
        let created_at=Local::now().to_rfc3339();
        tx.execute("UPDATE audit_user SET display_name=?1,role=?2,active=?3,revoked_at=CASE WHEN ?3=1 THEN NULL WHEN active=1 THEN ?4 ELSE revoked_at END WHERE id=?5", params![after.display_name,after.role,i64::from(after.active),if after.active {None}else{Some(created_at.as_str())},current.id])
            .map_err(|e|if e.to_string().contains("UNIQUE") {"Пользователь с таким ФИО уже существует.".into()}else{e.to_string()})?;
        let previous:String=tx.query_row("SELECT event_hash FROM audit_user_change ORDER BY seq DESC LIMIT 1",[],|row|row.get(0)).optional().map_err(|e|e.to_string())?.unwrap_or_default();
        let payload=json!({"version":1,"eventType":"user_updated","eventId":Uuid::new_v4().to_string(),"targetUserId":current.id,"actorUserId":admin.id,"actorName":admin.display_name,"actorKeyFingerprint":admin.key_fingerprint,"before":before,"after":after,"comment":comment,"createdAt":created_at,"previousHash":previous});
        let body=serde_json::to_string(&payload).map_err(|e|e.to_string())?;
        let mut digest=previous.as_bytes().to_vec();digest.extend_from_slice(body.as_bytes());
        let hash=sha256_hex(&digest);let signature=B64.encode(key.sign(hash.as_bytes()).to_bytes());
        tx.execute("INSERT INTO audit_user_change(target_user_id,actor_user_id,payload_json,previous_hash,event_hash,signature,public_key,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![current.id,admin.id,body,previous,hash,signature,admin.public_key,created_at]).map_err(|e|e.to_string())?;
        let updated=read_user(&tx,current.id)?;
        tx.commit().map_err(|e|e.to_string())?;
        drop(conn);
        let backup_warning=self.create_backup("auto-save").err().map(|error|format!("Профиль изменён, но резервная копия не создана: {error}"));
        Ok(UserUpdateResult {user:updated,backup_warning})
    }

    pub fn audit_user_statistics(&self,target_user_id:i64,admin_user_id:i64,admin_pin:&str,from:Option<String>,to:Option<String>,offset:u32)->Result<Value,String> {
        for date in [&from,&to].into_iter().flatten() {
            let parsed=chrono::NaiveDate::parse_from_str(date,"%Y-%m-%d").map_err(|_|"Укажите корректные даты периода.")?;
            if parsed.format("%Y-%m-%d").to_string()!=*date { return Err("Укажите корректные даты периода.".into()); }
        }
        if from.as_ref().zip(to.as_ref()).is_some_and(|(a,b)|a>b) { return Err("Начало периода не может быть позже окончания.".into()); }
        if offset>1_000_000 { return Err("Слишком большой номер страницы. Уточните период.".into()); }
        let conn=self.conn.lock().map_err(|_|"База данных занята")?;
        authenticate(&conn,admin_user_id,admin_pin,RoleRequirement::Admin)?;
        let user=read_user(&conn,target_user_id)?;
        let filter="e.actor_user_id=?1 AND (?2 IS NULL OR substr(e.created_at,1,10)>=?2) AND (?3 IS NULL OR substr(e.created_at,1,10)<=?3)";
        let counts=conn.query_row(&format!("SELECT count(*),count(DISTINCT e.project_id),COALESCE(sum(e.event_type IN ('project_created','project_changed','project_saved')),0),COALESCE(sum(e.event_type='stage_completed'),0) FROM audit_event e WHERE {filter}"),params![target_user_id,from,to],|row|Ok((row.get::<_,u64>(0)?,row.get::<_,u64>(1)?,row.get::<_,u64>(2)?,row.get::<_,u64>(3)?))).map_err(|e|e.to_string())?;
        let offset=if counts.0==0 {0}else{offset.min((((counts.0-1)/u64::from(PAGE_SIZE))*u64::from(PAGE_SIZE)).min(u64::from(u32::MAX)) as u32)};
        let last:Option<String>=conn.query_row(&format!("SELECT e.created_at FROM audit_event e WHERE {filter} ORDER BY e.seq DESC LIMIT 1"),params![target_user_id,from,to],|row|row.get(0)).optional().map_err(|e|e.to_string())?;
        let mut stmt=conn.prepare(&format!("SELECT e.seq,e.event_type,e.actor_name,e.created_at,e.comment,e.project_id,p.order_no,p.name,e.stage_reg_number FROM audit_event e JOIN production_cycle p ON p.id=e.project_id WHERE {filter} ORDER BY e.seq DESC LIMIT ?4 OFFSET ?5")).map_err(|e|e.to_string())?;
        let events=stmt.query_map(params![target_user_id,from,to,PAGE_SIZE,offset],|row|Ok(json!({"seq":row.get::<_,i64>(0)?,"eventType":row.get::<_,String>(1)?,"actorName":row.get::<_,String>(2)?,"createdAt":row.get::<_,String>(3)?,"comment":row.get::<_,String>(4)?,"projectId":row.get::<_,i64>(5)?,"orderNo":row.get::<_,String>(6)?,"projectName":row.get::<_,String>(7)?,"stageRegNumber":row.get::<_,Option<String>>(8)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        let integrity=verify_registered_changes(&conn)?;
        let changes=if integrity.errors.is_empty() {
            let mut changes_stmt=conn.prepare("SELECT payload_json FROM audit_user_change WHERE target_user_id=?1 ORDER BY seq DESC LIMIT 20").map_err(|e|e.to_string())?;
            let rows=changes_stmt.query_map([target_user_id],|row|row.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
            rows.into_iter().map(|body|serde_json::from_str::<Value>(&body).map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?
        }else{Vec::new()};
        Ok(json!({"user":user,"from":from,"to":to,"summary":{"journalEntries":counts.0,"projectsTouched":counts.1,"projectSaves":counts.2,"completionConfirmations":counts.3,"lastRecordedAt":last},"events":events,"offset":offset,"pageSize":PAGE_SIZE,"hasMore":u64::from(offset)+u64::from(PAGE_SIZE)<counts.0,"profileChanges":changes,"userChangeLogValid":integrity.errors.is_empty(),"userChangeLogErrors":integrity.errors}))
    }
}

#[cfg(not(feature="server"))]
#[tauri::command]
pub fn audit_admin_update_user(input:UpdateUserInput,db:State<'_,ProductionDb>,network:State<'_,NetworkRuntime>)->Result<UserUpdateResult,String> {
    network.with_write(&db,||db.update_audit_user(input))
}

#[cfg(not(feature="server"))]
#[tauri::command]
pub fn audit_admin_user_statistics(target_user_id:i64,admin_user_id:i64,admin_pin:String,from:Option<String>,to:Option<String>,offset:u32,db:State<'_,ProductionDb>,network:State<'_,NetworkRuntime>)->Result<Value,String> {
    network.with_read(&db,||db.audit_user_statistics(target_user_id,admin_user_id,&admin_pin,from,to,offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup()->(ProductionDb,AuditUserSummary,AuditUserSummary){
        let db=ProductionDb::memory().unwrap();
        let admin=db.create_audit_user(CreateAuditUserInput {display_name:"Администратор QA".into(),pin:"739201".into(),is_admin:false,role:None,admin_user_id:None,admin_pin:None}).unwrap();
        let user=db.create_audit_user(CreateAuditUserInput {display_name:"Сотрудник QA".into(),pin:"111111".into(),is_admin:false,role:Some("project_manager".into()),admin_user_id:Some(admin.id),admin_pin:Some("739201".into())}).unwrap();
        (db,admin,user)
    }
    fn change(admin:&AuditUserSummary,user:&AuditUserSummary)->UpdateUserInput{
        UpdateUserInput {target_user_id:user.id,display_name:"Новое ФИО QA".into(),role:"reviewer".into(),active:true,expected:UserState::from(user),admin_user_id:admin.id,admin_pin:"739201".into(),comment:"Изменение ответственного лица".into()}
    }
    fn sample()->ProductionSnapshot{
        serde_json::from_value(json!({"databaseId":null,"project":{"name":"Проект QA","orderNo":"USER-QA","initiator":"Инициатор","executor":"Исполнитель","enterprise":"Предприятие","start":"2026-09-01","deadline":"2026-12-31"},"stages":[{"uid":"s1","seq":1,"sort":1,"parentUid":null,"title":"Этап QA","executor":"Исполнитель","addressees":"Предприятие","start":"2026-09-01","deadline":"2026-12-30","status":"work","comment":""}],"nextSeq":2})).unwrap()
    }
    #[test]
    fn user_changes_require_real_admin_pin_and_comment(){
        let (db,admin,user)=setup();
        let mut input=change(&admin,&user);input.admin_user_id=user.id;input.admin_pin="111111".into();assert!(db.update_audit_user(input).is_err());
        let mut input=change(&admin,&user);input.admin_pin="wrong-pin".into();assert!(db.update_audit_user(input).is_err());
        let mut input=change(&admin,&user);input.comment="  ".into();assert!(db.update_audit_user(input).is_err());
        assert_eq!(db.list_audit_users().unwrap().iter().find(|u|u.id==user.id).unwrap().display_name,user.display_name);
        assert!(db.audit_user_statistics(user.id,user.id,"111111",None,None,0).is_err());
        assert!(db.audit_user_statistics(user.id,admin.id,"wrong-pin",None,None,0).is_err());
    }
    #[test]
    fn only_one_administrator_is_preserved_and_duplicates_roll_back(){
        let (db,admin,user)=setup();
        let mut input=change(&admin,&admin);input.role="reviewer".into();assert!(db.update_audit_user(input).is_err());
        let mut input=change(&admin,&admin);input.role="admin".into();input.active=false;assert!(db.update_audit_user(input).is_err());
        let mut input=change(&admin,&user);input.role="admin".into();assert!(db.update_audit_user(input).is_err());
        let mut input=change(&admin,&user);input.display_name=admin.display_name.clone();assert!(db.update_audit_user(input).is_err());
        assert_eq!(db.list_audit_users().unwrap().iter().filter(|u|u.role=="admin"&&u.active).count(),1);
        assert_eq!(verify_registered_changes(&db.conn.lock().unwrap()).unwrap().checked,0);
    }
    #[test]
    fn rename_and_role_changes_keep_pin_keys_history_and_detect_stale_edit(){
        let (db,admin,user)=setup();
        let saved=db.save_signed_snapshot(&sample(),user.id,"111111","До изменения профиля",None).unwrap();
        let input=change(&admin,&user);let updated=db.update_audit_user(input).unwrap().user;
        assert_eq!(updated.id,user.id);assert_eq!(updated.key_fingerprint,user.key_fingerprint);assert_eq!(updated.role,"reviewer");
        assert_eq!(db.authorize_user(user.id,"111111").unwrap().display_name,"Новое ФИО QA");
        assert!(authenticate(&db.conn.lock().unwrap(),user.id,"111111",RoleRequirement::Reviewer).is_ok());
        assert!(db.update_audit_user(change(&admin,&user)).is_err());
        let log=db.verify_audit_log(admin.id,"739201",Some(saved.project_id)).unwrap();assert_eq!(log.invalid_events,0);assert_eq!(log.checked_events,2);
        let stats=db.audit_user_statistics(user.id,admin.id,"739201",None,None,0).unwrap();assert_eq!(stats["summary"]["journalEntries"],1);assert_eq!(stats["events"][0]["actorName"],user.display_name);assert_eq!(stats["profileChanges"].as_array().unwrap().len(),1);
        let text=stats.to_string();assert!(!text.contains("111111")&&!text.contains("739201")&&!text.contains("encrypted_private_key")&&!text.contains("kdf_salt"));
    }
    #[test]
    fn deactivation_blocks_authentication_and_reactivation_keeps_key(){
        let (db,admin,user)=setup();let mut input=change(&admin,&user);input.active=false;
        let disabled=db.update_audit_user(input).unwrap().user;assert!(!disabled.active);assert!(db.authorize_user(user.id,"111111").is_err());
        let mut input=change(&admin,&disabled);input.active=true;input.role="project_manager".into();let restored=db.update_audit_user(input).unwrap().user;
        assert_eq!(restored.key_fingerprint,user.key_fingerprint);assert!(db.authorize_user(user.id,"111111").is_ok());
        assert!(authenticate(&db.conn.lock().unwrap(),user.id,"111111",RoleRequirement::Reviewer).is_err());
        assert_eq!(db.verify_audit_log(admin.id,"739201",None).unwrap().invalid_events,0);
    }
    #[test]
    fn profile_log_tampering_is_detected_and_blocks_more_changes(){
        let (db,admin,user)=setup();let updated=db.update_audit_user(change(&admin,&user)).unwrap().user;
        db.conn.lock().unwrap().execute("UPDATE audit_user_change SET payload_json='{}'",[]).unwrap();
        assert!(db.verify_audit_log(admin.id,"739201",None).unwrap().invalid_events>0);
        let mut input=change(&admin,&updated);input.display_name="Ещё одно ФИО".into();assert!(db.update_audit_user(input).is_err());
    }
    #[test]
    fn statistics_use_identity_dates_and_complete_totals_beyond_first_page(){
        let (db,admin,user)=setup();let snap=sample();let saved=db.save_signed_snapshot(&snap,user.id,"111111","Первая запись",None).unwrap();
        let hash=snapshot_hash(&db.load_snapshot_by_id(saved.project_id).unwrap().unwrap()).unwrap();
        {let mut conn=db.conn.lock().unwrap();let (actor,key)=authenticate(&conn,user.id,"111111",RoleRequirement::AnySigner).unwrap();let tx=conn.transaction().unwrap();
         // Use signed events without repeating Argon2 for each record.
         for _ in 0..52 {append_event(&tx,&actor,&key,saved.project_id,None,None,"project_saved","Дополнительное сохранение",None,None,&[],&hash).unwrap();}tx.commit().unwrap();}
        let all=db.audit_user_statistics(user.id,admin.id,"739201",None,None,0).unwrap();assert_eq!(all["summary"]["journalEntries"],53);assert_eq!(all["summary"]["projectSaves"],53);assert_eq!(all["events"].as_array().unwrap().len(),50);assert_eq!(all["hasMore"],true);
        let next=db.audit_user_statistics(user.id,admin.id,"739201",None,None,50).unwrap();assert_eq!(next["events"].as_array().unwrap().len(),3);assert_eq!(next["hasMore"],false);
        assert_eq!(db.audit_user_statistics(user.id,admin.id,"739201",None,None,5000).unwrap()["offset"],50);
        let admin_stats=db.audit_user_statistics(admin.id,admin.id,"739201",None,None,0).unwrap();assert_eq!(admin_stats["summary"]["journalEntries"],0);
        let tomorrow=(Local::now().date_naive()+chrono::Duration::days(1)).to_string();assert_eq!(db.audit_user_statistics(user.id,admin.id,"739201",Some(tomorrow),None,0).unwrap()["summary"]["journalEntries"],0);
        assert!(db.audit_user_statistics(user.id,admin.id,"739201",Some("2026-02-30".into()),None,0).is_err());
        assert!(db.audit_user_statistics(user.id,admin.id,"739201",Some("2026-12-01".into()),Some("2026-01-01".into()),0).is_err());
    }
}
