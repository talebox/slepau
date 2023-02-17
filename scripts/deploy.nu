#!/bin/nu

use build.nu *
use start.nu test

# Creates a volume for keys at docker
export def deploy_keys [] {
	docker context use anty
	docker volume create -d local talebox_keys
	docker build -t keys -f keys.dockerfile .
	docker run -v talebox_keys:/server/keys --name keys_s keys
}

export def deploy [name] {
	docker context use anty
	do -i {docker stop $"($name)_s"}
	do -i {docker rm $"($name)_s"}
	docker build -t $name $"./out/slepau/($name)" 
	docker volume create -d local talebox_keys
	docker volume create -d local $"($name)_data"
	docker volume create -d local $"($name)_backup"
	let ports = {
		"auth": 4501,
		"chunk": 4500,
		"media": 4502,
	}
	docker run -dp $"($ports | get $name):4000" -v talebox_keys:/server/keys -v  $"($name)_data:/server/data" -v $"($name)_backup:/server/backup" --name $"($name)_s" $name
}

export def deploy_talebox [] {
	
	# Build webapp
	enter web
		# Remove cache/build dirs
		rm -rf dist
		# rm -rf .parcel-cache
		# Build optimized
		yarn parcel build --no-source-maps --target talebox
	exit
	
	scp web/dist/talebox/* anty.dev:/srv/http/talebox/
}

export def deploy_all [] {
	# Deploy slepau first
	test
	build
	
	deploy_auth
	deploy_chunk
	deploy_talebox
	deploy_media
	
	# ['auth', 'chunk', 'talebox', 'media'] | each {|a|
	# }
	
}