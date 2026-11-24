# Установка DDNetPR v11.7.0 LTS из исходников (Ubuntu/Debian)

## 1. Зависимости и Rust
```bash
sudo apt update && sudo apt install -y build-essential cmake ninja-build git screen python3 libssl-dev libsqlite3-dev libcurl4-openssl-dev zlib1g-dev
curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source "\$HOME/.cargo/env"
```

## 2. Сборка C++ ядра DDNetPR
```bash
git clone --recursive https://github.com DDNetPP
cd DDNetPP && git apply ../02_core_patch/ddnetpr_core_v11.7_lts.patch
cmake -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCLIENT=OFF -DSERVER=ON
cmake --build build -j\$(nproc)
```

## 3. Сборка менеджера плагинов и плагинов
```bash
cd ../03_sdk_and_manager/ddnet_plugins && cargo build --release
```
Для создания нового плагина используйте `./03_sdk_and_manager/ddnet-plugin-template/new-plugin.sh <name>` и собирайте через `cargo build --release`.
