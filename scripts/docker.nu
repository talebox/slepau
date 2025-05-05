def get_context [] {
	let context = (docker context show)
	if $context not-in ['anty', 'rpi', 'pi_local', 'nerd', 'geek'] {
		error make {msg: $"Context '($context)' isn't rpi or anty, make sure it's right"}
	} else  {
        print $"Using '($context)' context."    
    }
	$context
}

def docker_args [name] {
    let context = get_context
    
    let ports = {
		"auth": 4501,
		"chunk": 4502,
		"media": 4503,
		"vreji": 4504,
		"samn": 4505,
		"lasna": 4506,
	}
	if $name not-in $ports {
		error make {msg: $"'($name)' doesn't exist in deploy_docker."}
	}
    mut args = [
		"-p", $"127.0.0.1:($ports | get $name):4000", # Bind outside localhost:port to container's 4000 port
		"-v", "talebox_keys:/server/keys", # Keys
		"-v", "vreji_db:/server/vreji_db", # Vreji (Logging)
		"-v", $"($name)_data:/server/data", # Data
		"-v", $"($name)_backup:/server/backup", # Backup
		# Maybe we should remove URL env altogether since it's only being used for user info purposes...
		"-e", $"URL=(if $context in ['rpi','pi_local', 'nerd', 'geek'] {'http'} else {'https'})://($name).anty.dev", # URL variable
		"--env-file=container/env.config", # Env config
	];

    # If it's samn, make sure it has access to these devices and set a few env variables
	if $name == "samn" and $context in ['rpi', 'pi_local', 'nerd', 'geek'] {
		$args = ($args | append [
			-v "samn_db:/server/samn_db" # Samn (Node Logging)
			--device=/dev/spidev0.0
			--device=/dev/spidev0.1
			--device=/dev/gpiochip0
			-e DB_PATH_LOG=samn_db
			-e RADIO=on
			-e RUST_BACKTRACE=1
			-e TZ=America/New_York
			# -v /etc/localtime:/etc/localtime:ro
		])
		if $context == 'nerd' {
			$args = ($args | append [
				# Modify radio io lines bc it's not connected the same way as rpi
				-e CC1101_G2_LINE=23
				# -e NRF24_CE_LINE=23
				# -e NRF24_IRQ_LINE=23 
			])
		}
	}

	if $name == "lasna" {
		if $context == "anty" {
			# Server config
			$args = ($args | append [
			
			-p "7000:7000"
			-p "127.0.0.1:7001:7001"
			-e LASNA_MODE=server
			-e LOG_LEVEL=trace
		])} else {
			# Device config
			$args = ($args | append [
				--add-host=host.docker.internal:host-gateway
				-e DEVICE_ID=lasob
			# -p "7000:7000"
			# -p "127.0.0.1:7001:7001"
			-e LASNA_MODE=talebox.dev 
			-e LOCAL_ADDR=host.docker.internal:80
		])}
	}
	

    return $args
}

# Runs a certain container
export def run_docker [name, build = true, ...cmd] {
    mut args = docker_args $name

    # Build the container
	if $build {
		docker build -t $name $"./out/slepau/($name)" 
		
	}
	# Stop a previous, named container
	do -i {docker stop $"($name)_s"}

    docker run ...$args $name ...$cmd
}

# Deploys and demonizes a certain container
export def deploy_docker [name, build_opts = [], run_opts = []] {
	mut args = docker_args $name

	docker build -t $name ...$build_opts $"./out/slepau/($name)" 
	docker volume create -d local $"($name)_data"
	docker volume create -d local $"($name)_backup"
	do -i {docker stop $"($name)_s"}
	do -i {docker rm $"($name)_s"}
    $args = ($args | append [
			-d # Deamonize
			--restart unless-stopped
		]);

	docker run ...$args ...$run_opts --name $"($name)_s" $name

	print "Done."
}

# Creates a volume for keys at docker
export def docker_setup [] {
	get_context
	print $"Deploying docker setup."
	
	docker volume create -d local talebox_keys
	docker volume create -d local vreji_db
	docker volume create -d local samn_db
	docker build -t gen_key ./out/slepau/gen_key
	docker run -v talebox_keys:/server/keys gen_key
	docker run -v talebox_keys:/server/keys -v vreji_db:/server/vreji_db -v samn_db:/server/samn_db gen_key sh -c 'touch vreji_db/main && touch samn_db/main'
	print "Done."
}