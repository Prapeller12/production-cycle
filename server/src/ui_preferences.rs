use crate::production::ProductionDb;

// This additive preferences table does not alter production records or signatures.
// A public key fingerprint prevents a restored/reused numeric ID claiming another user's history.
pub fn claim_onboarding(db:&ProductionDb,fingerprint:&str)->Result<bool,String>{
    let conn=db.conn.lock().map_err(|_|"База данных занята".to_string())?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS browser_user_preferences (key_fingerprint TEXT PRIMARY KEY, onboarding_seen_at TEXT NOT NULL);").map_err(|e|e.to_string())?;
    let changed=conn.execute("INSERT OR IGNORE INTO browser_user_preferences(key_fingerprint,onboarding_seen_at) VALUES(?1,?2)",rusqlite::params![fingerprint,chrono::Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;
    Ok(changed==1)
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn first_open_is_per_identity_not_browser(){
        let db=ProductionDb::memory().unwrap();
        assert!(claim_onboarding(&db,"key-a").unwrap());
        assert!(!claim_onboarding(&db,"key-a").unwrap());
        assert!(claim_onboarding(&db,"key-b").unwrap());
        assert!(!claim_onboarding(&db,"key-b").unwrap());
    }
}
