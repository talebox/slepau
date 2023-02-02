use rand::distributions::{Alphanumeric, DistString};

use super::*;

#[test]
fn users() {
	let mut db = DBAuth::default();
	assert_eq!(
		db.new_user("Nana3", "1234"),
		Err(DbError::InvalidUsername),
		"Username characters invalid, only lowercase"
	);
	assert_eq!(
		db.new_user("Nana&", "1234"),
		Err(DbError::InvalidUsername),
		"Username characters invalid, no special"
	);
	assert_eq!(
		db.new_user(":nana", "1234"),
		Err(DbError::InvalidUsername),
		"Username characters invalid, no special"
	);
	assert_eq!(
		db.new_user("assphalt", "1234"),
		Err(DbError::InvalidUsername),
		"No bad words"
	);
	assert_eq!(
		db.new_user("tits_44", "1234"),
		Err(DbError::InvalidUsername),
		"No bad words"
	);

	assert_eq!(
		db.new_user("na", "1234"),
		Err(DbError::InvalidUsername),
		"Username >= 3 in size"
	);
	assert_eq!(
		db.new_user("nan", "12"),
		Err(DbError::InvalidPassword),
		"Password >= 6 in size"
	);
	assert_eq!(
		db.new_user("nan", &Alphanumeric.sample_string(&mut rand::thread_rng(), 70)),
		Err(DbError::InvalidPassword),
		"Password <= 64 in size"
	);
	assert!(db.new_user("nina", "nina's pass").is_ok());

	// assert_eq!(db.users.len(), 1);

	assert!(db.login("nina", "wrong_pass").is_err(), "Password is wrong");
	assert!(db.login("nana", "wrong_pass").is_err(), "User nana doesn't exist");
	assert!(db.login("nina", "nina's pass").is_ok(), "Login success");
}
