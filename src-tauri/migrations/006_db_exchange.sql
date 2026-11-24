CREATE TABLE IF NOT EXISTS exchange_store(singleton INTEGER PRIMARY KEY CHECK(singleton=1),uid TEXT NOT NULL,generation TEXT NOT NULL);
INSERT OR IGNORE INTO exchange_store VALUES(1,lower(hex(randomblob(16))),lower(hex(randomblob(16))));
CREATE TABLE IF NOT EXISTS exchange_project(project_id INTEGER PRIMARY KEY REFERENCES production_cycle(id) ON DELETE CASCADE, guid TEXT NOT NULL UNIQUE,authority TEXT, generation TEXT, baseline TEXT);
CREATE TRIGGER IF NOT EXISTS exchange_new_project AFTER INSERT ON production_cycle BEGIN
 INSERT INTO exchange_project(project_id,guid) VALUES (new.id,lower(hex(randomblob(16))));
END;
INSERT OR IGNORE INTO exchange_project(project_id,guid) SELECT id,lower(hex(randomblob(16))) FROM production_cycle;
CREATE TABLE IF NOT EXISTS exchange_outbox(seq INTEGER PRIMARY KEY,project_id INTEGER NOT NULL REFERENCES production_cycle(id) ON DELETE CASCADE,op TEXT UNIQUE NOT NULL,body TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS exchange_inbox(op TEXT PRIMARY KEY, digest TEXT NOT NULL, project_id INTEGER NOT NULL REFERENCES production_cycle(id), body TEXT NOT NULL, state TEXT NOT NULL,reason TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS exchange_keys(pubkey TEXT PRIMARY KEY,display TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS exchange_mapping(source TEXT NOT NULL,remote_id TEXT NOT NULL,project_id INTEGER NOT NULL REFERENCES production_cycle(id),PRIMARY KEY(source,remote_id));
CREATE TABLE IF NOT EXISTS exchange_files(id TEXT PRIMARY KEY,digest TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS exchange_issued(project_id INTEGER NOT NULL REFERENCES production_cycle(id), digest TEXT NOT NULL, PRIMARY KEY(project_id,digest));
CREATE TABLE IF NOT EXISTS exchange_delivery(packet TEXT NOT NULL,guid TEXT NOT NULL,project_id INTEGER NOT NULL REFERENCES production_cycle(id),report TEXT NOT NULL,PRIMARY KEY(packet,guid));
