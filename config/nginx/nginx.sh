#!/bin/env sh

# The standalone run script.



if [ ! -z "$TALEBOX_DIR" ]; then
	echo "Setting talebox root to $TALEBOX_DIR";
	/bin/find . -type f -name "talebox.conf" -print0 | xargs -0 sed -i -E "s%root .*;%root $TALEBOX_DIR;%g"
fi

MESSAGE=$'\n\n\tTalebox started -> http://talebox.local\n\n'

sudo sh -c "echo \"$MESSAGE\" && nginx -g \"daemon off;pid /tmp/nginx.pid;user ${NGINX_AS_USER:-$(whoami)};\" -p \"$(pwd)\" -c nginx.conf"

fg