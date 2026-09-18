CREATE TABLE IF NOT EXISTS production_cycle (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    order_no        TEXT NOT NULL,
    root_reg_number TEXT NOT NULL,
    name            TEXT NOT NULL,
    initiator       TEXT NOT NULL,
    executor        TEXT NOT NULL,
    enterprise      TEXT NOT NULL,
    start_date      TEXT NOT NULL,
    deadline        TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS production_stage (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    cycle_id        INTEGER NOT NULL,
    uid             TEXT NOT NULL,
    seq             INTEGER NOT NULL,
    reg_number      TEXT NOT NULL,
    parent_stage_id INTEGER NULL,
    sort_order      INTEGER NOT NULL,
    title           TEXT NOT NULL,
    executor        TEXT NOT NULL,
    addressees      TEXT NOT NULL,
    start_date      TEXT NOT NULL,
    deadline        TEXT NOT NULL,
    status          TEXT NOT NULL CHECK(status IN ('new','work','hold','done')),
    comment         TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    FOREIGN KEY(cycle_id) REFERENCES production_cycle(id) ON DELETE CASCADE,
    FOREIGN KEY(parent_stage_id) REFERENCES production_stage(id) ON DELETE CASCADE,
    UNIQUE(cycle_id, uid),
    UNIQUE(cycle_id, seq),
    UNIQUE(cycle_id, reg_number)
);
CREATE INDEX IF NOT EXISTS idx_production_stage_cycle ON production_stage(cycle_id);
CREATE INDEX IF NOT EXISTS idx_production_stage_parent ON production_stage(parent_stage_id);
CREATE INDEX IF NOT EXISTS idx_production_stage_cycle_sort ON production_stage(cycle_id,parent_stage_id,sort_order);
CREATE INDEX IF NOT EXISTS idx_production_cycle_order ON production_cycle(order_no);

CREATE TABLE IF NOT EXISTS audit_user (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name          TEXT NOT NULL UNIQUE,
    role                  TEXT NOT NULL CHECK(role IN ('admin','signer')),
    public_key            TEXT NOT NULL,
    encrypted_private_key TEXT NOT NULL,
    kdf_salt              TEXT NOT NULL,
    encryption_nonce      TEXT NOT NULL,
    key_fingerprint       TEXT NOT NULL UNIQUE,
    active                INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)),
    created_at            TEXT NOT NULL,
    revoked_at            TEXT NULL
);

CREATE TABLE IF NOT EXISTS audit_event (
    seq                INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id           TEXT NOT NULL UNIQUE,
    project_id         INTEGER NOT NULL,
    stage_uid          TEXT NULL,
    stage_reg_number   TEXT NULL,
    event_type         TEXT NOT NULL,
    actor_user_id      INTEGER NOT NULL,
    actor_name         TEXT NOT NULL,
    key_fingerprint    TEXT NOT NULL,
    comment            TEXT NOT NULL,
    evidence_type      TEXT NULL,
    evidence_reference TEXT NULL,
    snapshot_hash      TEXT NOT NULL,
    previous_hash      TEXT NOT NULL,
    event_hash         TEXT NOT NULL UNIQUE,
    payload_json       TEXT NOT NULL,
    signature          TEXT NOT NULL,
    public_key         TEXT NOT NULL,
    created_at         TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES production_cycle(id) ON DELETE RESTRICT,
    FOREIGN KEY(actor_user_id) REFERENCES audit_user(id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_audit_event_project_seq ON audit_event(project_id,seq);
CREATE INDEX IF NOT EXISTS idx_audit_event_stage ON audit_event(project_id,stage_uid,event_type,seq);
