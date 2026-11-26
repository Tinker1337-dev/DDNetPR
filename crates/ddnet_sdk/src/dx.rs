use std::fmt;

/// Типизированный идентификатор клиента (CID), предотвращающий путаницу аргументов.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PlayerId(pub i32);

impl PlayerId {
    /// Возвращает true, если идентификатор неотрицательный.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// assert!(PlayerId(0).is_valid());
    /// assert!(!PlayerId(-1).is_valid());
    /// ```
    pub fn is_valid(self) -> bool {
        (0..MAX_CLIENTS).contains(&self.0)
    }
}

impl From<i32> for PlayerId {
    fn from(v: i32) -> Self {
        Self(v)
    }
}
impl From<PlayerId> for i32 {
    fn from(v: PlayerId) -> Self {
        v.0
    }
}
impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Двумерные мировые координаты в пикселях.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec2 {
    /// Горизонтальная координата X в пикселях.
    pub x: f32,
    /// Вертикальная координата Y в пикселях.
    pub y: f32,
}

impl Vec2 {
    /// Создаёт новый двумерный вектор.
    ///
    /// # Examples
    /// ```
    /// use ddnet_sdk::prelude::*;
    /// let p = Vec2::new(64.0, 128.0);
    /// assert_eq!(p.x, 64.0);
    /// ```
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Идентификаторы оружия DDNet.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weapon {
    /// Молот (слот 0).
    Hammer = 0,
    /// Пистолет (слот 1).
    Gun = 1,
    /// Дробовик (слот 2).
    Shotgun = 2,
    /// Гранатомёт (слот 3).
    Grenade = 3,
    /// Лазерная винтовка (слот 4).
    Laser = 4,
    /// Катана ниндзя (слот 5).
    Katana = 5,
}

impl From<i32> for Weapon {
    fn from(v: i32) -> Self {
        match v {
            1 => Self::Gun,
            2 => Self::Shotgun,
            3 => Self::Grenade,
            4 => Self::Laser,
            5 => Self::Katana,
            _ => Self::Hammer,
        }
    }
}
impl From<Weapon> for i32 {
    fn from(w: Weapon) -> Self {
        w as i32
    }
}

/// Битовая маска целей столкновения при трассировке луча (raycast).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct RayMask(pub u32);

impl RayMask {
    /// Столкновение с твёрдыми тайлами карты (бит 0).
    pub const TILE: Self = Self(1);
    /// Столкновение с живыми активными игроками (бит 1).
    pub const PLAYER: Self = Self(2);
    /// Столкновение как с тайлами карты, так и с игроками.
    pub const BOTH: Self = Self(3);
}

/// Результат синхронного запроса трассировки луча (raycast).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RayHit {
    /// Индекс задетого тайла (0, если попадание в игрока).
    pub tile: i32,
    /// Идентификатор задетого игрока (невалидный, если попадание в тайл).
    pub cid: PlayerId,
    /// Мировая координата X точки попадания в пикселях.
    pub x: f32,
    /// Мировая координата Y точки попадания в пикселях.
    pub y: f32,
    /// Расстояние от начала луча до точки попадания в пикселях.
    pub dist: f32,
}

/// Единый тип ошибок, возвращаемый примитивами SDK v10.0+.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdkError {
    /// В манифесте плагина отсутствует необходимый флаг разрешения.
    PermissionDenied {
        /// Имя отсутствующего флага разрешения.
        needed: &'static str,
    },
    /// Переданный идентификатор клиента (CID) выходит за допустимые пределы сервера.
    InvalidCid(PlayerId),
    /// Координаты или индекс слота выходят за допустимые границы.
    OutOfBounds {
        /// Горизонтальная координата или индекс слота.
        x: i32,
        /// Вертикальная координата.
        y: i32,
    },
    /// Достигнут общесерверный лимит снарядов.
    TooManyProjectiles {
        /// Максимально допустимое количество активных снарядов.
        limit: usize,
    },
    /// Целевой игрок сейчас не в сети.
    NotOnline(PlayerId),
    /// Значение аргумента не прошло проверку диапазона или формата.
    InvalidArg {
        /// Имя параметра.
        name: &'static str,
        /// Причина отклонения.
        reason: &'static str,
    },
}

impl SdkError {
    /// Возвращает практическую подсказку по устранению этой ошибки.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::PermissionDenied { .. } => "enable missing flag via api.set_manifest(...)",
            Self::InvalidCid(_) => "use a valid client ID in 0..64 from Event or api.cids",
            Self::OutOfBounds { .. } => "check map coordinates or HUD slot range (0..4)",
            Self::TooManyProjectiles { .. } => "use spawn_visual_pickup/spawn_visual_laser (0 FX/s)",
            Self::NotOnline(_) => "check player presence in api.online or api.player_cids",
            Self::InvalidArg { .. } => "check parameter bounds and non-empty string requirements",
        }
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied { needed } => write!(f, "permission denied: {} (hint: {})", needed, self.hint()),
            Self::InvalidCid(id) => write!(f, "invalid client id: {} (hint: {})", id, self.hint()),
            Self::OutOfBounds { x, y } => write!(f, "out of bounds: ({}, {}) (hint: {})", x, y, self.hint()),
            Self::TooManyProjectiles { limit } => write!(f, "too many projectiles: limit {} (hint: {})", limit, self.hint()),
            Self::NotOnline(id) => write!(f, "player not online: {} (hint: {})", id, self.hint()),
            Self::InvalidArg { name, reason } => write!(f, "invalid argument {}: {} (hint: {})", name, reason, self.hint()),
        }
    }
}
/// Максимальное число клиентских слотов в C++-ядре DDNet.
pub const MAX_CLIENTS: i32 = 64;
/// Количество слотов оружия в ядре (`0..=5`).
pub const MAX_WEAPONS: i32 = 6;
/// Количество слотов пользовательского HUD (`0..=4`).
pub const MAX_HUD_SLOTS: u8 = 5;
/// Максимальное число визуальных пикапов на плагин (`0..128`).
pub const MAX_VISUAL_PICKUPS: i32 = 128;
/// Максимальное число визуальных лазеров на плагин (`0..128`).
pub const MAX_VISUAL_LASERS: i32 = 128;
/// Максимальное число визуальных ти в ядре (`0..64`).
pub const MAX_VISUAL_TEES: i32 = 64;
/// Максимальное число векторных текстов в ядре (`0..32`).
pub const MAX_VISUAL_TEXTS: i32 = 32;
/// Максимальная длительность заморозки в секундах (защита от переполнения тиков).
pub const MAX_FREEZE_SECS: i32 = 3600;
/// Максимальный идентификатор игрового режима (`0..=3`).
pub const MAX_GAMEMODE: i32 = 3;
/// Предельная координата карты в пикселях (~1024 тайла).
pub const MAX_COORD: f32 = 32768.0;
/// Предельная проекция скорости/импульса.
pub const MAX_SPEED: f32 = 500.0;
/// Предельное абсолютное значение очков.
pub const MAX_SCORE: i32 = 1_000_000;

pub(crate) fn validate_cid(cid: i32) -> Result<(), SdkError> {
    if (0..MAX_CLIENTS).contains(&cid) { Ok(()) } else { Err(SdkError::InvalidCid(PlayerId(cid))) }
}
pub(crate) fn validate_sound_cid(cid: i32) -> Result<(), SdkError> {
    if cid == -1 || (0..MAX_CLIENTS).contains(&cid) { Ok(()) } else { Err(SdkError::InvalidCid(PlayerId(cid))) }
}
pub(crate) fn validate_weapon(w: i32) -> Result<(), SdkError> {
    if (0..MAX_WEAPONS).contains(&w) { Ok(()) } else { Err(SdkError::InvalidArg { name: "weapon", reason: "weapon must be in 0..=5" }) }
}
pub(crate) fn validate_ammo(ammo: i32) -> Result<(), SdkError> {
    if (-1..=9999).contains(&ammo) { Ok(()) } else { Err(SdkError::InvalidArg { name: "ammo", reason: "ammo must be -1 (remove) or 0..=9999" }) }
}
pub(crate) fn validate_visual_id(id: i32, limit: i32, name: &'static str) -> Result<(), SdkError> {
    if (0..limit).contains(&id) { Ok(()) } else { Err(SdkError::InvalidArg { name, reason: "visual id out of pool range" }) }
}
pub(crate) fn validate_coords(x: f32, y: f32) -> Result<(), SdkError> {
    if x.is_finite() && y.is_finite() && x.abs() <= MAX_COORD && y.abs() <= MAX_COORD { Ok(()) } else { Err(SdkError::OutOfBounds { x: x as i32, y: y as i32 }) }
}
pub(crate) fn validate_speed(vx: f32, vy: f32) -> Result<(), SdkError> {
    if vx.is_finite() && vy.is_finite() && vx.abs() <= MAX_SPEED && vy.abs() <= MAX_SPEED { Ok(()) } else { Err(SdkError::InvalidArg { name: "vel", reason: "velocity components must be finite and within +-500" }) }
}
pub(crate) fn validate_freeze_secs(secs: i32) -> Result<(), SdkError> {
    if (0..=MAX_FREEZE_SECS).contains(&secs) { Ok(()) } else { Err(SdkError::InvalidArg { name: "freeze_secs", reason: "freeze duration must be in 0..=3600" }) }
}
pub(crate) fn validate_gamemode(mode: i32) -> Result<(), SdkError> {
    if (0..=MAX_GAMEMODE).contains(&mode) { Ok(()) } else { Err(SdkError::InvalidArg { name: "gamemode", reason: "gamemode must be in 0..=3" }) }
}
pub(crate) fn validate_score(score: i32) -> Result<(), SdkError> {
    if (-MAX_SCORE..=MAX_SCORE).contains(&score) { Ok(()) } else { Err(SdkError::InvalidArg { name: "score", reason: "score must be within -1_000_000..=1_000_000" }) }
}

impl std::error::Error for SdkError {}

/// Удобные реэкспорты для плагинов DDNet++ v10.0+.
///
/// # Examples
/// ```
/// use ddnet_sdk::prelude::*;
/// let id = PlayerId(1);
/// let w = Weapon::Laser;
/// assert_eq!(i32::from(w), 4);
/// assert!(id.is_valid());
/// ```
pub mod prelude {
    pub use crate::dx::{PlayerId, RayHit, RayMask, SdkError, Vec2, Weapon};
    pub use crate::{Event, EventAction, EventHandler, PluginConfig, Priority, ServerApi, log_debug, log_error, log_info, log_warn, plugin_config};
}
