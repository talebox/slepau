from archlinux

run pacman -Sy --noconfirm openssl

workdir /server

copy ./dist ./
copy ./keys ./keys

expose 4000

env RUST_LOG=info  CACHE_PATH=data/cache.json  DB_PATH=data/db.json DB_BACKUP_FOLDER=backup  MEDIA_FOLDER=data/media

cmd ["./chunk-app"]