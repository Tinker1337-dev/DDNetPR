#!/bin/bash
set -e
cd "$(dirname "$0")"
echo "=== Мастер настройки DDNetPR v11.7.0 LTS ==="
read -rp "Название сервера [DDNetPR Server v11.7]: " SNAME
SNAME=${SNAME:-"DDNetPR Server v11.7"}
read -rp "Порт: " SPORT
SPORT=${SPORT:-27015}
while [ -z "$RCON" ]; do read -rsp "RCON-пароль: " RCON; echo ""; done
read -rp "Карта [BlmapChill]: " SMAP
SMAP=${SMAP:-"BlmapChill"}
mkdir -p ./plugins_disabled
for pl in plugin_city plugin_potato; do
  read -rp "Включить ${pl}? [Y/n]: " A
  [[ "${A:-Y}" =~ ^[Nn]$ ]] && [ -f "./plugins/${pl}" ] && mv "./plugins/${pl}" "./plugins_disabled/${pl}"
done
sed -i -e "s/^sv_name .*/sv_name \"${SNAME}\"/" -e "s/^sv_hostname .*/sv_hostname \"${SNAME}\"/" \
       -e "s/^sv_port .*/sv_port ${SPORT}/" -e "s/^sv_rcon_password .*/sv_rcon_password \"${RCON}\"/" \
       -e "s/^sv_map .*/sv_map \"${SMAP}\"/" autoexec_server.cfg
echo "Готово! Запускайте сервер: ./start.sh"
