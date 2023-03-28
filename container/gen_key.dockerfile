from archlinux

run pacman -Sy --noconfirm openssl

workdir /server
cmd ["./gen_key"]

copy ./gen_key ./