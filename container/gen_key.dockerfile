from archlinux

run pacman -Sy --noconfirm openssl

workdir /server

copy ./gen_key ./

cmd ["./gen_key"]