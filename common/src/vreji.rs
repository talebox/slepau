use chrono::Utc;
use lazy_static::lazy_static;
use sonnerie::{CreateTx, record};

lazy_static!{
    pub static ref DB_PATH_LOG: std::path::PathBuf = std::path::PathBuf::from (std::env::var("DB_PATH_LOG").unwrap().as_str());
}

fn transaction() -> CreateTx {
    sonnerie::CreateTx::new(DB_PATH_LOG.as_path()).unwrap()
}
fn commit(t: CreateTx) {
    t.commit().unwrap();
}

pub fn auth_log(name: &str, user: &str, ip: &str) {
    let mut t = transaction();
    t.add_record(format!("auth_{name}").as_str() , Utc::now().naive_utc(), record(user).add(ip)).unwrap();
    commit(t);
}
pub fn auth_log_ip(name: &str, ip: &str) {
    let mut t = transaction();
    t.add_record(format!("auth_{name}").as_str() , Utc::now().naive_utc(), record(ip)).unwrap();
    commit(t);
}