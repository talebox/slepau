#!/bin/nu

use env.nu *
use stop.nu *

export def build [] {
	# Just to make sure everything has stopped
	stop
	
	# Load all configs into build scope
	load_env
	open "config/prod.toml" | load-env

	# Create output dirs
	rm -rf container/dist
	mkdir container/dist

	# Build server
	cargo build --release
	cp target/release/chunk-app container/dist/

	# Build webapp
	enter web
		# Remove cache/build dirs
		rm -rf dist .parcel-cache
		# Build optimized
		yarn parcel build --public-url /web --no-source-maps
	exit

	# Copy webapp to output
	cp -r web/dist/* container/dist/
	cp -r web/public/* container/dist/web/
	rm -f container/dist/web/*.map
}

def main [] {
	build
}