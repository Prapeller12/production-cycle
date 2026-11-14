use crate::audit::AuditUserSummary;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand_core::{OsRng, RngCore};
use serde_json::{json, Value};
use std::{collections::HashMap, time::{Duration, Instant}};

pub struct Session {
    pub user: Option<AuditUserSummary>,
    pub started: Instant,
    pub seen: Instant,
    pub eviction: bool,
    pub admin_authorized: bool,
}
pub struct Sessions {
    entries: HashMap<String, Session>,
    timeout: Duration,
    pub maximum: usize,
}
impl Sessions {
    pub fn new(maximum: usize, timeout: Duration) -> Self { Self { entries: HashMap::new(), timeout, maximum } }
    pub fn expire(&mut self, now: Instant) { self.entries.retain(|_, s| now.duration_since(s.seen) < self.timeout); }
    pub fn user(&mut self, token: &str) -> Result<Option<AuditUserSummary>, String> {
        self.expire(Instant::now());
        let s = self.entries.get_mut(token).ok_or("Сеанс завершён. Войдите повторно.")?;
        s.seen = Instant::now(); Ok(s.user.clone())
    }
    pub fn status(&mut self, token: &str) -> Value {
        self.expire(Instant::now());
        let count = self.entries.len();
        if let Some(s) = self.entries.get_mut(token) {
            s.seen = Instant::now();
            json!({"enabled":true,"active":true,"occupied":count,"maximum":self.maximum,"evictionRequested":s.eviction,
                "userId":s.user.as_ref().map(|u|u.id),"displayName":s.user.as_ref().map(|u|u.display_name.clone()).unwrap_or("Первичная настройка".into()),
                "role":s.user.as_ref().map(|u|u.role.clone()),"sessionId":null})
        } else { json!({"enabled":true,"active":false,"occupied":count,"maximum":self.maximum,"evictionRequested":false}) }
    }
    pub fn login(&mut self, old: &str, user: Option<AuditUserSummary>) -> Result<(Value, Option<String>), String> {
        self.expire(Instant::now());
        if let Some(s) = self.entries.get(old) {
            if s.user.as_ref().map(|u|u.id) == user.as_ref().map(|u|u.id) {
                return Ok((json!({"state":"active","occupied":self.entries.len(),"maximum":self.maximum}), None));
            }
            return Err("Перед сменой профиля завершите текущий сеанс через кнопку «Выйти».".into());
        }
        if self.entries.len() >= self.maximum {
            if user.as_ref().map(|u| u.role.as_str()) == Some("admin") {
                if let Some((_, s)) = self.entries.iter_mut().filter(|(_, s)|s.user.as_ref().map(|u|u.role.as_str()) != Some("admin")).max_by_key(|(_,s)|s.started) {
                    s.eviction = true;
                    return Ok((json!({"state":"waitingRelease","occupied":self.entries.len(),"maximum":self.maximum}), None));
                }
            }
            return Ok((json!({"state":"full","occupied":self.entries.len(),"maximum":self.maximum}), None));
        }
        let mut bytes = [0u8; 32]; OsRng.fill_bytes(&mut bytes);
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let admin_authorized = user.as_ref().map(|u|u.role.as_str()) == Some("admin");
        self.entries.insert(token.clone(), Session { user, started: Instant::now(), seen: Instant::now(), eviction: false, admin_authorized });
        Ok((json!({"state":"active","occupied":self.entries.len(),"maximum":self.maximum}), Some(token)))
    }
    pub fn bind(&mut self, token: &str, user: AuditUserSummary) -> Result<(), String> {
        let s=self.entries.get_mut(token).ok_or("Первичный сеанс завершён")?;
        s.admin_authorized=user.role=="admin";s.user = Some(user); Ok(())
    }
    pub fn elevate(&mut self, token: &str) { if let Some(s)=self.entries.get_mut(token){s.admin_authorized=true;} }
    pub fn admin(&self, token: &str)->bool {self.entries.get(token).map(|s|s.admin_authorized).unwrap_or(false)}
    pub fn logout(&mut self, token: &str) { self.entries.remove(token); }
    pub fn invalidate(&mut self) { self.entries.clear(); }
    pub fn retain_current(&mut self, token:&str) { self.entries.retain(|key,_|key==token); }
}

#[cfg(test)] mod tests {
    use super::*;
    fn user(id:i64, role:&str)->AuditUserSummary { AuditUserSummary {id, display_name:format!("U{id}"),role:role.into(),key_fingerprint:"test".into(),active:true,created_at:String::new()} }
    #[test] fn admission_eviction_expiry_and_identity_are_server_owned() {
        let mut s = Sessions::new(3,Duration::from_secs(30));
        let (_,a)=s.login("",Some(user(1,"reviewer"))).unwrap();let a=a.unwrap();
        let (_,b)=s.login("",Some(user(2,"reviewer"))).unwrap();let b=b.unwrap();
        let (_,c)=s.login("",Some(user(3,"project_manager"))).unwrap();let c=c.unwrap();
        assert_eq!(s.login("",Some(user(4,"reviewer"))).unwrap().0["state"],"full");
        assert_eq!(s.login("",Some(user(5,"admin"))).unwrap().0["state"],"waitingRelease");
        assert_eq!(s.status(&c)["evictionRequested"],true);
        assert_eq!(s.status(&a)["evictionRequested"],false);
        assert!(s.login(&a,Some(user(5,"admin"))).is_err());
        s.logout(&c);assert_eq!(s.login("",Some(user(5,"admin"))).unwrap().0["state"],"active");
        assert!(s.user(&b).is_ok());s.expire(Instant::now()+Duration::from_secs(31));assert_eq!(s.status(&a)["active"],false);
    }
}
