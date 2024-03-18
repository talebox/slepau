from agners/archlinuxarm

run pacman-key --init
run pacman-key --populate archlinuxarm
run pacman -Syu --noconfirm --ignore filesystem

run pacman -Sy --noconfirm --needed openssl ffmpeg

workdir /server
cmd ["./BIN_NAME"]
expose 4000

copy ./BIN_NAME ./
