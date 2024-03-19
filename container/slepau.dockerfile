from alpine

RUN apk add --no-cache openssl ffmpeg

workdir /server
cmd ["./BIN_NAME"]
expose 4000

copy ./BIN_NAME ./
