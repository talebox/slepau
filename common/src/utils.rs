use axum::response::IntoResponse;
use hyper::StatusCode;
use lazy_static::lazy_static;
use proquint::Quintable;
use rand::prelude::*;
use regex::Regex;
use reqwest::Url;
use serde::{ Serialize, Deserialize};
use serde_json::Value;

pub type LockedAtomic<T> = Arc<RwLock<T>>;
pub type LockedWeak<T> = Weak<RwLock<T>>;

use std::{
	env,
	net::SocketAddr,
	str::FromStr,
	sync::{Arc, RwLock, Weak},
	time::{SystemTime, UNIX_EPOCH},
};

pub fn get_secs() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Before UNIX_EPOCH")
		.as_secs()
}
pub const SECS_IN_HOUR: u64 = 60 * 60;
pub const SECS_IN_DAY: u64 = SECS_IN_HOUR * 24;

pub fn gen_proquint32() -> String {
	random::<u32>().to_quint()
}
pub fn gen_proquint64() -> String {
	random::<u64>().to_quint()
}
pub fn gen_64() -> u64 {
	random::<u64>()
}

lazy_static! {
	pub static ref REGEX_TITLE: Regex = Regex::new(env!("REGEX_TITLE")).unwrap();
	pub static ref REGEX_ACCESS: Regex = Regex::new(format!("(?im){}", env!("REGEX_ACCESS")).as_str()).unwrap();
	pub static ref REGEX_PROPERTY: Regex = Regex::new(format!("(?m){}", env!("REGEX_PROPERTY")).as_str()).unwrap();
	pub static ref REGEX_USERNAME: Regex = Regex::new(env!("REGEX_USERNAME")).unwrap();
	pub static ref REGEX_PASSWORD: Regex = Regex::new(env!("REGEX_PASSWORD")).unwrap();
}

lazy_static! {
	pub static ref K_PUBLIC: String = env::var("K_PUBLIC").unwrap_or_else(|_| "keys/public.k".into());
	pub static ref K_SECRET: String = env::var("K_SECRET").unwrap_or_else(|_| "keys/secret.k".into());
	/// Use this file as your db storage
	pub static ref DB_PATH: Option<String> = Some(env::var("DB_PATH").unwrap_or_else(|_| "db.json".into()));
	/// Fetches magic bean if set
	pub static ref DB_INIT: Option<String> = env::var("DB_INIT").ok();
	pub static ref DB_BACKUP_FOLDER: String = env::var("DB_BACKUP_FOLDER").unwrap_or_else(|_| "backups".into());
	pub static ref MEDIA_FOLDER: String = env::var("MEDIA_FOLDER").unwrap_or_else(|_| "media".into());
	pub static ref CACHE_PATH: String = env::var("CACHE_PATH").unwrap_or_else(|_| "cache.json".into());
	pub static ref WEB_DIST: String = env::var("WEB_DIST").unwrap_or_else(|_| "web".into());
	/// The socket is we listen to requests from.
	pub static ref SOCKET: SocketAddr = SocketAddr::from_str(env::var("SOCKET").unwrap_or_else(|_| "0.0.0.0:4000".into()).as_str()).expect("Socket address to be valid");
	/// The URL is where users go to access this slepau.
	pub static ref URL: Url = Url::parse(env::var("URL").unwrap_or_else(|_| "http://localhost:4000".into()).as_str()).expect("URL to be valid");
	pub static ref SECURE: bool = URL.scheme() != "http";
}

pub const KEYWORD_BLACKLIST: [&str; 12] = [
	"admin", "root", "note", "chunk", "share", "access", "read", "write", "lock", "unlock", "public", "inherit",
];

/**
 * # Basic string normalizer
 * 1. Lowercases everything.
 * 1. Turns `[ -]` to spaces ` `.
 * 1. Only allows `[a-z0-9_]` through.
 */
pub fn standardize(v: &str) -> String {
	v.trim()
		.to_lowercase()
		.chars()
		.map(|v| match v {
			'-' => '_',
			' ' => '_',
			_ => v,
		})
		.filter(|v| matches!(v, 'a'..='z' | '0'..='9' | '_'))
		.collect()
}

/**
 * Describes a handled error.
 */
#[derive(Debug, PartialEq, Serialize, Eq)]
pub enum DbError {
	UserTaken,
	AuthError,
	InvalidHost,
	InvalidSite,
	InvalidUsername,
	InvalidPassword,
	InvalidChunk,
	NotFound,
}
impl IntoResponse for DbError {
	fn into_response(self) -> axum::response::Response {
		(StatusCode::FORBIDDEN, format!("{self:?}")).into_response()
	}
}

use diff::Result::*;

pub fn diff_calc(left: &str, right: &str) -> Vec<String> {
	let diffs = diff::lines(left, right);
	// SO it'll be ["B44", ""]
	let out: Vec<String> = diffs.iter().fold(vec![], |mut acc, v| {
		match *v {
			Left(_l) => {
				if acc.last().map(|v| v.starts_with('D')) == Some(true) {
					// Add 1
					*acc.last_mut().unwrap() = format!("D{}", (&acc.last().unwrap()[1..].parse::<u32>().unwrap() + 1));
				} else {
					acc.push("D1".to_string());
				}
			}
			Both(_, _) => {
				if acc.last().map(|v| v.starts_with('K')) == Some(true) {
					// Add 1
					*acc.last_mut().unwrap() = format!("K{}", (&acc.last().unwrap()[1..].parse::<u32>().unwrap() + 1));
				} else {
					acc.push("K1".to_string());
				}
			}
			Right(l) => {
				acc.push(format!("A{}", l));
			}
		}
		acc
	});
	// info!("{out:?}");
	// println!("{diffs:?}");
	out
}

pub fn log_env() {
	// let j = env::vars().filter(|(k, _)| k.contains("REGEX_") || k.contains("DB_") || k == "HOST" || k == "WEB_DIST");
	// println!("Relevant environment variables:");
	// j.for_each(|(k, v)| println!("{k}: {v}"));
	// println!();
}
#[derive(Serialize)]
pub struct DataSlice<T> {
	pub items: Vec<T>,
	pub total: usize,
}

