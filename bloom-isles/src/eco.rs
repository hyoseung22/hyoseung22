//! Species and who-likes-whom.

use crate::island::Terr;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sp {
    Flower,
    Bush,
    Tree,
    Palm,
    Lily,
    Mushroom,
    Bee,
    Rabbit,
    Fox,
    Bird,
    Frog,
    Crab,
}

pub const ALL: [Sp; 12] = [
    Sp::Flower,
    Sp::Bush,
    Sp::Tree,
    Sp::Palm,
    Sp::Lily,
    Sp::Mushroom,
    Sp::Bee,
    Sp::Rabbit,
    Sp::Fox,
    Sp::Bird,
    Sp::Frog,
    Sp::Crab,
];

impl Sp {
    pub fn idx(self) -> usize {
        self as usize
    }
}

/// `sp` within `rad` tiles changes happiness by `w` each, counted up to `cap`.
pub struct Need {
    pub sp: Sp,
    pub rad: u8,
    pub w: f32,
    pub cap: u8,
}

pub struct SpDef {
    pub plant: bool,
    pub terr: &'static [Terr],
    pub base: f32,
    pub terr_bonus: &'static [(Terr, f32)],
    pub needs: &'static [Need],
    /// Height of the drawing above its ground point, in island units.
    pub head: f32,
}

const fn n(sp: Sp, rad: u8, w: f32, cap: u8) -> Need {
    Need { sp, rad, w, cap }
}

use Terr::*;
const LAND: &[Terr] = &[Grass, Mead];
const LAND_SAND: &[Terr] = &[Grass, Mead, Sand];

static DEFS: [SpDef; 12] = [
    SpDef { plant: true, terr: LAND, base: 0.45, terr_bonus: &[(Mead, 0.1)], needs: &[n(Sp::Bee, 2, 0.22, 2), n(Sp::Flower, 1, 0.06, 2), n(Sp::Rabbit, 1, -0.08, 3)], head: 24.0 },
    SpDef { plant: true, terr: LAND, base: 0.45, terr_bonus: &[], needs: &[n(Sp::Bird, 2, 0.2, 2), n(Sp::Bee, 2, 0.1, 1), n(Sp::Rabbit, 1, -0.06, 3)], head: 24.0 },
    SpDef { plant: true, terr: LAND, base: 0.45, terr_bonus: &[(Grass, 0.05)], needs: &[n(Sp::Bird, 1, 0.15, 2), n(Sp::Mushroom, 1, 0.15, 2), n(Sp::Tree, 1, 0.05, 2)], head: 48.0 },
    SpDef { plant: true, terr: &[Sand], base: 0.5, terr_bonus: &[], needs: &[n(Sp::Crab, 1, 0.25, 2), n(Sp::Palm, 1, 0.04, 2)], head: 40.0 },
    SpDef { plant: true, terr: &[Pond], base: 0.45, terr_bonus: &[], needs: &[n(Sp::Frog, 1, 0.3, 2), n(Sp::Lily, 1, 0.05, 2)], head: 10.0 },
    SpDef { plant: true, terr: LAND, base: 0.2, terr_bonus: &[], needs: &[n(Sp::Tree, 1, 0.35, 2), n(Sp::Frog, 2, 0.05, 1)], head: 20.0 },
    SpDef { plant: false, terr: LAND_SAND, base: 0.3, terr_bonus: &[], needs: &[n(Sp::Flower, 1, 0.15, 4), n(Sp::Bush, 1, 0.08, 2), n(Sp::Frog, 1, -0.15, 2)], head: 34.0 },
    SpDef { plant: false, terr: LAND, base: 0.35, terr_bonus: &[], needs: &[n(Sp::Bush, 1, 0.15, 2), n(Sp::Flower, 1, 0.08, 2), n(Sp::Tree, 1, 0.05, 1), n(Sp::Fox, 2, -0.3, 3)], head: 32.0 },
    SpDef { plant: false, terr: LAND, base: 0.25, terr_bonus: &[], needs: &[n(Sp::Rabbit, 2, 0.18, 3), n(Sp::Tree, 1, 0.08, 2), n(Sp::Fox, 2, -0.15, 3)], head: 30.0 },
    SpDef { plant: false, terr: LAND_SAND, base: 0.3, terr_bonus: &[], needs: &[n(Sp::Tree, 1, 0.18, 3), n(Sp::Bush, 1, 0.1, 2), n(Sp::Palm, 1, 0.1, 2), n(Sp::Fox, 1, -0.1, 2)], head: 22.0 },
    SpDef { plant: false, terr: &[Pond], base: 0.3, terr_bonus: &[], needs: &[n(Sp::Lily, 1, 0.2, 2), n(Sp::Bee, 2, 0.12, 2), n(Sp::Frog, 1, -0.05, 3)], head: 18.0 },
    SpDef { plant: false, terr: &[Sand], base: 0.35, terr_bonus: &[], needs: &[n(Sp::Palm, 1, 0.25, 2), n(Sp::Crab, 1, 0.05, 1), n(Sp::Bird, 1, -0.1, 2)], head: 16.0 },
];

pub fn def(sp: Sp) -> &'static SpDef {
    &DEFS[sp.idx()]
}

/// How much `b` at distance `d` affects `a`.
pub fn need_w(a: Sp, b: Sp, d: i32) -> f32 {
    def(a).needs.iter().find(|n| n.sp == b && d <= n.rad as i32).map(|n| n.w).unwrap_or(0.0)
}

/// Thriving creatures slowly bring new life nearby.
pub struct SpawnRule {
    pub makes: &'static [Sp],
    pub rad: u8,
    pub p: f32,
    pub needs_pair: bool,
}

pub fn spawn_rule(sp: Sp) -> Option<SpawnRule> {
    let r = |makes: &'static [Sp], rad, p, needs_pair| Some(SpawnRule { makes, rad, p, needs_pair });
    match sp {
        Sp::Bee => r(&[Sp::Flower], 1, 0.55, false),
        Sp::Bird => r(&[Sp::Bush, Sp::Tree], 2, 0.4, false),
        Sp::Frog => r(&[Sp::Lily], 1, 0.5, false),
        Sp::Tree => r(&[Sp::Mushroom], 1, 0.3, false),
        Sp::Rabbit => r(&[Sp::Rabbit], 1, 0.2, true),
        Sp::Crab => r(&[Sp::Crab], 1, 0.18, false),
        Sp::Flower => r(&[Sp::Flower], 1, 0.14, false),
        Sp::Palm => r(&[Sp::Palm], 1, 0.15, false),
        _ => None,
    }
}
