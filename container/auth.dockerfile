from archlinux

run pacman -Sy --noconfirm openssl

workdir /server

copy ./auth ./
copy ./web ./web

env CACHE_PATH=data/cache.json 
env DB_PATH=data/db.json 
env DB_BACKUP_FOLDER=backup
env URL=https://auth.anty.dev

expose 4000

cmd ["./auth"]