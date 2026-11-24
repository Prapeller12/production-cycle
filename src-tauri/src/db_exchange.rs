pub(crate) use crate::exchange_core::capture;
use crate::{
    audit,
    exchange_core::*,
    exchange_file,
    infrastructure::{network::NetworkRuntime, portable::Paths},
    local_desktop::LocalSession,
    production::{load_snapshot_by_id_conn, save_snapshot_tx, ProductionDb, ProductionSnapshot},
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use tauri::State;

fn rows(c: &Connection, sql: &str, id: i64) -> R<Vec<String>> {
    let mut st = c.prepare(sql).map_err(err)?;
    let items = st.query_map([id], |r| r.get(0)).map_err(err)?;
    items.collect::<Result<Vec<_>, _>>().map_err(err)
}
fn active(c: &Connection, user: &audit::AuditUserSummary) -> R<()> {
    let ok:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM audit_user WHERE id=? AND key_fingerprint=? AND active=1 AND role IN ('admin','reviewer','project_manager'))",params![user.id,user.key_fingerprint],|r|r.get(0)).map_err(err)?;
    if ok {
        Ok(())
    } else {
        Err("Войдите под действующим профилем.".into())
    }
}
fn loaded(c: &Connection, id: i64) -> R<ProductionSnapshot> {
    load_snapshot_by_id_conn(c, id)
        .map_err(err)?
        .ok_or_else(|| "Проект не найден.".into())
}
fn make_packet(
    tx: &Transaction,
    ids: &[i64],
    kind: &str,
    user: &audit::AuditUserSummary,
    pin: &str,
) -> R<(Packet, String)> {
    active(tx, user)?;
    let key = audit::exchange_signer(tx, user.id, pin)?;
    let (origin, epoch) = store(tx)?;
    if ids.is_empty() || ids.len() > 1000 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err("Выберите проекты для передачи.".into());
    }
    let mut parts = vec![];
    for &id in ids {
        let current = clean(loaded(tx, id)?);
        let (guid,authority,old_epoch,baseline):(String,Option<String>,Option<String>,Option<String>)=tx.query_row("SELECT guid,authority,generation,baseline FROM exchange_project WHERE project_id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(err)?;
        let (base, auth, generation) = if kind == "changes" {
            (
                baseline
                    .map(|s| serde_json::from_str(&s))
                    .transpose()
                    .map_err(err)?,
                authority,
                old_epoch,
            )
        } else {
            tx.execute(
                "INSERT OR IGNORE INTO exchange_issued VALUES(?,?)",
                params![id, hash(&bytes(&current)?)],
            )
            .map_err(err)?;
            (
                Some(current.clone()),
                Some(origin.clone()),
                Some(epoch.clone()),
            )
        };
        let mut ops = vec![];
        let mut results = vec![];
        if kind == "changes" {
            for s in rows(
                tx,
                "SELECT body FROM exchange_outbox WHERE project_id=? ORDER BY seq",
                id,
            )? {
                ops.push(serde_json::from_str(&s).map_err(err)?);
            }
        }
        if kind == "receipt" {
            let mut st = tx
                .prepare(
                    "SELECT op,state,reason FROM exchange_inbox WHERE project_id=? ORDER BY rowid",
                )
                .map_err(err)?;
            results = st
                .query_map([id], |r| {
                    Ok(Outcome {
                        id: r.get(0)?,
                        state: r.get(1)?,
                        reason: r.get(2)?,
                    })
                })
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?;
        }
        let mut notes = vec![];
        if kind != "changes" {
            for item in rows(
                tx,
                "SELECT body FROM exchange_note WHERE project_id=? ORDER BY rowid",
                id,
            )? {
                notes.push(serde_json::from_str(&item).map_err(err)?);
            }
        }
        parts.push(Part {
            notes,
            guid,
            authority: auth,
            epoch: generation,
            base,
            current,
            ops,
            results,
        });
    }
    let p = Packet {
        format: "production-cycle-exchange-1".into(),
        id: uid(),
        kind: kind.into(),
        store: origin,
        epoch,
        exporter: user.display_name.clone(),
        key: pubkey(&key),
        parts,
    };
    let sig = sign(&key, &p)?;
    Ok((p, sig))
}
fn validate_route(shared: bool, kind: &str, sending: bool) -> R<()> {
    let allowed = match (shared, sending) {
        (true, true) | (false, false) => {
            matches!(kind, "assignment" | "receipt") || (!sending && kind == "legacy")
        }
        (false, true) => kind == "changes",
        (true, false) => matches!(kind, "changes" | "legacy"),
    };
    if allowed {
        Ok(())
    } else {
        Err("Этот тип передачи предназначен для другого рабочего места. Проверьте сетевой или автономный режим запуска.".into())
    }
}
#[tauri::command]
pub fn exchange_export(
    ids: Vec<i64>,
    kind: String,
    pin: String,
    passphrase: String,
    db: State<ProductionDb>,
    session: State<LocalSession>,
    network: State<NetworkRuntime>,
    paths: State<Paths>,
) -> R<String> {
    let session_guard = session.exchange_identity()?;
    let user = session_guard
        .as_ref()
        .ok_or("Войдите под своим профилем.")?;
    validate_route(network.enabled(), &kind, true)?;
    if !["assignment", "changes", "receipt"].contains(&kind.as_str()) {
        return Err("Неизвестный тип файла.".into());
    }
    crate::vault::validate_phrase(&passphrase)?;
    let (p, sig) = network.with_write(&db, || {
        let mut c = db.conn.lock().map_err(err)?;
        let tx = c.transaction().map_err(err)?;
        let result = make_packet(&tx, &ids, &kind, &user, &pin)?;
        tx.commit().map_err(err)?;
        Ok(result)
    })?;
    let path = paths.exports.join(format!("PC-{}-{}.pcdb", kind, p.id));
    std::fs::create_dir_all(&paths.exports).map_err(err)?;
    exchange_file::write(&path, &p, &sig, &passphrase)?;
    Ok(path.display().to_string())
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Choice {
    guid: String,
    target: i64,
} // -1 skip, 0 new, positive explicit existing id
fn parse(encoded: &str, pass: &str) -> R<Packet> {
    if encoded.len() > exchange_file::MAX * 4 / 3 + 8 {
        return Err("Слишком большой файл.".into());
    }
    exchange_file::read(&B64.decode(encoded).map_err(err)?, pass)
}
fn target(c: &Connection, p: &Packet, part: &Part) -> R<Option<i64>> {
    if let Some(id) = c
        .query_row(
            "SELECT project_id FROM exchange_mapping WHERE source=? AND remote_id=?",
            params![p.store, part.guid],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
    {
        return Ok(Some(id));
    }
    if p.kind == "legacy" {
        return Ok(None);
    }
    c.query_row(
        "SELECT project_id FROM exchange_project WHERE guid=?",
        [&part.guid],
        |r| r.get(0),
    )
    .optional()
    .map_err(err)
}
fn keys(c: &Connection, p: &Packet) -> R<Vec<Value>> {
    let mut all = BTreeMap::new();
    if !p.key.is_empty() {
        all.insert(p.key.clone(), p.exporter.clone());
    }
    for part in &p.parts {
        for signed in part.ops.iter().chain(&part.notes) {
            all.insert(signed.key.clone(), signed.change.author.clone());
        }
    }
    let mut result = vec![];
    for (key, name) in all {
        if !known(c, &key)? {
            result.push(
                json!({"key":key,"name":name,"fingerprint":hash(&B64.decode(&key).map_err(err)?)}),
            );
        }
    }
    Ok(result)
}
fn discover(c: &Connection, p: &Packet) -> R<Value> {
    let mut projects = vec![];
    for part in &p.parts {
        projects.push(json!({"guid":part.guid,"name":part.current.project.name,"order":part.current.project.order_no,"stages":part.current.stages.len(),"target":target(c,p,part)?}));
    }
    Ok(
        json!({"projects":projects,"kind":p.kind,"sender":p.exporter,"keys":keys(c,p)?,"legacy":p.kind=="legacy"}),
    )
}
fn existing(c: &Connection, id: &str) -> R<Option<(String, String, String)>> {
    c.query_row(
        "SELECT digest,state,reason FROM exchange_inbox WHERE op=?",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .map_err(err)
}
fn classify(
    tx: &Transaction,
    project: i64,
    snap: &mut ProductionSnapshot,
    signed: &Signed,
    uncertain: bool,
    blocked: &mut BTreeSet<(Option<String>, String)>,
) -> R<Value> {
    let op = &signed.change;
    let dig = hash(&bytes(signed)?);
    let field_key = (op.stage.clone(), op.field.clone());
    if let Some((prior, state, reason)) = existing(tx, &op.id)? {
        if prior != dig {
            return Err(
                "Код ранее полученной операции имеет другое содержимое. Приём остановлен.".into(),
            );
        }
        if state == "needs_review" {
            blocked.insert(field_key);
        }
        return Ok(
            json!({"id":op.id,"field":op.field,"stage":op.stage,"state":state,"reason":reason,"repeat":true}),
        );
    }
    let server = field(snap, op)?;
    let (state, reason) = if op.field == "$note" {
        add_note(tx, project, snap, signed)?
    } else if blocked.contains(&field_key) {
        (
            "needs_review",
            "Предыдущая правка этого поля ожидает решения.".into(),
        )
    } else if uncertain && server != op.after {
        (
            "needs_review",
            "Исходная версия не подтверждена. Проверьте предложение.".into(),
        )
    } else {
        apply(snap, op, false)?
    };
    if state == "needs_review" {
        blocked.insert(field_key);
    }
    tx.execute(
        "INSERT INTO exchange_inbox(op,digest,project_id,body,state,reason) VALUES(?,?,?,?,?,?)",
        params![
            op.id,
            dig,
            project,
            serde_json::to_string(signed).map_err(err)?,
            state,
            reason
        ],
    )
    .map_err(err)?;
    Ok(
        json!({"id":op.id,"field":op.field,"stage":op.stage,"author":op.author,"before":op.before,"server":server,"incoming":op.after,"state":state,"reason":reason,"repeat":false}),
    )
}
fn transfer(
    tx: &Transaction,
    p: &Packet,
    choices: &[Choice],
    user: &audit::AuditUserSummary,
    signature: Option<(&str, &str)>,
) -> R<Value> {
    active(tx, user)?;
    let (home, epoch) = store(tx)?;
    let packet_hash = hash(&bytes(p)?);
    if let Some(d) = tx
        .query_row(
            "SELECT digest FROM exchange_files WHERE id=?",
            [&p.id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(err)?
    {
        if d != packet_hash {
            return Err("Подменён файл с прежним кодом.".into());
        }
    }
    if choices.is_empty()
        || choices.iter().all(|x| x.target < 0)
        || choices
            .iter()
            .map(|v| &v.guid)
            .collect::<BTreeSet<_>>()
            .len()
            != choices.len()
    {
        return Err("Выберите хотя бы один проект и его назначение.".into());
    }
    let mut touched = BTreeSet::new();
    let mut projects = vec![];
    for selection in choices.iter().filter(|c| c.target >= 0) {
        let part = p
            .parts
            .iter()
            .find(|x| x.guid == selection.guid)
            .ok_or("Неизвестный проект в выборе.")?;
        let matched = target(tx, p, part)?;
        if let Some(id) = matched {
            if id != selection.target {
                return Err("Связь проекта изменилась. Повторите предпросмотр.".into());
            }
        }
        if let Some(report) = tx
            .query_row(
                "SELECT report FROM exchange_delivery WHERE packet=? AND guid=?",
                params![p.id, part.guid],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(err)?
        {
            let mut replay: Value = serde_json::from_str(&report).map_err(err)?;
            replay["changed"] = Value::Bool(false);
            if let Some(rows) = replay["rows"].as_array_mut() {
                for row in rows {
                    row["repeat"] = Value::Bool(true);
                    if let Some(id) = row["id"].as_str() {
                        if let Some((_, state, reason)) = existing(tx, id)? {
                            row["state"] = json!(state);
                            row["reason"] = json!(reason);
                        }
                    }
                }
            }
            projects.push(replay);
            continue;
        }
        let is_new = selection.target == 0;
        if !is_new && !touched.insert(selection.target) {
            return Err("Две карточки нельзя записать в один проект за один приём.".into());
        }
        if p.kind == "changes" && part.authority.as_ref().is_some_and(|v| v != &home) {
            return Err("Проект выдан другой общей базой.".into());
        }
        if p.kind == "changes" && part.authority.is_some() && part.epoch.as_deref() != Some(&epoch)
        {
            return Err("Общая база была восстановлена. Требуется повторная выдача.".into());
        }
        let old = if is_new {
            None
        } else {
            Some(loaded(tx, selection.target)?)
        };
        let mut snap = old.clone().unwrap_or_else(|| empty(&part.current));
        let mut report = vec![];
        let id = if is_new {
            // An assignment is an explicitly accepted, signed reference snapshot. Keep its original statuses.
            if p.kind == "assignment" || p.kind == "receipt" {
                snap = clean(part.current.clone());
            }
            let n = save_snapshot_tx(tx, &snap).map_err(err)?;
            if p.kind != "legacy" {
                tx.execute(
                    "UPDATE exchange_project SET guid=? WHERE project_id=?",
                    params![part.guid, n],
                )
                .map_err(err)?;
            }
            snap = loaded(tx, n)?;
            n
        } else {
            selection.target
        };
        let (bound, bound_epoch, baseline): (Option<String>, Option<String>, Option<String>) = tx
            .query_row(
                "SELECT authority,generation,baseline FROM exchange_project WHERE project_id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(err)?;
        let is_reply = p.kind == "receipt" || p.kind == "assignment";
        if is_reply && bound.as_ref().is_some_and(|v| v != &p.store) {
            return Err("Ответ создан другой общей базой.".into());
        }
        if p.kind == "receipt" && bound_epoch.as_ref().is_some_and(|v| v != &p.epoch) {
            return Err("Поколение базы изменилось. Получите новую выдачу.".into());
        }
        let verified_base = if let Some(b) = &part.base {
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM exchange_issued WHERE project_id=? AND digest=?)",
                params![id, hash(&bytes(b)?)],
                |r| r.get::<_, bool>(0),
            )
            .map_err(err)?
        } else {
            false
        };
        let mut blocked = BTreeSet::new();
        if is_reply {
            if !is_new {
                let base: Option<ProductionSnapshot> = baseline
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(err)?;
                let from = base.as_ref().or(old.as_ref());
                let proposals = synthetic(
                    part,
                    from,
                    &part.current,
                    &format!("{}-{}", p.store, p.epoch),
                )?;
                for s in proposals {
                    report.push(classify(
                        tx,
                        id,
                        &mut snap,
                        &s,
                        base.is_none(),
                        &mut blocked,
                    )?);
                }
            }
            if p.kind == "receipt" {
                for result in &part.results {
                    // Open disputes stay in the sending outbox until a final decision arrives.
                    if ["applied", "already_accounted", "rejected"].contains(&result.state.as_str())
                    {
                        tx.execute(
                            "DELETE FROM exchange_outbox WHERE project_id=? AND op=?",
                            params![id, result.id],
                        )
                        .map_err(err)?;
                    }
                }
            }
            tx.execute("UPDATE exchange_project SET authority=?,generation=?,baseline=? WHERE project_id=?",params![p.store,p.epoch,serde_json::to_string(&part.current).map_err(err)?,id]).map_err(err)?;
        } else {
            // Snapshot-only legacy records have no trustworthy base. Exact repeats are mapped by file digest.
            let mut proposals = part.ops.clone();
            if p.kind == "legacy" || proposals.is_empty() && part.base.is_none() {
                let source = if p.kind == "legacy" {
                    format!("old-to-{id}-{}", part.guid)
                } else {
                    p.store.clone()
                };
                proposals = synthetic(part, old.as_ref(), &part.current, &source)?;
                if p.kind == "legacy" {
                    proposals.retain(|s| s.change.field != "$delete");
                }
            }
            if p.kind == "changes" && is_new && !part.ops.is_empty() && part.base.is_none() {
                let covered: BTreeSet<_> = proposals
                    .iter()
                    .filter(|s| s.change.field == "$create")
                    .filter_map(|s| s.change.stage.clone())
                    .collect();
                // Old native projects may have stages created before the operation journal existed.
                let mut initial: Vec<_> = synthetic(part, None, &part.current, &p.store)?
                    .into_iter()
                    .filter(|s| {
                        s.change
                            .stage
                            .as_ref()
                            .is_some_and(|id| !covered.contains(id))
                    })
                    .collect();
                initial.append(&mut proposals);
                proposals = initial;
            }
            for s in proposals {
                let uncertain = !is_new
                    && (!verified_base && (matched.is_none() || part.base.is_some())
                        || p.kind == "legacy");
                report.push(classify(tx, id, &mut snap, &s, uncertain, &mut blocked)?);
            }
        }
        tx.execute(
            "INSERT OR IGNORE INTO exchange_mapping VALUES(?,?,?)",
            params![p.store, part.guid, id],
        )
        .map_err(err)?;
        for note in &part.notes {
            if !is_reply || note.change.field != "$note" {
                return Err("Некорректная коллекция примечаний.".into());
            }
            report.push(classify(tx, id, &mut snap, note, false, &mut blocked)?);
        }
        let changed = old.as_ref().map(|r| bytes(&clean(r.clone()))).transpose()?
            != Some(bytes(&clean(snap.clone()))?);
        let fresh = is_new || report.iter().any(|x| x["repeat"] == false);
        if changed {
            check(&snap)?;
            save_snapshot_tx(tx, &snap).map_err(err)?;
        }
        if let Some((pin, comment)) = signature {
            if changed || fresh {
                let current = loaded(tx, id)?;
                audit::record_exchange(
                    tx,
                    user.id,
                    pin,
                    id,
                    &format!("Обмен • {} • {}", p.id, comment),
                    old.as_ref(),
                    &current,
                )?;
            }
        }
        let result = json!({"project":part.current.project.name,"target":id,"guid":part.guid,"priorRevision":old.as_ref().and_then(|s|s.base_revision.clone()),"changed":changed,"rows":report,"receipt":part.results});
        tx.execute(
            "INSERT INTO exchange_delivery VALUES(?,?,?,?)",
            params![
                p.id,
                part.guid,
                id,
                serde_json::to_string(&result).map_err(err)?
            ],
        )
        .map_err(err)?;
        projects.push(result);
    }
    tx.execute(
        "INSERT OR IGNORE INTO exchange_files VALUES(?,?)",
        params![p.id, packet_hash],
    )
    .map_err(err)?;
    Ok(json!({"projects":projects,"packet":p.id}))
}
#[tauri::command]
pub fn exchange_preview(
    data: String,
    passphrase: String,
    choices: Option<Vec<Choice>>,
    db: State<ProductionDb>,
    session: State<LocalSession>,
    network: State<NetworkRuntime>,
) -> R<Value> {
    let session_guard = session.exchange_identity()?;
    let u = session_guard
        .as_ref()
        .ok_or("Войдите под своим профилем.")?;
    let p = parse(&data, &passphrase)?;
    validate_route(network.enabled(), &p.kind, false)?;
    network.with_read(&db, || {
        let mut c = db.conn.lock().map_err(err)?;
        active(&c, &u)?;
        let mut v = discover(&c, &p)?;
        if let Some(choices) = choices {
            let tx = c.transaction().map_err(err)?;
            let plan = transfer(&tx, &p, &choices, &u, None)?;
            let token = hash(&bytes(&json!([
                &plan,
                &choices,
                &u.key_fingerprint,
                &v["keys"]
            ]))?);
            v["plan"] = plan;
            v["token"] = json!(token);
            tx.rollback().map_err(err)?;
        }
        Ok(v)
    })
}
#[tauri::command]
pub fn exchange_commit(
    data: String,
    passphrase: String,
    choices: Vec<Choice>,
    token: String,
    trust: bool,
    pin: String,
    comment: String,
    db: State<ProductionDb>,
    session: State<LocalSession>,
    network: State<NetworkRuntime>,
) -> R<Value> {
    let session_guard = session.exchange_identity()?;
    let u = session_guard
        .as_ref()
        .ok_or("Войдите под своим профилем.")?;
    let p = parse(&data, &passphrase)?;
    validate_route(network.enabled(), &p.kind, false)?;
    if comment.trim().is_empty() {
        return Err("Введите комментарий к подписанному приёму.".into());
    }
    network.with_write(&db, || {
        let mut c = db.conn.lock().map_err(err)?;
        let tx = c.transaction().map_err(err)?;
        active(&tx, &u)?;
        audit::exchange_signer(&tx, u.id, &pin)?;
        let unknown = keys(&tx, &p)?;
        if (!unknown.is_empty() || p.kind == "legacy") && !trust {
            return Err("Подтвердите сверку источника перед приёмом.".into());
        }
        let result = transfer(&tx, &p, &choices, &u, Some((&pin, &comment)))?;
        if token
            != hash(&bytes(&json!([
                &result,
                &choices,
                &u.key_fingerprint,
                &unknown
            ]))?)
        {
            return Err("Проект или полномочия изменились. Повторите предпросмотр.".into());
        }
        for k in unknown {
            tx.execute(
                "INSERT OR IGNORE INTO exchange_keys VALUES(?,?)",
                params![k["key"].as_str(), k["name"].as_str()],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        drop(c);
        let mut result = result;
        if let Err(e) = db.create_backup("exchange") {
            result["backupWarning"] = json!(e);
        }
        Ok(result)
    })
}
fn add_note(
    tx: &Transaction,
    id: i64,
    snap: &ProductionSnapshot,
    signed: &Signed,
) -> R<(&'static str, String)> {
    let stage = signed
        .change
        .stage
        .as_ref()
        .ok_or("Не указан этап примечания.")?;
    let body = signed
        .change
        .after
        .as_str()
        .ok_or("Не указан текст примечания.")?;
    if body.trim().is_empty() || body.len() > 16000 {
        return Err("Примечание пустое или слишком длинное.".into());
    }
    if !snap.stages.iter().any(|s| &s.uid == stage) {
        return Ok(("needs_review", "Этап примечания отсутствует.".into()));
    }
    let payload = serde_json::to_string(signed).map_err(err)?;
    if let Some(existing) = tx
        .query_row(
            "SELECT body FROM exchange_note WHERE op=?",
            [&signed.change.id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(err)?
    {
        if existing != payload {
            return Err("Код примечания использован для другого содержимого.".into());
        }
        return Ok(("already_accounted", "Примечание уже есть.".into()));
    }
    tx.execute(
        "INSERT INTO exchange_note VALUES(?,?,?,?)",
        params![signed.change.id, id, stage, payload],
    )
    .map_err(err)?;
    Ok(("applied", "Примечание добавлено отдельной записью.".into()))
}
#[tauri::command]
pub fn exchange_notes(
    project: i64,
    stage: String,
    db: State<ProductionDb>,
    session: State<LocalSession>,
    network: State<NetworkRuntime>,
) -> R<Vec<Change>> {
    let guard = session.exchange_identity()?;
    let user = guard.as_ref().ok_or("Войдите под своим профилем.")?;
    network.with_read(&db, || {
        let c = db.conn.lock().map_err(err)?;
        active(&c, user)?;
        let mut s = c
            .prepare("SELECT body FROM exchange_note WHERE project_id=? AND stage=? ORDER BY rowid")
            .map_err(err)?;
        let it = s
            .query_map(params![project, stage], |r| r.get::<_, String>(0))
            .map_err(err)?;
        it.map(|v| {
            Ok(serde_json::from_str::<Signed>(&v.map_err(err)?)
                .map_err(err)?
                .change)
        })
        .collect()
    })
}
#[tauri::command]
pub fn exchange_add_note(
    project: i64,
    stage: String,
    text: String,
    pin: String,
    db: State<ProductionDb>,
    session: State<LocalSession>,
    network: State<NetworkRuntime>,
) -> R<()> {
    let guard = session.exchange_identity()?;
    let user = guard.as_ref().ok_or("Войдите под своим профилем.")?;
    network.with_write(&db, || {
        let mut c = db.conn.lock().map_err(err)?;
        let tx = c.transaction().map_err(err)?;
        active(&tx, user)?;
        let key = audit::exchange_signer(&tx, user.id, &pin)?;
        let snap = loaded(&tx, project)?;
        let guid = tx
            .query_row(
                "SELECT guid FROM exchange_project WHERE project_id=?",
                [project],
                |r| r.get::<_, String>(0),
            )
            .map_err(err)?;
        let op = Change {
            id: uid(),
            project: guid,
            stage: Some(stage),
            field: "$note".into(),
            before: Value::Null,
            after: json!(text.trim()),
            author: user.display_name.clone(),
            at: chrono::Utc::now().to_rfc3339(),
        };
        let signed = Signed {
            key: pubkey(&key),
            signature: sign(&key, &op)?,
            change: op,
        };
        let (state, msg) = add_note(&tx, project, &snap, &signed)?;
        if state != "applied" {
            return Err(msg);
        }
        tx.execute(
            "INSERT INTO exchange_outbox(project_id,op,body) VALUES(?,?,?)",
            params![
                project,
                signed.change.id,
                serde_json::to_string(&signed).map_err(err)?
            ],
        )
        .map_err(err)?;
        audit::record_exchange(
            &tx,
            user.id,
            &pin,
            project,
            &format!("Добавлено примечание {}", signed.change.id),
            Some(&snap),
            &snap,
        )?;
        tx.commit().map_err(err)?;
        Ok(())
    })
}
#[tauri::command]
pub fn exchange_reviews(
    db: State<ProductionDb>,
    session: State<LocalSession>,
    network: State<NetworkRuntime>,
) -> R<Vec<Value>> {
    let session_guard = session.exchange_identity()?;
    let u = session_guard
        .as_ref()
        .ok_or("Войдите под своим профилем.")?;
    network.with_read(&db,||{let c=db.conn.lock().map_err(err)?;active(&c,&u)?;
        let mut st=c.prepare("SELECT project_id,body,reason FROM exchange_inbox WHERE state='needs_review' ORDER BY rowid").map_err(err)?;
        let mut out=vec![];let rows=st.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(err)?;
        for row in rows{let(id,body,reason)=row.map_err(err)?;let s:Signed=serde_json::from_str(&body).map_err(err)?;let snapshot=loaded(&c,id)?;
            out.push(json!({"project":snapshot.project.name,"stage":s.change.stage,"op":s.change,"server":field(&snapshot,&s.change)?,"revision":snapshot.base_revision,"reason":reason}));}Ok(out)})
}
#[tauri::command]
pub fn exchange_resolve(
    op: String,
    revision: String,
    accept: bool,
    pin: String,
    comment: String,
    db: State<ProductionDb>,
    session: State<LocalSession>,
    network: State<NetworkRuntime>,
) -> R<()> {
    let session_guard = session.exchange_identity()?;
    let u = session_guard
        .as_ref()
        .ok_or("Войдите под своим профилем.")?;
    if comment.trim().is_empty() {
        return Err("Введите основание решения.".into());
    }
    network.with_write(&db, || {
        let mut c = db.conn.lock().map_err(err)?;
        let tx = c.transaction().map_err(err)?;
        active(&tx, &u)?;
        let (id, body): (i64, String) = tx
            .query_row(
                "SELECT project_id,body FROM exchange_inbox WHERE op=? AND state='needs_review'",
                [&op],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| "Решение уже принято или запись не найдена.")?;
        let old = loaded(&tx, id)?;
        if old.base_revision.as_deref() != Some(&revision) {
            return Err("Проект изменился. Обновите список решений.".into());
        }
        let signed: Signed = serde_json::from_str(&body).map_err(err)?;
        if !signed.key.is_empty() {
            known(&tx, &signed.key)?;
        }
        let mut next = old.clone();
        let status = if accept {
            let (state, reason) = if signed.change.field == "$note" {
                add_note(&tx, id, &next, &signed)?
            } else {
                apply(&mut next, &signed.change, true)?
            };
            if state == "needs_review" {
                return Err(reason);
            }
            state
        } else {
            "rejected"
        };
        if status == "applied" {
            save_snapshot_tx(&tx, &next).map_err(err)?;
        }
        let current = loaded(&tx, id)?;
        audit::record_exchange(
            &tx,
            u.id,
            &pin,
            id,
            &format!("Согласование {} • {}", op, comment),
            Some(&old),
            &current,
        )?;
        tx.execute(
            "UPDATE exchange_inbox SET state=?,reason=? WHERE op=?",
            params![status, comment, op],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(())
    })
}

#[cfg(test)]
mod exchange_tests {
    use super::*;
    use crate::audit::CreateAuditUserInput;
    const PIN: &str = "837492";
    fn operator(
        db: &ProductionDb,
        name: &str,
        role: &str,
        admin: Option<i64>,
    ) -> audit::AuditUserSummary {
        db.create_audit_user(CreateAuditUserInput {
            display_name: name.into(),
            pin: PIN.into(),
            role: Some(role.into()),
            is_admin: admin.is_none(),
            admin_user_id: admin,
            admin_pin: admin.map(|_| PIN.into()),
        })
        .unwrap()
    }
    fn sample() -> ProductionSnapshot {
        serde_json::from_value(json!({"project":{"name":"Образец","orderNo":"001","initiator":"Цех","executor":"Сборка","enterprise":"Площадка","start":"2026-12-01","deadline":"2026-12-31"},"stages":[{"uid":"S1","seq":1,"sort":1,"parentUid":null,"title":"Контроль","executor":"Цех","addressees":"ОТК","start":"2026-12-01","deadline":"2026-12-15","status":"new","comment":""}],"nextSeq":2})).unwrap()
    }
    fn outgoing(
        db: &ProductionDb,
        id: i64,
        kind: &str,
        u: &audit::AuditUserSummary,
    ) -> (Packet, String) {
        let mut c = db.conn.lock().unwrap();
        let tx = c.transaction().unwrap();
        let p = make_packet(&tx, &[id], kind, u, PIN).unwrap();
        tx.commit().unwrap();
        p
    }
    fn receive(db: &ProductionDb, p: &Packet, target: i64, u: &audit::AuditUserSummary) -> Value {
        let mut c = db.conn.lock().unwrap();
        let tx = c.transaction().unwrap();
        let choices = [Choice {
            guid: p.parts[0].guid.clone(),
            target,
        }];
        let v = transfer(&tx, p, &choices, u, Some((PIN, "Испытание"))).unwrap();
        tx.commit().unwrap();
        v
    }
    #[test]
    fn field_roundtrip_parallel_change_replay_receipt_and_all_roles() {
        let server = ProductionDb::memory().unwrap();
        let offline = ProductionDb::memory().unwrap();
        let a = operator(&server, "Администратор", "admin", None);
        let r = operator(&server, "Проверяющий", "reviewer", Some(a.id));
        let m = operator(&server, "Руководитель", "project_manager", Some(a.id));
        let b = operator(&offline, "Автономный оператор", "admin", None);
        let id = server
            .save_signed_snapshot(&sample(), a.id, PIN, "Исходный", None)
            .unwrap()
            .project_id;
        for u in [&a, &r, &m] {
            let (p, s) = outgoing(&server, id, "assignment", u);
            verify(&p.key, &s, &p).unwrap();
        }
        let (issued, sig) = outgoing(&server, id, "assignment", &r);
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("issue.pcdb");
        exchange_file::write(&path, &issued, &sig, "separate-test-phrase").unwrap();
        let raw = std::fs::read(path).unwrap();
        assert!(!raw.starts_with(b"SQLite format 3"));
        assert!(exchange_file::read(&raw, "wrong-test-phrase").is_err());
        let issued = exchange_file::read(&raw, "separate-test-phrase").unwrap();
        let v = receive(&offline, &issued, 0, &b);
        let lid = v["projects"][0]["target"].as_i64().unwrap();
        let mut left = server.load_snapshot_by_id(id).unwrap().unwrap();
        left.stages[0].deadline = "2026-12-19".into();
        server
            .save_signed_snapshot(&left, a.id, PIN, "Срок сервера", None)
            .unwrap();
        let mut right = offline.load_snapshot_by_id(lid).unwrap().unwrap();
        right.stages[0].deadline = "2026-12-17".into();
        right.stages[0].executor = "Новый цех".into();
        let mut extra = right.stages[0].clone();
        extra.uid = "S2".into();
        extra.seq = 2;
        extra.sort = 2;
        extra.title = "Новый этап".into();
        right.stages.push(extra);
        right.next_seq = 3;
        offline
            .save_signed_snapshot(&right, b.id, PIN, "Работа без связи", None)
            .unwrap();
        let (changes, _) = outgoing(&offline, lid, "changes", &b);
        let accepted = receive(&server, &changes, id, &m);
        let rows = accepted["projects"][0]["rows"].as_array().unwrap();
        assert_eq!(rows.iter().filter(|v| v["state"] == "applied").count(), 2);
        assert_eq!(
            rows.iter().filter(|v| v["state"] == "needs_review").count(),
            1
        );
        let replay = receive(&server, &changes, id, &r);
        assert!(replay["projects"][0]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["repeat"] == true));
        let s = server.load_snapshot_by_id(id).unwrap().unwrap();
        assert_eq!(s.stages.len(), 2);
        assert_eq!(s.stages[0].deadline, "2026-12-19");
        assert_eq!(s.stages[0].executor, "Новый цех");
        let (receipt, _) = outgoing(&server, id, "receipt", &a);
        receive(&offline, &receipt, lid, &b);
        let (changes, _) = outgoing(&offline, lid, "changes", &b);
        assert_eq!(changes.parts[0].ops.len(), 1);
    }
}
