use rand::distributions::{Alphanumeric, DistString};

use super::{stats::DBAuthStats, *};

#[test]
fn users() {
	let mut db = DBAuth::default();
	db.new_admin("john12", "john12").unwrap();
	let site_id = db.new_site("john12").unwrap();

	assert!(
		db.new_user("Nana3", "1234", site_id).is_err(),
		"Username characters invalid, only lowercase"
	);
	assert!(
		db.new_user("Nana&", "1234", site_id).is_err(),
		"Username characters invalid, no special"
	);
	assert!(
		db.new_user(":nana", "1234", site_id).is_err(),
		"Username characters invalid, no special"
	);
	assert!(db.new_user("assphalt", "1234", site_id).is_err(), "No bad words");
	assert!(db.new_user("tits_44", "1234", site_id).is_err(), "No bad words");

	assert!(db.new_user("na", "1234", site_id).is_err(), "Username >= 3 in size");
	assert!(db.new_user("nan", "12", site_id).is_err(), "Password >= 6 in size");
	assert!(
		db.new_user("nan", &Alphanumeric.sample_string(&mut rand::thread_rng(), 70), site_id)
			.is_err(),
		"Password <= 64 in size"
	);
	assert!(db.new_user("nina", "nina's pass", site_id).is_ok());

	assert!(
		db.login("nina", "wrong_pass", Some(site_id)).is_err(),
		"Password is wrong"
	);
	assert!(
		db.login("nana", "wrong_pass", Some(site_id)).is_err(),
		"User nana doesn't exist"
	);
	assert!(db.login("nina", "nina's pass", Some(site_id)).is_ok(), "Login success");
}

#[test]
fn visibility() {
	let mut db = DBAuth::default();
	db.new_admin("john_s", "john_s").unwrap();
	db.new_admin("john123", "john123").unwrap();
	let site_id = db.new_site("john123").unwrap();
	db.new_user("nico", "nicopass", site_id).unwrap();

	assert_eq!(
		DBAuthStats::from(&db),
		DBAuthStats {
			sites: 1,
			hosts: 0,
			admins: 2,
			users: 1,
		}
	);
}

#[test]
fn admins() {
	// Admins cannot remove their own super admin status.
	
	// Admins can see all sites.
	
}
