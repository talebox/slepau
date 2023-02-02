
use env.nu *
use stop.nu *

export def-env setup_dev [] {
	load_env
	open "config/dev.toml" | load-env
}

export def start [] {
	stop_force
	setup_dev
	
	/bin/env scripts/start.sh
}

export def run [] {
	setup_dev
	
	cargo run
}
export def run_auth [] {
	setup_dev
	open "config/auth/dev.toml" | load-env
	
	cargo run --bin auth
}

export def test [] {
	setup_dev
	
	cargo test
}

export def check [] {
	setup_dev
	
	cargo check
}