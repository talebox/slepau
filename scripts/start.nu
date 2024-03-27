
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
export def run_web [script] {
	load_env_dev
	
	enter web
		if $script == 'clean' {
			rm -rf .parcel-cache dist
		} else {
			yarn $script
		}
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
export def run_vreji [] {
	load_env_dev
	open "config/vreji/dev.toml" | load-env
	
	cargo run --bin vreji
}
export def run_samn [] {
	load_env_dev
	open "config/vreji/dev.toml" | load-env
	
	cargo run --bin samn
}
export def build_media [] {
	load_env_dev
	open "config/media/dev.toml" | load-env
	
	cargo build --bin media
}
export def run_setup [] {
	load_env_dev
	
	cargo run --bin setup
}
export def run_nginx [] {
	$env.WEB_DIR = $"(pwd)/web/dist"

	enter config/nginx
		sudo -E ./nginx.sh
}

export def test [] {
	load_env_dev
	
	cargo test
}

export def check [] {
	load_env_dev
	
	cargo check
}