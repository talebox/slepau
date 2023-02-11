from archlinux

run pacman -Sy --noconfirm openssl

workdir /server

copy ./bin/auth ./
copy ./web/auth ./web
copy ./keys ./keys

env CACHE_PATH=data/cache.json 
env DB_PATH=data/db.json 
env DB_BACKUP_FOLDER=backup
env URL=https://auth.anty.dev

expose 4000

cmd ["./auth"]