from archlinux

run pacman -Sy --noconfirm openssl

workdir /server

copy ./media ./
copy ./web ./web

env CACHE_PATH=data/cache.json
env MEDIA_FOLDER=data/media
env DB_PATH=data/db.json 
env DB_BACKUP_FOLDER=backup
env URL=https://media.anty.dev

expose 4000

cmd ["./media"]