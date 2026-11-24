use crate::dx::{PlayerId, Vec2};
use crate::{PlayerPos, ServerApi};

impl Vec2 {
    /// Squared Euclidean distance to another point.
    pub fn distance_sq(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// Euclidean distance to another point in pixels.
    pub fn distance(self, other: Self) -> f32 {
        self.distance_sq(other).sqrt()
    }

    /// Vector length (magnitude).
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Returns a unit-length vector, or `(0.0, 0.0)` if zero-length.
    pub fn normalized(self) -> Self {
        let len = self.length();
        if len > 1e-6 {
            Self { x: self.x / len, y: self.y / len }
        } else {
            Self::default()
        }
    }

    /// Dot product with another vector.
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Subtracts `other` from `self`.
    pub fn sub(self, other: Self) -> Self {
        Self { x: self.x - other.x, y: self.y - other.y }
    }
}

impl ServerApi {
    /// Returns the world position of an alive cached player, if available.
    pub fn player_pos(&self, cid: PlayerId) -> Option<Vec2> {
        self.states.get(&cid.0).filter(|p| p.alive).map(|p| Vec2::new(p.x, p.y))
    }

    /// Returns alive players within `radius` pixels of `center`, sorted nearest-first.
    pub fn players_in_radius(
        &self,
        center: Vec2,
        radius: f32,
        exclude: Option<PlayerId>,
    ) -> Vec<(PlayerId, PlayerPos, f32)> {
        let r_sq = radius.max(0.0) * radius.max(0.0);
        let mut out: Vec<(PlayerId, PlayerPos, f32)> = self
            .states
            .iter()
            .filter(|&(&cid, pos)| pos.alive && exclude.map_or(true, |ex| ex.0 != cid))
            .filter_map(|(&cid, &pos)| {
                let d_sq = center.distance_sq(Vec2::new(pos.x, pos.y));
                (d_sq <= r_sq).then(|| (PlayerId(cid), pos, d_sq.sqrt()))
            })
            .collect();
        out.sort_by(|a, b| a.2.total_cmp(&b.2));
        out
    }

    /// Returns the closest alive player within `max_radius` pixels of `center`.
    pub fn nearest_player(
        &self,
        center: Vec2,
        max_radius: f32,
        exclude: Option<PlayerId>,
    ) -> Option<(PlayerId, PlayerPos, f32)> {
        self.players_in_radius(center, max_radius, exclude).into_iter().next()
    }

    /// Checks whether the segment `from -> to` is unobstructed by other alive players.
    pub fn line_of_sight(
        &self,
        from: Vec2,
        to: Vec2,
        body_radius: f32,
        exclude: &[PlayerId],
    ) -> bool {
        let seg = to.sub(from);
        let seg_len_sq = seg.x * seg.x + seg.y * seg.y;
        let r_sq = body_radius.max(0.0) * body_radius.max(0.0);
        for (&cid, pos) in &self.states {
            if !pos.alive || exclude.iter().any(|e| e.0 == cid) {
                continue;
            }
            let p = Vec2::new(pos.x, pos.y);
            let t = if seg_len_sq > 1e-6 {
                (p.sub(from).dot(seg) / seg_len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let proj = Vec2::new(from.x + seg.x * t, from.y + seg.y * t);
            if p.distance_sq(proj) <= r_sq {
                return false;
            }
        }
        true
    }

    /// Invokes `f(cid, pos, dist)` for every alive player inside a forward cone.
    pub fn for_each_player_in_cone<F>(
        &self,
        origin: Vec2,
        dir: Vec2,
        max_dist: f32,
        half_angle_rad: f32,
        exclude: Option<PlayerId>,
        mut f: F,
    ) where
        F: FnMut(PlayerId, PlayerPos, f32),
    {
        let fwd = dir.normalized();
        if fwd.length() <= 1e-6 {
            return;
        }
        let cos_thresh = half_angle_rad.clamp(0.0, std::f32::consts::PI).cos();
        for (cid, pos, dist) in self.players_in_radius(origin, max_dist, exclude) {
            if dist <= 1e-6 {
                f(cid, pos, dist);
                continue;
            }
            let to_p = Vec2::new(pos.x - origin.x, pos.y - origin.y).normalized();
            if fwd.dot(to_p) >= cos_thresh {
                f(cid, pos, dist);
            }
        }
    }
}
