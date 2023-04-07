#!/bin/env sh

# The standalone run script.



if [ ! -z "$WEB_DIR" ]; then
	echo "Setting web root to $WEB_DIR";
	/bin/find ./sites -type f -print0 | xargs -0 sed -i -E "s%root .*;#TALEBOX%root $WEB_DIR/talebox;#TALEBOX%g"
	/bin/find ./sites -type f -print0 | xargs -0 sed -i -E "s%root .*;#GIBOS%root $WEB_DIR/gibos;#GIBOS%g"
	/bin/find ./sites -type f -print0 | xargs -0 sed -i -E "s%root .*;#WEB_MONO%root $WEB_DIR;#WEB_MONO%g"
	/bin/find ./sites -type f -print0 | xargs -0 sed -i -E "s%alias .*;#WEB_MONO%alias $WEB_DIR/;#WEB_MONO%g"
fi

MESSAGE=$'\n\n\tTalebox started -> http://talebox.local\n\n'

sudo sh -c "echo \"$MESSAGE\" && nginx -g \"daemon off;pid /tmp/nginx.pid;user ${NGINX_AS_USER:-$(whoami)};\" -p \"$(pwd)\" -c nginx.conf"