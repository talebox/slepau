#!/bin/env sh
echo "Launching everything:"


sh -c "cd auth; URL='http://auth.talebox.local:8080' SOCKET='0.0.0.0:4001' ./auth" &
sh -c "cd chunk; URL='http://chunk.talebox.local:8080' SOCKET='0.0.0.0:4002' ./chunk" &
sh -c "cd media; URL='http://media.talebox.local:8080' SOCKET='0.0.0.0:4003' ./media" &

sleep 1s

export WEB_DIR="$(pwd)/web"
export NGINX_AS_USER=$(whoami)

sh -c "cd nginx; chmod +x nginx.sh; ./nginx.sh"

pkill -P $$