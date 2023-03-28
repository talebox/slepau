hash nginx 2>/dev/null || { echo >&2 "I require nginx but it's not installed. Maybe run 'apt install nginx'?  Aborting."; exit 1; }

wget -N https://talebox.anty.dev/standalone.tar.xz
tar -xpaf standalone.tar.xz

cd standalone

	./gen_key
	chmod -x run.sh
	./run.sh
	
cd ..