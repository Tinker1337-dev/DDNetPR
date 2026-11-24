# DDNetPR v11.7.0 LTS — Server-First Plugin Platform
Автор: **Tinker1337** | Совместимость: стандартные клиенты **DDNet 0.6 (20.x)** без модификаций.

## Состав платформы
- **`DDNet-Server`** — ядро C++ с IPC-логами событий, RCON-управлением через FIFO и пулами `OnSnap` (визуальные пикапы, лазеры, векторный текст, ти, персональные скины).
- **`ddnet_plugins`** — асинхронный менеджер плагинов на Rust (изоляция визуальных пулов, Rate-Limit, Watchdog 12с, горячий релоад без кика игроков, KV-база данных `plugins_kv.db`).
- **`ddnet_sdk` v11.0.0** — Rust SDK с макросом `ddnet_sdk::plugin!`, типизированным `ServerApi`, таймерами и spatial-математикой.

## Вариант 1: Быстрый старт из готового архива (`ddnetpr-v11.7.0-lts-linux-x86_64.tar.gz`)
1. Установите системные зависимости:
   ```bash
   sudo apt update && sudo apt install -y screen python3 libsqlite3-0 libsdl2-2.0-0 libssl3 libcurl4 zlib1g
   ```
2. Распакуйте архив в любую папку (например `/opt/ddnetpr`):
   ```bash
   mkdir -p /opt/ddnetpr && tar -xzf ddnetpr-v11.7.0-lts-linux-x86_64.tar.gz --strip-components=1 -C /opt/ddnetpr
   cd /opt/ddnetpr
   ```
3. Запустите мастер настройки `./setup.sh` (имя, порт, RCON-пароль, карта и выбор плагинов).
4. Запустите сервер: `./start.sh` (остановка: `./stop.sh`).
5. В игре зарегистрируйте аккаунт `/register <логин> <пароль> <пароль>` и выдайте себе права владельца: `!auth_admin <rcon_пароль>`.

## Вариант 2: Сборка из исходников (`ddnetpr-v11.7.0-lts.tar.gz`)
См. `00_INSTALL_FROM_SCRATCH.md`.
