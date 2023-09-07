#!/bin/env sh
hash nginx 2>/dev/null || { echo >&2 "I require 'nginx' but it's not installed. Install it before we keep going.  Aborting."; exit 1; }
hash ffmpeg 2>/dev/null || { echo >&2 "I require 'ffmpeg' but it's not installed. Install it before we keep going.  Aborting."; exit 1; }

wget -N https://talebox.dev/standalone.tar.xz
tar -xpaf standalone.tar.xz

cd standalone
	./gen_key
	chmod +x ./run.sh
cd ..


HOSTS_CONFIG="127.0.0.1 auth.talebox.local vreji.talebox.local chunk.talebox.local media.talebox.local talebox.local"

echo "We're using domain '*.talebox.local', you should have this in your /etc/hosts file already '($HOSTS_CONFIG)' so those domains are resolved to the loopback ip 127.0.0.1"

echo "As a first time script, we'll check for a specific line in hosts and add it if it doesn't exist."
if [[ ! -z $(grep "$HOSTS_CONFIG" "/etc/hosts") ]]; then 
	echo "Found /etc/hosts line, good :), doing nothing."
else
	echo "/etc/hosts line not found, adding line at the end, we need root access."
	echo "$HOSTS_CONFIG\n" | sudo tee -a /etc/hosts
fi

echo "But nginx is setup to handle any domain you want without any config changes here. So using something other than '*.talebox.local' would also work, just make sure it begins with 'auth.' 'media.' etc.... Have fun :)"

cd standalone
	./run.sh
cd ..