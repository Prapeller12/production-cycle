use crate::production::{validate_snapshot, ProductionSnapshot, ProductionStageInput, StageStatus};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
pub type R<T> = Result<T, String>;
pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
pub fn bytes<T: Serialize>(v: &T) -> R<Vec<u8>> {
    serde_json::to_vec(v).map_err(err)
}
pub fn uid() -> String {
    Uuid::new_v4().to_string()
}
pub fn check(v: &ProductionSnapshot) -> R<()> {
    let result = validate_snapshot(v).map_err(err)?;
    if result.valid {
        Ok(())
    } else {
        Err(result
            .errors
            .iter()
            .take(3)
            .map(|v| v.message.as_str())
            .collect::<Vec<_>>()
            .join(" "))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub project: String,
    pub stage: Option<String>,
    pub field: String,
    pub before: Value,
    pub after: Value,
    pub author: String,
    pub at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signed {
    pub change: Change,
    pub key: String,
    pub signature: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub guid: String,
    pub authority: Option<String>,
    pub epoch: Option<String>,
    pub base: Option<ProductionSnapshot>,
    pub current: ProductionSnapshot,
    pub ops: Vec<Signed>,
    pub results: Vec<Outcome>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outcome {
    pub id: String,
    pub state: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    pub format: String,
    pub id: String,
    pub kind: String,
    pub store: String,
    pub epoch: String,
    pub exporter: String,
    pub key: String,
    pub parts: Vec<Part>,
}
pub fn verify<T: Serialize>(key: &str, sig: &str, obj: &T) -> R<()> {
    let raw: [u8; 32] = B64
        .decode(key)
        .map_err(err)?
        .try_into()
        .map_err(|_| "Неверный открытый ключ.")?;
    let key = VerifyingKey::from_bytes(&raw).map_err(err)?;
    let s = Signature::from_slice(&B64.decode(sig).map_err(err)?).map_err(err)?;
    key.verify_strict(&bytes(obj)?, &s)
        .map_err(|_| "Подпись источника не подтверждена.".into())
}
pub fn pubkey(k: &SigningKey) -> String {
    B64.encode(k.verifying_key().as_bytes())
}
pub fn sign<T: Serialize>(k: &SigningKey, v: &T) -> R<String> {
    Ok(B64.encode(k.sign(&bytes(v)?).to_bytes()))
}
pub fn clean(mut s: ProductionSnapshot) -> ProductionSnapshot {
    s.database_id = None;
    s.base_revision = None;
    s
}
pub fn empty(s: &ProductionSnapshot) -> ProductionSnapshot {
    ProductionSnapshot {
        database_id: None,
        base_revision: None,
        project: s.project.clone(),
        stages: vec![],
        next_seq: 1,
    }
}
const STAGE_FIELDS: [&str; 9] = [
    "title",
    "executor",
    "addressees",
    "start",
    "deadline",
    "comment",
    "status",
    "parentUid",
    "sort",
];
const PROJECT_FIELDS: [&str; 8] = [
    "name",
    "orderNo",
    "initiator",
    "executor",
    "enterprise",
    "start",
    "deadline",
    "topic",
];
pub fn differences(
    guid: &str,
    old: Option<&ProductionSnapshot>,
    new: &ProductionSnapshot,
    author: &str,
) -> R<Vec<Change>> {
    let mut changes = vec![];
    let at = chrono::Utc::now().to_rfc3339();
    let mut emit = |stage: Option<&str>, field: &str, a: Value, b: Value| {
        if a != b {
            changes.push(Change {
                id: uid(),
                project: guid.into(),
                stage: stage.map(str::to_owned),
                field: field.into(),
                before: a,
                after: b,
                author: author.into(),
                at: at.clone(),
            })
        }
    };
    if let Some(old) = old {
        let a = serde_json::to_value(&old.project).map_err(err)?;
        let b = serde_json::to_value(&new.project).map_err(err)?;
        for f in PROJECT_FIELDS {
            emit(None, f, a[f].clone(), b[f].clone())
        }
    }
    let past: HashMap<_, _> = old
        .into_iter()
        .flat_map(|v| v.stages.iter())
        .map(|s| (s.uid.as_str(), s))
        .collect();
    let current: HashSet<_> = new.stages.iter().map(|s| s.uid.as_str()).collect();
    // Parent before child, regardless of the display ordering of siblings.
    let mut indexed = HashMap::new();
    for s in &new.stages {
        indexed.insert(s.uid.as_str(), s);
    }
    let mut ordered = Vec::new();
    let mut visited = HashSet::new();
    for s in &new.stages {
        let mut chain = vec![];
        let mut p = Some(s);
        while let Some(stage) = p {
            if !visited.insert(stage.uid.as_str()) {
                break;
            }
            chain.push(stage);
            p = stage
                .parent_uid
                .as_deref()
                .and_then(|k| indexed.get(k).copied());
        }
        ordered.extend(chain.into_iter().rev());
    }
    for s in ordered {
        let b = serde_json::to_value(s).map_err(err)?;
        if let Some(prior) = past.get(s.uid.as_str()) {
            let a = serde_json::to_value(prior).map_err(err)?;
            for f in STAGE_FIELDS {
                emit(Some(&s.uid), f, a[f].clone(), b[f].clone())
            }
        } else {
            emit(Some(&s.uid), "$create", Value::Null, b)
        }
    }
    if let Some(old) = old {
        for s in &old.stages {
            if !current.contains(s.uid.as_str()) {
                emit(
                    Some(&s.uid),
                    "$delete",
                    serde_json::to_value(s).map_err(err)?,
                    Value::Null,
                )
            }
        }
    }
    Ok(changes)
}
pub fn capture(
    tx: &Transaction,
    id: i64,
    old: Option<&ProductionSnapshot>,
    new: &ProductionSnapshot,
    actor: &str,
    key: &SigningKey,
) -> R<()> {
    let guid: String = tx
        .query_row(
            "SELECT guid FROM exchange_project WHERE project_id=?",
            [id],
            |r| r.get(0),
        )
        .map_err(err)?;
    let author_key = pubkey(key);
    for op in differences(&guid, old, new, actor)? {
        let s = Signed {
            signature: sign(key, &op)?,
            key: author_key.clone(),
            change: op,
        };
        tx.execute(
            "INSERT INTO exchange_outbox(project_id,op,body) VALUES(?,?,?)",
            params![id, s.change.id, serde_json::to_string(&s).map_err(err)?],
        )
        .map_err(err)?;
    }
    Ok(())
}
pub fn field(s: &ProductionSnapshot, op: &Change) -> R<Value> {
    if let Some(id) = &op.stage {
        let Some(stage) = s.stages.iter().find(|v| &v.uid == id) else {
            return Ok(Value::Null);
        };
        let row = serde_json::to_value(stage).map_err(err)?;
        return Ok(if op.field == "$create" || op.field == "$delete" {
            row
        } else {
            row[&op.field].clone()
        });
    }
    Ok(serde_json::to_value(&s.project).map_err(err)?[&op.field].clone())
}
pub fn apply(s: &mut ProductionSnapshot, op: &Change, manual: bool) -> R<(&'static str, String)> {
    let current = field(s, op)?;
    let review = |why: &str| Ok(("needs_review", why.to_string()));
    if op.field == "$create" {
        if !current.is_null() {
            let mut a = current;
            let mut b = op.after.clone();
            for k in ["seq", "sort"] {
                a.as_object_mut().map(|v| v.remove(k));
                b.as_object_mut().map(|v| v.remove(k));
            }
            return if a == b {
                Ok(("already_accounted", "Этап уже существует.".into()))
            } else {
                review("У этого кода этапа уже есть другие данные.")
            };
        }
        let mut new: ProductionStageInput =
            serde_json::from_value(op.after.clone()).map_err(err)?;
        if op.stage.as_deref() != Some(new.uid.as_str()) || new.status != StageStatus::New {
            return review(
                "Проверьте код и статус нового этапа. Смена статуса проводится с основанием.",
            );
        }
        if new
            .parent_uid
            .as_ref()
            .is_some_and(|p| !s.stages.iter().any(|a| &a.uid == p))
        {
            return review("Сначала требуется вышестоящий этап.");
        }
        if !manual
            && s.stages.iter().any(|a| {
                a.parent_uid == new.parent_uid
                    && a.title.trim().eq_ignore_ascii_case(new.title.trim())
            })
        {
            return review("Возможный дубль по названию и вышестоящему этапу.");
        }
        new.seq = s.next_seq;
        new.sort = s
            .stages
            .iter()
            .filter(|r| r.parent_uid == new.parent_uid)
            .map(|r| r.sort)
            .max()
            .unwrap_or(0)
            + 1;
        let mut next = s.clone();
        next.next_seq += 1;
        next.stages.push(new);
        if let Err(e) = check(&next) {
            return Ok(("needs_review", e));
        }
        *s = next;
        return Ok(("applied", "Этап добавлен.".into()));
    }
    if current == op.after {
        return Ok(("already_accounted", "Значение уже учтено.".into()));
    }
    if op.stage.is_none()
        || ![
            "title",
            "executor",
            "addressees",
            "start",
            "deadline",
            "comment",
        ]
        .contains(&op.field.as_str())
    {
        return review(
            "Требуется штатное изменение реквизитов, структуры или статуса с проверкой полномочий.",
        );
    }
    let Some(pos) = s
        .stages
        .iter()
        .position(|a| Some(&a.uid) == op.stage.as_ref())
    else {
        return review("Этап отсутствует в принимающем проекте.");
    };
    if s.stages[pos].status == StageStatus::Done {
        return review("Этап уже закрыт.");
    }
    if !manual && current != op.before {
        return review("Одно поле изменено обеими сторонами.");
    }
    let mut next = s.clone();
    let mut row = serde_json::to_value(&next.stages[pos]).map_err(err)?;
    row[&op.field] = op.after.clone();
    next.stages[pos] = serde_json::from_value(row).map_err(err)?;
    if let Err(e) = check(&next) {
        return Ok(("needs_review", e));
    }
    *s = next;
    Ok(("applied", "Изменение принято.".into()))
}
pub fn store(c: &Connection) -> R<(String, String)> {
    c.query_row(
        "SELECT uid,generation FROM exchange_store WHERE singleton=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map_err(err)
}
pub fn known(c: &Connection, key: &str) -> R<bool> {
    let active: Option<bool> = c
        .query_row(
            "SELECT active FROM audit_user WHERE public_key=?",
            [key],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?;
    if active == Some(false) {
        return Err("Ключ автора отозван.".into());
    }
    if active == Some(true) {
        return Ok(true);
    }
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM exchange_keys WHERE pubkey=?)",
        [key],
        |r| r.get(0),
    )
    .map_err(err)
}
pub fn synthetic(
    p: &Part,
    old: Option<&ProductionSnapshot>,
    new: &ProductionSnapshot,
    source: &str,
) -> R<Vec<Signed>> {
    let mut result = vec![];
    for mut op in differences(&p.guid, old, new, "Сверка снимка")? {
        op.at.clear();
        op.id = hash(&bytes(&json!([
            source, p.guid, op.stage, op.field, op.before, op.after
        ]))?);
        result.push(Signed {
            change: op,
            key: String::new(),
            signature: String::new(),
        })
    }
    Ok(result)
}
