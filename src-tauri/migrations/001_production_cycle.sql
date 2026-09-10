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
