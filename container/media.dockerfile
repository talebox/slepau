from archlinux:latest

run pacman-key --init
run pacman -Sy --noconfirm archlinux-keyring
run pacman -Syu --noconfirm

run pacman -Sy --noconfirm openssl ffmpeg

workdir /server
cmd ["./media"]
expose 4000

env CACHE_FOLDER=data/media_cache
env MEDIA_FOLDER=data/media

copy ./media ./
# copy ./web ./web