#!/bin/nu

use build.nu *
use start.nu test

# Creates a volume for keys at docker
export def deploy_keys [] {
	print $"Deploying docker keys."
	let context = (docker context show)
	if $context not-in ['rpi', 'anty'] {
		print "Context isn't rpi or anty, make sure it's right"
		return;
	}
	
	docker volume create -d local talebox_keys
	docker build -t keys ./out/slepau/gen_key
	docker run -v talebox_keys:/server/keys --name keys_s keys
	
	print "Done."
}

export def deploy_docker [name] {
	let context = (docker context show)
	if $context not-in ['rpi', 'anty'] {
		print "Context isn't rpi or anty, make sure it's right"
		return;
	}
	let ports = {
		"auth": 4501,
		"chunk": 4502,
		"media": 4503,
		"vreji": 4504,
		"samn": 4505,
	}
	if $name not-in $ports {
		print $"'($name)' doesn't exist in deploy_docker.";
		return;
	}
	print $"Deploying docker container '($name)', with '($context)' context."
	docker build -t $name $"./out/slepau/($name)" 
	docker volume create -d local $"($name)_data"
	docker volume create -d local $"($name)_backup"
	do -i {docker stop $"($name)_s"}
	do -i {docker rm $"($name)_s"}
	mut args = [
		"-d", # Deamonize
		"--restart", "unless-stopped",
		"-p", $"127.0.0.1:($ports | get $name):4000", # Bind outside localhost:port to container's 4000 port
		"-v", "talebox_keys:/server/keys", # Keys
		"-v", "vreji_db:/server/vreji_db", # Vreji (Logging)
		"-v", $"($name)_data:/server/data", # Data
		"-v", $"($name)_backup:/server/backup", # Backup
		"-e", $"URL=https://($name).anty.dev", # URL variable
		"--env-file=container/env.config", # Env config
	];

	# If we're deploying samn, make sure it has access to these devices
	if $name == "samn" {$args = ($args | append "--device=/dev/spidev0.0" | append "--device=/dev/gpiochip0")}

	docker run $args --name $"($name)_s" $name
	
	print "Done."
	
}

export def deploy_static [name, host = 'anty.dev'] {
	
	print $"Deploying static site '($name)'."
	if $name == "tale_web" {
		rsync -av $"out/web/*" $"($host):/srv/http/($name)/"
	} else {
		rsync -av $"out/web/($name)/*" $"($host):/srv/http/($name)/"
		if $name in ["talebox"] {
			rsync -av out/*.tar.xz $"($host):/srv/http/($name)/"
		}
		print "Done."
	}
	
}

export def deploy_nginx [host = 'anty.dev'] {
	print $"Deploying nginx to root@($host)"
	rsync -av out/nginx/sites/* $"root@($host):/etc/nginx/sites/"
	print "Restarting nginx."
	ssh $"root@($host)" systemctl restart nginx
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
	
	print "Deploy finished!"
}