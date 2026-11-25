#![warn(missing_docs)]
//! SDK платформы плагинов DDNetPR / DDNet++ (v11.0.0 LTS).

/// Типизированные DX-примитивы, ошибки и реэкспорты prelude.
pub mod config;
pub use config::PluginConfig;
/// Диспетчер событий на основе трейта EventHandler.
pub mod handler;
pub use handler::EventHandler;
/// Вспомогательные пространственные запросы поверх кэша позиций игроков.
pub mod spatial;
/// Асинхронный файловый логгер (`logs/<plugin>.log`).
pub mod log;
/// Типизированные DX-примитивы, ошибки и реэкспорты prelude.
pub mod dx;

pub use dx::prelude;

pub use dx::{PlayerId, RayHit, RayMask, SdkError, Vec2, Weapon};

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::thread;

/// Возвращает строку версии API SDK (`"11.0.0"`).
///
/// # Примеры
/// ```
/// assert_eq!(ddnet_sdk::plugin_api_version(), "11.0.0");
/// ```
pub fn plugin_api_version() -> &'static str {
    "11.0.0"
}

/// Приоритет выполнения плагина.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    /// Высокий приоритет (выполняется первым).
    High,
    /// Обычный приоритет по умолчанию.
    Normal,
    /// Низкий приоритет (выполняется последним).
    Low,
}

/// Действие, возвращаемое перехватывающими обработчиками событий.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventAction {
    /// Передать событие дальше в ядро и другим плагинам.
    Pass,
    /// Устаревший псевдоним v9.2 для Pass.
    Continue,
    /// Отменить дальнейшую обработку события.
    Cancel,
}

/// Снимок позиции и скорости игрока из кэша.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerPos {
    /// Мировая координата X в пикселях.
    pub x: f32,
    /// Мировая координата Y в пикселях.
    pub y: f32,
    /// Горизонтальная скорость.
    pub vx: f32,
    /// Вертикальная скорость.
    pub vy: f32,
    /// Жив ли персонаж игрока в данный момент.
    pub alive: bool,
    /// Оставшиеся тики заморозки.
    pub frozen: i32,
    /// Идентификатор активного оружия.
    pub weapon: i32,
}

/// События сервера и менеджера, доставляемые плагинам.
#[derive(Clone, Debug)]
pub enum Event {
    /// Игрок зашёл на сервер.
    Join {
        /// Никнейм игрока.
        player: String,
    },
    /// Игрок вышел с сервера.
    Leave {
        /// Никнейм игрока.
        player: String,
    },
    /// Игрок сменил никнейм.
    Rename {
        /// Предыдущий никнейм.
        old: String,
        /// Новый никнейм.
        new: String,
        /// Устаревшее поле нового никнейма.
        new_name: String,
    },
    /// Игрок сменил команду (`0` = Red, `1` = Blue, `-1` = Наблюдатель).
    TeamChange {
        /// Идентификатор клиента игрока (`0..63`).
        cid: i32,
        /// Никнейм игрока.
        player: String,
        /// Идентификатор команды (`0` = Red, `1` = Blue, `-1` = Наблюдатель).
        team: i32,
    },
    /// Получено сообщение в чате или чат-команда.
    Chat {
        /// Канал чата (`0` = общий).
        team: i32,
        /// Никнейм отправителя.
        player: String,
        /// Исходный текст сообщения.
        message: String,
    },
    /// Получено рукопожатие клиента BestClient.
    BestClient {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Строка версии клиента.
        version: String,
    },
    /// Игрок авторизовался через систему аккаунтов.
    Auth {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Никнейм игрока.
        player: String,
        /// Логин аккаунта.
        account: String,
    },
    /// Обновлена роль аккаунта.
    Role {
        /// Логин аккаунта.
        account: String,
        /// Назначенное имя роли.
        role: String,
    },
    /// Периодический серверный тик раз в секунду.
    Tick {
        /// Аптайм сервера в секундах.
        uptime_sec: u64,
    },
    /// Высокочастотный тик каждые 100 мс.
    FastTick {
        /// Аптайм сервера в миллисекундах.
        uptime_ms: u64,
    },
    /// Сработал запланированный таймер.
    Scheduled {
        /// Идентификатор таймера, заданный плагином.
        callback_id: u64,
    },
    /// Получено межплагинное сообщение.
    Ipc {
        /// Имя плагина-отправителя.
        from: String,
        /// Строка полезных данных.
        payload: String,
    },
    /// Ответ от key-value базы данных.
    DbValue {
        /// Идентификатор запроса для сопоставления ответа.
        req_id: String,
        /// Возвращённое строковое значение.
        value: String,
    },
    /// Ответ списка записей из БД по префиксу.
    DbList {
        /// Идентификатор запроса для сопоставления ответа.
        req_id: String,
        /// Разобранные пары `(ключ, значение)`.
        items: Vec<(String, String)>,
    },
    /// Снимок управления и действий игрока.
    Input {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Направление по горизонтали (`-1`, `0`, `1`).
        dir: i32,
        /// Флаг прыжка.
        jump: i32,
        /// Флаг использования крюка.
        hook: i32,
        /// Счётчик/флаг выстрела.
        fire: i32,
        /// Координата прицела X.
        target_x: i32,
        /// Координата прицела Y.
        target_y: i32,
    },
    /// Устаревшее событие обновления списка игроков онлайн.
    Online {
        /// Никнеймы игроков онлайн.
        players: Vec<String>,
    },
    /// Устаревшее событие результата БД.
    DbResult {
        /// Идентификатор запроса.
        req_id: String,
        /// Строковое значение.
        value: String,
    },
    /// Устаревший детальный снимок состояния игрока.
    PlayerState {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Никнейм игрока.
        player: String,
        /// Мировая координата X.
        x: f32,
        /// Мировая координата Y.
        y: f32,
        /// Скорость по X.
        vx: f32,
        /// Скорость по Y.
        vy: f32,
        /// Тики заморозки.
        frozen: i32,
        /// Активное оружие.
        weapon: i32,
    },
    /// Обновление состояния и позиции игрока.
    State {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Снимок состояния игрока.
        pos: PlayerPos,
    },
    /// Событие убийства игрока.
    Kill {
        /// Никнейм убийцы.
        killer: String,
        /// Никнейм жертвы.
        victim: String,
        /// Идентификатор оружия.
        weapon: i32,
    },
    /// Событие смерти игрока.
    Death {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Идентификатор клиента убийцы (`-1`, если мир/карта).
        killer: i32,
        /// Идентификатор оружия.
        weapon: i32,
    },
    /// Игрок получил урон.
    Damage {
        /// Идентификатор клиента жертвы.
        cid: i32,
        /// Идентификатор клиента атакующего.
        attacker: i32,
        /// Количество нанесённого урона.
        amount: i32,
        /// Идентификатор оружия.
        weapon: i32,
    },
    /// Игрок финишировал на карте или в гонке.
    Finish {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Время прохождения в секундах.
        time: f32,
    },
    /// Игрок коснулся специального тайла.
    Tile {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Индекс тайла.
        tile: i32,
        /// Координата тайла X.
        x: i32,
        /// Координата тайла Y.
        y: i32,
    },
    /// Событие попадания оружия в игрока.
    WeaponHit {
        /// Идентификатор клиента атакующего.
        attacker: i32,
        /// Идентификатор клиента жертвы.
        victim: i32,
        /// Идентификатор оружия.
        weapon: i32,
    },
    /// Событие удара молотом по игроку.
    HammerHit {
        /// Никнейм атакующего.
        attacker: String,
        /// Никнейм жертвы.
        victim: String,
    },
    /// Событие подбора предмета (пикапа).
    Pickup {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Тип подобранного предмета.
        kind: i32,
    },
    /// Сменилось активное оружие игрока.
    WeaponChange {
        /// Идентификатор клиента (CID).
        cid: i32,
        /// Идентификатор предыдущего оружия.
        old: i32,
        /// Идентификатор нового оружия.
        new: i32,
    },
    /// Крюк игрока зацепился за тайл или другого игрока.
    HookAttach {
        /// Идентификатор клиента, выпустившего крюк.
        cid: i32,
        /// Идентификатор зацепленного игрока или None, если крюк зацепился за геометрию карты.
        hooked_cid: Option<i32>,
        /// Мировые координаты точки зацепа крюка.
        pos: crate::dx::Vec2,
    },
    /// Крюк игрока отцепился.
    HookDetach {
        /// Идентификатор клиента игрока.
        cid: i32,
        /// Идентификатор ранее зацепленного игрока, если был.
        prev_hooked_cid: Option<i32>,
    },
}

type LifecycleHook = Box<dyn FnMut(&mut ServerApi)>;

/// Основной дескриптор SDK, передаваемый в обработчики событий плагина.
pub struct ServerApi {
    /// Никнеймы игроков, находящихся сейчас в сети.
    pub online: HashSet<String>,
    /// Идентификаторы клиентов, подтвердивших клиент BestClient.
    pub best_clients: HashSet<i32>,
    /// Карта соответствия никнейма и идентификатора клиента (`i32`).
    pub cids: HashMap<String, i32>,
    /// Устаревший псевдоним v9.2 для поля `cids`.
    pub player_cids: HashMap<String, i32>,
    /// Карта соответствия идентификатора клиента (`i32`) и никнейма.
    pub names: HashMap<i32, String>,
    /// Карта соответствия никнейма и IP-адреса клиента.
    pub ips: HashMap<String, String>,
    /// Устаревший псевдоним v9.2 для поля `ips`.
    pub player_ips: HashMap<String, String>,
    /// Карта соответствия никнейма и логина авторизованного аккаунта.
    pub accounts: HashMap<String, String>,
    /// Устаревший псевдоним v9.2 для поля `accounts`.
    pub authed_accounts: HashMap<String, String>,
    /// Карта соответствия логина аккаунта и назначенной роли.
    pub roles: HashMap<String, String>,
    /// Кэш позиций игроков по идентификатору клиента (CID).
    pub states: HashMap<i32, PlayerPos>,
    /// Идентификатор клиента (CID) игрока, вышедшего в последнем событии `Event::Leave`.
    pub last_leave_cid: Option<i32>,
    permissions: HashSet<String>,
    on_load_cb: Option<LifecycleHook>,
    on_reload_cb: Option<LifecycleHook>,
    on_unload_cb: Option<LifecycleHook>,
}

impl Default for ServerApi {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerApi {
    /// Создаёт новый экземпляр `ServerApi` и сообщает версию SDK менеджеру плагинов.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.online.is_empty());
    /// ```
    pub fn new() -> Self {
        let api = Self {
            online: HashSet::new(),
            best_clients: HashSet::new(),
            cids: HashMap::new(),
            player_cids: HashMap::new(),
            names: HashMap::new(),
            ips: HashMap::new(),
            player_ips: HashMap::new(),
            accounts: HashMap::new(),
            authed_accounts: HashMap::new(),
            roles: HashMap::new(),
            states: HashMap::new(),
            last_leave_cid: None,
            permissions: HashSet::new(),
            on_load_cb: None,
            on_reload_cb: None,
            on_unload_cb: None,
        };
        api.raw(&format!("@PM SDK_VER|{}", plugin_api_version()));
        api
    }

    /// Отправляет сырую строку команды менеджеру плагинов через stdout.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.raw("@PONG");
    /// ```
    pub fn raw(&self, cmd: &str) {
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{}", cmd);
        let _ = out.flush();
    }

    /// Объявляет версию API плагина и требуемые разрешения.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.set_manifest("10.0", &["tile_edit"]);
    /// assert!(api.has_permission("tile_edit"));
    /// ```
    pub fn set_manifest(&mut self, api_ver: &str, perms: &[&str]) {
        self.permissions.clear();
        for p in perms {
            self.permissions.insert((*p).to_string());
        }
        self.raw(&format!("@PM MANIFEST|{}|{}", api_ver, perms.join(",")));
    }

    /// Возвращает `true`, если плагин объявил указанное разрешение в манифесте.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.set_manifest("10.0", &["noclip"]);
    /// assert!(api.has_permission("noclip"));
    /// assert!(!api.has_permission("damage"));
    /// ```
    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.contains(perm)
    }

    /// Загружает или создаёт конфигурационный файл `.cfg` плагина.
    pub fn load_config<C: PluginConfig>(&self, n: &str) -> C { C::load_or_create(n) }

    /// Регистрирует чат-команду и её описание для справки.
    pub fn register_plugin(&mut self, cmd: &str, desc: &str) {
        crate::log::init(cmd);
        self.raw(&format!("@REG_CMD {}|{}", cmd, desc));
    }

    /// Устанавливает приоритет выполнения плагина (`High`, `Normal`, `Low`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.set_priority(Priority::High);
    /// ```
    pub fn set_priority(&self, prio: Priority) {
        let tag = match prio {
            Priority::High => "HIGH",
            Priority::Normal => "NORMAL",
            Priority::Low => "LOW",
        };
        self.raw(&format!("@PM PRIORITY|{}", tag));
    }

    /// Включает или отключает доставку высокочастотного тика `Event::FastTick` (100 мс).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.enable_fast_tick(true);
    /// ```
    pub fn enable_fast_tick(&self, enabled: bool) {
        self.raw(&format!("@PM FASTTICK|{}", if enabled { 1 } else { 0 }));
    }

    /// Включает или отключает периодический опрос координат игроков в `api.states`.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.track_positions(true);
    /// ```
    pub fn track_positions(&self, enabled: bool) {
        self.raw(&format!("@TRACK_POS {}", if enabled { 1 } else { 0 }));
    }

    /// Регистрирует обработчик, вызываемый при первичной загрузке плагина.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.on_load(|_api| {});
    /// ```
    pub fn on_load<F: FnMut(&mut ServerApi) + 'static>(&mut self, cb: F) {
        self.on_load_cb = Some(Box::new(cb));
    }

    /// Регистрирует обработчик, вызываемый при горячей перезагрузке плагина.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.on_reload(|_api| {});
    /// ```
    pub fn on_reload<F: FnMut(&mut ServerApi) + 'static>(&mut self, cb: F) {
        self.on_reload_cb = Some(Box::new(cb));
    }

    /// Регистрирует обработчик, вызываемый при выгрузке плагина после отката состояния.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.on_unload(|_api| {});
    /// ```
    pub fn on_unload<F: FnMut(&mut ServerApi) + 'static>(&mut self, cb: F) {
        self.on_unload_cb = Some(Box::new(cb));
    }

    /// Находит типизированный `PlayerId` игрока по его никнейму.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.cids.insert("nameless".into(), 3);
    /// assert_eq!(api.player_id("nameless"), Some(PlayerId(3)));
    /// ```
    pub fn player_id(&self, nick: &str) -> Option<PlayerId> {
        self.cids.get(nick).copied().map(PlayerId)
    }

    /// Отправляет сообщение в общий чат всем игрокам.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.say("Hello server!");
    /// ```
    pub fn say(&self, msg: &str) {
        self.raw(&format!("say \"{}\"", msg.replace('"', "'")));
    }

    /// Отправляет личное сообщение (шёпот) игроку по никнейму.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.whisper("nameless", "Hi!");
    /// ```
    pub fn whisper(&self, player: &str, msg: &str) {
        self.raw(&format!(
            "whisper \"{}\" \"{}\"",
            player.replace('"', "'"),
            msg.replace('"', "'")
        ));
    }

    /// Отправляет личное сообщение (шёпот) игроку по типизированному `PlayerId`.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.whisper_id(PlayerId(0), "Hi!");
    /// ```
    pub fn whisper_id(&self, cid: PlayerId, msg: &str) {
        if let Some(nick) = self.names.get(&cid.0) {
            self.whisper(nick, msg);
        }
    }

    /// Отправляет прямое межплагинное IPC-сообщение плагину `target`.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.send_to("plugin_hub", "PING");
    /// ```
    pub fn send_to(&self, target: &str, payload: &str) {
        self.raw(&format!("@TO {}|{}", target, payload));
    }

    /// Рассылает данные по общей межплагинной шине IPC.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.broadcast_bus("SYNC");
    /// ```
    pub fn broadcast_bus(&self, payload: &str) {
        self.raw(&format!("@BUS {}", payload));
    }

    /// Записывает пару ключ-значение в `plugins_kv.db`.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_set("demo:key", "1");
    /// ```
    pub fn db_set(&self, key: &str, val: &str) {
        self.raw(&format!("@DB SET|{}|{}", key, val));
    }

    /// Запрашивает значение по ключу из `plugins_kv.db` (ответ приходит в `Event::DbValue`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_get("req1", "demo:key");
    /// ```
    pub fn db_get(&self, req_id: &str, key: &str) {
        self.raw(&format!("@DB GET|{}|{}", req_id, key));
    }

    /// Атомарно прибавляет `delta` к числовому ключу в `plugins_kv.db`.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_add("demo:count", 5);
    /// ```
    pub fn db_add(&self, key: &str, delta: i64) {
        self.raw(&format!("@DB ADD|{}|{}", key, delta));
    }

    /// Удаляет ключ из `plugins_kv.db`.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_del("demo:key");
    /// ```
    pub fn db_del(&self, key: &str) {
        self.raw(&format!("@DB DEL|{}", key));
    }

    /// Удаляет все ключи с префиксом `prefix` из `plugins_kv.db`.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_del_prefix("demo:");
    /// ```
    pub fn db_del_prefix(&self, prefix: &str) {
        self.raw(&format!("@DB DEL_PREFIX|{}", prefix));
    }

    /// Запрашивает все ключи с префиксом `prefix` (ответ приходит в `Event::DbList`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_prefix("req_p", "demo:");
    /// ```
    pub fn db_prefix(&self, req_id: &str, prefix: &str) {
        self.raw(&format!("@DB PREFIX|{}|{}", req_id, prefix));
    }

    /// Устанавливает постоянную строку HUD в слоте `1..=5` для игрока.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.set_hud_line(PlayerId(0), 1, "Wave 1").is_ok());
    /// ```
    /// Возвращает `true`, если клиент подтверждён как BestClient.
    pub fn is_best_client(&self, cid: i32) -> bool {
        self.best_clients.contains(&cid)
    }

    /// Отправляет пользовательский системный пакет (MsgId 100) подтверждённому клиенту BestClient.
    pub fn send_custom_packet(&self, cid: i32, kind: &str, payload: &str) {
        if self.best_clients.contains(&cid) {
            let clean_k: String = kind.chars().filter(|c| !c.is_whitespace() && *c != ';').collect();
            let clean_p: String = payload.chars().filter(|c| *c != '\n' && *c != '\r' && *c != ';').collect();
            if !clean_k.is_empty() {
                self.raw(&format!("bc_packet {} {} {}", cid, clean_k, clean_p));
            }
        }
    }

    /// Обновляет строку в указанном слоте HUD для игрока.
    pub fn set_hud_line(&self, cid: PlayerId, slot: u8, text: &str) -> Result<(), SdkError> {
        if !cid.is_valid() {
            return Err(SdkError::InvalidCid(cid));
        }
        if slot > 4 || text.chars().count() > 200 {
            return Err(SdkError::InvalidArg {
                name: "slot/text",
                reason: "slot must be 0..=4 and text <= 200 chars",
            });
        }
        let clean = text.replace(['\n', '\r'], " ").replace('"', "'");
        self.raw(&format!("set_hud_line {} {} \"{}\"", cid.0, slot, clean));
        Ok(())
    }

    /// Очищает строку HUD в слоте `1..=5` для игрока.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.clear_hud_line(PlayerId(0), 1).is_ok());
    /// ```
    pub fn clear_hud_line(&self, cid: PlayerId, slot: u8) -> Result<(), SdkError> {
        if !cid.is_valid() {
            return Err(SdkError::InvalidCid(cid));
        }
        if slot > 4 {
            return Err(SdkError::InvalidArg {
                name: "slot",
                reason: "slot must be 0..=4",
            });
        }
        self.raw(&format!("clear_hud_line {} {}", cid.0, slot));
        Ok(())
    }

    /// Отрисовывает UTF-8 полосу прогресса (`█`/`░` и проценты) в слоте HUD 4.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.send_progress_bar(PlayerId(0), "HP", 50, 100, 10).is_ok());
    /// ```
    pub fn send_progress_bar(
        &self,
        cid: PlayerId,
        title: &str,
        current: u32,
        max: u32,
        width: u8,
    ) -> Result<(), SdkError> {
        let w = (width.clamp(4, 30)) as usize;
        let denom = max.max(1);
        let clamped = current.min(denom);
        let filled = ((clamped as u64 * w as u64) / denom as u64) as usize;
        let pct = (clamped as u64 * 100) / denom as u64;
        let bar = format!(
            "{} [{}{}] {}%",
            title,
            "█".repeat(filled),
            "░".repeat(w.saturating_sub(filled)),
            pct
        );
        self.set_hud_line(cid, 4, &bar)
    }

    /// Создаёт кастомный серверный снаряд, отслеживаемый в реестре состояния.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// assert!(api.spawn_projectile(1, PlayerId(0), Vec2::new(100.0, 100.0), Vec2::new(10.0, 0.0), 0.5, 2000).is_ok());
    /// ```
    pub fn spawn_projectile(
        &self,
        id: i32,
        owner_cid: PlayerId,
        pos: Vec2,
        vel: Vec2,
        gravity: f32,
        ttl_ms: u32,
    ) -> Result<(), SdkError> {
        self.raw(&format!(
            "spawn_proj {} {} {:.1} {:.1} {:.2} {:.2} {:.3} {}",
            id, owner_cid.0, pos.x, pos.y, vel.x, vel.y, gravity, ttl_ms
        ));
        Ok(())
    }

    /// Удаляет ранее созданный кастомный снаряд по его идентификатору.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.despawn_projectile(1);
    /// ```
    pub fn despawn_projectile(&self, id: i32) {
        self.raw(&format!("despawn_proj {}", id));
    }

    /// Переопределяет ввод движения игрока на следующий тик (требуется разрешение `force_input`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.set_manifest("10.0", &["force_input"]);
    /// assert!(api.force_input(PlayerId(0), 1, 1, 0, 0).is_ok());
    /// ```
    pub fn force_input(
        &self,
        cid: PlayerId,
        dir: i32,
        jump: i32,
        hook: i32,
        fire: i32,
    ) -> Result<(), SdkError> {
        if !self.has_permission("force_input") {
            return Err(SdkError::PermissionDenied {
                needed: "force_input",
            });
        }
        if !cid.is_valid() {
            return Err(SdkError::InvalidCid(cid));
        }
        self.raw(&format!(
            "force_input {} {} {} {} {}",
            cid.0,
            dir.clamp(-1, 1),
            if jump != 0 { 1 } else { 0 },
            if hook != 0 { 1 } else { 0 },
            if fire != 0 { 1 } else { 0 }
        ));
        Ok(())
    }

    /// Сбрасывает принудительный ввод движения для игрока (требуется разрешение `force_input`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let mut api = ServerApi::new();
    /// api.set_manifest("10.0", &["force_input"]);
    /// assert!(api.clear_force_input(PlayerId(0)).is_ok());
    /// ```
    pub fn clear_force_input(&self, cid: PlayerId) -> Result<(), SdkError> {
        if !self.has_permission("force_input") {
            return Err(SdkError::PermissionDenied {
                needed: "force_input",
            });
        }
        if !cid.is_valid() {
            return Err(SdkError::InvalidCid(cid));
        }
        self.raw(&format!("clear_force_input {}", cid.0));
        Ok(())
    }

    /// Атомарно прибавляет `delta` к числовому ключу аккаунта (`account:key`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_add_by_account("alice", "coins", 100);
    /// ```
    pub fn db_add_by_account(&self, account: &str, key: &str, delta: i64) {
        self.raw(&format!("@DB ADD_ACC|{}|{}|{}", account, key, delta));
    }

    /// Атомарно переводит сумму `amount` между двумя аккаунтами (статус приходит в `Event::DbValue`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.db_transfer_by_account("tx1", "alice", "bob", "coins", 25);
    /// ```
    pub fn db_transfer_by_account(
        &self,
        req_id: &str,
        from_acc: &str,
        to_acc: &str,
        key: &str,
        amount: i64,
    ) {
        self.raw(&format!(
            "@DB XFER|{}|{}|{}|{}|{}",
            req_id, from_acc, to_acc, key, amount
        ));
    }

    /// Сохраняет значение состояния в пространстве имён плагина (`pstate:<plugin>:<key>`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.state_set("wave", "3");
    /// ```
    pub fn state_set(&self, key: &str, val: &str) {
        self.raw(&format!("@PSTATE SET|{}|{}", key, val));
    }

    /// Читает значение состояния из пространства имён плагина (ответ приходит в `Event::DbValue`).
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.state_get("st1", "wave");
    /// ```
    pub fn state_get(&self, req_id: &str, key: &str) {
        self.raw(&format!("@PSTATE GET|{}|{}", req_id, key));
    }

    /// Планирует однократное событие `Event::Scheduled { callback_id }` через `delay_ms` мс.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.schedule_after(1000, 42);
    /// ```
    pub fn schedule_after(&self, delay_ms: u32, callback_id: u64) {
        self.raw(&format!("@SCHED AFTER|{}|{}", delay_ms, callback_id));
    }

    /// Планирует повторяющееся событие `Event::Scheduled { callback_id }` каждые `interval_ms` мс.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.schedule_every(500, 7);
    /// ```
    pub fn schedule_every(&self, interval_ms: u32, callback_id: u64) {
        self.raw(&format!("@SCHED EVERY|{}|{}", interval_ms, callback_id));
    }

    /// Отменяет ранее запланированный таймер по `callback_id`.
    ///
    /// # Примеры
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.cancel_schedule(42);
    /// ```
    pub fn cancel_schedule(&self, callback_id: u64) {
        self.raw(&format!("@SCHED CANCEL|{}", callback_id));
    }

    /// Запускает главный цикл обработки событий плагина с изоляцией паник.
    ///
    /// # Примеры
    /// ```no_run
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.run(|_api, _ev| {});
    /// ```
    pub fn run<F: FnMut(&mut ServerApi, Event)>(self, mut handler: F) {
        self.run_intercept(move |api, ev| {
            handler(api, ev);
            EventAction::Pass
        });
    }

    /// Запускает главный цикл обработки событий плагина с поддержкой отмены событий и изоляцией паник.
    ///
    /// # Примеры
    /// ```no_run
    /// use ddnet_sdk::prelude::*;
    /// let api = ServerApi::new();
    /// api.run_intercept(|_api, _ev| EventAction::Pass);
    /// ```
    pub fn run_intercept<F: FnMut(&mut ServerApi, Event) -> EventAction>(mut self, mut handler: F) {
        let (tx, rx) = mpsc::channel::<String>();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        for raw_line in rx {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "PING" {
                self.raw("@PONG");
                continue;
            }
            if let Some(lc) = line.strip_prefix("LIFECYCLE|") {
                match lc {
                    "LOAD" => {
                        if let Some(mut cb) = self.on_load_cb.take() {
                            let res = catch_unwind(AssertUnwindSafe(|| cb(&mut self)));
                            self.on_load_cb = Some(cb);
                            if res.is_err() {
                                self.raw("@PM PANIC|on_load");
                                break;
                            }
                        }
                    }
                    "RELOAD" => {
                        if let Some(mut cb) = self.on_reload_cb.take() {
                            let res = catch_unwind(AssertUnwindSafe(|| cb(&mut self)));
                            self.on_reload_cb = Some(cb);
                            if res.is_err() {
                                self.raw("@PM PANIC|on_reload");
                                break;
                            }
                        }
                    }
                    "UNLOAD" => {
                        if let Some(mut cb) = self.on_unload_cb.take() {
                            let _ = catch_unwind(AssertUnwindSafe(|| cb(&mut self)));
                            self.on_unload_cb = Some(cb);
                        }
                        self.raw("@PM UNLOAD_ACK");
                        break;
                    }
                    _ => {}
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("SYNC_PLAYER|") {
                let p: Vec<&str> = rest.splitn(4, '|').collect();
                if let (Some(nick), Some(cid_s), Some(ip), Some(acc)) =
                    (p.first(), p.get(1), p.get(2), p.get(3))
                {
                    let n = (*nick).to_string();
                    self.online.insert(n.clone());
                    if let Ok(cid) = cid_s.parse::<i32>()
                        && cid >= 0
                    {
                        self.cids.insert(n.clone(), cid);
                        self.names.insert(cid, n.clone());
                    }
                    if !ip.is_empty() {
                        self.ips.insert(n.clone(), (*ip).to_string());
                    }
                    if !acc.is_empty() {
                        self.accounts.insert(n.clone(), (*acc).to_string());
                        self.authed_accounts.insert(n.clone(), (*acc).to_string());
                    }
                    if let Ok(cid) = cid_s.parse::<i32>() && cid >= 0 {
                        self.player_cids.insert(n, cid);
                    }
                }
                continue;
            }

            let mut ev_id: Option<u64> = None;
            let body = if let Some(rest) = line.strip_prefix("EV|") {
                if let Some((id_s, tail)) = rest.split_once('|') {
                    ev_id = id_s.parse::<u64>().ok();
                    tail
                } else {
                    line
                }
            } else {
                line
            };

            if let Some(ev) = self.parse_event(body) {
                let res = catch_unwind(AssertUnwindSafe(|| handler(&mut self, ev)));
                match res {
                    Ok(act) => {
                        if let Some(id) = ev_id {
                            let tag = if act == EventAction::Cancel {
                                "CANCEL"
                            } else {
                                "PASS"
                            };
                            self.raw(&format!("@ACK {}|{}", id, tag));
                        }
                    }
                    Err(_) => {
                        self.raw("@PM PANIC|event_handler");
                        break;
                    }
                }
            } else if let Some(id) = ev_id {
                self.raw(&format!("@ACK {}|PASS", id));
            }
        }
    }

    /// Отправляет сырую консольную команду сервера (устаревший псевдоним v9.2).
    pub fn cmd(&self, c: &str) {
        self.raw(c);
    }
    /// Регистрирует команду плагина только для администраторов (устаревший псевдоним v9.2).
    pub fn register_admin_plugin(&mut self, cmd: &str, desc: &str) {
        crate::log::init(cmd);
        self.raw(&format!("@REG_ADMIN {}|{}", cmd, desc));
    }
    /// Регистрирует описание подкоманды (устаревший псевдоним v9.2).
    pub fn register_cmd(&mut self, cmd: &str, args: &str, desc: &str) {
        self.raw(&format!("@REG_SUB {}|{}|{}", cmd, args, desc));
    }
    /// Проверяет, имеет ли игрок указанную роль.
    pub fn has_role(&self, player: &str, role: &str) -> bool {
        self.accounts
            .get(player)
            .and_then(|a| self.roles.get(a))
            .map(|r| r.eq_ignore_ascii_case(role))
            .unwrap_or(false)
    }
    /// Проверяет, относится ли игрок к персоналу (`owner`, `admin`, `mod`, `helper`).
    pub fn is_staff(&self, player: &str) -> bool {
        ["owner", "admin", "mod", "helper"]
            .iter()
            .any(|r| self.has_role(player, r))
    }
    /// Рассылает данные по общей межплагинной шине IPC (устаревший псевдоним v9.2).
    pub fn broadcast_ipc(&self, payload: &str) {
        self.broadcast_bus(payload);
    }
    /// Отправляет экранный баннер (broadcast) конкретному клиенту по CID.
    pub fn broadcast_to(&self, cid: i32, msg: &str) {
        self.raw(&format!("broadcast_to {} {}", cid, msg));
    }
    /// Запрашивает ключи, начинающиеся с префикса (устаревший псевдоним v9.2).
    pub fn db_get_prefix(&self, req_id: &str, prefix: &str) {
        self.db_prefix(req_id, prefix);
    }
    /// Запрашивает немедленный снимок состояния игрока.
    pub fn request_state(&self, cid: i32) {
        self.raw(&format!("get_state {}", cid));
    }
    /// Сбрасывает крюк игрока.
    pub fn reset_hook(&self, cid: i32) {
        self.raw(&format!("reset_hook {}", cid));
    }
    /// Проигрывает звуковой эффект для клиента по CID (`-1` — для всех).
    pub fn play_sound(&self, cid: i32, id: i32) {
        self.raw(&format!("play_sound {} {}", cid, id));
    }
    /// Замораживает или размораживает игрока на `secs` секунд.
    pub fn set_freeze(&self, cid: i32, secs: i32) {
        self.raw(&format!("set_freeze {} {}", cid, secs));
    }
    /// Устанавливает вектор скорости игрока.
    pub fn set_vel(&self, cid: i32, vx: f32, vy: f32) {
        self.raw(&format!("set_vel {} {:.2} {:.2}", cid, vx, vy));
    }
    /// Добавляет импульс (vx, vy) к текущей скорости игрока.
    pub fn apply_impulse(&self, cid: i32, vx: f32, vy: f32) {
        self.raw(&format!("plugin_apply_impulse {} {:.2} {:.2}", cid, vx, vy));
    }
    /// Телепортирует игрока в указанные мировые координаты.
    pub fn set_pos(&self, cid: i32, x: f32, y: f32) {
        self.raw(&format!("set_pos {} {:.1} {:.1}", cid, x, y));
    }

    /// Выдаёт игроку оружие по идентификатору (`0..=5`).
    pub fn give_weapon(&self, cid: i32, w: i32) {
        self.raw(&format!("give_weapon {} {}", cid, w));
    }
    /// Устанавливает количество патронов для оружия игрока.
    pub fn set_ammo(&self, cid: i32, w: i32, ammo: i32) {
        self.raw(&format!("set_ammo {} {} {}", cid, w, ammo));
    }
    /// Включает или отключает неуязвимость игрока.
    pub fn set_invincible(&self, cid: i32, on: bool) {
        self.raw(&format!(
            "plugin_set_invincible {} {}",
            cid,
            if on { 1 } else { 0 }
        ));
    }
    /// Включает или отключает бесконечные прыжки для игрока.
    pub fn set_endless_jump(&self, cid: i32, on: bool) {
        self.raw(&format!(
            "plugin_set_endless_jump {} {}",
            cid,
            if on { 1 } else { 0 }
        ));
    }
    /// Включает или отключает бесконечный крюк для игрока.
    pub fn set_endless_hook(&self, cid: i32, on: bool) {
        self.raw(&format!(
            "plugin_set_endless_hook {} {}",
            cid,
            if on { 1 } else { 0 }
        ));
    }
    /// Устанавливает физический флаг игрока (`jetpack`, `collision`, `hammer_hit`, `hook_hit`).
    pub fn set_phys(&self, cid: i32, key: &str, on: bool) {
        self.raw(&format!(
            "set_phys {} {} {}",
            cid,
            key,
            if on { 1 } else { 0 }
        ));
    }
    /// Устанавливает пользовательский цвет тела и ног ти.
    pub fn set_color(&self, cid: i32, custom: bool, body: i32, feet: i32) {
        self.raw(&format!(
            "plugin_set_color {} {} {} {}",
            cid,
            if custom { 1 } else { 0 },
            body,
            feet
        ));
    }
    /// Подменяет видимый скин игрока `target` персонально для `viewer` (передайте `""` для сброса).
    pub fn set_skin_to(&self, viewer: i32, target: i32, skin: &str) {
        let clean: String = skin
            .chars()
            .filter(|c| !c.is_control() && *c != '"' && *c != '\\' && !c.is_whitespace())
            .take(23)
            .collect();
        if clean.is_empty() {
            self.raw(&format!("plugin_set_skin_to {} {}", viewer, target));
        } else {
            self.raw(&format!("plugin_set_skin_to {} {} {}", viewer, target, clean));
        }
    }
    /// Возвращает видимый скин игрока `target` для `viewer` к исходному.
    pub fn clear_skin_to(&self, viewer: i32, target: i32) {
        self.set_skin_to(viewer, target, "");
    }
    /// Включает или отключает статус VIP+ для игрока.
    pub fn set_vip_plus(&self, cid: i32, on: bool) {
        self.raw(&format!("set_vip_plus {} {}", cid, if on { 1 } else { 0 }));
    }
    /// Принудительно убивает персонажа игрока.
    pub fn force_kill(&self, cid: i32) {
        self.raw(&format!("force_kill {}", cid));
    }
    /// Убивает игрока `cid` с зачётом фрага убийце `killer` из оружия `weapon`.
    pub fn kill_by(&self, cid: i32, killer: i32, w: i32) {
        self.raw(&format!("kill_by {} {} {}", cid, killer, w));
    }
    /// Создаёт визуальный эффект частиц (`explosion`, `hammer`, `damage`).
    pub fn spawn_fx(&self, kind: &str, x: f32, y: f32) {
        self.raw(&format!("spawn_fx {} {:.1} {:.1}", kind, x, y));
    }
    /// Создаёт стандартный лазерный луч.
    pub fn spawn_laser(&self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.raw(&format!(
            "spawn_laser {:.1} {:.1} {:.1} {:.1}",
            x1, y1, x2, y2
        ));
    }
    /// Создаёт стилизованный лазерный луч.
    pub fn spawn_laser_ex(&self, x1: f32, y1: f32, x2: f32, y2: f32, kind: i32) {
        self.raw(&format!(
            "spawn_laser_ex {:.1} {:.1} {:.1} {:.1} {}",
            x1, y1, x2, y2, kind
        ));
    }
    /// Создаёт постоянный визуальный лазер OnSnap с указанным ID.
    pub fn spawn_visual_laser(&self, id: i32, x1: f32, y1: f32, x2: f32, y2: f32, kind: i32) {
        self.raw(&format!(
            "spawn_visual_laser {} {:.1} {:.1} {:.1} {:.1} {}",
            id, x1, y1, x2, y2, kind
        ));
    }
    /// Удаляет все постоянные визуальные лазеры.
    /// Удаляет постоянный визуальный лазер по его ID.
    pub fn remove_visual_laser(&self, id: i32) {
        self.raw(&format!("remove_visual_laser {}", id));
    }
    /// Удаляет все постоянные визуальные лазеры этого плагина.
    pub fn clear_visual_lasers(&self) {
        self.raw("clear_visual_lasers");
    }
    /// Создаёт постоянный визуальный пикап OnSnap с указанным ID.
    pub fn spawn_visual_pickup(&self, id: i32, kind: i32, sub: i32, x: f32, y: f32) {
        self.raw(&format!(
            "spawn_visual_pickup {} {} {} {:.1} {:.1}",
            id, kind, sub, x, y
        ));
    }
    /// Создаёт постоянный визуальный пикап, привязанный к игроку по CID.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_visual_pickup_follow(
        &self,
        id: i32,
        kind: i32,
        sub: i32,
        cid: i32,
        off_x: f32,
        off_y: f32,
        bob_amp: f32,
    ) {
        self.raw(&format!(
            "spawn_visual_pickup_follow {} {} {} {} {:.1} {:.1} {:.1}",
            id, kind, sub, cid, off_x, off_y, bob_amp
        ));
    }
    /// Меняет активную карту сервера командой `change_map "<map>"`.
    pub fn change_map(&self, map: &str) {
        let safe: String = map.chars().filter(|&c| c != '"' && c != '\n' && c != '\r').collect();
        if !safe.is_empty() {
            self.raw(&format!(r#"change_map "{}""#, safe));
        }
    }
    /// Устанавливает режим таблицы очков (`0` = гонка/время, `1` = очки, `2` = командные очки).
    pub fn set_gamemode(&self, mode: i32) { self.raw(&format!("set_gamemode {}", mode)); }
    /// Устанавливает очки игрока в таблице по никнейму.
    pub fn set_player_score_name(&self, name: &str, score: i32) { self.raw(&format!(r#"set_player_score "{}" {}"#, name, score)); }
    /// Устанавливает таймаут автосмерти в тайле заморозки в секундах (`0` = выкл).
    pub fn set_freeze_autokill(&self, sec: i32) { self.raw(&format!("set_freeze_autokill {}", sec)); }
    /// Устанавливает длительность заморозки лазером в секундах (`0` = выкл).
    pub fn set_laser_freeze(&self, sec: i32) { self.raw(&format!("set_laser_freeze {}", sec)); }
    /// Включает или отключает разморозку молотом только для союзников.
    pub fn set_hammer_team_unfreeze(&self, on: bool) { self.raw(&format!("set_hammer_team_unfreeze {}", if on { 1 } else { 0 })); }
    /// Включает или отключает стартовый набор FNG (только молот и лазер).
    pub fn set_fng_weapons(&self, on: bool) { self.raw(&format!("set_fng_weapons {}", if on { 1 } else { 0 })); }
    /// Устанавливает целочисленные очки игрока в таблице.
    pub fn set_player_score(&self, cid: i32, score: i32) { self.raw(&format!("set_player_score {} {}", cid, score)); }
    /// Переводит игрока в команду (`0` = Red, `1` = Blue, `-1` = Наблюдатели).
    pub fn set_player_team(&self, cid: i32, team: i32) { self.raw(&format!("set_player_team {} {}", cid, team)); }
    /// Устанавливает счёт команд Red и Blue.
    pub fn set_team_score(&self, red: i32, blue: i32) { self.raw(&format!("set_team_score {} {}", red, blue)); }
    /// Удаляет постоянный визуальный пикап по его ID.
    pub fn remove_visual_pickup(&self, id: i32) {
        self.raw(&format!("remove_visual_pickup {}", id));
    }
    /// Создаёт постоянного визуального ти (ID 0..63) в OnSnap.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_visual_tee(&self, id: i32, x: f32, y: f32, skin: &str, body: i32, feet: i32, freeze: i32) {
        self.raw(&format!(
            "spawn_visual_tee {} {:.1} {:.1} \"{}\" {} {} {}",
            id, x, y, skin.replace('"', ""), body, feet, freeze
        ));
    }
    /// Удаляет постоянного визуального ти по его ID.
    pub fn remove_visual_tee(&self, id: i32) {
        self.raw(&format!("remove_visual_tee {}", id));
    }
    /// Удаляет всех постоянных визуальных ти этого плагина.
    pub fn clear_visual_tees(&self) {
        self.raw("clear_visual_tees");
    }
    /// Создаёт постоянный векторный лазерный текст в OnSnap (ID 0..31, 0 FX/с).
    pub fn spawn_visual_text(&self, id: i32, x: f32, y: f32, scale: f32, kind: i32, text: &str) {
        let clean = text.replace('"', "");
        self.raw(&format!(
            "spawn_visual_text {} {:.1} {:.1} {:.2} {} \"{}\"",
            id, x, y, scale, kind, clean
        ));
    }
    /// Удаляет постоянный векторный текст по его ID.
    pub fn remove_visual_text(&self, id: i32) {
        self.raw(&format!("remove_visual_text {}", id));
    }
    /// Удаляет все постоянные векторные тексты этого плагина.
    pub fn clear_visual_texts(&self) {
        self.raw("clear_visual_texts");
    }

    fn parse_event(&mut self, body: &str) -> Option<Event> {
        if let Some(rest) = body.strip_prefix("ONLINE|") {
            self.online = rest
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .collect();
            return None;
        }
        if let Some(s) = body.strip_prefix("TICK|") {
            let sec: u64 = s.parse().ok()?;
            return Some(Event::Tick { uptime_sec: sec });
        }
        if let Some(s) = body.strip_prefix("FTICK|") {
            let ms: u64 = s.parse().ok()?;
            return Some(Event::FastTick { uptime_ms: ms });
        }
        if let Some(s) = body.strip_prefix("SCHED|") {
            let callback_id: u64 = s.parse().ok()?;
            return Some(Event::Scheduled { callback_id });
        }
        if let Some(rest) = body.strip_prefix("IPC|") {
            let (from, payload) = rest.split_once('|')?;
            if from == "manager"
                && let Some(t_rest) = payload.strip_prefix("TEAM|")
            {
                let p: Vec<&str> = t_rest.splitn(3, '|').collect();
                if let (Some(cid_s), Some(team_s), Some(nick_s)) = (p.first(), p.get(1), p.get(2))
                    && let (Ok(cid), Ok(team)) = (cid_s.parse::<i32>(), team_s.parse::<i32>())
                {
                    let player = nick_s.to_string();
                    self.cids.insert(player.clone(), cid);
                    self.names.insert(cid, player.clone());
                    return Some(Event::TeamChange { cid, player, team });
                }
            }
            return Some(Event::Ipc {
                from: from.to_string(),
                payload: payload.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("DB_VAL|") {
            let (req_id, value) = rest.split_once('|')?;
            return Some(Event::DbValue {
                req_id: req_id.to_string(),
                value: value.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("DB_LIST|") {
            let (req_id, raw_items) = rest.split_once('|')?;
            let items = raw_items
                .split(';')
                .filter(|s| !s.is_empty())
                .filter_map(|kv| {
                    kv.split_once('=')
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect();
            return Some(Event::DbList {
                req_id: req_id.to_string(),
                items,
            });
        }
        if let Some(rest) = body.strip_prefix("JOIN|") {
            let p: Vec<&str> = rest.splitn(3, '|').collect();
            let player = p.first()?.to_string();
            let cid = p.get(1).and_then(|s| s.parse().ok()).unwrap_or(-1);
            let ip = p.get(2).copied().unwrap_or("").to_string();
            self.online.insert(player.clone());
            if cid >= 0 {
                self.cids.insert(player.clone(), cid);
                self.player_cids.insert(player.clone(), cid);
                self.names.insert(cid, player.clone());
            }
            if !ip.is_empty() {
                self.ips.insert(player.clone(), ip.clone());
                self.player_ips.insert(player.clone(), ip.clone());
            }
            return Some(Event::Join { player });
        }
        if let Some(nick) = body.strip_prefix("LEAVE|") {
            let player = nick.to_string();
            self.online.remove(&player);
            let cid_opt = self.cids.remove(&player).or_else(|| self.player_cids.remove(&player));
            self.player_cids.remove(&player);
            self.last_leave_cid = cid_opt;
            if let Some(cid) = cid_opt {
                self.names.remove(&cid);
                self.states.remove(&cid);
                self.best_clients.remove(&cid);
            }
            self.ips.remove(&player);
            self.player_ips.remove(&player);
            self.accounts.remove(&player);
            self.authed_accounts.remove(&player);
            return Some(Event::Leave { player });
        }
        if let Some(rest) = body.strip_prefix("RENAME|") {
            let (old, new) = rest.split_once('|')?;
            self.online.remove(old);
            self.online.insert(new.to_string());
            if let Some(cid) = self.cids.remove(old).or_else(|| self.player_cids.remove(old)) {
                self.player_cids.remove(old);
                self.cids.insert(new.to_string(), cid);
                self.player_cids.insert(new.to_string(), cid);
                self.names.insert(cid, new.to_string());
            }
            if let Some(ip) = self.ips.remove(old).or_else(|| self.player_ips.remove(old)) {
                self.player_ips.remove(old);
                self.ips.insert(new.to_string(), ip.clone());
                self.player_ips.insert(new.to_string(), ip);
            }
            if let Some(acc) = self.accounts.remove(old).or_else(|| self.authed_accounts.remove(old)) {
                self.authed_accounts.remove(old);
                self.accounts.insert(new.to_string(), acc.clone());
                self.authed_accounts.insert(new.to_string(), acc);
            }
            return Some(Event::Rename {
                old: old.to_string(),
                new: new.to_string(),
                new_name: new.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("CHAT|") {
            let p: Vec<&str> = rest.splitn(3, '|').collect();
            return Some(Event::Chat {
                team: p.first()?.parse().ok()?,
                player: (*p.get(1)?).to_string(),
                message: (*p.get(2)?).to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("BESTCLIENT|") {
            let (cid_s, ver) = rest.split_once('|')?;
            let cid = cid_s.parse().ok()?;
            self.best_clients.insert(cid);
            return Some(Event::BestClient { cid, version: ver.to_string() });
        }
        if let Some(rest) = body.strip_prefix("AUTH|") {
            let p: Vec<&str> = rest.splitn(3, '|').collect();
            let cid = p.first()?.parse().ok()?;
            let player = (*p.get(1)?).to_string();
            let account = (*p.get(2)?).to_string();
            self.accounts.insert(player.clone(), account.clone());
            self.authed_accounts.insert(player.clone(), account.clone());
            return Some(Event::Auth {
                cid,
                player,
                account,
            });
        }
        if let Some(rest) = body.strip_prefix("ROLE|") {
            let (acc, role) = rest.split_once('|')?;
            if role.is_empty() {
                self.roles.remove(acc);
            } else {
                self.roles.insert(acc.to_string(), role.to_string());
            }
            return Some(Event::Role {
                account: acc.to_string(),
                role: role.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("KILL|") {
            let p: Vec<&str> = rest.splitn(3, '|').collect();
            return Some(Event::Kill {
                killer: (*p.first()?).to_string(),
                victim: (*p.get(1)?).to_string(),
                weapon: p.get(2)?.parse().ok()?,
            });
        }
        if let Some(rest) = body.strip_prefix("STATE|") {
            let p: Vec<&str> = if rest.contains(':') {
                rest.split(':').collect()
            } else {
                rest.split_whitespace().collect()
            };
            let off = if p.len() >= 7 && p[1].parse::<f32>().is_err() { 1 } else { 0 };
            if p.len() >= 6 + off {
                let cid = p[0].parse().ok()?;
                let pos = PlayerPos {
                    x: p[1 + off].parse().ok()?,
                    y: p[2 + off].parse().ok()?,
                    vx: p[3 + off].parse().ok()?,
                    vy: p[4 + off].parse().ok()?,
                    frozen: p[5 + off].parse().ok().unwrap_or(0),
                    alive: p.get(6 + off).map(|v| *v == "1").unwrap_or(true),
                    weapon: p.get(7 + off).and_then(|v| v.parse().ok()).unwrap_or(0),
                };
                self.states.insert(cid, pos);
                return Some(Event::State { cid, pos });
            }
        }
        if let Some(rest) = body.strip_prefix("INPUT|") {
            let is_colon = rest.contains(':');
            let p: Vec<&str> = if is_colon {
                rest.split(':').collect()
            } else {
                rest.split_whitespace().collect()
            };
            if p.len() >= 5 {
                let (dir_i, jump_i, hook_i, fire_i) = if is_colon { (4, 2, 3, 1) } else { (1, 2, 3, 4) };
                return Some(Event::Input {
                    cid: p.first()?.parse().ok()?,
                    dir: p.get(dir_i)?.parse().ok()?,
                    jump: p.get(jump_i)?.parse().ok()?,
                    hook: p.get(hook_i)?.parse().ok()?,
                    fire: p.get(fire_i)?.parse().ok()?,
                    target_x: p.get(5).and_then(|v| v.parse().ok()).unwrap_or(0),
                    target_y: p.get(6).and_then(|v| v.parse().ok()).unwrap_or(0),
                });
            }
        }
        if let Some(rest) = body.strip_prefix("DEATH|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 3 {
                return Some(Event::Death {
                    cid: p.first()?.parse().ok()?,
                    killer: p.get(1)?.parse().ok()?,
                    weapon: p.get(2)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("DAMAGE|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 4 {
                return Some(Event::Damage {
                    cid: p.first()?.parse().ok()?,
                    attacker: p.get(1)?.parse().ok()?,
                    amount: p.get(2)?.parse().ok()?,
                    weapon: p.get(3)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("FINISH|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 2 {
                return Some(Event::Finish {
                    cid: p.first()?.parse().ok()?,
                    time: p.get(1)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("TILE|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 4 {
                return Some(Event::Tile {
                    cid: p.first()?.parse().ok()?,
                    tile: p.get(1)?.parse().ok()?,
                    x: p.get(2)?.parse().ok()?,
                    y: p.get(3)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("WHIT|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 3 {
                return Some(Event::WeaponHit {
                    attacker: p.first()?.parse().ok()?,
                    victim: p.get(1)?.parse().ok()?,
                    weapon: p.get(2)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("HAMMER|")
            && let Some((a, v)) = rest.split_once('|')
        {
            return Some(Event::HammerHit {
                attacker: a.to_string(),
                victim: v.to_string(),
            });
        }
        if let Some(rest) = body.strip_prefix("PICKUP|") {
            let p: Vec<&str> = rest.split_whitespace().collect();
            if p.len() >= 2 {
                return Some(Event::Pickup {
                    cid: p.first()?.parse().ok()?,
                    kind: p.get(1)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("WPN_CHG|") {
            let p: Vec<&str> = rest.split(|c: char| c == ':' || c == '|' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
            if p.len() >= 3 {
                return Some(Event::WeaponChange {
                    cid: p.first()?.parse().ok()?,
                    old: p.get(1)?.parse().ok()?,
                    new: p.get(2)?.parse().ok()?,
                });
            }
        }
        if let Some(rest) = body.strip_prefix("HOOK_ATTACH|") {
            let p: Vec<&str> = rest.split(':').collect();
            if p.len() >= 4 {
                let cid: i32 = p[0].parse().ok()?;
                let raw_hooked: i32 = p[1].parse().ok()?;
                let x: f32 = p[2].parse().ok()?;
                let y: f32 = p[3].parse().ok()?;
                return Some(Event::HookAttach {
                    cid,
                    hooked_cid: if raw_hooked >= 0 { Some(raw_hooked) } else { None },
                    pos: crate::dx::Vec2::new(x, y),
                });
            }
        }
        if let Some(rest) = body.strip_prefix("HOOK_DETACH|") {
            let p: Vec<&str> = rest.split(':').collect();
            if p.len() >= 2 {
                let cid: i32 = p[0].parse().ok()?;
                let raw_prev: i32 = p[1].parse().ok()?;
                return Some(Event::HookDetach {
                    cid,
                    prev_hooked_cid: if raw_prev >= 0 { Some(raw_prev) } else { None },
                });
            }
        }
        None
    }
}

/// Объявляет точку входа плагина DDNetPR, регистрацию, конфигурацию, состояние и цикл событий.
#[macro_export]
macro_rules! plugin {
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? handler: $h:expr $(,)?) => {
        fn main() {
            let mut api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { api.register_admin_plugin($n, $d); } else { api.register_plugin($n, $d); }
            $( api.set_priority($p); )? $( $( api.register_cmd($c, $ca, $cd); )* )?
            api.run_handler($h);
        }
    };
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? config: $ct:ty, $(state: { $( $sn:ident : $st:ty = $sv:expr ),* $(,)? },)? intercept($api:ident, $cfg:ident, $ev:ident) $b:block) => {
        fn main() {
            let mut $api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { $api.register_admin_plugin($n, $d); } else { $api.register_plugin($n, $d); }
            $( $api.set_priority($p); )? $( $( $api.register_cmd($c, $ca, $cd); )* )?
            let mut $cfg = <$ct as $crate::PluginConfig>::load_or_create($n);
            $( $( let mut $sn: $st = $sv; )* )?
            $api.run_intercept(move |$api, $ev| $b);
        }
    };
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? config: $ct:ty, $(state: { $( $sn:ident : $st:ty = $sv:expr ),* $(,)? },)? handle($api:ident, $cfg:ident, $ev:ident) $b:block) => {
        fn main() {
            let mut $api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { $api.register_admin_plugin($n, $d); } else { $api.register_plugin($n, $d); }
            $( $api.set_priority($p); )? $( $( $api.register_cmd($c, $ca, $cd); )* )?
            let mut $cfg = <$ct as $crate::PluginConfig>::load_or_create($n);
            $( $( let mut $sn: $st = $sv; )* )?
            $api.run(move |$api, $ev| $b);
        }
    };
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? $(state: { $( $sn:ident : $st:ty = $sv:expr ),* $(,)? },)? intercept($api:ident, $ev:ident) $b:block) => {
        fn main() {
            let mut $api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { $api.register_admin_plugin($n, $d); } else { $api.register_plugin($n, $d); }
            $( $api.set_priority($p); )? $( $( $api.register_cmd($c, $ca, $cd); )* )? $( $( let mut $sn: $st = $sv; )* )?
            $api.run_intercept(move |$api, $ev| $b);
        }
    };
    (name: $n:expr, desc: $d:expr, $(admin: $a:expr,)? $(priority: $p:expr,)? $(cmds: [ $( ($c:expr, $ca:expr, $cd:expr) ),* $(,)? ],)? $(state: { $( $sn:ident : $st:ty = $sv:expr ),* $(,)? },)? handle($api:ident, $ev:ident) $b:block) => {
        fn main() {
            let mut $api = $crate::ServerApi::new();
            let mut _adm = false; $( _adm = $a; )?
            if _adm { $api.register_admin_plugin($n, $d); } else { $api.register_plugin($n, $d); }
            $( $api.set_priority($p); )? $( $( $api.register_cmd($c, $ca, $cd); )* )? $( $( let mut $sn: $st = $sv; )* )?
            $api.run(move |$api, $ev| $b);
        }
    };
}
