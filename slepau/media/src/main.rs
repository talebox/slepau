pub mod ends;

pub static ref MEDIA_FOLDER: String = env::var("MEDIA_FOLDER").unwrap_or_else(|_| "media".into());

pub fn main() {
	println!("Started auth");
}
