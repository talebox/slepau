use lazy_static::lazy_static;
use sonnerie::CreateTx;

lazy_static! {
	pub static ref DB_PATH_SAMN: std::path::PathBuf =
		std::path::PathBuf::from(std::env::var("DB_PATH_SAMN").unwrap().as_str());
}

pub fn db() -> sonnerie::DatabaseReader {
	sonnerie::DatabaseReader::new(DB_PATH_SAMN.as_path()).unwrap()
}

pub fn transaction() -> CreateTx {
	sonnerie::CreateTx::new(&DB_PATH_SAMN.as_path()).unwrap()
}
pub fn commit(t: CreateTx) {
	t.commit().unwrap();
}