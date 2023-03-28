
use env.nu *
use stop.nu *

export def main [] {
	stop_force
	load_env_dev
	
	/bin/env scripts/start.sh
}

export def run [] {
	load_env_dev
	
	cargo run
}
export def run_auth [] {
	load_env_dev
	open "config/auth/dev.toml" | load-env
	
	cargo run --bin auth
}
export def run_chunk [] {
	load_env_dev
	open "config/chunk/dev.toml" | load-env
	
	cargo run --bin chunk
}
export def run_media [] {
	load_env_dev
	open "config/media/dev.toml" | load-env
	
	cargo run --bin media
}
export def build_media [] {
	load_env_dev
	open "config/media/dev.toml" | load-env
	
	cargo build --bin media
}
export def run_gen_key [] {
	load_env_dev
	
	cargo run --bin gen_key
}
export def run_nginx [] {
	enter config/nginx
		sudo ./nginx.sh
	exit
}

export def test [] {
	load_env_dev
	
	cargo test
}

export def check [] {
	load_env_dev
	
	cargo check
}