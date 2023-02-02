#!/bin/sh

# I have to do this background spawning in the sh shell because nushell doesn't have this 'feature'.

cd web
	# rm -rf .parcel-cache dist
	nohup yarn parcel watch --public-url /web --log-level warn &>start.log &
cd ..
nohup cargo watch -w src -qN -x r &>start.log &

echo "Started cargo & parcel in background"

