# Установка DDNetPR v11.7.0 LTS из исходников (Ubuntu/Debian)

## 1. Зависимости и Rust
```bash
sudo apt update && sudo apt install -y build-essential cmake ninja-build git screen python3 libssl-dev libsqlite3-dev libcurl4-openssl-dev libpng-dev zlib1g-dev
curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source "\$HOME/.cargo/env"
```

## 2. Сборка C++ ядра DDNetPR
```bash
git clone --recursive https://github.com/DDNetPP/DDNetPP.git DDNetPP
cd DDNetPP && git checkout 4e458fa81 && git submodule update --init --recursive && git apply ../core/ddnetpr_core_v11.7_lts.patch
cmake -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCLIENT=OFF -DSERVER=ON
cmake --build build -j\$(nproc)
```

## 3. Сборка менеджера плагинов и плагинов
```bash
cd ../crates/ddnet_plugins && cargo build --release
```
Для создания нового плагина используйте `./crates/ddnet-plugin-template/new-plugin.sh <name>` и собирайте через `cargo build --release`.
