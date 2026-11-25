use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Трейт конфигурации плагина, реализуемый макросом `plugin_config!`.
pub trait PluginConfig: Default + Clone + 'static {
    /// Сериализует текущие значения с комментариями в формат `.cfg`.
    fn to_cfg_string(&self) -> String;
    /// Разбирает пары `key = value` в структуру конфигурации, подставляя значения по умолчанию для отсутствующих ключей.
    fn from_cfg_str(content: &str) -> (Self, bool);
    /// Загружает `configs/<plugin_name>.cfg`, создавая файл или дополняя его недостающими ключами по умолчанию.
    fn load_or_create(plugin_name: &str) -> Self {
        let dir = match std::env::var("DDNET_SERVER_DIR") {
            Ok(base) if !base.trim().is_empty() => PathBuf::from(base).join("configs"),
            _ => PathBuf::from("configs"),
        };
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(format!("{}.cfg", plugin_name));
        match fs::read_to_string(&path) {
            Ok(raw) => {
                let (cfg, missing) = Self::from_cfg_str(&raw);
                if missing {
                    let _ = fs::write(&path, cfg.to_cfg_string());
                }
                cfg
            }
            Err(_) => {
                let cfg = Self::default();
                let _ = fs::write(&path, cfg.to_cfg_string());
                cfg
            }
        }
    }
}

/// Разбирает строки `.cfg` в карту ключ-значение, игнорируя комментарии `#` и `//`.
pub fn parse_cfg_map(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") { continue; }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim().to_string();
            let val = v.trim().trim_matches('"').to_string();
            if !key.is_empty() { map.insert(key, val); }
        }
    }
    map
}

/// Определяет структуру конфигурации плагина с автогенерацией сериализации в `.cfg`.
#[macro_export]
macro_rules! plugin_config {
    ( $(#[$m:meta])* $v:vis struct $n:ident { $( $(#[doc = $d:expr])* $fv:vis $f:ident : $t:ty = $def:expr ),* $(,)? } ) => {
        $(#[$m])* #[derive(Clone, Debug)] $v struct $n { $( $(#[doc = $d])* $fv $f : $t, )* }
        impl Default for $n { fn default() -> Self { Self { $( $f: $def, )* } } }
        impl $crate::config::PluginConfig for $n {
            fn to_cfg_string(&self) -> String {
                let mut out = String::new();
                $( $( out.push_str(&format!("#{}\n", $d)); )* out.push_str(&format!("{} = {}\n\n", stringify!($f), self.$f)); )*
                out
            }
            fn from_cfg_str(c: &str) -> (Self, bool) {
                let m = $crate::config::parse_cfg_map(c); let def = Self::default(); let mut miss = false;
                (Self { $( $f: m.get(stringify!($f)).and_then(|r| r.parse::<$t>().ok()).unwrap_or_else(|| { miss = true; def.$f }), )* }, miss)
            }
        }
    };
}
