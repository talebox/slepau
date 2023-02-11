#!/bin/nu

use env.nu *
use stop.nu *

export def build [] {
	# Just to make sure everything has stopped
	stop_force
	
	# Load all configs into build scope
	load_env
	open "config/prod.toml" | load-env
	
	enter container
		# Create output dirs
		rm -rf bin web # Not the keys, we need that 
		mkdir bin keys web
	exit

	# Build server
	cargo build --release
	cp target/release/auth container/bin/
	cp target/release/gen_key container/bin/

	# Build webapp
	enter web
		# Remove cache/build dirs
		rm -rf dist
		# rm -rf .parcel-cache
		# Build optimized
		yarn parcel build --public-url /web --no-source-maps
	exit

	# Copy webapp to output
	cp -r web/dist/* container/web/
	cp -r web/public/* container/web/
	
	# Gen key if nonexistent
	enter container
		./bin/gen_key
	exit
}

# Makes standalone
export def standalone [] {
	build
	
	let out = "talebox_x86_64"
	
	# Create out dir
	rm -rf $out
	mkdir $out
	
	cp container/bin/gen_key $"($out)/"
	cp readme_standalone.md $"($out)/readme.md"
	
	enter $out
		mkdir keys
		./gen_key
	exit
	
	# Copy files
	['auth'] | each {|a|
		mkdir $"($out)/($a)"
		cp $"container/bin/($a)" $"($out)/($a)/"
		cp -r $"container/web/($a)" $"($out)/($a)/web"
		
		enter $"($out)/($a)"
			ln -s ../keys keys
		exit
	}
	
	tar -cavf $"($out).tar.xz" $out
	
	# ls container | get name | where {|n| $n !~ gitignore and $n !~ Dockerfile} | each {|v| cp $v standalone/}
}

# Makes standalone
export def standalone_windows [] {
	let out = "talebox_x86_64"
	
	# Load all configs into build scope
	load_env
	open "config/prod.toml" | load-env
	
	cargo build --release --target x86_64-pc-windows-gnu
	
	cp target/x86_64-pc-windows-gnu/release/gen_key.exe $"($out)/"
	cp target/x86_64-pc-windows-gnu/release/auth.exe $"($out)/auth/"
	
	tar -cavf $"($out).zip" $out
}