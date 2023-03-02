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

export def deploy_static [name] {
	# Building was disabled cause build command already correctly build static targets

	# Build webapp
	# enter web
		# Remove cache/build dirs
		# rm -rf dist/talebox
		# rm -rf .parcel-cache
		# Build optimized
		# yarn parcel build --no-source-maps --target $name
	# exit
	# scp $"web/dist/($name)/*" $"anty.dev:/srv/http/($name)/" 
	
	scp $"out/web/($name)/*" $"anty.dev:/srv/http/($name)/" 
}

export def deploy_all [] {
	# Deploy slepau first
	test
	build
	
	deploy auth
	deploy chunk
	deploy media
	
	deploy_talebox
}