use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, path::Path, sync::Mutex};
use tauri::State;

const MIGRATION: &str = include_str!("../../migrations/001_production_cycle.sql");

pub const OUT_HEADER: [&str; 9] = [
    "Количество связей, поясняющих суть документа",
    "Регистрационный номер",
    "Дата",
    "Получатель исходящего",
    "Заголовок",
    "Подписант",
    "Срок исполнения",
    "Адресаты",
    "Ссылка",
];
pub const IN_HEADER: [&str; 8] = [
    "Количество связей, поясняющих суть документа",
    "Краткое содержание",
    "Рег. номер",
    "Дата регистрации",
    "Корреспондент",
    "Адресаты",
    "Срок исполнения",
    "Ссылка",
];

#[derive(Debug, thiserror::Error)]
pub enum ProductionError {
    #[error("{0}")]
    Validation(String),
    #[error("SQLite: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Дата имеет неверный формат: {0}")]
    Date(String),
}
type Result<T> = std::result::Result<T, ProductionError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus { New, Work, Hold, Done }
impl StageStatus {
    fn as_str(self) -> &'static str { match self { Self::New=>"new", Self::Work=>"work", Self::Hold=>"hold", Self::Done=>"done" } }
    fn parse(v:&str) -> Result<Self> { match v { "new"=>Ok(Self::New), "work"=>Ok(Self::Work), "hold"=>Ok(Self::Hold), "done"=>Ok(Self::Done), _=>Err(ProductionError::Validation(format!("Неизвестный статус: {v}"))) } }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageVisualStatus { New, Work, Hold, Done, Overdue }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionProjectInput {
    pub name: String,
    pub order_no: String,
    pub initiator: String,
    pub executor: String,
    pub enterprise: String,
    pub start: String,
    pub deadline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionStageInput {
    pub uid: String,
    pub seq: i64,
    pub sort: i64,
    pub parent_uid: Option<String>,
    pub title: String,
    pub executor: String,
    pub addressees: String,
    pub start: String,
    pub deadline: String,
    pub status: StageStatus,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSnapshot {
    pub project: ProductionProjectInput,
    pub stages: Vec<ProductionStageInput>,
    pub next_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue { pub field: Option<String>, pub code: String, pub message: String, pub entity_id: Option<String> }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult { pub valid: bool, pub errors: Vec<ValidationIssue>, pub warnings: Vec<ValidationIssue> }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxtExportBundle { pub outgoing_file_name:String, pub outgoing_text:String, pub incoming_file_name:String, pub incoming_text:String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCounts { pub total:usize, pub done:usize, pub work:usize, pub overdue:usize, pub new_count:usize, pub hold:usize, pub done_percent:u8, pub due_within_7_days:usize }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportStageRow { pub stage:ProductionStageInput, pub depth:usize, pub visual_status:StageVisualStatus, pub days_remaining:i64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionReport {
    pub project:ProductionProjectInput,
    pub counts:ReportCounts,
    pub stages:Vec<ReportStageRow>,
    pub status:StageVisualStatus,
    pub project_days:i64,
    pub project_overdue:bool,
    pub overdue_held:usize,
    pub generated_at:String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub order_no:String,
    pub name:String,
    pub enterprise:String,
    pub deadline:String,
    pub updated_at:String,
    pub stage_count:usize,
    pub done_count:usize,
    pub done_percent:u8,
}

pub struct ProductionDb { conn: Mutex<Connection> }
fn initialize_schema(conn:&Connection)->Result<()> {
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(MIGRATION)?;
    let mut stmt=conn.prepare("PRAGMA table_info(production_stage)")?;
    let columns=stmt.query_map([],|row|row.get::<_,String>(1))?.collect::<std::result::Result<Vec<_>,_>>()?;
    drop(stmt);
    if !columns.iter().any(|name|name=="comment") {conn.execute_batch("ALTER TABLE production_stage ADD COLUMN comment TEXT NOT NULL DEFAULT '';")?;}
    Ok(())
}
impl ProductionDb {
    pub fn open(path:&Path) -> Result<Self> {
        let conn=Connection::open(path)?;
        initialize_schema(&conn)?;
        Ok(Self{conn:Mutex::new(conn)})
    }
    #[cfg(test)]
    fn memory() -> Result<Self> {
        let conn=Connection::open_in_memory()?;
        initialize_schema(&conn)?;
        Ok(Self{conn:Mutex::new(conn)})
    }
    pub fn save_snapshot(&self, snapshot:&ProductionSnapshot) -> Result<()> {
        let validation=validate_snapshot(snapshot)?;
        if !validation.valid { return Err(ProductionError::Validation("Проект не прошёл валидацию".into())); }
        let mut conn=self.conn.lock().map_err(|_|ProductionError::Validation("База данных занята".into()))?;
        let tx=conn.transaction()?;
        save_snapshot_tx(&tx,snapshot)?;
        tx.commit()?;
        Ok(())
    }
    pub fn load_snapshot(&self, order_no:&str) -> Result<Option<ProductionSnapshot>> {
        let conn=self.conn.lock().map_err(|_|ProductionError::Validation("База данных занята".into()))?;
        load_snapshot_conn(&conn,order_no)
    }
    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let conn=self.conn.lock().map_err(|_|ProductionError::Validation("База данных занята".into()))?;
        let mut stmt=conn.prepare(
            "SELECT c.order_no,c.name,c.enterprise,c.deadline,c.updated_at,COUNT(s.id),COALESCE(SUM(CASE WHEN s.status='done' THEN 1 ELSE 0 END),0) \
             FROM production_cycle c LEFT JOIN production_stage s ON s.cycle_id=c.id \
             GROUP BY c.id ORDER BY c.updated_at DESC,c.order_no"
        )?;
        let rows=stmt.query_map([],|r|{
            let stage_count:i64=r.get(5)?;let done_count:i64=r.get(6)?;
            let done_percent=if stage_count>0{((done_count*100+stage_count/2)/stage_count) as u8}else{0};
            Ok(ProjectSummary{order_no:r.get(0)?,name:r.get(1)?,enterprise:r.get(2)?,deadline:r.get(3)?,updated_at:r.get(4)?,stage_count:stage_count as usize,done_count:done_count as usize,done_percent})
        })?;
        rows.collect::<std::result::Result<Vec<_>,_>>().map_err(ProductionError::from)
    }
}

fn clean(v:&str)->String{v.trim().to_string()}
fn has_tsv_breaker(v:&str)->bool{v.contains('\t')||v.contains('\r')||v.contains('\n')}
fn parse_date(v:&str)->Result<NaiveDate>{NaiveDate::parse_from_str(v,"%Y-%m-%d").map_err(|_|ProductionError::Date(v.into()))}
fn format_date(v:&str,fmt:&str)->Result<String>{Ok(parse_date(v)?.format(fmt).to_string())}
fn issue(field:&str,code:&str,message:&str,entity:Option<&str>)->ValidationIssue{ValidationIssue{field:Some(field.into()),code:code.into(),message:message.into(),entity_id:entity.map(str::to_string)}}

pub fn validate_snapshot(snapshot:&ProductionSnapshot)->Result<ValidationResult>{
    let mut r=ValidationResult::default();
    let p=&snapshot.project;
    let fields=[("Название проекта",p.name.as_str()),("Номер заказа",p.order_no.as_str()),("Инициатор",p.initiator.as_str()),("Исполнитель",p.executor.as_str()),("Предприятие",p.enterprise.as_str()),("Начало",p.start.as_str()),("Дедлайн",p.deadline.as_str())];
    for (name,v) in fields { if clean(v).is_empty(){r.errors.push(issue(name,"REQUIRED",&format!("Поле «{name}» обязательно."),None));} else if has_tsv_breaker(v){r.errors.push(issue(name,"TSV_BREAKER",&format!("Поле «{name}» содержит TAB/перенос строки."),None));} }
    if !p.start.is_empty()&&!p.deadline.is_empty(){match(parse_date(&p.start),parse_date(&p.deadline)){(Ok(a),Ok(b)) if a>b=>r.errors.push(issue("Дедлайн","DATE_ORDER","Дата начала проекта не может быть позже дедлайна.",None)),(Err(_),_)|(_,Err(_))=>r.errors.push(issue("Дата","DATE_FORMAT","Используйте формат YYYY-MM-DD.",None)),_=>{}}}
    let mut uids=HashSet::new();let mut seqs=HashSet::new();
    for s in &snapshot.stages {
        if !uids.insert(s.uid.clone()){r.errors.push(issue("uid","DUPLICATE_UID","Повторяющийся UID этапа.",Some(&s.uid)));}
        if !seqs.insert(s.seq){r.errors.push(issue("seq","DUPLICATE_SEQ","Повторяющийся порядковый номер этапа.",Some(&s.uid)));}
        for (name,v) in [("Название этапа",s.title.as_str()),("Исполнитель",s.executor.as_str()),("Адресаты",s.addressees.as_str()),("Начало",s.start.as_str()),("Дедлайн",s.deadline.as_str())] { if clean(v).is_empty(){r.errors.push(issue(name,"REQUIRED",&format!("Поле «{name}» обязательно."),Some(&s.uid)));} else if has_tsv_breaker(v){r.errors.push(issue(name,"TSV_BREAKER",&format!("Поле «{name}» содержит TAB/перенос строки."),Some(&s.uid)));} }
        if !s.start.is_empty()&&!s.deadline.is_empty(){match(parse_date(&s.start),parse_date(&s.deadline)){(Ok(a),Ok(b)) if a>b=>r.errors.push(issue("Дедлайн","DATE_ORDER","Дата начала этапа не может быть позже дедлайна.",Some(&s.uid))),(Err(_),_)|(_,Err(_))=>r.errors.push(issue("Дата","DATE_FORMAT","Используйте формат YYYY-MM-DD.",Some(&s.uid))),_=>{}}}
    }
    let uid_set:HashSet<_>=snapshot.stages.iter().map(|s|s.uid.as_str()).collect();
    for s in &snapshot.stages {if let Some(parent)=s.parent_uid.as_deref(){if parent==s.uid{r.errors.push(issue("Родитель","SELF_PARENT","Этап не может быть собственным родителем.",Some(&s.uid)));}else if !uid_set.contains(parent){r.errors.push(issue("Родитель","MISSING_PARENT","Родительский этап не найден.",Some(&s.uid)));}}}
    if has_cycle(&snapshot.stages){r.errors.push(issue("Родитель","CYCLE","Обнаружен цикл в иерархии этапов.",None));}
    if snapshot.stages.is_empty(){r.warnings.push(issue("Этапы","NO_STAGES","Проект не содержит этапов.",None));}
    r.valid=r.errors.is_empty();
    Ok(r)
}

fn has_cycle(stages:&[ProductionStageInput])->bool{
    let parents:HashMap<&str,Option<&str>>=stages.iter().map(|s|(s.uid.as_str(),s.parent_uid.as_deref())).collect();
    for s in stages {let mut seen=HashSet::new();let mut cur=Some(s.uid.as_str());while let Some(id)=cur{if !seen.insert(id){return true;}cur=parents.get(id).copied().flatten();}}
    false
}

pub fn preorder(stages:&[ProductionStageInput])->Vec<(ProductionStageInput,usize)>{
    let mut children:HashMap<Option<&str>,Vec<&ProductionStageInput>>=HashMap::new();
    for s in stages {children.entry(s.parent_uid.as_deref()).or_default().push(s);}
    for values in children.values_mut(){values.sort_by_key(|s|(s.sort,s.seq));}
    fn walk(parent:Option<&str>,depth:usize,map:&HashMap<Option<&str>,Vec<&ProductionStageInput>>,out:&mut Vec<(ProductionStageInput,usize)>){if let Some(items)=map.get(&parent){for s in items{out.push(((*s).clone(),depth));walk(Some(s.uid.as_str()),depth+1,map,out);}}}
    let mut out=Vec::new();walk(None,0,&children,&mut out);out
}
fn child_count(stages:&[ProductionStageInput],parent:Option<&str>)->usize{stages.iter().filter(|s|s.parent_uid.as_deref()==parent).count()}
fn stage_reg(order_no:&str,seq:i64)->String{format!("{}-{seq:03}",clean(order_no))}

pub fn visual_status(status:StageStatus,today:NaiveDate,start:NaiveDate,deadline:NaiveDate)->StageVisualStatus{
    if status==StageStatus::Done{return StageVisualStatus::Done;}if status==StageStatus::Hold{return StageVisualStatus::Hold;}if today<start{return StageVisualStatus::New;}if today>deadline{return StageVisualStatus::Overdue;}if status==StageStatus::New{return StageVisualStatus::New;}StageVisualStatus::Work
}

pub fn build_txt_export(snapshot:&ProductionSnapshot)->Result<TxtExportBundle>{
    let validation=validate_snapshot(snapshot)?;if !validation.valid{return Err(ProductionError::Validation("Проект не прошёл валидацию".into()));}
    let p=&snapshot.project;let mut out=OUT_HEADER.join("\t");out.push_str("\r\n");
    let outgoing=[child_count(&snapshot.stages,None).to_string(),clean(&p.order_no),format_date(&p.start,"%d.%m.%y")?,clean(&p.executor),clean(&p.name),clean(&p.initiator),format_date(&p.deadline,"%d.%m.%Y")?,clean(&p.enterprise),String::new()];out.push_str(&outgoing.join("\t"));out.push_str("\r\n");
    let mut incoming=IN_HEADER.join("\t");incoming.push_str("\r\n");
    for (s,_) in preorder(&snapshot.stages){let row=[child_count(&snapshot.stages,Some(&s.uid)).to_string(),clean(&s.title),stage_reg(&p.order_no,s.seq),format_date(&s.start,"%d.%m.%Y")?,clean(&s.executor),clean(&s.addressees),format_date(&s.deadline,"%d.%m.%Y")?,String::new()];incoming.push_str(&row.join("\t"));incoming.push_str("\r\n");}
    Ok(TxtExportBundle{outgoing_file_name:format!("Список исх. Заказ {}.txt",clean(&p.order_no)),outgoing_text:out,incoming_file_name:format!("Список вхд. Заказ {}.txt",clean(&p.order_no)),incoming_text:incoming})
}

pub fn build_report(snapshot:&ProductionSnapshot,today:NaiveDate)->Result<ProductionReport>{
    let validation=validate_snapshot(snapshot)?;if !validation.valid{return Err(ProductionError::Validation("Проект не прошёл валидацию".into()));}
    let mut counts=ReportCounts{total:snapshot.stages.len(),done:0,work:0,overdue:0,new_count:0,hold:0,done_percent:0,due_within_7_days:0};let mut rows=Vec::new();let mut overdue_held=0;
    for (s,depth) in preorder(&snapshot.stages){let deadline=parse_date(&s.deadline)?;let vs=visual_status(s.status,today,parse_date(&s.start)?,deadline);let days=(deadline-today).num_days();match vs{StageVisualStatus::Done=>counts.done+=1,StageVisualStatus::Work=>counts.work+=1,StageVisualStatus::Overdue=>counts.overdue+=1,StageVisualStatus::New=>counts.new_count+=1,StageVisualStatus::Hold=>counts.hold+=1}if vs!=StageVisualStatus::Done&&vs!=StageVisualStatus::Overdue&&days>=0&&days<=7{counts.due_within_7_days+=1;}if vs==StageVisualStatus::Hold&&days<0{overdue_held+=1;}rows.push(ReportStageRow{stage:s,depth,visual_status:vs,days_remaining:days});}
    if counts.total>0{counts.done_percent=((counts.done*100+counts.total/2)/counts.total) as u8;}
    let project_start=parse_date(&snapshot.project.start)?;let project_deadline=parse_date(&snapshot.project.deadline)?;let project_days=(project_deadline-today).num_days();
    let status=if !snapshot.stages.is_empty()&&snapshot.stages.iter().all(|s|s.status==StageStatus::Done){StageVisualStatus::Done}else if counts.overdue>0{StageVisualStatus::Overdue}else if today<project_start{StageVisualStatus::New}else{StageVisualStatus::Work};
    let project_overdue=status!=StageVisualStatus::Done&&project_days<0;
    Ok(ProductionReport{project:snapshot.project.clone(),counts,stages:rows,status,project_days,project_overdue,overdue_held,generated_at:Local::now().to_rfc3339()})
}

fn save_snapshot_tx(tx:&Transaction<'_>,snapshot:&ProductionSnapshot)->Result<()> {
    let now=Local::now().to_rfc3339();
    let existing:Option<i64>=tx.query_row("SELECT id FROM production_cycle WHERE order_no=?1",[clean(&snapshot.project.order_no)],|r|r.get(0)).optional()?;
    let cycle_id=if let Some(id)=existing{tx.execute("UPDATE production_cycle SET name=?1,initiator=?2,executor=?3,enterprise=?4,start_date=?5,deadline=?6,updated_at=?7 WHERE id=?8",params![clean(&snapshot.project.name),clean(&snapshot.project.initiator),clean(&snapshot.project.executor),clean(&snapshot.project.enterprise),snapshot.project.start,snapshot.project.deadline,now,id])?;tx.execute("DELETE FROM production_stage WHERE cycle_id=?1",[id])?;id}else{tx.execute("INSERT INTO production_cycle(order_no,root_reg_number,name,initiator,executor,enterprise,start_date,deadline,created_at,updated_at) VALUES(?1,?1,?2,?3,?4,?5,?6,?7,?8,?8)",params![clean(&snapshot.project.order_no),clean(&snapshot.project.name),clean(&snapshot.project.initiator),clean(&snapshot.project.executor),clean(&snapshot.project.enterprise),snapshot.project.start,snapshot.project.deadline,now])?;tx.last_insert_rowid()};
    let mut id_by_uid=HashMap::<String,i64>::new();
    for (s,_) in preorder(&snapshot.stages){let parent_id=s.parent_uid.as_ref().and_then(|u|id_by_uid.get(u)).copied();tx.execute("INSERT INTO production_stage(cycle_id,uid,seq,reg_number,parent_stage_id,sort_order,title,executor,addressees,start_date,deadline,status,comment,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)",params![cycle_id,s.uid,s.seq,stage_reg(&snapshot.project.order_no,s.seq),parent_id,s.sort,clean(&s.title),clean(&s.executor),clean(&s.addressees),s.start,s.deadline,s.status.as_str(),clean(&s.comment),now])?;id_by_uid.insert(s.uid,tx.last_insert_rowid());}
    Ok(())
}

fn load_snapshot_conn(conn:&Connection,order_no:&str)->Result<Option<ProductionSnapshot>>{
    let project=conn.query_row("SELECT id,name,order_no,initiator,executor,enterprise,start_date,deadline FROM production_cycle WHERE order_no=?1",[clean(order_no)],|r|Ok((r.get::<_,i64>(0)?,ProductionProjectInput{name:r.get(1)?,order_no:r.get(2)?,initiator:r.get(3)?,executor:r.get(4)?,enterprise:r.get(5)?,start:r.get(6)?,deadline:r.get(7)?}))).optional()?;let Some((cycle_id,project))=project else{return Ok(None)};
    let mut stmt=conn.prepare("SELECT id,uid,seq,sort_order,parent_stage_id,title,executor,addressees,start_date,deadline,status,comment FROM production_stage WHERE cycle_id=?1 ORDER BY seq")?;let rows=stmt.query_map([cycle_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,Option<i64>>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?,r.get::<_,String>(9)?,r.get::<_,String>(10)?,r.get::<_,String>(11)?)))?;let mut raw=Vec::new();let mut uid_by_id=HashMap::new();for row in rows{let v=row?;uid_by_id.insert(v.0,v.1.clone());raw.push(v)}let mut stages=Vec::new();let mut max_seq=0;for (_id,uid,seq,sort,parent,title,executor,addressees,start,deadline,status,comment) in raw{max_seq=max_seq.max(seq);stages.push(ProductionStageInput{uid,seq,sort,parent_uid:parent.and_then(|p|uid_by_id.get(&p).cloned()),title,executor,addressees,start,deadline,status:StageStatus::parse(&status)?,comment});}
    Ok(Some(ProductionSnapshot{project,stages,next_seq:max_seq+1}))
}

#[tauri::command]
pub fn production_validate(snapshot:ProductionSnapshot)->std::result::Result<ValidationResult,String>{validate_snapshot(&snapshot).map_err(|e|e.to_string())}
#[tauri::command]
pub fn production_save_snapshot(snapshot:ProductionSnapshot,db:State<'_,ProductionDb>)->std::result::Result<(),String>{db.save_snapshot(&snapshot).map_err(|e|e.to_string())}
#[tauri::command]
pub fn production_load_snapshot(order_no:String,db:State<'_,ProductionDb>)->std::result::Result<Option<ProductionSnapshot>,String>{db.load_snapshot(&order_no).map_err(|e|e.to_string())}
#[tauri::command]
pub fn production_list_projects(db:State<'_,ProductionDb>)->std::result::Result<Vec<ProjectSummary>,String>{db.list_projects().map_err(|e|e.to_string())}
#[tauri::command]
pub fn production_export_txt(snapshot:ProductionSnapshot)->std::result::Result<TxtExportBundle,String>{build_txt_export(&snapshot).map_err(|e|e.to_string())}
#[tauri::command]
pub fn production_get_management_report(snapshot:ProductionSnapshot)->std::result::Result<ProductionReport,String>{build_report(&snapshot,Local::now().date_naive()).map_err(|e|e.to_string())}

#[cfg(test)]
mod tests{
    use super::*;
    fn sample()->ProductionSnapshot{ProductionSnapshot{project:ProductionProjectInput{name:"Изделие".into(),order_no:"916".into(),initiator:"Инициатор".into(),executor:"Исполнитель".into(),enterprise:"Предприятие".into(),start:"2026-09-01".into(),deadline:"2026-10-01".into()},stages:vec![ProductionStageInput{uid:"a".into(),seq:1,sort:1,parent_uid:None,title:"A".into(),executor:"E".into(),addressees:"A".into(),start:"2026-09-01".into(),deadline:"2026-09-20".into(),status:StageStatus::Work,comment:"Контрольный комментарий".into()},ProductionStageInput{uid:"b".into(),seq:2,sort:1,parent_uid:Some("a".into()),title:"B".into(),executor:"E".into(),addressees:"A".into(),start:"2026-09-02".into(),deadline:"2026-09-18".into(),status:StageStatus::Done,comment:String::new()}],next_seq:3}}
    #[test]fn txt_contract_and_empty_link(){let b=build_txt_export(&sample()).unwrap();assert_eq!(OUT_HEADER.len(),9);assert_eq!(IN_HEADER.len(),8);let incoming:Vec<_>=b.incoming_text.split("\r\n").collect();assert!(incoming[1].ends_with('\t'));assert!(incoming[1].contains("A\t916-001"));assert!(incoming[2].contains("B\t916-002"));}
    #[test]fn sqlite_roundtrip(){let db=ProductionDb::memory().unwrap();let s=sample();db.save_snapshot(&s).unwrap();let loaded=db.load_snapshot("916").unwrap().unwrap();assert_eq!(loaded.project.enterprise,"Предприятие");assert_eq!(loaded.stages.len(),2);assert_eq!(loaded.stages[0].comment,"Контрольный комментарий");assert_eq!(loaded.stages[1].parent_uid.as_deref(),Some("a"));}
    #[test]fn existing_database_gets_comment_column(){let conn=Connection::open_in_memory().unwrap();conn.execute_batch("CREATE TABLE production_stage(id INTEGER PRIMARY KEY,cycle_id INTEGER,parent_stage_id INTEGER,sort_order INTEGER,comment_placeholder TEXT);").unwrap();initialize_schema(&conn).unwrap();let columns=conn.prepare("PRAGMA table_info(production_stage)").unwrap().query_map([],|row|row.get::<_,String>(1)).unwrap().collect::<std::result::Result<Vec<_>,_>>().unwrap();assert!(columns.iter().any(|name|name=="comment"));}
    #[test]fn project_list_has_progress_and_latest_metadata(){let db=ProductionDb::memory().unwrap();db.save_snapshot(&sample()).unwrap();let list=db.list_projects().unwrap();assert_eq!(list.len(),1);assert_eq!(list[0].order_no,"916");assert_eq!(list[0].stage_count,2);assert_eq!(list[0].done_count,1);assert_eq!(list[0].done_percent,50);}
    #[test]fn status_is_derived(){let today=NaiveDate::from_ymd_opt(2026,9,8).unwrap();let start=NaiveDate::from_ymd_opt(2026,9,1).unwrap();let deadline=NaiveDate::from_ymd_opt(2026,9,7).unwrap();assert_eq!(visual_status(StageStatus::Work,today,start,deadline),StageVisualStatus::Overdue);assert_eq!(visual_status(StageStatus::Done,today,start,deadline),StageVisualStatus::Done);}
    #[test]fn report_contains_management_risks(){let mut s=sample();s.project.deadline="2026-09-07".into();s.stages[0].status=StageStatus::Hold;s.stages[0].deadline="2026-09-07".into();let today=NaiveDate::from_ymd_opt(2026,9,8).unwrap();let r=build_report(&s,today).unwrap();assert_eq!(r.overdue_held,1);assert_eq!(r.project_days,-1);assert!(r.project_overdue);assert_eq!(r.status,StageVisualStatus::Work);}
    #[test]fn cycle_is_rejected(){let mut s=sample();s.stages[0].parent_uid=Some("b".into());let r=validate_snapshot(&s).unwrap();assert!(!r.valid);assert!(r.errors.iter().any(|e|e.code=="CYCLE"));}
}
