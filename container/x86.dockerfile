from archlinux

run pacman-key --init
run pacman-key --populate archlinux
run pacman -Syu --noconfirm

run pacman -Sy --noconfirm --needed openssl ffmpeg

workdir /server
cmd ["./BIN_NAME"]
expose 4000

copy ./BIN_NAME ./
