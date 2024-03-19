from alpine

RUN apk add --no-cache nginx nginx-mod-http-brotli

cmd ["nginx", "-g", "daemon off;"]
expose 80

copy ./* /etc/nginx/
copy ./web/* /srv/http/tale_web/