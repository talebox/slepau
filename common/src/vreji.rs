use std::net::IpAddr;

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
pub fn ip_to_u32(ip: IpAddr) -> u32 {
    match ip {
        IpAddr::V4(v4) => v4.into(),
        IpAddr::V6(_) => 0
    }
}

pub fn log_ip(name: &str, ip: IpAddr) {
    let mut t = transaction();
    t.add_record(name , Utc::now().naive_utc(), record(ip_to_u32(ip))).unwrap();
    commit(t);
}
pub fn log_ip_user(name: &str, ip: IpAddr, user: &str) {
    let mut t = transaction();
    t.add_record(name , Utc::now().naive_utc(), record(ip_to_u32(ip)).add(user)).unwrap();
    commit(t);
}
pub fn log_ip_user_id(name: &str, ip: IpAddr, user: &str, id: u64) {
    let mut t = transaction();
    t.add_record(name , Utc::now().naive_utc(), record(ip_to_u32(ip)).add(user).add(id)).unwrap();
    commit(t);
}