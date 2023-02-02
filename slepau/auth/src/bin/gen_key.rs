use common::utils::{K_PUBLIC, K_SECRET};
use pasetors::{
	keys::{AsymmetricKeyPair, AsymmetricPublicKey, AsymmetricSecretKey, Generate},
	version4::V4,
};

fn main() {
	println!(
		"\
	Generates a public/secret key if nonexistent.\n\
	On K_PUBLIC:'{}', and K_SECRET:'{}'\n\
	\n\
	--force  will generate and write always\n\
	",
		K_PUBLIC.as_str(),
		K_SECRET.as_str()
	);

	let force = std::env::args_os().any(|a| a == "--force");

	fn generate() -> AsymmetricKeyPair<V4> {
		eprint!("generating...");
		let kp = AsymmetricKeyPair::<V4>::generate().unwrap();
		eprintln!("done!");
		eprint!("Writing...");
		std::fs::write(K_PUBLIC.as_str(), kp.public.as_bytes()).unwrap();
		std::fs::write(K_SECRET.as_str(), kp.secret.as_bytes()).unwrap();
		eprintln!("done!");
		kp
	}

	// let kp;
	if force {
		generate();
	} else if std::fs::read(K_PUBLIC.as_str())
		.ok()
		.and_then(|b| AsymmetricPublicKey::<V4>::from(b.as_slice()).ok())
		.is_some()
		&& std::fs::read(K_SECRET.as_str())
			.ok()
			.and_then(|b| AsymmetricSecretKey::<V4>::from(b.as_slice()).ok())
			.is_some()
	{
		println!("Keys found!");
	} else {
		eprint!("Keys not found! ");
		generate();
	}
}
