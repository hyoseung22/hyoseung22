//! Species, who-likes-whom, and the environments that biome cards create.

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
    Fern,
    Pine,
    Cactus,
    Reed,
    Fireweed,
    Deer,
    Goat,
    Duck,
    Fish,
    Lizard,
    Camel,
}

pub const N: usize = 23;

pub const ALL: [Sp; N] = [
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
    Sp::Fern,
    Sp::Pine,
    Sp::Cactus,
    Sp::Reed,
    Sp::Fireweed,
    Sp::Deer,
    Sp::Goat,
    Sp::Duck,
    Sp::Fish,
    Sp::Lizard,
    Sp::Camel,
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
const FLYER: &[Terr] = &[Grass, Mead, Sand, Snow, Ash, Dune];
const COLD_LAND: &[Terr] = &[Grass, Mead, Snow];
const WATER: &[Terr] = &[Pond, Lake];

static DEFS: [SpDef; N] = [
    // flower
    SpDef { plant: true, terr: LAND, base: 0.45, terr_bonus: &[(Mead, 0.1)], needs: &[n(Sp::Bee, 2, 0.22, 2), n(Sp::Flower, 1, 0.06, 2), n(Sp::Rabbit, 1, -0.08, 3), n(Sp::Goat, 1, -0.1, 2)], head: 24.0 },
    // bush
    SpDef { plant: true, terr: LAND, base: 0.45, terr_bonus: &[], needs: &[n(Sp::Bird, 2, 0.2, 2), n(Sp::Bee, 2, 0.1, 1), n(Sp::Rabbit, 1, -0.06, 3), n(Sp::Deer, 1, -0.05, 2)], head: 24.0 },
    // tree
    SpDef { plant: true, terr: LAND, base: 0.45, terr_bonus: &[(Grass, 0.05)], needs: &[n(Sp::Bird, 1, 0.15, 2), n(Sp::Mushroom, 1, 0.15, 2), n(Sp::Tree, 1, 0.05, 2), n(Sp::Fern, 1, 0.05, 2)], head: 48.0 },
    // palm
    SpDef { plant: true, terr: &[Sand, Dune], base: 0.5, terr_bonus: &[], needs: &[n(Sp::Crab, 1, 0.25, 2), n(Sp::Palm, 1, 0.04, 2), n(Sp::Camel, 1, 0.12, 1)], head: 40.0 },
    // lily
    SpDef { plant: true, terr: WATER, base: 0.45, terr_bonus: &[], needs: &[n(Sp::Frog, 1, 0.3, 2), n(Sp::Lily, 1, 0.05, 2), n(Sp::Fish, 1, 0.15, 2), n(Sp::Duck, 1, -0.05, 2)], head: 10.0 },
    // mushroom
    SpDef { plant: true, terr: LAND, base: 0.2, terr_bonus: &[], needs: &[n(Sp::Tree, 1, 0.35, 2), n(Sp::Pine, 1, 0.3, 2), n(Sp::Frog, 2, 0.05, 1), n(Sp::Fern, 1, 0.06, 2)], head: 20.0 },
    // bee
    SpDef { plant: false, terr: LAND_SAND, base: 0.3, terr_bonus: &[], needs: &[n(Sp::Flower, 1, 0.15, 4), n(Sp::Bush, 1, 0.08, 2), n(Sp::Fireweed, 1, 0.15, 3), n(Sp::Cactus, 1, 0.08, 2), n(Sp::Frog, 1, -0.15, 2)], head: 34.0 },
    // rabbit
    SpDef { plant: false, terr: COLD_LAND, base: 0.35, terr_bonus: &[], needs: &[n(Sp::Bush, 1, 0.15, 2), n(Sp::Flower, 1, 0.08, 2), n(Sp::Tree, 1, 0.05, 1), n(Sp::Fox, 2, -0.3, 3)], head: 32.0 },
    // fox
    SpDef { plant: false, terr: COLD_LAND, base: 0.25, terr_bonus: &[], needs: &[n(Sp::Rabbit, 2, 0.18, 3), n(Sp::Tree, 1, 0.08, 2), n(Sp::Pine, 1, 0.08, 2), n(Sp::Fox, 2, -0.15, 3)], head: 30.0 },
    // bird
    SpDef { plant: false, terr: FLYER, base: 0.3, terr_bonus: &[], needs: &[n(Sp::Tree, 1, 0.18, 3), n(Sp::Bush, 1, 0.1, 2), n(Sp::Palm, 1, 0.1, 2), n(Sp::Pine, 1, 0.12, 2), n(Sp::Fox, 1, -0.1, 2)], head: 22.0 },
    // frog
    SpDef { plant: false, terr: WATER, base: 0.3, terr_bonus: &[(Pond, 0.05)], needs: &[n(Sp::Lily, 1, 0.2, 2), n(Sp::Reed, 1, 0.1, 2), n(Sp::Bee, 2, 0.12, 2), n(Sp::Frog, 1, -0.05, 3), n(Sp::Duck, 1, -0.08, 2)], head: 18.0 },
    // crab
    SpDef { plant: false, terr: &[Sand], base: 0.35, terr_bonus: &[], needs: &[n(Sp::Palm, 1, 0.25, 2), n(Sp::Crab, 1, 0.05, 1), n(Sp::Bird, 1, -0.1, 2)], head: 16.0 },
    // fern: a shade lover under trees
    SpDef { plant: true, terr: LAND, base: 0.3, terr_bonus: &[], needs: &[n(Sp::Tree, 1, 0.2, 2), n(Sp::Pine, 1, 0.2, 2), n(Sp::Mushroom, 1, 0.08, 2), n(Sp::Deer, 1, -0.06, 2)], head: 20.0 },
    // pine: loves the cold
    SpDef { plant: true, terr: &[Snow, Grass], base: 0.4, terr_bonus: &[(Snow, 0.14)], needs: &[n(Sp::Bird, 1, 0.12, 2), n(Sp::Pine, 1, 0.08, 2), n(Sp::Goat, 2, 0.06, 2), n(Sp::Mushroom, 1, 0.05, 1)], head: 46.0 },
    // cactus
    SpDef { plant: true, terr: &[Dune, Sand, Ash], base: 0.45, terr_bonus: &[(Dune, 0.12)], needs: &[n(Sp::Lizard, 1, 0.18, 2), n(Sp::Cactus, 1, 0.05, 2), n(Sp::Bee, 2, 0.08, 1), n(Sp::Camel, 1, -0.06, 2)], head: 30.0 },
    // reed
    SpDef { plant: true, terr: WATER, base: 0.45, terr_bonus: &[], needs: &[n(Sp::Duck, 1, 0.2, 2), n(Sp::Frog, 1, 0.1, 2), n(Sp::Reed, 1, 0.05, 2), n(Sp::Fish, 1, 0.06, 2)], head: 28.0 },
    // fireweed: first bloom on fresh ash
    SpDef { plant: true, terr: &[Ash], base: 0.5, terr_bonus: &[], needs: &[n(Sp::Lizard, 1, 0.15, 2), n(Sp::Bee, 2, 0.15, 2), n(Sp::Fireweed, 1, 0.05, 2)], head: 22.0 },
    // deer
    SpDef { plant: false, terr: COLD_LAND, base: 0.3, terr_bonus: &[], needs: &[n(Sp::Fern, 1, 0.15, 2), n(Sp::Tree, 1, 0.12, 2), n(Sp::Pine, 1, 0.12, 2), n(Sp::Deer, 1, 0.05, 1), n(Sp::Fox, 2, -0.2, 2)], head: 40.0 },
    // goat: happiest on rocks and snow
    SpDef { plant: false, terr: &[Snow, Rock, Grass], base: 0.35, terr_bonus: &[(Rock, 0.12), (Snow, 0.08)], needs: &[n(Sp::Pine, 1, 0.12, 2), n(Sp::Goat, 1, 0.06, 2), n(Sp::Fox, 2, -0.12, 2)], head: 32.0 },
    // duck
    SpDef { plant: false, terr: WATER, base: 0.35, terr_bonus: &[(Lake, 0.05)], needs: &[n(Sp::Reed, 1, 0.18, 2), n(Sp::Lily, 1, 0.1, 2), n(Sp::Duck, 1, 0.06, 2), n(Sp::Fish, 2, 0.08, 2), n(Sp::Fox, 2, -0.15, 2)], head: 22.0 },
    // fish: deep water only
    SpDef { plant: false, terr: &[Lake], base: 0.4, terr_bonus: &[], needs: &[n(Sp::Lily, 1, 0.15, 2), n(Sp::Reed, 1, 0.15, 2), n(Sp::Fish, 1, 0.06, 2), n(Sp::Bird, 1, -0.08, 2)], head: 14.0 },
    // lizard: sun, rock and ash
    SpDef { plant: false, terr: &[Ash, Dune, Sand, Rock], base: 0.35, terr_bonus: &[(Rock, 0.08), (Ash, 0.08)], needs: &[n(Sp::Cactus, 1, 0.18, 2), n(Sp::Fireweed, 1, 0.15, 2), n(Sp::Bird, 1, -0.12, 2)], head: 14.0 },
    // camel
    SpDef { plant: false, terr: &[Dune, Sand], base: 0.35, terr_bonus: &[(Dune, 0.1)], needs: &[n(Sp::Palm, 1, 0.2, 2), n(Sp::Cactus, 1, 0.1, 2), n(Sp::Camel, 2, 0.05, 1)], head: 40.0 },
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
        Sp::Bee => r(&[Sp::Flower, Sp::Fireweed], 1, 0.55, false),
        Sp::Bird => r(&[Sp::Bush, Sp::Tree, Sp::Pine], 2, 0.4, false),
        Sp::Frog => r(&[Sp::Lily, Sp::Reed], 1, 0.5, false),
        Sp::Tree => r(&[Sp::Mushroom, Sp::Fern], 1, 0.3, false),
        Sp::Pine => r(&[Sp::Pine, Sp::Fern], 1, 0.2, false),
        Sp::Rabbit => r(&[Sp::Rabbit], 1, 0.2, true),
        Sp::Crab => r(&[Sp::Crab], 1, 0.18, false),
        Sp::Flower => r(&[Sp::Flower], 1, 0.14, false),
        Sp::Palm => r(&[Sp::Palm], 1, 0.15, false),
        Sp::Fish => r(&[Sp::Fish], 1, 0.25, true),
        Sp::Duck => r(&[Sp::Reed], 1, 0.3, false),
        Sp::Cactus => r(&[Sp::Cactus], 1, 0.12, false),
        Sp::Fireweed => r(&[Sp::Fireweed], 1, 0.2, false),
        Sp::Lizard => r(&[Sp::Cactus], 1, 0.15, false),
        Sp::Goat => r(&[Sp::Goat], 1, 0.12, true),
        Sp::Deer => r(&[Sp::Fern], 1, 0.2, false),
        _ => None,
    }
}

/// Environments a biome card raises on the island.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Biome {
    Volcano,
    Lake,
    Peak,
    Desert,
}

pub const BIOMES: [Biome; 4] = [Biome::Volcano, Biome::Lake, Biome::Peak, Biome::Desert];

impl Biome {
    pub fn idx(self) -> usize {
        self as usize
    }
    /// Terrain for the centre tile and for the ring around it.
    pub fn terrain(self) -> (Terr, Terr) {
        match self {
            Biome::Volcano => (Volcano, Ash),
            Biome::Lake => (Lake, Lake),
            Biome::Peak => (Peak, Snow),
            Biome::Desert => (Dune, Dune),
        }
    }
    /// Life that turns up by itself once the environment exists.
    pub fn natives(self) -> &'static [Sp] {
        match self {
            Biome::Volcano => &[Sp::Fireweed, Sp::Fireweed, Sp::Lizard],
            Biome::Lake => &[Sp::Fish, Sp::Reed, Sp::Fish, Sp::Duck],
            Biome::Peak => &[Sp::Pine, Sp::Goat, Sp::Pine],
            Biome::Desert => &[Sp::Cactus, Sp::Camel, Sp::Cactus, Sp::Lizard],
        }
    }
}

/// What a plant turns into when its ground becomes `to`, if it manages to adapt.
pub fn adapt_to(sp: Sp, to: Terr) -> Option<Sp> {
    use Sp::*;
    match to {
        Ash => Some(Fireweed),
        Dune => Some(match sp {
            Tree | Palm => Palm,
            _ => Cactus,
        }),
        Snow => match sp {
            Tree | Bush | Fern | Pine => Some(Pine),
            _ => None,
        },
        Lake | Pond => match sp {
            Lily => Some(Lily),
            Flower | Fern | Bush | Mushroom => Some(Reed),
            _ => None,
        },
        Grass | Mead => match sp {
            Cactus | Fireweed => Some(Flower),
            Reed | Lily => Some(Fern),
            Palm => Some(Tree),
            _ => None,
        },
        Sand => match sp {
            Tree => Some(Palm),
            Fireweed => Some(Cactus),
            _ => None,
        },
        _ => None,
    }
}
