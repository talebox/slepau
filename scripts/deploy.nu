#!/bin/nu

use build.nu *
use start.nu test

export def deploy_sites [host = 'anty.dev'] {
	print $"Deploying static sites to ($host)."
	rsync -av $"out/web/" $"root@($host):/srv/http/tale_web/"
	
	print $"Deploying standalone compressed builds to ($host)."
	rsync -av out/*.tar.xz $"root@($host):/srv/http/tale_web/talebox/"
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
	
	deploy_sites
	deploy_nginx
	
	print "Deploy finished!"
}