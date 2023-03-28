#!/bin/env sh

# The standalone run script.

nginx -g 'daemon off;pid /tmp/nginx.pid;' -p "$(pwd)" -c nginx.conf