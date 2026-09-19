use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};
use chrono::Local;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tauri::State;
use uuid::Uuid;

use crate::production::{
    load_snapshot_by_id_conn, save_snapshot_tx, validate_snapshot, DictionaryKind,
    DictionaryReplaceResult, ProductionDb, ProductionReport, ProductionSnapshot, StageStatus,
};

const OFFICIAL_EVIDENCE_TYPES: [&str; 5] = [
    "Входящее письмо",
    "Инвойс",
    "Накладная",
    "Акт приёмки",
    "Иной официальный документ",
];

pub fn initialize_audit_schema(conn: &Connection) -> std::result::Result<(), String> {
    let users: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='audit_user')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let events: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='audit_event')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !users || !events {
        return Err("Не удалось создать таблицы пользователей и журнала подтверждений.".into());
    }
    let mut stmt = conn.prepare("PRAGMA table_info(audit_event)").map_err(|e| e.to_string())?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1)).map_err(|e| e.to_string())?
        .collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    drop(stmt);
    if !columns.iter().any(|name| name == "changes_json") {
        conn.execute_batch("ALTER TABLE audit_event ADD COLUMN changes_json TEXT NOT NULL DEFAULT '[]';").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditUserSummary {
    pub id: i64,
    pub display_name: String,
    pub role: String,
    pub key_fingerprint: String,
    pub active: bool,
    pub created_at: String,
}

#[derive(Debug)]
struct AuditUserSecret {
    id: i64,
    display_name: String,
    role: String,
    public_key: String,
    encrypted_private_key: String,
    kdf_salt: String,
    encryption_nonce: String,
    key_fingerprint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAuditUserInput {
    pub display_name: String,
    pub pin: String,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default)]
    pub admin_user_id: Option<i64>,
    #[serde(default)]
    pub admin_pin: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionEvidenceInput {
    pub document_type: String,
    pub document_reference: String,
    pub comment: String,
    #[serde(default)]
    pub stage_uids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedSaveResult {
    pub project_id: i64,
    pub change_event_id: String,
    pub completion_event_ids: Vec<String>,
    pub backup: Option<crate::backup::BackupSummary>,
    pub backup_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventSummary {
    pub seq: i64,
    pub event_id: String,
    pub project_id: i64,
    pub stage_uid: Option<String>,
    pub stage_reg_number: Option<String>,
    pub event_type: String,
    pub actor_name: String,
    pub key_fingerprint: String,
    pub comment: String,
    pub evidence_type: Option<String>,
    pub evidence_reference: Option<String>,
    pub changes: Vec<AuditChange>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditChange {
    pub entity: String,
    pub entity_id: Option<String>,
    pub field: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditVerificationReport {
    pub checked_events: usize,
    pub valid_events: usize,
    pub invalid_events: usize,
    pub chain_heads: Vec<String>,
    pub errors: Vec<String>,
    pub verified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionConfirmation {
    pub confirmer: String,
    pub key_fingerprint: String,
    pub created_at: String,
    pub document_type: String,
    pub document_reference: String,
    pub comment: String,
    pub signature_valid: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditPayload<'a> {
    event_id: &'a str,
    project_id: i64,
    stage_uid: Option<&'a str>,
    stage_reg_number: Option<&'a str>,
    event_type: &'a str,
    actor_user_id: i64,
    actor_name: &'a str,
    key_fingerprint: &'a str,
    comment: &'a str,
    evidence_type: Option<&'a str>,
    evidence_reference: Option<&'a str>,
    changes: &'a [AuditChange],
    snapshot_hash: &'a str,
    previous_hash: &'a str,
    created_at: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAuditPayload {
    event_id: String,
    project_id: i64,
    stage_uid: Option<String>,
    stage_reg_number: Option<String>,
    event_type: String,
    actor_user_id: i64,
    actor_name: String,
    key_fingerprint: String,
    comment: String,
    evidence_type: Option<String>,
    evidence_reference: Option<String>,
    #[serde(default)]
    changes: Vec<AuditChange>,
    snapshot_hash: String,
    previous_hash: String,
    created_at: String,
}

fn clean(value: &str) -> String {
    value.trim().to_string()
}

fn validate_comment(value: &str, label: &str) -> std::result::Result<String, String> {
    let value = clean(value);
    if value.is_empty() {
        return Err(format!("Поле «{label}» обязательно."));
    }
    if value.chars().count() > 2000 {
        return Err(format!("Поле «{label}» не должно превышать 2000 символов."));
    }
    Ok(value)
}

fn shown(value: &str) -> String {
    let value = clean(value);
    if value.is_empty() { "—".into() } else { value }
}

fn status_label(status: StageStatus) -> &'static str {
    match status {
        StageStatus::New => "Не начато",
        StageStatus::Work => "В работе",
        StageStatus::Hold => "Приостановлено",
        StageStatus::Done => "Выполнено",
    }
}

fn stage_number(snapshot: &ProductionSnapshot, uid: &str) -> String {
    snapshot.stages.iter().find(|stage| stage.uid == uid)
        .map(|stage| format!("{}-{:03}", clean(&snapshot.project.order_no), stage.seq))
        .unwrap_or_else(|| uid.to_string())
}

fn parent_label(snapshot: &ProductionSnapshot, parent_uid: Option<&str>) -> String {
    parent_uid.map(|uid| stage_number(snapshot, uid))
        .unwrap_or_else(|| format!("Заказ № {}", clean(&snapshot.project.order_no)))
}

fn add_change(changes: &mut Vec<AuditChange>, entity: &str, entity_id: Option<String>, field: &str, before: String, after: String) {
    if before != after {
        changes.push(AuditChange { entity: entity.into(), entity_id, field: field.into(), before, after });
    }
}

fn snapshot_changes(previous: Option<&ProductionSnapshot>, current: &ProductionSnapshot) -> Vec<AuditChange> {
    let Some(previous) = previous else {
        return vec![AuditChange {
            entity: "Проект".into(),
            entity_id: None,
            field: "Создание".into(),
            before: "—".into(),
            after: shown(&current.project.name),
        }];
    };
    let mut changes = Vec::new();
    let before_project = &previous.project;
    let after_project = &current.project;
    for (field, before, after) in [
        ("Название", before_project.name.as_str(), after_project.name.as_str()),
        ("Номер заказа", before_project.order_no.as_str(), after_project.order_no.as_str()),
        ("Инициатор / подписант", before_project.initiator.as_str(), after_project.initiator.as_str()),
        ("Исполнитель / получатель", before_project.executor.as_str(), after_project.executor.as_str()),
        ("Предприятие", before_project.enterprise.as_str(), after_project.enterprise.as_str()),
        ("Начало", before_project.start.as_str(), after_project.start.as_str()),
        ("Дедлайн", before_project.deadline.as_str(), after_project.deadline.as_str()),
    ] {
        add_change(&mut changes, "Проект", None, field, shown(before), shown(after));
    }
    let before_by_uid = previous.stages.iter().map(|stage| (stage.uid.as_str(), stage)).collect::<HashMap<_, _>>();
    let after_by_uid = current.stages.iter().map(|stage| (stage.uid.as_str(), stage)).collect::<HashMap<_, _>>();
    for stage in &current.stages {
        let entity_id = Some(stage_number(current, &stage.uid));
        let Some(before) = before_by_uid.get(stage.uid.as_str()).copied() else {
            changes.push(AuditChange { entity: "Этап".into(), entity_id, field: "Создание".into(), before: "—".into(), after: shown(&stage.title) });
            continue;
        };
        for (field, old, new) in [
            ("Название", shown(&before.title), shown(&stage.title)),
            ("Родитель", parent_label(previous, before.parent_uid.as_deref()), parent_label(current, stage.parent_uid.as_deref())),
            ("Исполнитель", shown(&before.executor), shown(&stage.executor)),
            ("Адресат", shown(&before.addressees), shown(&stage.addressees)),
            ("Начало", shown(&before.start), shown(&stage.start)),
            ("Дедлайн", shown(&before.deadline), shown(&stage.deadline)),
            ("Статус", status_label(before.status).into(), status_label(stage.status).into()),
            ("Комментарий", shown(&before.comment), shown(&stage.comment)),
            ("Порядок", before.sort.to_string(), stage.sort.to_string()),
        ] {
            add_change(&mut changes, "Этап", entity_id.clone(), field, old, new);
        }
    }
    for stage in &previous.stages {
        if !after_by_uid.contains_key(stage.uid.as_str()) {
            changes.push(AuditChange {
                entity: "Этап".into(),
                entity_id: Some(stage_number(previous, &stage.uid)),
                field: "Удаление".into(),
                before: shown(&stage.title),
                after: "Удалён".into(),
            });
        }
    }
    changes
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn fixed<const N: usize>(bytes: Vec<u8>, label: &str) -> std::result::Result<[u8; N], String> {
    bytes
        .try_into()
        .map_err(|_| format!("Повреждён размер поля «{label}»."))
}

fn derive_key(pin: &str, salt: &[u8]) -> std::result::Result<[u8; 32], String> {
    if pin.chars().count() < 6 {
        return Err("PIN должен содержать не менее 6 символов.".into());
    }
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(pin.as_bytes(), salt, &mut key)
        .map_err(|_| "Не удалось сформировать ключ защиты PIN.".to_string())?;
    Ok(key)
}

fn encrypt_signing_key(
    signing_key: &SigningKey,
    pin: &str,
) -> std::result::Result<(String, String, String), String> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let mut key = derive_key(pin, &salt)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| "Не удалось создать шифр защиты ключа.".to_string())?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), signing_key.to_bytes().as_ref())
        .map_err(|_| "Не удалось зашифровать закрытый ключ.".to_string())?;
    key.fill(0);
    Ok((B64.encode(encrypted), B64.encode(salt), B64.encode(nonce)))
}

fn decrypt_signing_key(user: &AuditUserSecret, pin: &str) -> std::result::Result<SigningKey, String> {
    let salt = B64
        .decode(&user.kdf_salt)
        .map_err(|_| "Повреждена соль ключа пользователя.".to_string())?;
    let nonce = fixed::<12>(
        B64.decode(&user.encryption_nonce)
            .map_err(|_| "Повреждён nonce ключа пользователя.".to_string())?,
        "nonce",
    )?;
    let encrypted = B64
        .decode(&user.encrypted_private_key)
        .map_err(|_| "Повреждён закрытый ключ пользователя.".to_string())?;
    let mut key = derive_key(pin, &salt)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| "Не удалось создать шифр проверки PIN.".to_string())?;
    let decrypted = cipher
        .decrypt(Nonce::from_slice(&nonce), encrypted.as_ref())
        .map_err(|_| "Неверный PIN или повреждён закрытый ключ.".to_string())?;
    key.fill(0);
    let secret = fixed::<32>(decrypted, "закрытый ключ")?;
    let signing_key = SigningKey::from_bytes(&secret);
    if B64.encode(signing_key.verifying_key().to_bytes()) != user.public_key {
        return Err("Закрытый и открытый ключ пользователя не совпадают.".into());
    }
    Ok(signing_key)
}

fn load_user(conn: &Connection, user_id: i64) -> std::result::Result<AuditUserSecret, String> {
    conn.query_row(
        "SELECT id,display_name,role,public_key,encrypted_private_key,kdf_salt,encryption_nonce,key_fingerprint FROM audit_user WHERE id=?1 AND active=1",
        [user_id],
        |row| {
            Ok(AuditUserSecret {
                id: row.get(0)?,
                display_name: row.get(1)?,
                role: row.get(2)?,
                public_key: row.get(3)?,
                encrypted_private_key: row.get(4)?,
                kdf_salt: row.get(5)?,
                encryption_nonce: row.get(6)?,
                key_fingerprint: row.get(7)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Активный пользователь подписи не найден.".to_string())
}

fn authenticate(
    conn: &Connection,
    user_id: i64,
    pin: &str,
    require_admin: bool,
) -> std::result::Result<(AuditUserSecret, SigningKey), String> {
    let user = load_user(conn, user_id)?;
    if require_admin && user.role != "admin" {
        return Err("Для этой операции требуется профиль администратора.".into());
    }
    let signing_key = decrypt_signing_key(&user, pin)?;
    Ok((user, signing_key))
}

pub(crate) fn authenticate_admin(conn: &Connection, user_id: i64, pin: &str) -> std::result::Result<(), String> {
    authenticate(conn, user_id, pin, true).map(|_| ())
}

fn snapshot_hash(snapshot: &ProductionSnapshot) -> std::result::Result<String, String> {
    serde_json::to_vec(snapshot)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|e| e.to_string())
}

fn append_event(
    tx: &Transaction<'_>,
    user: &AuditUserSecret,
    signing_key: &SigningKey,
    project_id: i64,
    stage_uid: Option<&str>,
    stage_reg_number: Option<&str>,
    event_type: &str,
    comment: &str,
    evidence_type: Option<&str>,
    evidence_reference: Option<&str>,
    changes: &[AuditChange],
    snapshot_hash: &str,
) -> std::result::Result<String, String> {
    let previous_hash: String = tx
        .query_row(
            "SELECT event_hash FROM audit_event WHERE project_id=?1 ORDER BY seq DESC LIMIT 1",
            [project_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let event_id = Uuid::new_v4().to_string();
    let created_at = Local::now().to_rfc3339();
    let payload = AuditPayload {
        event_id: &event_id,
        project_id,
        stage_uid,
        stage_reg_number,
        event_type,
        actor_user_id: user.id,
        actor_name: &user.display_name,
        key_fingerprint: &user.key_fingerprint,
        comment,
        evidence_type,
        evidence_reference,
        changes,
        snapshot_hash,
        previous_hash: &previous_hash,
        created_at: &created_at,
    };
    let payload_json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let mut digest_input = previous_hash.as_bytes().to_vec();
    digest_input.extend_from_slice(payload_json.as_bytes());
    let event_hash = sha256_hex(&digest_input);
    let signature = signing_key.sign(event_hash.as_bytes());
    let changes_json = serde_json::to_string(changes).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO audit_event(event_id,project_id,stage_uid,stage_reg_number,event_type,actor_user_id,actor_name,key_fingerprint,comment,evidence_type,evidence_reference,changes_json,snapshot_hash,previous_hash,event_hash,payload_json,signature,public_key,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![event_id,project_id,stage_uid,stage_reg_number,event_type,user.id,user.display_name,user.key_fingerprint,comment,evidence_type,evidence_reference,changes_json,snapshot_hash,previous_hash,event_hash,payload_json,B64.encode(signature.to_bytes()),user.public_key,created_at],
    ).map_err(|e| e.to_string())?;
    Ok(event_id)
}

impl ProductionDb {
    pub fn list_audit_users(&self) -> std::result::Result<Vec<AuditUserSummary>, String> {
        let conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        let mut stmt = conn
            .prepare("SELECT id,display_name,role,key_fingerprint,active,created_at FROM audit_user ORDER BY active DESC,display_name")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(AuditUserSummary {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    role: row.get(2)?,
                    key_fingerprint: row.get(3)?,
                    active: row.get::<_, i64>(4)? == 1,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn create_audit_user(
        &self,
        input: CreateAuditUserInput,
    ) -> std::result::Result<AuditUserSummary, String> {
        let display_name = clean(&input.display_name);
        if display_name.is_empty() {
            return Err("ФИО подтверждающего обязательно.".into());
        }
        if display_name.chars().count() > 160 {
            return Err("ФИО не должно превышать 160 символов.".into());
        }
        if input.pin.chars().count() < 6 {
            return Err("PIN должен содержать не менее 6 символов.".into());
        }
        let mut conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_user", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if count > 0 {
            let admin_id = input
                .admin_user_id
                .ok_or_else(|| "Укажите администратора, создающего профиль.".to_string())?;
            let admin_pin = input
                .admin_pin
                .as_deref()
                .ok_or_else(|| "Введите PIN администратора.".to_string())?;
            authenticate(&conn, admin_id, admin_pin, true)?;
        }
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let public_key = signing_key.verifying_key().to_bytes();
        let fingerprint = sha256_hex(&public_key)[..16].to_uppercase();
        let (encrypted, salt, nonce) = encrypt_signing_key(&signing_key, &input.pin)?;
        let role = if count == 0 || input.is_admin { "admin" } else { "signer" };
        let now = Local::now().to_rfc3339();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO audit_user(display_name,role,public_key,encrypted_private_key,kdf_salt,encryption_nonce,key_fingerprint,active,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,1,?8)",
            params![display_name,role,B64.encode(public_key),encrypted,salt,nonce,fingerprint,now],
        ).map_err(|e| if e.to_string().contains("UNIQUE") {"Пользователь с таким ФИО уже существует.".to_string()} else {e.to_string()})?;
        let id = tx.last_insert_rowid();
        tx.commit().map_err(|e| e.to_string())?;
        Ok(AuditUserSummary { id, display_name, role: role.into(), key_fingerprint: fingerprint, active: true, created_at: now })
    }

    pub fn save_signed_snapshot(
        &self,
        snapshot: &ProductionSnapshot,
        user_id: i64,
        pin: &str,
        comment: &str,
        evidence: Option<CompletionEvidenceInput>,
    ) -> std::result::Result<SignedSaveResult, String> {
        let validation = validate_snapshot(snapshot).map_err(|e| e.to_string())?;
        if !validation.valid {
            return Err("Проект не прошёл проверку.".into());
        }
        let comment = validate_comment(comment, "Комментарий к изменению")?;
        let mut conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        let (user, signing_key) = authenticate(&conn, user_id, pin, false)?;
        let previous = match snapshot.database_id {
            Some(project_id) => load_snapshot_by_id_conn(&conn, project_id).map_err(|e| e.to_string())?,
            None => None,
        };
        let old_statuses = previous.as_ref().map(|value| value.stages.iter()
            .map(|stage| (stage.uid.clone(), stage.status.as_str().to_string())).collect::<HashMap<_, _>>())
            .unwrap_or_default();
        let changes = snapshot_changes(previous.as_ref(), snapshot);
        let completed: Vec<_> = snapshot
            .stages
            .iter()
            .filter(|stage| {
                stage.status == StageStatus::Done
                    && old_statuses.get(&stage.uid).map(String::as_str) != Some("done")
            })
            .collect();
        let completed_uids = completed.iter().map(|stage| stage.uid.as_str()).collect::<std::collections::HashSet<_>>();
        let evidence = if completed.is_empty() {
            if evidence.as_ref().is_some_and(|value| !value.stage_uids.is_empty()) {
                return Err("Указаны этапы для завершения, но их статус не изменён на «Выполнено».".into());
            }
            None
        } else {
            let value = evidence.ok_or_else(|| {
                "Для перевода этапа в «Выполнено» укажите официальный документ и комментарий."
                    .to_string()
            })?;
            let requested_uids = value.stage_uids.iter().map(String::as_str).collect::<std::collections::HashSet<_>>();
            if requested_uids.len() != value.stage_uids.len() || requested_uids != completed_uids {
                return Err("Состав подтверждаемых этапов не совпадает с фактическими изменениями. Закройте один этап либо явно выберите закрытие всей родительской ветки.".into());
            }
            let document_type = clean(&value.document_type);
            let document_reference = validate_comment(&value.document_reference, "Номер и дата документа")?;
            let evidence_comment = validate_comment(&value.comment, "Основание выполнения")?;
            if !OFFICIAL_EVIDENCE_TYPES.contains(&document_type.as_str()) {
                return Err("Выберите допустимый тип официального документа.".into());
            }
            Some((document_type, document_reference, evidence_comment))
        };
        let is_new = snapshot.database_id.is_none();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let project_id = save_snapshot_tx(&tx, snapshot).map_err(|e| e.to_string())?;
        let mut persisted = snapshot.clone();
        persisted.database_id = Some(project_id);
        let hash = snapshot_hash(&persisted)?;
        let change_event_id = append_event(
            &tx,
            &user,
            &signing_key,
            project_id,
            None,
            None,
            if is_new { "project_created" } else { "project_changed" },
            &comment,
            None,
            None,
            &changes,
            &hash,
        )?;
        let mut completion_event_ids = Vec::new();
        if let Some((document_type, document_reference, evidence_comment)) = evidence {
            for stage in completed {
                let reg = format!("{}-{:03}", clean(&snapshot.project.order_no), stage.seq);
                completion_event_ids.push(append_event(
                    &tx,
                    &user,
                    &signing_key,
                    project_id,
                    Some(&stage.uid),
                    Some(&reg),
                    "stage_completed",
                    &evidence_comment,
                    Some(&document_type),
                    Some(&document_reference),
                    &[],
                    &hash,
                )?);
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        drop(conn);
        let (backup, backup_warning) = match self.create_backup("auto-save") {
            Ok(value) => (Some(value), None),
            Err(error) => (None, Some(format!("Проект сохранён, но резервная копия не создана: {error}"))),
        };
        Ok(SignedSaveResult { project_id, change_event_id, completion_event_ids, backup, backup_warning })
    }

    pub fn list_audit_events(
        &self,
        project_id: Option<i64>,
    ) -> std::result::Result<Vec<AuditEventSummary>, String> {
        let conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        let sql = "SELECT seq,event_id,project_id,stage_uid,stage_reg_number,event_type,actor_name,key_fingerprint,comment,evidence_type,evidence_reference,changes_json,created_at FROM audit_event WHERE (?1 IS NULL OR project_id=?1) ORDER BY seq DESC";
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![project_id], |row| {
                Ok(AuditEventSummary {
                    seq: row.get(0)?,
                    event_id: row.get(1)?,
                    project_id: row.get(2)?,
                    stage_uid: row.get(3)?,
                    stage_reg_number: row.get(4)?,
                    event_type: row.get(5)?,
                    actor_name: row.get(6)?,
                    key_fingerprint: row.get(7)?,
                    comment: row.get(8)?,
                    evidence_type: row.get(9)?,
                    evidence_reference: row.get(10)?,
                    changes: serde_json::from_str(&row.get::<_, String>(11)?).unwrap_or_default(),
                    created_at: row.get(12)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn verify_audit_log(
        &self,
        admin_user_id: i64,
        admin_pin: &str,
        project_id: Option<i64>,
    ) -> std::result::Result<AuditVerificationReport, String> {
        let conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        authenticate(&conn, admin_user_id, admin_pin, true)?;
        let mut stmt = conn.prepare("SELECT seq,project_id,event_id,stage_uid,stage_reg_number,event_type,actor_user_id,actor_name,key_fingerprint,comment,evidence_type,evidence_reference,changes_json,snapshot_hash,previous_hash,event_hash,payload_json,signature,public_key,created_at FROM audit_event WHERE (?1 IS NULL OR project_id=?1) ORDER BY project_id,seq").map_err(|e| e.to_string())?;
        let events = stmt.query_map(params![project_id], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<String>>(3)?,row.get::<_,Option<String>>(4)?,row.get::<_,String>(5)?,row.get::<_,i64>(6)?,row.get::<_,String>(7)?,row.get::<_,String>(8)?,row.get::<_,String>(9)?,row.get::<_,Option<String>>(10)?,row.get::<_,Option<String>>(11)?,row.get::<_,String>(12)?,row.get::<_,String>(13)?,row.get::<_,String>(14)?,row.get::<_,String>(15)?,row.get::<_,String>(16)?,row.get::<_,String>(17)?,row.get::<_,String>(18)?,row.get::<_,String>(19)?))).map_err(|e| e.to_string())?.collect::<std::result::Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
        let mut previous_by_project = HashMap::<i64, String>::new();
        let mut snapshot_by_project = HashMap::<i64, String>::new();
        let mut valid_events = 0usize;
        let mut errors = Vec::new();
        for (seq,pid,event_id,stage_uid,stage_reg_number,event_type,actor_user_id,actor_name,key_fingerprint,comment,evidence_type,evidence_reference,changes_json,snapshot_hash,previous_hash,event_hash,payload_json,signature,public_key,created_at) in &events {
            let expected_previous = previous_by_project.get(pid).cloned().unwrap_or_default();
            let mut event_errors = Vec::new();
            if *previous_hash != expected_previous { event_errors.push("нарушена цепочка предыдущих хешей".to_string()); }
            let mut digest_input = previous_hash.as_bytes().to_vec();
            digest_input.extend_from_slice(payload_json.as_bytes());
            if sha256_hex(&digest_input) != *event_hash { event_errors.push("контрольная сумма события не совпадает".to_string()); }
            if verify_signature(event_hash, signature, public_key).is_err() { event_errors.push("криптографическая подпись недействительна".to_string()); }
            let stored_changes = serde_json::from_str::<Vec<AuditChange>>(changes_json).ok();
            match serde_json::from_str::<StoredAuditPayload>(payload_json) {
                Ok(payload) if payload.event_id==event_id.as_str()&&payload.project_id==*pid&&payload.stage_uid.as_deref()==stage_uid.as_deref()&&payload.stage_reg_number.as_deref()==stage_reg_number.as_deref()&&payload.event_type==event_type.as_str()&&payload.actor_user_id==*actor_user_id&&payload.actor_name==actor_name.as_str()&&payload.key_fingerprint==key_fingerprint.as_str()&&payload.comment==comment.as_str()&&payload.evidence_type.as_deref()==evidence_type.as_deref()&&payload.evidence_reference.as_deref()==evidence_reference.as_deref()&&stored_changes.as_ref()==Some(&payload.changes)&&payload.snapshot_hash==snapshot_hash.as_str()&&payload.previous_hash==previous_hash.as_str()&&payload.created_at==created_at.as_str() => {},
                Ok(_) => event_errors.push("поля журнала не совпадают с подписанными данными".to_string()),
                Err(_) => event_errors.push("подписанные данные события повреждены".to_string()),
            }
            if event_errors.is_empty() { valid_events += 1; } else { errors.push(format!("Событие #{seq} {event_id}: {}.", event_errors.join(", "))); }
            previous_by_project.insert(*pid, event_hash.clone());
            snapshot_by_project.insert(*pid, snapshot_hash.clone());
        }
        for (pid, expected_snapshot_hash) in snapshot_by_project {
            match load_snapshot_by_id_conn(&conn,pid).map_err(|e|e.to_string())? {
                Some(snapshot) if snapshot_hash(&snapshot)?==expected_snapshot_hash => {},
                Some(_) => errors.push(format!("Проект {pid}: текущее содержимое базы не совпадает с последним подписанным состоянием.")),
                None => errors.push(format!("Проект {pid}: подписанный проект удалён из базы данных.")),
            }
        }
        let chain_heads = previous_by_project.into_iter().map(|(pid,hash)|format!("Проект {pid}: {hash}")).collect::<Vec<_>>();
        let invalid_events=events.len()-valid_events+errors.iter().filter(|value|value.starts_with("Проект ")).count();
        Ok(AuditVerificationReport { checked_events: events.len(), valid_events, invalid_events, chain_heads, errors, verified_at: Local::now().to_rfc3339() })
    }

    pub fn replace_dictionary_value_signed(
        &self,
        kind: DictionaryKind,
        from_value: &str,
        to_value: &str,
        user_id: i64,
        pin: &str,
        comment: &str,
    ) -> std::result::Result<DictionaryReplaceResult, String> {
        let from = clean(from_value);
        let to = clean(to_value);
        let comment = validate_comment(comment, "Комментарий к изменению справочника")?;
        if from.is_empty() || to.is_empty() {
            return Err("Старое и новое наименование должны быть заполнены.".into());
        }
        if from == to {
            return Err("Новое наименование совпадает с текущим.".into());
        }
        if to.chars().any(|value| matches!(value, '\t' | '\r' | '\n')) {
            return Err("Новое наименование не должно содержать TAB или перенос строки.".into());
        }
        let mut conn = self.conn.lock().map_err(|_| "База данных занята".to_string())?;
        let (user, signing_key) = authenticate(&conn, user_id, pin, false)?;
        let project_query = match kind {
            DictionaryKind::Enterprise => "SELECT id FROM production_cycle WHERE enterprise=?1 UNION SELECT cycle_id FROM production_stage WHERE addressees=?1",
            DictionaryKind::Executor => "SELECT id FROM production_cycle WHERE executor=?1 UNION SELECT cycle_id FROM production_stage WHERE executor=?1",
            DictionaryKind::Initiator => "SELECT id FROM production_cycle WHERE initiator=?1",
            DictionaryKind::ProjectName => "SELECT id FROM production_cycle WHERE name=?1",
            DictionaryKind::StageTitle => "SELECT cycle_id FROM production_stage WHERE title=?1",
        };
        let project_ids = {
            let mut stmt = conn.prepare(project_query).map_err(|e| e.to_string())?;
            let values = stmt.query_map([&from], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            values
        };
        if project_ids.is_empty() {
            return Err("Совпадающие записи справочника не найдены.".into());
        }
        let now = Local::now().to_rfc3339();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let (project_sql, stage_sql, touch_sql) = match kind {
            DictionaryKind::Enterprise => (Some("UPDATE production_cycle SET enterprise=?1,updated_at=?3 WHERE enterprise=?2"), Some("UPDATE production_stage SET addressees=?1,updated_at=?3 WHERE addressees=?2"), Some("UPDATE production_cycle SET updated_at=?1 WHERE id IN (SELECT DISTINCT cycle_id FROM production_stage WHERE addressees=?2)")),
            DictionaryKind::Executor => (Some("UPDATE production_cycle SET executor=?1,updated_at=?3 WHERE executor=?2"), Some("UPDATE production_stage SET executor=?1,updated_at=?3 WHERE executor=?2"), Some("UPDATE production_cycle SET updated_at=?1 WHERE id IN (SELECT DISTINCT cycle_id FROM production_stage WHERE executor=?2)")),
            DictionaryKind::Initiator => (Some("UPDATE production_cycle SET initiator=?1,updated_at=?3 WHERE initiator=?2"), None, None),
            DictionaryKind::ProjectName => (Some("UPDATE production_cycle SET name=?1,updated_at=?3 WHERE name=?2"), None, None),
            DictionaryKind::StageTitle => (None, Some("UPDATE production_stage SET title=?1,updated_at=?3 WHERE title=?2"), Some("UPDATE production_cycle SET updated_at=?1 WHERE id IN (SELECT DISTINCT cycle_id FROM production_stage WHERE title=?2)")),
        };
        if let Some(sql) = touch_sql { tx.execute(sql, params![now, from]).map_err(|e| e.to_string())?; }
        let affected_stage_rows = if let Some(sql) = stage_sql { tx.execute(sql, params![to, from, now]).map_err(|e| e.to_string())? } else { 0 };
        let affected_project_rows = if let Some(sql) = project_sql { tx.execute(sql, params![to, from, now]).map_err(|e| e.to_string())? } else { 0 };
        let kind_name = match kind { DictionaryKind::Enterprise=>"предприятие/адресат",DictionaryKind::Executor=>"исполнитель",DictionaryKind::Initiator=>"инициатор",DictionaryKind::ProjectName=>"название проекта",DictionaryKind::StageTitle=>"название этапа" };
        let dictionary_change = vec![AuditChange { entity:"Справочник".into(),entity_id:None,field:kind_name.into(),before:from.clone(),after:to.clone() }];
        for project_id in project_ids {
            let snapshot = load_snapshot_by_id_conn(&tx, project_id).map_err(|e| e.to_string())?.ok_or_else(|| "Изменённый проект не найден.".to_string())?;
            let hash = snapshot_hash(&snapshot)?;
            append_event(&tx,&user,&signing_key,project_id,None,None,"dictionary_replaced",&format!("{comment} [{kind_name}: «{from}» → «{to}»]"),None,None,&dictionary_change,&hash)?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(DictionaryReplaceResult { affected_project_rows, affected_stage_rows })
    }
}

fn verify_signature(event_hash: &str, signature: &str, public_key: &str) -> std::result::Result<(), String> {
    let key_bytes = fixed::<32>(B64.decode(public_key).map_err(|_| "Открытый ключ повреждён.".to_string())?, "открытый ключ")?;
    let signature_bytes = B64.decode(signature).map_err(|_| "Подпись повреждена.".to_string())?;
    let verifying_key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| "Открытый ключ недействителен.".to_string())?;
    let signature = Signature::from_slice(&signature_bytes).map_err(|_| "Подпись имеет неверный размер.".to_string())?;
    verifying_key.verify(event_hash.as_bytes(), &signature).map_err(|_| "Подпись не прошла проверку.".to_string())
}

pub fn attach_completion_confirmations(
    db: &ProductionDb,
    project_id: i64,
    report: &mut ProductionReport,
) -> std::result::Result<(), String> {
    let conn = db.conn.lock().map_err(|_| "База данных занята".to_string())?;
    let mut stmt = conn.prepare("SELECT payload_json,previous_hash,event_hash,signature,public_key FROM audit_event WHERE project_id=?1 AND event_type='stage_completed' ORDER BY seq DESC").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([project_id], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?))).map_err(|e| e.to_string())?.collect::<std::result::Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    let mut latest = HashMap::new();
    for (payload_json,previous_hash,event_hash,signature,public_key) in rows {
        let Ok(payload)=serde_json::from_str::<StoredAuditPayload>(&payload_json) else { continue };
        let Some(uid)=payload.stage_uid.clone() else { continue };
        let mut digest_input=previous_hash.as_bytes().to_vec();digest_input.extend_from_slice(payload_json.as_bytes());
        let signature_valid=sha256_hex(&digest_input)==event_hash&&verify_signature(&event_hash,&signature,&public_key).is_ok();
        latest.entry(uid).or_insert_with(|| CompletionConfirmation { confirmer:payload.actor_name,key_fingerprint:payload.key_fingerprint,created_at:payload.created_at,document_type:payload.evidence_type.unwrap_or_default(),document_reference:payload.evidence_reference.unwrap_or_default(),comment:payload.comment,signature_valid });
    }
    for row in &mut report.stages { row.completion_confirmation = latest.remove(&row.stage.uid); }
    Ok(())
}

#[tauri::command]
pub fn audit_list_users(db: State<'_, ProductionDb>) -> std::result::Result<Vec<AuditUserSummary>, String> { db.list_audit_users() }

#[tauri::command]
pub fn audit_create_user(input: CreateAuditUserInput, db: State<'_, ProductionDb>) -> std::result::Result<AuditUserSummary, String> { db.create_audit_user(input) }

#[tauri::command]
pub fn audit_save_snapshot(snapshot: ProductionSnapshot, user_id: i64, pin: String, comment: String, evidence: Option<CompletionEvidenceInput>, db: State<'_, ProductionDb>) -> std::result::Result<SignedSaveResult, String> { db.save_signed_snapshot(&snapshot,user_id,&pin,&comment,evidence) }

#[tauri::command]
pub fn audit_list_events(project_id: Option<i64>, db: State<'_, ProductionDb>) -> std::result::Result<Vec<AuditEventSummary>, String> { db.list_audit_events(project_id) }

#[tauri::command]
pub fn audit_verify_log(admin_user_id: i64, admin_pin: String, project_id: Option<i64>, db: State<'_, ProductionDb>) -> std::result::Result<AuditVerificationReport, String> { db.verify_audit_log(admin_user_id,&admin_pin,project_id) }

#[tauri::command]
pub fn audit_replace_dictionary_value(kind: DictionaryKind, from_value: String, to_value: String, user_id: i64, pin: String, comment: String, db: State<'_, ProductionDb>) -> std::result::Result<DictionaryReplaceResult, String> { db.replace_dictionary_value_signed(kind,&from_value,&to_value,user_id,&pin,&comment) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::{ProductionProjectInput, ProductionStageInput};

    fn sample(status: StageStatus) -> ProductionSnapshot {
        ProductionSnapshot {
            database_id: None,
            project: ProductionProjectInput { name:"Подписываемый проект".into(),order_no:"AUD-1".into(),initiator:"Инициатор".into(),executor:"Исполнитель".into(),enterprise:"Предприятие".into(),start:"2026-09-01".into(),deadline:"2026-09-30".into() },
            stages: vec![ProductionStageInput { uid:"stage-1".into(),seq:1,sort:1,parent_uid:None,title:"Приёмка".into(),executor:"Исполнитель".into(),addressees:"Предприятие".into(),start:"2026-09-01".into(),deadline:"2026-09-20".into(),status,comment:String::new() }],
            next_seq: 2,
        }
    }

    fn admin(db: &ProductionDb) -> AuditUserSummary {
        db.create_audit_user(CreateAuditUserInput { display_name:"Владелец".into(),pin:"739201".into(),is_admin:false,admin_user_id:None,admin_pin:None }).unwrap()
    }

    #[test]
    fn completion_requires_evidence_and_creates_verifiable_signature() {
        let db=ProductionDb::memory().unwrap();let owner=admin(&db);
        let mut initial=sample(StageStatus::Work);
        let created=db.save_signed_snapshot(&initial,owner.id,"739201","Создание",None).unwrap();
        initial.database_id=Some(created.project_id);initial.stages[0].status=StageStatus::Done;
        assert!(db.save_signed_snapshot(&initial,owner.id,"739201","Завершение",None).is_err());
        let saved=db.save_signed_snapshot(&initial,owner.id,"739201","Завершение",Some(CompletionEvidenceInput{document_type:"Акт приёмки".into(),document_reference:"№15 от 18.09.2026".into(),comment:"Работы приняты".into(),stage_uids:vec!["stage-1".into()]})).unwrap();
        assert_eq!(saved.completion_event_ids.len(),1);
        let report=db.verify_audit_log(owner.id,"739201",Some(saved.project_id)).unwrap();
        assert_eq!(report.invalid_events,0);assert_eq!(report.checked_events,3);
        let mut printable=crate::production::build_report(&initial,chrono::NaiveDate::from_ymd_opt(2026,9,18).unwrap()).unwrap();
        attach_completion_confirmations(&db,saved.project_id,&mut printable).unwrap();
        assert!(printable.stages[0].completion_confirmation.as_ref().unwrap().signature_valid);
    }

    #[test]
    fn completion_scope_must_match_exactly_and_branch_requires_explicit_targets() {
        let db=ProductionDb::memory().unwrap();let owner=admin(&db);
        let mut snapshot=sample(StageStatus::Work);
        snapshot.stages.push(ProductionStageInput { uid:"stage-2".into(),seq:2,sort:1,parent_uid:Some("stage-1".into()),title:"Дочерний этап".into(),executor:"Исполнитель".into(),addressees:"Предприятие".into(),start:"2026-09-02".into(),deadline:"2026-09-19".into(),status:StageStatus::Work,comment:String::new() });
        snapshot.next_seq=3;
        let created=db.save_signed_snapshot(&snapshot,owner.id,"739201","Создание",None).unwrap();
        snapshot.database_id=Some(created.project_id);
        snapshot.stages.iter_mut().for_each(|stage| stage.status=StageStatus::Done);
        let one_stage=CompletionEvidenceInput { document_type:"Акт приёмки".into(),document_reference:"№16".into(),comment:"Подтверждён один этап".into(),stage_uids:vec!["stage-1".into()] };
        let error=db.save_signed_snapshot(&snapshot,owner.id,"739201","Завершение",Some(one_stage)).unwrap_err();
        assert!(error.contains("Состав подтверждаемых этапов"));
        let branch=CompletionEvidenceInput { document_type:"Акт приёмки".into(),document_reference:"№17".into(),comment:"Подтверждена вся ветка".into(),stage_uids:vec!["stage-1".into(),"stage-2".into()] };
        let saved=db.save_signed_snapshot(&snapshot,owner.id,"739201","Завершение ветки",Some(branch)).unwrap();
        assert_eq!(saved.completion_event_ids.len(),2);
    }

    #[test]
    fn wrong_pin_empty_comment_and_tampering_are_detected() {
        let db=ProductionDb::memory().unwrap();let owner=admin(&db);let snapshot=sample(StageStatus::Work);
        assert!(db.save_signed_snapshot(&snapshot,owner.id,"bad-pin","Создание",None).is_err());
        assert!(db.save_signed_snapshot(&snapshot,owner.id,"739201","",None).is_err());
        let saved=db.save_signed_snapshot(&snapshot,owner.id,"739201","Создание",None).unwrap();
        { let conn=db.conn.lock().unwrap();conn.execute("UPDATE audit_event SET comment='Подмена' WHERE event_id=?1",[saved.change_event_id]).unwrap(); }
        let report=db.verify_audit_log(owner.id,"739201",Some(saved.project_id)).unwrap();
        assert_eq!(report.invalid_events,1);assert!(report.errors[0].contains("не совпадают"));
    }

    #[test]
    fn signed_change_event_contains_automatic_before_after_values() {
        let db=ProductionDb::memory().unwrap();let owner=admin(&db);
        let mut snapshot=sample(StageStatus::Work);
        let created=db.save_signed_snapshot(&snapshot,owner.id,"739201","Создание",None).unwrap();
        snapshot.database_id=Some(created.project_id);
        snapshot.project.deadline="2026-10-15".into();
        snapshot.stages[0].executor="Новый исполнитель".into();
        snapshot.stages[0].comment="Получено письмо".into();
        db.save_signed_snapshot(&snapshot,owner.id,"739201","Уточнение графика",None).unwrap();
        let events=db.list_audit_events(Some(created.project_id)).unwrap();
        let changed=&events[0];
        assert!(changed.changes.iter().any(|change|change.entity=="Проект"&&change.field=="Дедлайн"&&change.before=="2026-09-30"&&change.after=="2026-10-15"));
        assert!(changed.changes.iter().any(|change|change.entity=="Этап"&&change.field=="Исполнитель"&&change.before=="Исполнитель"&&change.after=="Новый исполнитель"));
        assert!(changed.changes.iter().any(|change|change.entity=="Этап"&&change.field=="Комментарий"&&change.before=="—"&&change.after=="Получено письмо"));
        let report=db.verify_audit_log(owner.id,"739201",Some(created.project_id)).unwrap();
        assert_eq!(report.invalid_events,0);
    }
}
