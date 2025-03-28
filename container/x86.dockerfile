from archlinux

run pacman-key --init
run pacman -Sy --noconfirm archlinux-keyring
run pacman -Syu --noconfirm

run pacman -Sy --noconfirm --needed openssl ffmpeg
run pacman -S --noconfirm --needed tzdata

workdir /server
cmd ["./BIN_NAME"]
expose 4000

copy ./BIN_NAME ./
