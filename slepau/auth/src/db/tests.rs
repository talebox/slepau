use rand::distributions::{Alphanumeric, DistString};
use serde_json::json;

use crate::db::site::AdminSet;

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
	// First admin is always super
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
fn modify() {
	let mut db = DBAuth::default();
	// First admin is always super
	db.new_admin("john_s", "john_s").unwrap();
	db.new_admin("john_123", "john_123").unwrap();

	// Can supers do this?
	{
		assert!(
			db.mod_admin(
				"john_s",
				"john_s",
				serde_json::from_value(json! ({
					"active": true,
					"claims": {"test":"yes"},
					"sites": [],
					"super": true
				}))
				.unwrap()
			)
			.is_ok(),
			"Can indeed edit themselves"
		);

		assert!(
			db.mod_admin(
				"john_s",
				"john_123",
				serde_json::from_value(json! ({
					"active": false,
					"claims": {},
					"sites": [],
					"super": true
				}))
				.unwrap()
			)
			.is_ok(),
			"Can modify other's active and super"
		);
		assert!(
			db.mod_admin(
				"john_s",
				"john_123",
				serde_json::from_value(json! ({
					"active": true,
					"claims": {},
					"sites": [],
					"super": false
				}))
				.unwrap()
			)
			.is_ok(),
			"Can modify other's active and super"
		);

		assert!(
			db.mod_admin(
				"john_s",
				"john_s",
				serde_json::from_value(json! ({
					"active": false,
					"claims": {},
					"sites": [],
					"super": true
				}))
				.unwrap()
			)
			.is_err(),
			"Cannot remove their own active status"
		);

		assert!(
			db.mod_admin(
				"john_s",
				"john_s",
				serde_json::from_value(json! ({
					"active": true,
					"claims": {},
					"sites": [],
					"super": false
				}))
				.unwrap()
			)
			.is_err(),
			"Cannot remove their own super status"
		);
	}
}
