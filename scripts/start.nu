
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
export def run_chunk [] {
	setup_dev
	open "config/chunk/dev.toml" | load-env
	
	cargo run --bin chunk
}
export def run_media [] {
	setup_dev
	open "config/media/dev.toml" | load-env
	
	cargo run --bin media
}
export def build_media [] {
	setup_dev
	open "config/media/dev.toml" | load-env
	
	cargo build --bin media
}
export def run_gen_key [] {
	setup_dev
	
	cargo run --bin gen_key
}

export def test [] {
	setup_dev
	
	cargo test -j 1
}

export def check [] {
	setup_dev
	
	cargo check
}