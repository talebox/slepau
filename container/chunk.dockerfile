from archlinux

run pacman -Sy --noconfirm openssl

workdir /server
cmd ["./chunk"]
expose 4000

env CACHE_PATH=data/cache.json 
env DB_PATH=data/db.json 
env DB_BACKUP_FOLDER=backup

copy ./chunk ./
copy ./web ./web