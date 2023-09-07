from archlinux

run pacman-key --init
run pacman -Sy --noconfirm archlinux-keyring
run pacman -Syu --noconfirm

run pacman -Sy --noconfirm openssl

workdir /server
cmd ["./vreji"]
expose 4000


copy ./vreji ./
