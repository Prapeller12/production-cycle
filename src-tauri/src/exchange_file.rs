use crate::{
    exchange_core::*,
    production::{load_snapshot_by_id_conn, SCHEMA_VERSION},
    vault,
};
use rusqlite::{params, Connection, OpenFlags};
use std::{io::Write, path::Path};
pub const MAX: usize = 48 * 1024 * 1024;
fn ordinary(c: &Connection, name: &str) -> R<bool> {
    c.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_list WHERE schema='main' AND type='table' AND name=?1)",[name],|r|r.get(0)).map_err(err)
}
pub fn write(path: &Path, p: &Packet, signature: &str, pass: &str) -> R<()> {
    vault::validate_phrase(pass)?;
    let tmp = tempfile::NamedTempFile::new_in(path.parent().ok_or("Нет папки назначения.")?)
        .map_err(err)?;
    let mut c = vault::keyed(tmp.path(), OpenFlags::SQLITE_OPEN_READ_WRITE, Some(pass))?;
    c.execute_batch("PRAGMA journal_mode=DELETE; CREATE TABLE exchange_header(header TEXT NOT NULL,signature TEXT NOT NULL); CREATE TABLE projects(position INTEGER PRIMARY KEY,body TEXT NOT NULL);").map_err(err)?;
    let tx = c.transaction().map_err(err)?;
    let mut header = p.clone();
    header.parts.clear();
    tx.execute(
        "INSERT INTO exchange_header VALUES(?,?)",
        params![serde_json::to_string(&header).map_err(err)?, signature],
    )
    .map_err(err)?;
    for (index, part) in p.parts.iter().enumerate() {
        tx.execute(
            "INSERT INTO projects VALUES(?,?)",
            params![index, serde_json::to_string(part).map_err(err)?],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    drop(c);
    tmp.as_file().sync_all().map_err(err)?;
    if tmp.as_file().metadata().map_err(err)?.len() > MAX as u64 {
        return Err("Файл слишком велик. Выберите меньше проектов.".into());
    }
    tmp.persist_noclobber(path).map_err(err)?;
    Ok(())
}
fn legacy(c: &Connection, id: String) -> R<Packet> {
    if !ordinary(c, "production_cycle")? || !ordinary(c, "production_stage")? {
        return Err("Файл не является базой «Производственный цикл».".into());
    }
    let v: i64 = c
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(err)?;
    if v > SCHEMA_VERSION {
        return Err("База создана более новой версией программы.".into());
    }
    let mut s = c
        .prepare("SELECT id FROM production_cycle ORDER BY id LIMIT 1001")
        .map_err(err)?;
    let ids = s
        .query_map([], |r| r.get::<_, i64>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    if ids.is_empty() || ids.len() > 1000 {
        return Err("Нет проектов либо превышен предел в 1000 проектов.".into());
    }
    let mut parts = vec![];
    for n in ids {
        let snapshot=load_snapshot_by_id_conn(c,n).map_err(|_|"Неизвестный вариант схемы старой базы. Откройте резервную копию в совместимой прежней версии.".to_string())?.ok_or("Проект отсутствует.")?;
        check(&snapshot)?;
        parts.push(Part {
            guid: format!("legacy-{n}"),
            authority: None,
            epoch: None,
            base: None,
            current: clean(snapshot),
            ops: vec![],
            notes: vec![],
            results: vec![],
        })
    }
    Ok(Packet {
        format: "legacy".into(),
        id: id.clone(),
        kind: "legacy".into(),
        store: id,
        epoch: String::new(),
        exporter: "Старая база, автор не подтверждён".into(),
        key: String::new(),
        parts,
    })
}
pub fn read(raw: &[u8], pass: &str) -> R<Packet> {
    if raw.is_empty() || raw.len() > MAX {
        return Err("Размер файла должен быть от 1 байта до 48 МиБ.".into());
    }
    let plain = raw.starts_with(b"SQLite format 3\0");
    let tmp;
    let c = if plain {
        // Read the legacy snapshot in RAM. No plaintext staging file.
        if raw.get(18) == Some(&2) || raw.get(19) == Some(&2) {
            return Err(
                "Снимок в режиме WAL. Создайте согласованную резервную копию в исходной программе."
                    .into(),
            );
        }
        let mut c = Connection::open_in_memory().map_err(err)?;
        let ptr = std::ptr::NonNull::new(
            unsafe { rusqlite::ffi::sqlite3_malloc64(raw.len() as u64) }.cast::<u8>(),
        )
        .ok_or("Недостаточно памяти.")?;
        // sqlite3 owns this exact allocation once deserialize takes OwnedData.
        let owned = unsafe {
            std::ptr::copy_nonoverlapping(raw.as_ptr(), ptr.as_ptr(), raw.len());
            rusqlite::serialize::OwnedData::from_raw_nonnull(ptr, raw.len())
        };
        c.deserialize(rusqlite::DatabaseName::Main, owned, true)
            .map_err(err)?;
        c
    } else {
        vault::validate_phrase(pass)?;
        tmp = {
            let mut f = tempfile::NamedTempFile::new().map_err(err)?;
            f.write_all(raw).map_err(err)?;
            f.into_temp_path()
        };
        vault::keyed(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY, Some(pass))?
    };
    c.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON; PRAGMA temp_store=MEMORY;")
        .map_err(err)?;
    c.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, MAX as i32);
    let health: String = c
        .query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
        .map_err(err)?;
    if health != "ok" {
        return Err("Копия повреждена.".into());
    }
    if !ordinary(&c, "exchange_header")? {
        return legacy(&c, hash(raw));
    }
    if plain || !ordinary(&c, "projects")? {
        return Err("Штатный пакет должен быть зашифрован и содержать таблицу проектов.".into());
    }
    let (header, sig): (String, String) = c
        .query_row("SELECT header,signature FROM exchange_header", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(err)?;
    let mut p: Packet = serde_json::from_str(&header).map_err(err)?;
    if p.format != "production-cycle-exchange-1"
        || !["assignment", "changes", "receipt"].contains(&p.kind.as_str())
        || !p.parts.is_empty()
    {
        return Err("Версия файла обмена не поддерживается.".into());
    }
    let mut stmt = c
        .prepare("SELECT body FROM projects ORDER BY position LIMIT 1001")
        .map_err(err)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
    let mut projects = std::collections::HashSet::new();
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let part: Part = serde_json::from_str(&row.map_err(err)?).map_err(err)?;
        if !projects.insert(part.guid.clone()) || p.parts.len() >= 1000 {
            return Err("Повтор проекта или превышен лимит пакета.".into());
        }
        check(&part.current)?;
        if let Some(b) = &part.base {
            check(b)?
        }
        for s in part.ops.iter().chain(&part.notes) {
            if s.change.project != part.guid || !seen.insert(s.change.id.clone()) {
                return Err("Неверная привязка или повтор кода операции.".into());
            }
            verify(&s.key, &s.signature, &s.change)?
        }
        p.parts.push(part)
    }
    verify(&p.key, &sig, &p)?;
    if p.parts.is_empty() {
        return Err("Нет проектов для обмена.".into());
    }
    Ok(p)
}
