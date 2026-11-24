use std::fmt;

/// Typed client identifier (CID) preventing argument mix-ups.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PlayerId(pub i32);

impl PlayerId {
    /// Returns true if the identifier is non-negative.
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

/// 2D world coordinates in pixels.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec2 {
    /// Horizontal X coordinate in pixels.
    pub x: f32,
    /// Vertical Y coordinate in pixels.
    pub y: f32,
}

impl Vec2 {
    /// Creates a new 2D vector.
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

/// DDNet weapon identifiers.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weapon {
    /// Hammer (slot 0).
    Hammer = 0,
    /// Pistol / Gun (slot 1).
    Gun = 1,
    /// Shotgun (slot 2).
    Shotgun = 2,
    /// Grenade launcher (slot 3).
    Grenade = 3,
    /// Laser / Rifle (slot 4).
    Laser = 4,
    /// Ninja katana (slot 5).
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

/// Bitmask controlling raycast collision targets.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct RayMask(pub u32);

impl RayMask {
    /// Collide with solid map tiles (bit 0).
    pub const TILE: Self = Self(1);
    /// Collide with alive active players (bit 1).
    pub const PLAYER: Self = Self(2);
    /// Collide with both map tiles and players.
    pub const BOTH: Self = Self(3);
}

/// Result of a synchronous world raycast query.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RayHit {
    /// Hit tile index (0 if player hit).
    pub tile: i32,
    /// Hit player identifier (invalid if tile hit).
    pub cid: PlayerId,
    /// World X coordinate of impact in pixels.
    pub x: f32,
    /// World Y coordinate of impact in pixels.
    pub y: f32,
    /// Distance from ray origin to impact in pixels.
    pub dist: f32,
}

/// Unified error type returned by v10.0+ SDK primitives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdkError {
    /// Plugin manifest lacks the required capability flag.
    PermissionDenied {
        /// Name of the missing permission flag.
        needed: &'static str,
    },
    /// Provided client ID is outside valid server bounds.
    InvalidCid(PlayerId),
    /// Coordinates or slot index are outside valid boundaries.
    OutOfBounds {
        /// Horizontal coordinate or slot index.
        x: i32,
        /// Vertical coordinate.
        y: i32,
    },
    /// Server-wide projectile cap has been reached.
    TooManyProjectiles {
        /// Maximum active projectiles allowed.
        limit: usize,
    },
    /// Target player is not currently online.
    NotOnline(PlayerId),
    /// Argument value failed range or format validation.
    InvalidArg {
        /// Parameter name.
        name: &'static str,
        /// Reason for rejection.
        reason: &'static str,
    },
}

impl SdkError {
    /// Returns an actionable troubleshooting hint for this error.
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

/// Convenient re-exports for DDNet++ v10.0+ plugins.
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
