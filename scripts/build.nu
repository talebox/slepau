#!/bin/nu

use env.nu *
use stop.nu *

export def clean [] {
	rm -rf out
	mkdir out
	
	cargo clean
	
	enter web
		rm -rf dist .parcel-cache
	dexit 
}

export def organize_out [bin_dir = "bin"] {
	print $"Organizing all built files. bin_dir=($bin_dir)"
	
	enter out
		rm -rf slepau
		
		['auth','vreji', 'chunk', 'samn', 'media', 'gen_key', 'talebox'] | each {|a|
			echo $"Doing ($a)."
			# Make slepau dir
			mkdir $"slepau/($a)"
			
			# Don't for ...
			if $a not-in ["talebox"]  {
				echo $"Bin/docker for ($a)."
				# Copy bin
				cp $"($bin_dir)/($a)" $"slepau/($a)/"
				# Copy dockerfile
				mut dockerfile = 'x86.dockerfile'
				if $bin_dir == 'bin_armv7hf' {$dockerfile = 'armv7hf.dockerfile'}
				open $"../container/($dockerfile)" | str replace -a 'BIN_NAME' $a | save $"slepau/($a)/dockerfile"
			}
			# Don't for ...
			# if $a not-in ["setup"]  {
			# 	echo $"Web for ($a)."
			# 	mkdir $"slepau/($a)/web"
			# 	# Copy web
			# 	cp -r $"web/($a)/*" $"slepau/($a)/web/"
			# }
			
			# Copy login project
			# if $a not-in ["talebox", "setup"]  {
			# 	cp -r web/login $"slepau/($a)/web/"
			# }
		};
		
		cp -r ../config/nginx ./
		enter nginx
		
			# Don't enable https, or change logs for arm build
			if $bin_dir != bin_armv7hf {
				/bin/find ./sites -type f -print0 | xargs -0 sed -i -e 's/ 80/ 443 ssl/g'
				/bin/find ./sites -type f -print0 | xargs -0 sed -i -e 's$#KEYS$ssl_certificate /etc/letsencrypt/live/talebox.dev/fullchain.pem; # managed by Certbot\n\tssl_certificate_key /etc/letsencrypt/live/talebox.dev/privkey.pem; # managed by Certbot$g'

				/bin/find ./sites -type f -print0 | xargs -0 sed -i -E 's/#(\w+)\.access/access_log logs\/\1\-access\.log compression;/g'
			}
			
			/bin/find ./sites -type f -print0 | xargs -0 sed -i -E 's/400([0-9])/450\1/g'
			
			/bin/find ./sites -type f -print0 | xargs -0 sed -i -E 's$root .*;#TALEBOX$root /srv/http/tale_web/talebox;#TALEBOX$g'
			/bin/find ./sites -type f -print0 | xargs -0 sed -i -E 's$root .*;#GIBOS$root /srv/http/tale_web/gibos;#GIBOS$g'
			/bin/find ./sites -type f -print0 | xargs -0 sed -i -E 's$root .*;#WEB_MONO$root /srv/http/tale_web;#WEB_MONO$g'
			/bin/find ./sites -type f -print0 | xargs -0 sed -i -E 's$alias .*;#WEB_MONO$alias /srv/http/tale_web/;#WEB_MONO$g'
		
		dexit
		
		
	dexit
	
	
	
	print "Seprating done."
}

export def build_server [bin_dir:string = "bin", options = []] {
	load_env_prod
	
	rm -rf $"out/($bin_dir)"
	mkdir $"out/($bin_dir)"
	
	print $"Building binaries to out/($bin_dir)."
	# Build server
	['auth','vreji', 'chunk', 'media', 'samn', 'gen_key'] | each {|a|
		if $a not-in ["talebox"]  {
			cargo build -Zunstable-options --out-dir $"out/($bin_dir)" ...$options --release --bin $a
		}
	};
	print "Binaries built."
}
export def build_web [] {
	load_env_prod
	
	print "Building webc."
	# Build webapp
	enter web
		# MODULAR BUILD --------
		rm -rf ../out/web
		mkdir ../out/web
		rm -rf dist #.parcel-cache
		
		yarn parcel build --public-url /web --no-source-maps
		
		# ["login", "auth",'vreji', "chunk", "media"] | each {|v| 
		# 	# Build optimized
		# 	yarn parcel build --public-url /web --no-source-maps --target $v
		# };
		
		# Copy webapp to output
		cp -r dist/* ../out/web/
		
		# # MONOLITHIC BUILD ---------
		rm -rf ../out/web_mono
		# mkdir ../out/web_mono
		# rm -rf dist #.parcel-cache
		
		# ["login", "auth",'vreji', "chunk", "media", "talebox", "gibos"] | each {|v| 
		# 	let public = $"/web/($v)"
			
		# 	# Build optimized
		# 	yarn parcel build --public-url $public --no-source-maps --target $v
		# };
		
		# # Copy webapp to output
		# cp -r dist/* ../out/web_mono/
		
	dexit
	
	# Copy talebox script to download & install standalone build.
	["linux_x86_64", "musl_x86_64", "arm64", "armv7", "armv7hf"] | each {|a|
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
			
			["auth",'vreji',"chunk","media"] | each {|a|
				cp -r $"../slepau/($a)" ./
				enter $a
					ln -s ../keys keys
				dexit
			};
			
			cp -r ../../config/nginx ./
			
			cp ../../standalone_readme.md ./readme.md
			# In case i want to copy/replace inline.
			# cat ../../standalone_run.sh | sed -E $'s#standalone\.tar\.xz#standalone_($dir)\.tar\.xz#g' | save -f ./run.sh
			cp ../../standalone_run.sh ./run.sh
			
			cp -r ../web ./
			
			# enter talebox/web
			# 	ln -s $"../../../standalone_($dir).tar.xz" ./
			# dexit
		dexit
		
		print $"Compressing standalone_($dir)"
		tar -cavf $"standalone_($dir).tar.xz" $"standalone_($dir)"
	dexit
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
	
	
	
	
	# build_server bin_arm64 ["--target", "aarch64-unknown-linux-gnu"]
	# organize_out bin_arm64
	# make_standalone arm64
	
	# build_server bin_armv7 ["--target", "armv7-unknown-linux-gnueabi"]
	# organize_out bin_armv7
	# make_standalone armv7
	
	build_server bin_armv7hf ["--target", "armv7-unknown-linux-gnueabihf", "-Zbuild-std"]
	organize_out bin_armv7hf
	make_standalone armv7hf
	
	# build_server_musl
	# organize_out bin_musl_x86_64
	# make_standalone musl_x86_64
	
	
	build_server bin_linux_x86_64
	organize_out bin_linux_x86_64
	make_standalone linux_x86_64
	
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
	musl_builder nu -c $'source scripts/source.nu; build_server ($bin_dir) ["--target-dir", "target_musl"]'
	print "Done."
}

# # Makes standalone (TESTING)
# export def standalone [] {
# 	build_all
	
# 	let out = "talebox_x86_64"
	
# 	# Create out dir
# 	rm -rf $out
# 	mkdir $out
	
# 	cp out/bin/setup $"($out)/"
# 	cp standalone_readme.md $"($out)/readme.md"
	
# 	enter $out
# 		mkdir keys
# 		./setup
# 	dexit
	
# 	# Copy files
# 	['auth'] | each {|a|
# 		mkdir $"($out)/($a)"
# 		cp $"out/bin/($a)" $"($out)/($a)/"
# 		cp -r $"out/web/($a)" $"($out)/($a)/web"
		
# 		enter $"($out)/($a)"
# 			ln -s ../keys keys
# 		dexit
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
	
# 	cp target/x86_64-pc-windows-gnu/release/setup.exe $"($out)/"
# 	cp target/x86_64-pc-windows-gnu/release/auth.exe $"($out)/auth/"
	
# 	tar -cavf $"($out).zip" $out
# }