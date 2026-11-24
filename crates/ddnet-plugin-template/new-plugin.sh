#!/usr/bin/env bash
set -euo pipefail
NAME="${1:-}"
if [[ -z "$NAME" || ! "$NAME" =~ ^[a-z0-9_]+$ ]]; then
  echo "Использование: $0 <plugin_name> (только строчные латинские буквы, цифры и _)"
  exit 1
fi
DEST="/root/plugin_${NAME}"
if [[ -e "$DEST" ]]; then
  echo "Ошибка: $DEST уже существует!"
  exit 1
fi
mkdir -p "$DEST/src"
sed "s/template/${NAME}/g" /root/ddnet-plugin-template/Cargo.toml > "$DEST/Cargo.toml"
sed -e "s/template/${NAME}/g" -e "s/Template/${NAME^}/g" /root/ddnet-plugin-template/src/main.rs > "$DEST/src/main.rs"
echo "Создан новый плагин: $DEST"
echo "Деплой в продакшен: ddnet-plugin-update plugin_${NAME}"
