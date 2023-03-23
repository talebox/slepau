echo "Launching everything:"


sh -c "cd auth; URL='http://auth.local:8080' SOCKET='0.0.0.0:4001' ./auth" &
sh -c "cd chunk; URL='http://chunk.local:8080' SOCKET='0.0.0.0:4002' ./chunk" &
sh -c "cd media; URL='http://media.local:8080' SOCKET='0.0.0.0:4003' ./media" &

sleep 1s

echo "We're using domain '*.local', you should have this in your /etc/hosts file already '127.0.0.1 auth.local chunk.local media.local' so those domains are resolved to the loopback ip 127.0.0.1"

echo "But nginx is setup to handle any domain you want without any config changes here. So using something other than '*.local' would also work, just make sure it begins with 'auth.' 'media.' etc.... Have fun :)"

sh -c "cd nginx; chmod +x nginx.sh; ./nginx.sh"