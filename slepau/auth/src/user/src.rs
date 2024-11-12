use argon2::{
	password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
	Argon2,
};

use common::utils::{DbError, REGEX_PASSWORD, REGEX_PASSWORD_HUMAN, REGEX_USERNAME, REGEX_USERNAME_HUMAN};

use super::{blacklist::BLACKLIST, User};

#[allow(dead_code)]
impl User {
	pub fn verify_login(&self, pass: &str) -> Result<(), DbError> {
		if !self.active {
			return Err(DbError::AuthError);
		}
		// PHC string -> PasswordHash.
		let parsed_hash = PasswordHash::new(&self.pass).expect("Error parsing existing password field");

		// Compare pass hash vs PasswordHash
		Argon2::default()
			.verify_password(pass.as_bytes(), &parsed_hash)
			.map_err(|_| DbError::AuthError)
	}
	fn hash(pass: &str) -> Result<String, DbError> {
		if !REGEX_PASSWORD.is_match(pass) {
			return Err(DbError::InvalidPassword(REGEX_PASSWORD_HUMAN.as_str()));
		}

		let salt = SaltString::generate(&mut OsRng);
		Ok(
			Argon2::default()
				.hash_password(pass.as_bytes(), &salt)
				.unwrap()
				.to_string(),
		)
	}
	pub fn new(user: &str, pass: &str) -> Result<Self, DbError> {
		if !REGEX_USERNAME.is_match(user) {
			return Err(DbError::InvalidUsername(REGEX_USERNAME_HUMAN.as_str()));
		}
		if BLACKLIST.iter().any(|v| user == *v) {
			return Err(DbError::InvalidUsername("Username blacklisted... :|"));
		}

		Ok(Self {
			user: user.into(),
			pass: Self::hash(pass)?,
			active: true,
			claims: Default::default(),
		})
	}

	pub fn reset_pass(&mut self, old_pass: Option<&str>, pass: &str) -> Result<(), DbError> {
		if let Some(old_pass) = old_pass {
			self.verify_login(old_pass)?;
		}
		self.pass = Self::hash(pass)?;

		Ok(())
	}
}
