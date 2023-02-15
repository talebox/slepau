#!/bin/nu

use build.nu *
use start.nu test

# export def deploy [] {
# 	test
# 	build
# 	docker context use anty
# 	do -i {docker stop chunk_s}
# 	do -i {docker rm chunk_s}
# 	docker build -t chunk ./container
# 	docker volume create -d local chunk_data
# 	docker volume create -d local chunk_backup
# 	docker run -dp 4500:4000 -v chunk_data:/server/data -v chunk_backup:/server/backup --name chunk_s chunk
# }

export def deploy_auth [] {
	test
	build
	
	enter container
		docker context use anty
		do -i {docker stop auth_s}
		do -i {docker rm auth_s}
		docker build -t auth -f Auth.dockerfile .
		docker volume create -d local auth_data
		docker volume create -d local auth_backup
		docker run -dp 4501:4000 -v auth_data:/server/data -v auth_backup:/server/backup --name auth_s auth
	exit
	
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