from archlinux

run pacman-key --init
run pacman -Sy --noconfirm archlinux-keyring
run pacman -Sy --noconfirm openssl ffmpeg

workdir /server
cmd ["./media"]
expose 4000

env CACHE_PATH=data/cache.json
env CACHE_FOLDER=data/media_cache
env MEDIA_FOLDER=data/media
env DB_PATH=data/db.json 
env DB_BACKUP_FOLDER=backup
env URL=https://media.anty.dev

copy ./media ./
copy ./web ./web