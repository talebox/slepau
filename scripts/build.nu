#!/bin/nu

use env.nu *
use stop.nu *

export def clean [] {
	enter out
		rm -rf bin web
		mkdir bin web
	exit
	
	cargo clean
	
	enter web
		rm -rf dist .parcel-cache
	exit 
}

def separate_out [] {
	
	enter out
		rm -rf slepau
		
		['auth', 'chunk', 'media', 'gen_key', 'talebox'] | each {|a|
			echo $"Doing ($a)."
			# Make slepau dir
			mkdir $"slepau/($a)"
			# Don't for ...
			if $a not-in ["talebox"]  {
				echo $"Bin/docker for ($a)."
				# Copy bin
				cp $"bin/($a)" $"slepau/($a)/"
				# Copy dockerfile
				cp $"../container/($a).dockerfile" $"slepau/($a)/dockerfile"
			}
			# Don't for ...
			if $a not-in ["gen_key"]  {
				echo $"Web for ($a)."
				mkdir $"slepau/($a)/web"
				# Copy web
				cp -r $"web/($a)/*" $"slepau/($a)/web/"
			}
		}
		
	exit 
}
export def build [] {
	
	# Just to make sure everything has stopped
	stop_force
	
	# Load all configs into build scope
	load_env
	open "config/prod.toml" | load-env
	
	enter out
		# Create output dirs
		rm -rf bin web
		mkdir bin web
	exit

	# Build server
	['auth', 'chunk', 'media', 'gen_key', 'talebox'] | each {|a|
	
		if $a not-in ["talebox"]  {
			cargo build --release --bin $a
			cp $"target/release/($a)" out/bin/
		}
	}
	
	# Build webapp
	enter web
		# Remove cache/build dirs
		rm -rf dist .parcel-cache
		# Build optimized
		yarn parcel build --public-url /web --no-source-maps
	exit

	# Copy webapp to output
	cp -r web/dist/* out/web/
	
	separate_out
}

# Makes standalone (TESTING)
export def standalone [] {
	build
	
	let out = "talebox_x86_64"
	
	# Create out dir
	rm -rf $out
	mkdir $out
	
	cp out/bin/gen_key $"($out)/"
	cp readme_standalone.md $"($out)/readme.md"
	
	enter $out
		mkdir keys
		./gen_key
	exit
	
	# Copy files
	['auth'] | each {|a|
		mkdir $"($out)/($a)"
		cp $"out/bin/($a)" $"($out)/($a)/"
		cp -r $"out/web/($a)" $"($out)/($a)/web"
		
		enter $"($out)/($a)"
			ln -s ../keys keys
		exit
	}
	
	tar -cavf $"($out).tar.xz" $out
	
	# ls out | get name | where {|n| $n !~ gitignore and $n !~ Dockerfile} | each {|v| cp $v standalone/}
}

# Makes standalone for windows (TESTING)
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