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

export def organize_out [] {
	print "Organizing all built files."
	
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
			
			# Copy login project
			if $a not-in ["talebox", "gen_key"]  {
				cp -r web/login $"slepau/($a)/web/"
			}
		};
		
		cp -r ../config/nginx ./
		enter nginx/sites
		
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -e 's/8080/443 ssl/g'
			
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -e 's/\.\*/.anty.dev/g'
			
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -e 's$#KEYS$ssl_certificate /etc/letsencrypt/live/anty.dev/fullchain.pem; # managed by Certbot\n\tssl_certificate_key /etc/letsencrypt/live/anty.dev/privkey.pem; # managed by Certbot$g'
			
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -E 's/#(\w+)\.access/access_log logs\/\1\-access\.log compression;/g'
			
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -E 's/400([0-9])/450\1/g'
		
		exit
		
		print "Making standalone."
		rm -rf standalone
		mkdir standalone
		enter standalone
			mkdir keys
			cp ../bin/gen_key ./
			
			["auth","chunk","media"] | each {|a|
				cp -r $"../slepau/($a)" ./
				enter $a
					ln -s ../keys keys
				exit
			};
			
			cp -r ../../config/nginx ./
			
			cp ../../standalone_readme.md ./readme.md
			cp ../../standalone_run.sh ./run.sh
		exit
		
		print "Compressing standalone."
		tar -cavf standalone.tar.xz standalone
	exit
	
	print "Seprating done."
}

export def build_server [] {
	load_env_prod
	
	rm -rf out/bin
	mkdir out/bin
	
	print "Building server."
	# Build server
	['auth', 'chunk', 'media', 'gen_key', 'talebox'] | each {|a|
		if $a not-in ["talebox"]  {
			cargo build --release --bin $a
			
			if ("target/x86_64-unknown-linux-musl" | path exists) {
				cp $"target/x86_64-unknown-linux-musl/release/($a)" out/bin/	
			} else {
				cp $"target/release/($a)" out/bin/
			}
		}
	};
	print "Building server done."
}
export def build_web [] {
	load_env_prod
	
	rm -rf out/web
	mkdir out/web
	
	print "Building web."
	# Build webapp
	enter web
		# Remove cache/build dirs
		rm -rf dist #.parcel-cache
		# Build optimized
		yarn parcel build --public-url /web --no-source-maps
	exit

	# Copy webapp to output
	cp -r web/dist/* out/web/
	
	print "Finished building web."
}


export def build_all [] {
	print "Building everying."
	
	# Just to make sure everything has stopped
	stop_force
	
	build_server_musl
	build_web
	
	organize_out
	
	print "Build of everything finished. You can safely deploy now."
}

export def create_musl_builder [] {
	docker build -t musl_builder -f ./container/musl_builder.dockerfile ./container
}

export def build_server_musl [] {
	alias musl_builder = docker run --rm -it -v $"(pwd):/volume" -v cargo-registry:/root/.cargo/registry musl_builder
	musl_builder nu -c "source scripts/source.nu; build_server"
}

# Makes standalone (TESTING)
export def standalone [] {
	build_all
	
	let out = "talebox_x86_64"
	
	# Create out dir
	rm -rf $out
	mkdir $out
	
	cp out/bin/gen_key $"($out)/"
	cp standalone_readme.md $"($out)/readme.md"
	
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
	};
	
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