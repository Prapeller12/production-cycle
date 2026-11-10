use crate::production::{self,ProductionProjectInput,ProductionSnapshot,ProductionStageInput,StageStatus};
use base64::{engine::general_purpose::STANDARD,Engine};
use calamine::{Data,DataType,Reader,Xlsx};
use chrono::{Duration,NaiveDate};
use serde::Serialize;
use std::{collections::{HashMap,HashSet},io::Cursor};

const MAX_FILE:usize=5*1024*1024;
const MAX_ROWS:u32=10_032;
const MAX_COLUMNS:u32=32;
const PROJECT_FIELDS:[&str;7]=["номер заказа","наименование проекта","инициатор","исполнитель","предприятие","начало","срок исполнения"];
const STAGE_FIELDS:[&str;8]=["код этапа","код головного этапа","наименование этапа","исполнитель","адресаты","начало","срок исполнения","комментарий"];

#[derive(Serialize)]#[serde(rename_all="camelCase")]
pub struct PreviewRow{pub code:String,pub parent_code:Option<String>,pub title:String,pub executor:String,pub addressees:String,pub start:String,pub deadline:String,pub comment:String}
#[derive(Serialize)]#[serde(rename_all="camelCase")]
pub struct ImportResult{pub snapshot:ProductionSnapshot,pub rows:Vec<PreviewRow>,pub warnings:Vec<String>}

pub fn template()->Result<serde_json::Value,String>{serde_json::from_str(include_str!("../../resources/project-import-template.json")).map_err(|e|e.to_string())}
pub fn parse(encoded:&str,file_name:&str)->Result<ImportResult,String>{
    if !file_name.to_lowercase().ends_with(".xlsx"){return Err("Выберите файл .xlsx по образцу программы.".into())}
    if encoded.len()>MAX_FILE.div_ceil(3)*4{return Err("Файл Excel больше 5 МиБ. Разделите проекты по отдельным файлам.".into())}
    let bytes=STANDARD.decode(encoded).map_err(|_|"Не удалось прочитать данные Excel.".to_string())?;
    parse_bytes(&bytes)
}
fn u16_at(bytes:&[u8],at:usize)->Result<u16,String>{bytes.get(at..at+2).map(|v|u16::from_le_bytes([v[0],v[1]])).ok_or_else(||"Повреждённый архив Excel.".into())}
fn u32_at(bytes:&[u8],at:usize)->Result<u32,String>{bytes.get(at..at+4).map(|v|u32::from_le_bytes([v[0],v[1],v[2],v[3]])).ok_or_else(||"Повреждённый архив Excel.".into())}
fn inspect_zip(bytes:&[u8])->Result<(),String>{
    if bytes.len()<22||bytes.len()>MAX_FILE{return Err("Файл пуст, повреждён или превышает 5 МиБ.".into())}
    let end=(bytes.len().saturating_sub(65_557)..=bytes.len()-22).rev().find(|&i|bytes[i..i+4]==*b"PK\x05\x06"&&u16_at(bytes,i+20).map(|n|i+22+n as usize==bytes.len()).unwrap_or(false)).ok_or("Выбранный файл не является обычной книгой .xlsx.")?;
    let count=u16_at(bytes,end+10)? as usize;
    if u16_at(bytes,end+4)?!=0||u16_at(bytes,end+6)?!=0||count==0||count>512{return Err("Сложный или многотомный архив Excel не поддерживается. Сохраните файл по образцу .xlsx.".into())}
    let mut pos=u32_at(bytes,end+16)? as usize;let central_end=pos.checked_add(u32_at(bytes,end+12)? as usize).ok_or("Повреждённый архив Excel.")?;
    if central_end>end{return Err("Повреждённый архив Excel.".into())}
    let mut total=0u64;let mut names=HashSet::new();
    for _ in 0..count{
        if bytes.get(pos..pos+4)!=Some(&b"PK\x01\x02"[..]){return Err("Повреждённая структура Excel.".into())}
        let flags=u16_at(bytes,pos+8)?;let size=u32_at(bytes,pos+24)? as u64;
        total=total.checked_add(size).ok_or("Слишком большая книга Excel.")?;
        if flags&1!=0||size>16*1024*1024||total>32*1024*1024{return Err("Книга защищена паролем или содержит слишком много данных. Используйте образец программы.".into())}
        let length=u16_at(bytes,pos+28)? as usize;
        let name=String::from_utf8_lossy(bytes.get(pos+46..pos+46+length).ok_or("Повреждённый архив Excel.")?).to_string();
        if !names.insert(name.clone()){return Err("В книге повторяются внутренние файлы.".into())}
        if name.to_lowercase().contains("vbaproject")||name.starts_with("xl/externalLinks/"){return Err("Импортируйте обычный .xlsx без макросов и связей с другими книгами.".into())}
        pos=pos.checked_add(46+length+u16_at(bytes,pos+30)? as usize+u16_at(bytes,pos+32)? as usize).ok_or("Повреждённый архив Excel.")?;
        if pos>central_end{return Err("Повреждённая структура Excel.".into())}
    }
    if !names.contains("xl/workbook.xml"){return Err("В файле не найдена книга Excel.".into())}Ok(())
}
fn sheet(book:&mut Xlsx<Cursor<&[u8]>>,name:&str)->Result<Vec<(usize,Vec<Data>)>,String>{
    {
        let mut cells=book.worksheet_cells_reader(name).map_err(|_|format!("Не найден или повреждён лист «{name}». Используйте образец программы."))?;
        let dim=cells.dimensions();
        if dim.end.0>=MAX_ROWS||dim.end.1>=MAX_COLUMNS{return Err(format!("Лист «{name}» слишком велик. Удалите лишние строки и столбцы за пределами таблицы."))}
        let mut count=0;
        while let Some(cell)=cells.next_cell().map_err(|_|format!("Не удалось прочитать лист «{name}»."))?{
            count+=1;let(r,c)=cell.get_position();
            if r>=MAX_ROWS||c>=MAX_COLUMNS||count>MAX_ROWS as usize*MAX_COLUMNS as usize{return Err(format!("Лист «{name}» превышает допустимый размер."))}
        }
    }
    {
        let mut cells=book.worksheet_cells_reader(name).map_err(|e|e.to_string())?;
        if let Some(cell)=cells.next_formula().map_err(|_|format!("Не удалось проверить формулы листа «{name}»."))?{
            return Err(format!("Лист «{name}», строка {}: замените формулы готовыми значениями перед импортом.",cell.get_position().0+1))
        }
    }
    let range=book.worksheet_range(name).map_err(|_|format!("Не удалось прочитать лист «{name}»."))?;
    let start=range.start().map(|(row,_)|row as usize).unwrap_or(0);
    Ok(range.rows().enumerate().map(|(i,row)|(start+i+1,row.to_vec())).collect())
}
fn text(data:&Data,label:&str)->Result<String,String>{
    let value=match data{Data::Empty=>String::new(),Data::String(s)=>s.trim().to_string(),Data::Int(n)=>n.to_string(),Data::Float(n)if n.is_finite()=>n.to_string(),_=>return Err(format!("{label}: укажите текст или число, без ошибки Excel."))};
    if value.len()>8192||value.chars().any(|c|matches!(c,'\t'|'\r'|'\n')){return Err(format!("{label}: слишком длинное значение, TAB или перенос строки."))}Ok(value)
}
fn key(data:&Data)->String{data.to_string().split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches('*').trim().to_lowercase()}
fn date(data:&Data,is_1904:bool,label:&str)->Result<String,String>{
    let parsed=match data{
        Data::DateTime(_)|Data::DateTimeIso(_)=>data.as_date(),
        Data::String(s)=>NaiveDate::parse_from_str(s.trim(),"%d.%m.%Y").or_else(|_|NaiveDate::parse_from_str(s.trim(),"%Y-%m-%d")).ok(),
        Data::Int(_)|Data::Float(_)=>data.as_f64().filter(|n|n.is_finite()&&*n>=61.0&&*n<=2_958_465.0&&n.fract()==0.0).and_then(|n|NaiveDate::from_ymd_opt(1899,12,30)?.checked_add_signed(Duration::days(n as i64+if is_1904{1462}else{0}))),
        _=>None,
    };
    parsed.map(|d|d.format("%Y-%m-%d").to_string()).ok_or_else(||format!("{label}: укажите дату Excel или дату ДД.ММ.ГГГГ."))
}
pub fn parse_bytes(bytes:&[u8])->Result<ImportResult,String>{
    inspect_zip(bytes)?;
    let mut book=Xlsx::new(Cursor::new(bytes)).map_err(|_|"Не удалось открыть Excel. Сохраните книгу заново в формате .xlsx.".to_string())?;
    let is_1904=book.has_1904_epoch();
    let project_rows=sheet(&mut book,"Проект")?;
    let stage_rows=sheet(&mut book,"Этапы")?;
    let mut fields:HashMap<String,Data>=HashMap::new();
    for(row_no,row)in project_rows{
        for(i,cell)in row.iter().enumerate(){let name=key(cell);if PROJECT_FIELDS.contains(&name.as_str()){
            let value=row.get(i+1).cloned().unwrap_or(Data::Empty);
            if fields.insert(name.clone(),value).is_some(){return Err(format!("Лист «Проект», строка {row_no}: поле «{name}» указано дважды."))}
        }}
    }
    for name in PROJECT_FIELDS{if !fields.contains_key(name){return Err(format!("Лист «Проект»: отсутствует поле «{name}»."))}}
    let project=ProductionProjectInput{
        name:text(&fields["наименование проекта"],"Наименование проекта")?,order_no:text(&fields["номер заказа"],"Номер заказа")?,
        initiator:text(&fields["инициатор"],"Инициатор")?,executor:text(&fields["исполнитель"],"Исполнитель")?,enterprise:text(&fields["предприятие"],"Предприятие")?,
        start:date(&fields["начало"],is_1904,"Начало проекта")?,deadline:date(&fields["срок исполнения"],is_1904,"Срок проекта")?,
    };
    let header=stage_rows.iter().position(|(_,row)|{let keys:HashSet<_>=row.iter().map(key).collect();STAGE_FIELDS.iter().all(|name|keys.contains(*name))}).ok_or("Лист «Этапы»: не найдены все восемь заголовков таблицы. Используйте образец программы.")?;
    let columns:Vec<_>=STAGE_FIELDS.iter().map(|name|stage_rows[header].1.iter().position(|cell|key(cell)==*name).unwrap()).collect();
    if columns.iter().copied().collect::<HashSet<_>>().len()!=8{return Err("Повторяются заголовки таблицы этапов.".into())}
    let mut rows=Vec::new();let mut code_indices=HashMap::new();
    for(row_no,row)in stage_rows.iter().skip(header+1){
        let cells:Vec<_>=columns.iter().map(|&column|row.get(column).cloned().unwrap_or(Data::Empty)).collect();
        if cells.iter().all(|cell|matches!(cell,Data::Empty)||matches!(cell,Data::String(s)if s.trim().is_empty())){continue}
        if rows.len()>=10_000{return Err("В одном файле допускается не более 10 000 этапов.".into())}
        let label=|name:&str|format!("Лист «Этапы», строка {row_no}, {name}");
        let code=text(&cells[0],&label("код этапа"))?;if code.is_empty(){return Err(format!("{}: заполните код.",label("код этапа")))}
        if code_indices.insert(code.clone(),rows.len()).is_some(){return Err(format!("Лист «Этапы», строка {row_no}: код «{code}» повторяется."))}
        let parent=text(&cells[1],&label("код головного этапа"))?;
        rows.push(PreviewRow{code,parent_code:if parent.is_empty(){None}else{Some(parent)},title:text(&cells[2],&label("наименование"))?,executor:text(&cells[3],&label("исполнитель"))?,addressees:text(&cells[4],&label("адресаты"))?,start:date(&cells[5],is_1904,&label("начало"))?,deadline:date(&cells[6],is_1904,&label("срок исполнения"))?,comment:text(&cells[7],&label("комментарий"))?});
    }
    if rows.is_empty(){return Err("Лист «Этапы»: добавьте хотя бы один этап.".into())}
    let mut sorts=HashMap::new();let mut stages=Vec::new();
    for(i,row)in rows.iter().enumerate(){
        let parent_uid=if let Some(code)=&row.parent_code{
            let parent=*code_indices.get(code).ok_or_else(||format!("Этап «{}»: код головного этапа «{code}» не найден.",row.code))?;
            if parent==i{return Err(format!("Этап «{}» не может подчиняться самому себе.",row.code))}
            Some(format!("excel-{}",parent+1))
        }else{None};
        let sort=sorts.entry(parent_uid.clone()).or_insert(0i64);*sort+=1;
        stages.push(ProductionStageInput{uid:format!("excel-{}",i+1),seq:(i+1)as i64,sort:*sort,parent_uid,title:row.title.clone(),executor:row.executor.clone(),addressees:row.addressees.clone(),start:row.start.clone(),deadline:row.deadline.clone(),status:StageStatus::New,comment:row.comment.clone()});
    }
    let snapshot=ProductionSnapshot{database_id:None,base_revision:None,project,stages,next_seq:rows.len()as i64+1};
    let validation=production::validate_snapshot(&snapshot).map_err(|e|e.to_string())?;
    if !validation.valid{return Err(validation.errors.into_iter().map(|issue|issue.message).collect::<Vec<_>>().join(" "))}
    let mut warnings=vec!["Все импортированные этапы получат статус «Не начато». Для записи в базу потребуется подпись.".into()];
    warnings.extend(validation.warnings.into_iter().map(|issue|issue.message));
    Ok(ImportResult{snapshot,rows,warnings})
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn approved_template_is_a_complete_project(){
        let value=template().unwrap();let bytes=STANDARD.decode(value["base64"].as_str().unwrap()).unwrap();
        let imported=parse_bytes(&bytes).unwrap();assert_eq!(imported.snapshot.project.order_no,"000924-27");
        assert_eq!(imported.snapshot.project.start,"2027-01-11");assert_eq!(imported.snapshot.stages.len(),5);
        assert_eq!(imported.snapshot.stages[3].parent_uid.as_deref(),Some("excel-3"));
        assert!(imported.snapshot.stages.iter().all(|stage|stage.status==StageStatus::New));
        assert!(imported.snapshot.database_id.is_none());
    }
    #[test]fn damaged_and_non_xlsx_input_is_rejected(){assert!(parse_bytes(b"not a workbook").is_err());assert!(parse("","file.xls").is_err());}
}
