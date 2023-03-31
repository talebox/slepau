#!/bin/nu

use env.nu *
use stop.nu *

export def clean [] {
	rm -rf out
	mkdir out
	
	cargo clean
	
	enter web
		rm -rf dist .parcel-cache
	exit 
}

export def organize_out [bin_dir = "bin_linux_x86_64"] {
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
				cp $"($bin_dir)/($a)" $"slepau/($a)/"
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
		enter nginx
		
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -e 's/ 80/ 443 ssl/g'
			
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -e 's/\.\*/.anty.dev/g'
			
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -e 's$#KEYS$ssl_certificate /etc/letsencrypt/live/anty.dev/fullchain.pem; # managed by Certbot\n\tssl_certificate_key /etc/letsencrypt/live/anty.dev/privkey.pem; # managed by Certbot$g'
			
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -E 's/#(\w+)\.access/access_log logs\/\1\-access\.log compression;/g'
			
			/bin/find . -type f -name "*.conf" -print0 | xargs -0 sed -i -E 's/400([0-9])/450\1/g'
			
			/bin/find . -type f -name "talebox.conf" -print0 | xargs -0 sed -i -E 's$root .*;$root /srv/http/talebox;$g'
		
		exit
		
		
	exit
	
	
	
	print "Seprating done."
}

export def build_server [bin_dir:string = "bin_linux_x86_64", target_dir:string = "target"] {
	load_env_prod
	
	rm -rf $"out/($bin_dir)"
	mkdir $"out/($bin_dir)"
	
	print $"Building binaries to out/($bin_dir)."
	# Build server
	['auth', 'chunk', 'media', 'gen_key'] | each {|a|
		if $a not-in ["talebox"]  {
			cargo build -Z unstable-options --target-dir $target_dir --out-dir $"out/($bin_dir)"  --release --bin $a
			
			# if $is_musl {
			# 	cp $"target/x86_64-unknown-linux-musl/release/($a)" out/bin/	
			# } else {
			# 	cp $"target/release/($a)" out/bin/
			# }
		}
	};
	print "Binaries built."
}
export def build_web [] {
	load_env_prod
	
	rm -rf out/web
	mkdir out/web
	
	print "Building webc."
	# Build webapp
	enter web
		# Remove cache/build dirs
		rm -rf dist .parcel-cache
		# Build optimized
		yarn parcel build --public-url /web --no-source-maps
	exit

	# Copy webapp to output
	cp -r web/dist/* out/web/
	
	# Copy talebox script to download & install standalone build.
	["linux_x86_64", "musl_x86_64"] | each {|a|
		cat ./standalone.sh | sed -E $'s#standalone#standalone_($a)#g' | save -f $"out/web/talebox/standalone_($a).sh"
	};
	
	print "Finished building web."
}
# linux_x86_64 and musl_x86_64

# Creates standalone compressed in out folder
export def make_standalone [dir = "linux_x86_64"] {
	enter out
		print $"Making standalone_($dir)"
		rm -rf $"standalone_($dir)"
		mkdir $"standalone_($dir)"
		enter $"standalone_($dir)"
			mkdir keys
			cp $"../bin_($dir)/gen_key" ./
			
			["auth","chunk","media","talebox"] | each {|a|
				cp -r $"../slepau/($a)" ./
				enter $a
					ln -s ../keys keys
				exit
			};
			
			cp -r ../../config/nginx ./
			
			cp ../../standalone_readme.md ./readme.md
			# In case i want to copy/replace inline.
			# cat ../../standalone_run.sh | sed -E $'s#standalone\.tar\.xz#standalone_($dir)\.tar\.xz#g' | save -f ./run.sh
			cp ../../standalone_run.sh ./run.sh
		exit
		
		print $"Compressing standalone_($dir)"
		tar -cavf $"standalone_($dir).tar.xz" $"standalone_($dir)"
	exit
}

# export def build_standalone [] {
# 	print "Making musl binaries, so we can make a truly standalone app."
# 	stop_force
	
# 	build_server
# 	build_server_musl
	
# 	build_web
# 	organize_out
# 	make_standalone
# 	print "Finished standalone binaries."
# }

export def build_all [] {
	print "Building everying."
	
	# Just to make sure everything has stopped
	stop_force
	
	build_web
	
	
	build_server
	organize_out
	make_standalone
	
	# build_server_musl
	# organize_out bin_musl_x86_64
	# make_standalone musl_x86_64
	
	print "Build of everything finished. You can safely deploy now."
}

export def create_musl_builder [] {
	print "Creating image musl_builder on docker context default."
	docker context use default
	docker build -t musl_builder -f ./container/musl_builder.dockerfile ./container
	
	print "Done."
}

export def build_server_musl [bin_dir = "bin_musl_x86_64"] {
	print "Executing build_server with musl_builder image on docker context default."
	docker context use default
	alias musl_builder = docker run --rm -it -v $"(pwd):/volume" -v $"(pwd):/volume" -v cargo-registry:/root/.cargo/registry musl_builder
	musl_builder nu -c $"source scripts/source.nu; build_server ($bin_dir) target_musl"
	print "Done."
}

# # Makes standalone (TESTING)
# export def standalone [] {
# 	build_all
	
# 	let out = "talebox_x86_64"
	
# 	# Create out dir
# 	rm -rf $out
# 	mkdir $out
	
# 	cp out/bin/gen_key $"($out)/"
# 	cp standalone_readme.md $"($out)/readme.md"
	
# 	enter $out
# 		mkdir keys
# 		./gen_key
# 	exit
	
# 	# Copy files
# 	['auth'] | each {|a|
# 		mkdir $"($out)/($a)"
# 		cp $"out/bin/($a)" $"($out)/($a)/"
# 		cp -r $"out/web/($a)" $"($out)/($a)/web"
		
# 		enter $"($out)/($a)"
# 			ln -s ../keys keys
# 		exit
# 	};
	
# 	tar -cavf $"($out).tar.xz" $out
	
# 	# ls out | get name | where {|n| $n !~ gitignore and $n !~ Dockerfile} | each {|v| cp $v standalone/}
# }

# # Makes standalone for windows (TESTING)
# export def standalone_windows [] {
# 	let out = "talebox_x86_64"
	
# 	# Load all configs into build scope
# 	load_env
# 	open "config/prod.toml" | load-env
	
# 	cargo build --release --target x86_64-pc-windows-gnu
	
# 	cp target/x86_64-pc-windows-gnu/release/gen_key.exe $"($out)/"
# 	cp target/x86_64-pc-windows-gnu/release/auth.exe $"($out)/auth/"
	
# 	tar -cavf $"($out).zip" $out
# }