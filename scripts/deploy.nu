#!/bin/nu

use build.nu *
use start.nu test

export def deploy [] {
	
	test
	build

	docker context use anty
	do -i {docker stop chunk_s}
	do -i {docker rm chunk_s}
	docker build -t chunk ./container
	docker volume create -d local chunk_data
	docker volume create -d local chunk_backup
	docker run -dp 4500:4000 -v chunk_data:/server/data -v chunk_backup:/server/backup --name chunk_s chunk
}


def main [] {
	deploy
}