from archlinux

run pacman -Sy --noconfirm openssl rethinkdb

workdir /server

env CACHE_PATH=data/cache.json 
env DB_PATH=data/db.json 
env DB_BACKUP_FOLDER=backup
env URL=https://chunk.anty.dev

expose 4000

cmd ["./chunk"]