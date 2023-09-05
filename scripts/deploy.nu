#!/bin/nu

use build.nu *
use start.nu test

# Creates a volume for keys at docker
export def deploy_keys [] {
	print $"Deploying docker keys, with anty context."
	docker context use anty
	docker volume create -d local talebox_keys
	docker build -t keys -f keys.dockerfile .
	docker run -v talebox_keys:/server/keys --name keys_s keys
	
	print "Done."
}

export def deploy_docker [name] {
	print $"Deploying docker container '($name)', with anty context."
	docker context use anty
	do -i {docker stop $"($name)_s"}
	do -i {docker rm $"($name)_s"}
	docker build -t $name $"./out/slepau/($name)" 
	docker volume create -d local $"($name)_data"
	docker volume create -d local $"($name)_backup"
	let ports = {
		"auth": 4501,
		"chunk": 4502,
		"media": 4503,
		"vreji": 4504,
	}
	docker run -d --restart unless-stopped -p $"($ports | get $name):4000" -v talebox_keys:/server/keys -v vreji_db:/server/vreji -v  $"($name)_data:/server/data" -v $"($name)_backup:/server/backup" -e $"URL=https://($name).anty.dev" --name $"($name)_s" $name
	
	print "Done."
}

export def deploy_static [name] {
	
	print $"Deploying static site '($name)'."
	if $name == "tale_web" {
		rsync -av $"out/web/*" $"anty.dev:/srv/http/($name)/"
	} else {
		rsync -av $"out/web/($name)/*" $"anty.dev:/srv/http/($name)/"
		if $name in ["talebox"] {
			rsync -av out/*.tar.xz $"anty.dev:/srv/http/($name)/"
		}
		print "Done."
	}
	
}

export def deploy_nginx [] {
	print "Deploying nginx to root@anty.dev"
	rsync -av out/nginx/sites/* root@anty.dev:/etc/nginx/sites/
	print "Restarting nginx."
	ssh root@anty.dev systemctl restart nginx
	print "Done."
}

export def deploy_all [] {
	print "Deploying slepau + nginx."
	
	deploy_docker auth
	deploy_docker chunk
	deploy_docker media
	deploy_docker vreji
	
	deploy_static tale_web
	deploy_static talebox
	deploy_static gibos
	
	deploy_nginx
	
	print "Done."
}