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
        self.0 >= 0
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
