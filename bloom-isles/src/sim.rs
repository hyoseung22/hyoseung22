//! Game rules, independent of rendering: islands, cards, threats and bots.

use crate::eco::{self, def, Sp, ALL};
use crate::island::{IslandMap, SQ};
use crate::ecology::creature_mod;
use crate::rng::Rng;

pub const THRIVE: f32 = 0.72;
pub use crate::eco::Biome;
pub const SAD: f32 = 0.32;
pub const DUR: f32 = 480.0;
pub const GOAL: f32 = 300.0;
pub const CARD_CD: f32 = 8.0;
pub const STORM_T: f32 = 12.0;
pub const WITHER_T: f32 = 28.0;
pub const REROLL_CD: f32 = 14.0;
/// Seconds between good placements that keep a combo going.
pub const COMBO_T: f32 = 10.0;
/// Island index used for the title-screen island.
pub const TITLE: usize = 9;

/// Seconds between natural hazards (beetle swarms, thunderclouds) per difficulty; 0 = none.
pub const HAZARD_EVERY: [f32; 3] = [0.0, 110.0, 70.0];
/// Score needed for one, two and three stars.
pub const STAR_AT: [f32; 3] = [140.0, 240.0, 340.0];

pub fn state_of(h: f32) -> u8 {
    if h >= THRIVE {
        2
    } else if h >= SAD {
        1
    } else {
        0
    }
}

pub fn ent_score(h: f32, sp: Sp) -> f32 {
    let base = if h >= THRIVE { 3.0 } else if h >= SAD { 1.5 } else { 0.4 };
    base * if def(sp).plant { 1.0 } else { 1.25 }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Card {
    Nature(Sp),
    /// Raises a whole new environment around a tile.
    Biome(Biome),
}

/// Shared weather over all islands.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Weather {
    Clear,
    Rain,
    Drought,
    Wind,
}

/// A visible interaction between two things on an island.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link {
    /// Bee visits a flower or bush.
    Pollinate,
    /// Rabbit nibbles a plant or eats berries.
    Graze,
    /// Fox lunges at a rabbit (caught or not).
    Hunt,
    /// Frog catches a beetle or bee.
    Tongue,
    /// Bird eats berries or a beetle.
    Peck,
    /// Bird drops a seed that sprouts.
    Seed,
    /// Lightning strikes a tree instead of the creatures around it.
    Rod,
    /// Crab pinches a beetle.
    Pinch,
    /// Something died and a mushroom grew from it.
    Compost,
    /// A camel drinks from nearby water.
    Drink,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Death {
    Wither,
    Zap,
    Eaten,
    Dig,
    /// An animal with nowhere to go left the island.
    Leave,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PKind {
    Heart,
    Spark,
    Leaf,
    LeafDead,
    Puff,
    Soot,
    Splat,
    Drop,
    Dust,
    Splash,
    Gold,
    Ember,
    Snow,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sfx {
    Place,
    PlaceSoft,
    Sprout,
    Pick,
    Nope,
    Whoosh,
    Alarm,
    Thunder,
    ThunderFar,
    Puff,
    Blown,
    Squish,
    Munch,
    Wither,
    Shield,
    Bounce,
    Reroll,
    Dig,
    Click,
    Win,
    Lose,
    Tock,
    Wow,
    CrowdWow,
    Cheer,
    BigCheer,
    Applause,
    Aww,
    Drumroll,
    Cymbal,
    Thud,
    Firework,
    Combo(u8),
    Jackpot,
    Splash,
    Whale,
    Coin,
    Pollinate,
    Chomp,
    Tongue,
    Peck,
    SeedDrop,
    Fear,
    Bloom,
    RainStart,
    WindStart,
    DroughtStart,
    ClearStart,
    RainLoop,
    WindLoop,
    /// A biome card lands (by biome index).
    Biome(u8),
    Erupt,
    Adapt,
    Discover,
    /// A star lights up on the results screen (1..=3).
    Star(u8),
    NewBest,
}

/// Things the renderer should react to.
#[derive(Clone, Debug)]
pub enum Ev {
    Burst { isl: usize, x: f32, y: f32, kind: PKind, n: u8 },
    Float { isl: usize, x: f32, y: f32, kind: PKind },
    Sfx(Sfx),
    Shake,
    Flash,
    /// New land rose from the sea on island `isl`.
    Grow { isl: usize, tiles: Vec<usize> },
    /// Floating "+N" above something that just started thriving.
    Score { isl: usize, x: f32, y: f32, amt: i32 },
    /// Player placement streak.
    Combo { n: u32, x: f32, y: f32 },
    /// A mega bloom or whale treasure.
    Jackpot { isl: usize, x: f32, y: f32 },
    /// Every creature on the island jumps for joy.
    Party { isl: usize },
    /// A new environment rose on the island.
    Biome { isl: usize, biome: Biome, x: f32, y: f32 },
    /// A plant adapted to new ground by turning into a local cousin.
    Adapt { isl: usize, x: f32, y: f32 },
    /// A species appeared on the island for the first time.
    Discover { isl: usize, sp: Sp, x: f32, y: f32 },
    /// The volcano erupted.
    Erupt { isl: usize, x: f32, y: f32 },
    /// Two things interacted: draw a line between them.
    Link { isl: usize, kind: Link, ax: f32, ay: f32, bx: f32, by: f32, ok: bool },
    /// A creature got scared (x, y is its position).
    Fear { isl: usize, x: f32, y: f32 },
    /// The weather changed.
    Weather(Weather),
}

#[derive(Clone, Debug)]
pub struct Creature {
    pub id: u32,
    pub sp: Sp,
    pub tile: usize,
    pub x: f32,
    pub y: f32,
    pub h: f32,
    /// Displayed happiness, eased toward `h`.
    pub hd: f32,
    pub sad_t: f32,
    pub thr_t: f32,
    pub next: f32,
    pub born: f32,
    /// 0 while alive, then counts up during the death animation.
    pub dying: f32,
    pub how: Death,
    pub v: u8,
    pub ph: f32,
    /// Hop progress 0..1 while moving between tiles like a board-game piece (0 = standing).
    pub hop: f32,
    /// How long the current hop takes (longer for migrations across the island).
    pub hop_dur: f32,
    pub fx: f32,
    pub fy: f32,
    pub tx: f32,
    pub ty: f32,
    pub move_cd: f32,
    /// Celebration jump timer.
    pub jump: f32,
    pub thriving: bool,
    /// Animals: how full they are (0 starving .. 1 fed).
    pub fed: f32,
    pub starve: f32,
    /// Plants: pollinated (flowers) or fruiting (bushes) for this many seconds.
    pub polli: f32,
    /// Plants: health, eaten away by grazing.
    pub hp: f32,
    /// Trees: scorched by lightning for this many seconds.
    pub scorch: f32,
    /// Birds: carrying a seed that drops when this runs out.
    pub seed: f32,
    /// Time until the next interaction attempt.
    pub act: f32,
}

#[derive(Clone, Debug)]
pub struct Whale {
    pub x: f32,
    pub y: f32,
    pub t: f32,
    pub dur: f32,
    pub caught: bool,
    pub gone: f32,
    pub bot: Option<f32>,
}

impl Whale {
    pub fn active(&self) -> bool {
        !self.caught && self.t < self.dur
    }
}

#[derive(Clone, Debug)]
pub struct Storm {
    pub id: u32,
    pub tile: usize,
    pub x: f32,
    pub y: f32,
    pub t: f32,
    pub taps: u8,
    pub need: u8,
    pub struck: bool,
    pub blown: bool,
    pub st: f32,
    pub bt: f32,
    pub cx: f32,
    pub cy: f32,
    pub sc: f32,
    pub pulse: f32,
    pub bot: Option<f32>,
    pub done: bool,
}

impl Storm {
    pub fn active(&self) -> bool {
        !self.struck && !self.blown
    }
}

#[derive(Clone, Debug)]
pub struct Beetle {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub tgt: Option<u32>,
    pub eat: f32,
    pub life: f32,
    pub dead: f32,
    pub dir: f32,
    pub leave: bool,
    pub lift: f32,
    pub bot: Option<f32>,
    pub done: bool,
}

impl Beetle {
    pub fn active(&self) -> bool {
        self.dead == 0.0 && !self.leave
    }
}

#[derive(Clone, Debug)]
pub struct Island {
    pub map: IslandMap,
    pub ents: Vec<Creature>,
    pub storms: Vec<Storm>,
    pub beetles: Vec<Beetle>,
    pub score: f32,
    pub ds: f32,
    pub lush: f32,
    pub kinds: usize,
    /// Bumped whenever something is placed or removed.
    pub ver: u32,
    pub recalc_t: f32,
    pub next_id: u32,
    /// Bumped whenever tiles are added.
    pub map_ver: u32,
    pub grow_level: u8,
    pub milestone: u32,
    pub whale: Option<Whale>,
    pub whale_t: f32,
    /// Weather this island is under (set by the game each frame).
    pub weather: Weather,
    pub eco_t: f32,
    /// Tiles where something died recently; mushrooms may sprout there.
    pub compost: Vec<usize>,
    /// Bumped when soil moisture or fertility shifts enough to repaint.
    pub soil_ver: u32,
    /// Species that have lived here at some point.
    pub seen: [bool; crate::eco::N],
    /// Environments raised by biome cards: (biome, centre tile).
    pub biomes: Vec<(Biome, usize)>,
    /// Natives on their way: (seconds left, species, near this tile).
    pub arrivals: Vec<(f32, Sp, usize)>,
    pub erupt_t: f32,
}

/// Scores at which an island raises new land.
pub const GROW_AT: [f32; 3] = [65.0, 135.0, 210.0];
pub const PARTY_EVERY: f32 = 60.0;

impl Island {
    pub fn new(map: IslandMap) -> Self {
        Island {
            map,
            ents: vec![],
            storms: vec![],
            beetles: vec![],
            score: 0.0,
            ds: 0.0,
            lush: 0.0,
            kinds: 0,
            ver: 1,
            recalc_t: 0.0,
            next_id: 1,
            map_ver: 1,
            grow_level: 0,
            milestone: 0,
            whale: None,
            whale_t: 30.0,
            weather: Weather::Clear,
            eco_t: 0.0,
            compost: vec![],
            soil_ver: 0,
            seen: [false; crate::eco::N],
            biomes: vec![],
            arrivals: vec![],
            erupt_t: 20.0,
        }
    }

    pub fn threatened(&self) -> bool {
        self.storms.iter().any(|s| s.active()) || self.beetles.iter().any(|b| b.active())
    }

    pub fn has_threats(&self) -> bool {
        !self.storms.is_empty() || !self.beetles.is_empty()
    }

    pub fn ent(&self, id: u32) -> Option<&Creature> {
        self.ents.iter().find(|e| e.id == id)
    }

    pub fn new_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub fn happ(&self, tile: usize, sp: Sp, virt: Option<(usize, Sp)>) -> f32 {
        let d = def(sp);
        let t = &self.map.tiles[tile];
        let mut h = d.base + d.terr_bonus.iter().find(|(tt, _)| *tt == t.t).map(|(_, b)| *b).unwrap_or(0.0);
        for need in d.needs {
            let list = if need.rad == 1 { &t.n1 } else { &t.n2 };
            let mut n = 0u8;
            for &u in list {
                match self.map.tiles[u].occ {
                    Some((_, s)) if s == need.sp => n += 1,
                    None if virt == Some((u, need.sp)) => n += 1,
                    _ => {}
                }
            }
            h += n.min(need.cap) as f32 * need.w;
        }
        (h + self.env_mod(tile, sp, virt)).clamp(0.0, 1.0)
    }

    pub fn can_place(&self, tile: usize, sp: Sp) -> bool {
        let t = &self.map.tiles[tile];
        t.occ.is_none() && def(sp).terr.contains(&t.t) && !self.storms.iter().any(|s| s.active() && s.tile == tile)
    }

    pub fn has_spot(&self, sp: Sp) -> bool {
        (0..self.map.tiles.len()).any(|i| self.can_place(i, sp))
    }

    /// Score change from placing `sp` on `tile`, plus the newcomer's own happiness.
    pub fn place_value(&self, tile: usize, sp: Sp) -> (f32, f32) {
        let mut before = 0.0;
        let mut after = 0.0;
        for &u in &self.map.tiles[tile].n2 {
            if let Some((_, s)) = self.map.tiles[u].occ {
                before += ent_score(self.happ(u, s, None), s);
                after += ent_score(self.happ(u, s, Some((tile, sp))), s);
            }
        }
        let h = self.happ(tile, sp, None);
        (after - before + ent_score(h, sp), h)
    }

    pub fn recalc(&mut self) {
        let mut s = 0.0;
        let mut kinds = [false; crate::eco::N];
        let hs: Vec<f32> = self.ents.iter().map(|e| if e.dying > 0.0 { e.h } else { (self.happ(e.tile, e.sp, None) + creature_mod(e)).clamp(0.0, 1.0) }).collect();
        for (e, h) in self.ents.iter_mut().zip(hs) {
            if e.dying > 0.0 {
                continue;
            }
            e.h = h;
            s += ent_score(h, e.sp);
            if h >= THRIVE {
                kinds[e.sp.idx()] = true;
            }
        }
        self.kinds = kinds.iter().filter(|k| **k).count();
        self.score = s + self.kinds as f32 * 3.0;
    }

    pub fn add_ent(&mut self, isl: usize, tile: usize, sp: Sp, natural: bool, rng: &mut Rng, ev: &mut Vec<Ev>) -> u32 {
        let id = self.new_id();
        let t = &self.map.tiles[tile];
        let e = Creature {
            id,
            sp,
            tile,
            x: t.x + (rng.f() - 0.5) * 5.0,
            y: t.y + (rng.f() - 0.5) * 3.0,
            h: 0.5,
            hd: 0.5,
            sad_t: 0.0,
            thr_t: 0.0,
            next: 12.0 + rng.f() * 10.0,
            born: 0.0,
            dying: 0.0,
            how: Death::Dig,
            v: rng.below(4) as u8,
            ph: rng.f() * std::f32::consts::TAU,
            hop: 0.0,
            hop_dur: HOP_T,
            fx: 0.0,
            fy: 0.0,
            tx: 0.0,
            ty: 0.0,
            move_cd: 5.0 + rng.f() * 6.0,
            jump: 0.0,
            thriving: false,
            fed: 0.75,
            starve: 0.0,
            polli: 0.0,
            hp: 1.0,
            scorch: 0.0,
            seed: 0.0,
            act: 2.0 + rng.f() * 3.0,
        };
        let (x, y) = (e.x, e.y);
        self.map.tiles[tile].occ = Some((id, sp));
        self.ents.push(e);
        if !self.seen[sp.idx()] {
            self.seen[sp.idx()] = true;
            ev.push(Ev::Discover { isl, sp, x, y });
        }
        self.ver += 1;
        self.recalc();
        if let Some(e) = self.ents.last_mut() {
            e.hd = e.h;
        }
        ev.push(Ev::Burst { isl, x, y: y - 8.0, kind: if natural { PKind::Spark } else { PKind::Leaf }, n: if natural { 4 } else { 8 } });
        id
    }

    pub fn kill(&mut self, isl: usize, id: u32, how: Death, ev: &mut Vec<Ev>) {
        let Some(e) = self.ents.iter_mut().find(|e| e.id == id) else { return };
        if e.dying > 0.0 {
            return;
        }
        e.dying = 0.001;
        e.how = how;
        let (x, y, tile) = (e.x, e.y, e.tile);
        if matches!(self.map.tiles[tile].occ, Some((oid, _)) if oid == id) {
            self.map.tiles[tile].occ = None;
        }
        if !matches!(how, Death::Dig | Death::Leave) {
            // the dead return to the soil
            self.map.tiles[tile].fert = (self.map.tiles[tile].fert + 0.25).min(1.0);
            self.compost.push(tile);
        }
        self.ver += 1;
        let kind = match how {
            Death::Zap => PKind::Soot,
            Death::Wither => PKind::LeafDead,
            Death::Leave => PKind::Splash,
            _ => PKind::Puff,
        };
        ev.push(Ev::Burst { isl, x, y: y - 10.0, kind, n: 8 });
        self.recalc();
    }

    fn try_spawn(&mut self, isl: usize, idx: usize, rng: &mut Rng, ev: &mut Vec<Ev>) -> bool {
        let (sp, tile) = (self.ents[idx].sp, self.ents[idx].tile);
        let Some(rule) = eco::spawn_rule(sp) else { return false };
        let boost = if self.ents[idx].polli > 0.0 { 1.6 } else { 1.0 } * if self.weather == Weather::Wind && def(sp).plant { 1.5 } else { 1.0 };
        if rng.f() > rule.p * boost {
            return false;
        }
        let t = &self.map.tiles[tile];
        if rule.needs_pair && !t.n1.iter().any(|&u| matches!(self.map.tiles[u].occ, Some((_, s)) if s == sp)) {
            return false;
        }
        let make = *rng.pick(rule.makes);
        // wind carries seeds further
        let list = if rule.rad == 1 && !(self.weather == Weather::Wind && def(sp).plant) { t.n1.clone() } else { t.n2.clone() };
        let mut best = None;
        let mut bv = f32::MIN;
        for u in list {
            if !self.can_place(u, make) {
                continue;
            }
            let v = self.place_value(u, make).0 + rng.f() * 1.5;
            if v > bv {
                bv = v;
                best = Some(u);
            }
        }
        match best {
            Some(u) if bv > 0.0 => {
                self.add_ent(isl, u, make, true, rng, ev);
                true
            }
            _ => false,
        }
    }

    pub fn reset(&mut self) {
        for t in &mut self.map.tiles {
            t.occ = None;
        }
        self.ents.clear();
        self.storms.clear();
        self.beetles.clear();
        self.score = 0.0;
        self.ds = 0.0;
        self.lush = 0.0;
        self.ver += 1;
    }

    /// Decorate an island with a handful of happy residents (title screen).
    pub fn seed_showcase(&mut self, rng: &mut Rng) {
        let mut ev = vec![];
        for sp in [Sp::Tree, Sp::Bird, Sp::Flower, Sp::Bee, Sp::Palm, Sp::Crab, Sp::Lily, Sp::Frog, Sp::Bush, Sp::Flower, Sp::Tree, Sp::Rabbit] {
            let mut best = None;
            let mut bv = f32::MIN;
            for i in 0..self.map.tiles.len() {
                if self.can_place(i, sp) {
                    let v = self.place_value(i, sp).0 + rng.f();
                    if v > bv {
                        bv = v;
                        best = Some(i);
                    }
                }
            }
            if let Some(i) = best {
                self.add_ent(TITLE, i, sp, true, rng, &mut ev);
            }
        }
        for e in &mut self.ents {
            e.born = 1.0;
            e.hd = e.h;
        }
        self.lush = (self.score / GOAL * 2.5).clamp(0.0, 0.8);
    }

    /// Advance creatures, threats and scores. `viewed`: the player is looking at this island.
    pub fn update(&mut self, isl: usize, dt: f32, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        self.recalc_t -= dt;
        if self.recalc_t <= 0.0 {
            self.recalc();
            self.recalc_t = 0.4;
        }
        self.ds += (self.score - self.ds) * (1.0 - (-dt * 2.5).exp());
        self.lush += ((self.score / GOAL).clamp(0.0, 1.0) - self.lush) * (1.0 - (-dt * 1.5).exp());
        self.update_arrivals(isl, dt, viewed, rng, ev);

        let mut i = 0;
        while i < self.ents.len() {
            let e = &mut self.ents[i];
            e.born += dt;
            if e.dying > 0.0 {
                e.dying += dt;
                i += 1;
                continue;
            }
            e.hd += (e.h - e.hd) * (1.0 - (-dt * 3.0).exp());
            let st = state_of(e.h);
            if st == 0 {
                e.sad_t += dt;
                if e.sad_t > WITHER_T {
                    let id = e.id;
                    self.kill(isl, id, Death::Wither, ev);
                    if viewed {
                        ev.push(Ev::Sfx(Sfx::Wither));
                    }
                    i += 1;
                    continue;
                }
            } else {
                e.sad_t = (e.sad_t - dt * 2.0).max(0.0);
            }
            if st == 2 && !e.thriving {
                e.thriving = true;
                if viewed && e.born > 0.6 {
                    ev.push(Ev::Score { isl, x: e.x, y: e.y - def(e.sp).head - 8.0, amt: 3 });
                }
            } else if st < 2 {
                e.thriving = false;
            }
            if st == 2 {
                e.thr_t += dt;
                let (x, y, head, plant) = (e.x, e.y, def(e.sp).head, def(e.sp).plant);
                if e.thr_t > e.next {
                    e.thr_t = 0.0;
                    e.next = 16.0 + rng.f() * 14.0;
                    if self.try_spawn(isl, i, rng, ev) && viewed {
                        ev.push(Ev::Sfx(Sfx::Sprout));
                    }
                }
                if viewed && rng.f() < dt * 0.22 {
                    ev.push(Ev::Float { isl, x: x + (rng.f() - 0.5) * 10.0, y: y - head - 4.0, kind: if plant { PKind::Spark } else { PKind::Heart } });
                }
            }
            i += 1;
        }
        self.ents.retain(|e| e.dying < 0.8);
        self.eco_t -= dt;
        if self.eco_t <= 0.0 {
            self.eco_t = crate::ecology::TICK;
            self.ecology(isl, viewed, rng, ev);
        }
        self.move_animals(isl, dt, viewed, rng, ev);
        self.check_growth(isl, viewed, rng, ev);
        self.update_whale(isl, dt, rng, ev);

        // storms
        for si in 0..self.storms.len() {
            let s = &mut self.storms[si];
            s.t += dt;
            s.pulse = (s.pulse - dt * 4.0).max(0.0);
            if s.blown {
                s.bt += dt;
                s.cx += dt * 380.0;
                s.cy -= dt * 160.0;
                s.done = s.bt > 1.2;
                continue;
            }
            let k = ease_io((s.t / 1.2).clamp(0.0, 1.0));
            s.sc = 1.0 - s.taps as f32 / s.need as f32 * 0.45;
            s.cx = lerp(s.x + 240.0, s.x + (s.t * 1.3).sin() * 6.0, k);
            s.cy = lerp(s.y - 190.0, s.y - 78.0 + (s.t * 2.0).sin() * 3.0, k);
            if !s.struck && s.t >= STORM_T {
                s.struck = true;
                s.st = 0.0;
                let (tile, x, y) = (s.tile, s.x, s.y);
                let mut victims = vec![tile];
                victims.extend(self.map.tiles[tile].n1.iter().copied());
                // a tree in the blast takes the bolt and shelters everything else
                let rod = victims.iter().copied().find(|&u| matches!(self.map.tiles[u].occ, Some((_, Sp::Tree))));
                if let Some(u) = rod {
                    let (id, _) = self.map.tiles[u].occ.unwrap();
                    let (tx, ty) = (self.map.tiles[u].x, self.map.tiles[u].y);
                    ev.push(Ev::Link { isl, kind: Link::Rod, ax: x, ay: y - 90.0, bx: tx, by: ty, ok: true });
                    let burnt = self.ents.iter().find(|e| e.id == id).map(|e| e.scorch > 0.0).unwrap_or(false);
                    if burnt {
                        self.kill(isl, id, Death::Zap, ev);
                    } else if let Some(e) = self.ents.iter_mut().find(|e| e.id == id) {
                        e.scorch = 18.0;
                    }
                } else {
                    for u in victims {
                        if let Some((id, _)) = self.map.tiles[u].occ {
                            if u == tile || rng.f() < 0.75 {
                                self.kill(isl, id, Death::Zap, ev);
                            }
                        }
                    }
                }
                ev.push(Ev::Burst { isl, x, y, kind: PKind::Soot, n: 14 });
                if viewed {
                    ev.push(Ev::Shake);
                    ev.push(Ev::Flash);
                    ev.push(Ev::Sfx(Sfx::Thunder));
                } else if isl == 0 {
                    ev.push(Ev::Sfx(Sfx::ThunderFar));
                }
            }
            let s = &mut self.storms[si];
            if s.struck {
                s.st += dt;
                if rng.f() < dt * 40.0 {
                    ev.push(Ev::Burst { isl, x: s.cx + (rng.f() - 0.5) * 70.0, y: s.cy + 10.0, kind: PKind::Drop, n: 1 });
                }
                s.done = s.st > 1.8;
            }
        }
        self.storms.retain(|s| !s.done);

        // beetles
        for bi in 0..self.beetles.len() {
            let b = &mut self.beetles[bi];
            if b.dead > 0.0 {
                b.dead += dt;
                b.done = b.dead > 0.7;
                continue;
            }
            b.life -= dt * if self.weather == Weather::Rain { 2.5 } else { 1.0 };
            if b.life <= 0.0 || b.leave {
                b.leave = true;
                b.lift = (b.lift - dt * 1.2).max(0.0);
                b.y -= dt * 60.0;
                b.x += b.dir.cos() * dt * 40.0;
                b.done = b.lift <= 0.0;
                continue;
            }
            let tgt_alive = b.tgt.and_then(|id| self.ents.iter().find(|e| e.id == id && e.dying == 0.0)).map(|e| (e.x, e.y, e.id));
            let (bx, by) = (b.x, b.y);
            let tgt = match tgt_alive {
                Some(t) => Some(t),
                None => {
                    let mut best = None;
                    let mut bd = f32::MAX;
                    for e in &self.ents {
                        if e.dying > 0.0 || !def(e.sp).plant {
                            continue;
                        }
                        let d = (e.x - bx).hypot((e.y - by) / SQ);
                        if d < bd {
                            bd = d;
                            best = Some((e.x, e.y, e.id));
                        }
                    }
                    best
                }
            };
            let b = &mut self.beetles[bi];
            match tgt {
                Some((tx, ty, id)) => {
                    if b.tgt != Some(id) {
                        b.eat = 0.0;
                    }
                    b.tgt = Some(id);
                    let (dx, dy) = (tx - b.x, ty + 2.0 - b.y);
                    let d = dx.hypot(dy);
                    if d > 5.0 {
                        b.dir = dy.atan2(dx);
                        b.x += dx / d * 24.0 * dt;
                        b.y += dy / d * 24.0 * dt;
                    } else {
                        b.eat += dt;
                        if b.eat >= 5.0 {
                            b.tgt = None;
                            b.eat = 0.0;
                            self.kill(isl, id, Death::Eaten, ev);
                            if viewed {
                                ev.push(Ev::Sfx(Sfx::Munch));
                            }
                        }
                    }
                }
                None => {
                    b.tgt = None;
                    b.dir += (rng.f() - 0.5) * dt * 4.0;
                    b.x += b.dir.cos() * 13.0 * dt;
                    b.y += b.dir.sin() * 9.0 * dt;
                }
            }
        }
        self.beetles.retain(|b| !b.done);
    }
}

pub const HOP_T: f32 = 0.75;

impl Island {
    /// Animals hop to a neighbouring tile when it suits them better, like pieces on a board.
    fn move_animals(&mut self, isl: usize, dt: f32, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        for i in 0..self.ents.len() {
            let e = &mut self.ents[i];
            if e.dying > 0.0 {
                continue;
            }
            e.jump = (e.jump - dt * 1.6).max(0.0);
            if e.hop > 0.0 {
                e.hop += dt / e.hop_dur.max(0.1);
                if e.hop >= 1.0 {
                    e.hop = 0.0;
                    e.x = e.tx;
                    e.y = e.ty;
                    ev.push(Ev::Burst { isl, x: e.x, y: e.y, kind: PKind::Dust, n: 5 });
                    if viewed {
                        ev.push(Ev::Sfx(Sfx::Tock));
                    }
                } else {
                    let k = ease_io(e.hop);
                    e.x = lerp(e.fx, e.tx, k);
                    e.y = lerp(e.fy, e.ty, k);
                }
                continue;
            }
            if def(e.sp).plant || e.born < 1.0 {
                continue;
            }
            e.move_cd -= dt;
            if e.move_cd > 0.0 {
                continue;
            }
            e.move_cd = 7.0 + rng.f() * 8.0;
            let (sp, from, id) = (e.sp, e.tile, e.id);
            self.map.tiles[from].occ = None;
            let cur = self.happ(from, sp, None);
            let mut best = None;
            let mut bh = f32::MIN;
            for u in self.map.tiles[from].n1.clone() {
                if !self.can_place(u, sp) {
                    continue;
                }
                let h = self.happ(u, sp, None) + rng.f() * 0.03;
                if h > bh {
                    bh = h;
                    best = Some(u);
                }
            }
            let wander = if state_of(cur) < 2 { 0.3 } else { 0.12 };
            let target = match best {
                Some(u) if bh > cur + 0.04 => Some(u),
                Some(u) if bh >= cur - 0.01 && rng.f() < wander => Some(u),
                _ => None,
            };
            let dest = target.unwrap_or(from);
            self.map.tiles[dest].occ = Some((id, sp));
            if let Some(u) = target {
                let t = &self.map.tiles[u];
                let (tx, ty) = (t.x + (rng.f() - 0.5) * 5.0, t.y + (rng.f() - 0.5) * 3.0);
                let e = &mut self.ents[i];
                e.tile = u;
                e.fx = e.x;
                e.fy = e.y;
                e.tx = tx;
                e.ty = ty;
                e.hop = 0.001;
                e.hop_dur = HOP_T;
                self.ver += 1;
            }
        }
    }

    /// Raise land at score milestones and throw a party every so often.
    fn check_growth(&mut self, isl: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        while (self.grow_level as usize) < GROW_AT.len() && self.score >= GROW_AT[self.grow_level as usize] {
            self.grow_level += 1;
            let tiles = self.map.grow(7, rng);
            if tiles.is_empty() {
                continue;
            }
            self.map_ver += 1;
            self.ver += 1;
            for &t in &tiles {
                let tl = &self.map.tiles[t];
                ev.push(Ev::Burst { isl, x: tl.x, y: tl.y, kind: PKind::Splash, n: 8 });
            }
            ev.push(Ev::Grow { isl, tiles });
            if viewed {
                ev.push(Ev::Sfx(Sfx::CrowdWow));
                ev.push(Ev::Sfx(Sfx::Splash));
            }
        }
        let m = (self.score / PARTY_EVERY) as u32;
        if m > self.milestone {
            self.milestone = m;
            for e in &mut self.ents {
                if e.dying == 0.0 {
                    e.jump = 1.0;
                }
            }
            ev.push(Ev::Party { isl });
            if viewed {
                ev.push(Ev::Sfx(Sfx::Cheer));
            }
        }
    }

    /// Now and then a golden whale surfaces by the coast. Tap it for treasure.
    fn update_whale(&mut self, isl: usize, dt: f32, rng: &mut Rng, ev: &mut Vec<Ev>) {
        if let Some(w) = &mut self.whale {
            w.t += dt;
            if !w.active() {
                w.gone += dt;
                if w.gone > 1.5 {
                    self.whale = None;
                }
            }
            return;
        }
        self.whale_t -= dt;
        if self.whale_t > 0.0 || self.map.edge.is_empty() {
            return;
        }
        self.whale_t = 75.0 + rng.f() * 40.0;
        let t = &self.map.tiles[*rng.pick(&self.map.edge)];
        let (cx, cy) = (self.map.b.cx(), self.map.b.cy());
        let d = Vec2f::new(t.x - cx, (t.y - cy) / SQ).norm();
        self.whale = Some(Whale { x: t.x + d.0 * 80.0, y: t.y + d.1 * 80.0 * SQ, t: 0.0, dur: 12.0, caught: false, gone: 0.0, bot: None });
        if isl != TITLE {
            ev.push(Ev::Sfx(Sfx::Whale));
        }
    }

    pub fn catch_whale(&mut self, isl: usize, rng: &mut Rng, ev: &mut Vec<Ev>) -> bool {
        let Some(w) = &mut self.whale else { return false };
        if !w.active() {
            return false;
        }
        w.caught = true;
        let (wx, wy) = (w.x, w.y);
        ev.push(Ev::Jackpot { isl, x: wx, y: wy });
        ev.push(Ev::Burst { isl, x: wx, y: wy - 20.0, kind: PKind::Gold, n: 24 });
        for _ in 0..3 {
            let options: Vec<Sp> = ALL.iter().copied().filter(|&sp| self.has_spot(sp)).collect();
            if options.is_empty() {
                break;
            }
            let sp = *rng.pick(&options);
            let mut best = None;
            let mut bv = f32::MIN;
            for t in 0..self.map.tiles.len() {
                if self.can_place(t, sp) {
                    let v = self.place_value(t, sp).0 + rng.f();
                    if v > bv {
                        bv = v;
                        best = Some(t);
                    }
                }
            }
            if let Some(t) = best {
                self.add_ent(isl, t, sp, true, rng, ev);
            }
        }
        true
    }
}

struct Vec2f(f32, f32);
impl Vec2f {
    fn new(x: f32, y: f32) -> Self {
        Vec2f(x, y)
    }
    fn norm(self) -> (f32, f32) {
        let l = self.0.hypot(self.1).max(0.001);
        (self.0 / l, self.1 / l)
    }
}

/// When each star is judged on the results screen (after the score counts up).
pub const STAR_REVEAL: [f32; 3] = [2.2, 3.1, 4.0];
/// Results-screen buttons appear after this.
pub const END_BUTTONS_AT: f32 = 5.4;

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn ease_io(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

pub fn ease_back(t: f32) -> f32 {
    let c1 = 1.9;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub card: Option<Card>,
    pub cd: f32,
    /// Shake animation after an invalid move.
    pub wig: f32,
    /// Flip-in animation for a freshly drawn card.
    pub flip: f32,
}

/// Most biome cards one game deals.
pub const MAX_BIOMES: u8 = 4;

/// One timed game: grow the richest island you can before the sun sets.
pub struct Game {
    pub hand: Vec<Slot>,
    /// Reroll cooldown.
    pub rr: f32,
    /// The island (a one-element list so island indices stay uniform).
    pub isl: Vec<Island>,
    pub t: f32,
    pub sel: Option<usize>,
    pub shovel: bool,
    pub shake: f32,
    pub flash: f32,
    pub alert: f32,
    pub fade: f32,
    pub placed: u32,
    pub biomes_placed: u32,
    pub defends: u32,
    pub inspect: Option<usize>,
    pub diff: usize,
    pub paused: bool,
    pub over: bool,
    pub end_t: f32,
    pub rng: Rng,
    pub ev: Vec<Ev>,
    pub combo: u32,
    pub last_place: f32,
    pub weather: Weather,
    pub weather_t: f32,
    pub forecast: Weather,
    pub hazard_t: f32,
    /// Biome cards dealt so far.
    pub biomes_dealt: u8,
    /// Best score before this game, and whether this game beat it.
    pub best: f32,
    pub new_best: bool,
    /// Final score and stars, fixed when the sun sets.
    pub final_score: f32,
    pub stars: u8,
    /// A simple autopilot plays the cards (tests and screenshots).
    pub auto: bool,
    /// Game speed multiplier chosen by the player (1 or 2).
    pub speed: u8,
    think: f32,
    cer: u8,
}

impl Game {
    pub fn new(map: IslandMap, diff: usize, best: f32, rng: Rng) -> Self {
        let mut g = Game {
            hand: vec![],
            rr: 0.0,
            isl: vec![Island::new(map)],
            t: 0.0,
            sel: None,
            shovel: false,
            shake: 0.0,
            flash: 0.0,
            alert: 0.0,
            fade: 1.0,
            placed: 0,
            biomes_placed: 0,
            defends: 0,
            inspect: None,
            diff: diff.min(2),
            paused: false,
            over: false,
            end_t: 0.0,
            rng,
            ev: vec![],
            combo: 0,
            last_place: -10.0,
            weather: Weather::Clear,
            weather_t: 50.0,
            forecast: Weather::Rain,
            hazard_t: 0.0,
            biomes_dealt: 0,
            best,
            new_best: false,
            final_score: 0.0,
            stars: 0,
            auto: false,
            speed: 1,
            think: 1.0,
            cer: 0,
        };
        g.hazard_t = HAZARD_EVERY[g.diff] * 0.8;
        g.isl[0].whale_t = 50.0 + g.rng.f() * 40.0;
        for _ in 0..4 {
            let c = g.draw_card(true);
            g.hand.push(Slot { card: Some(c), cd: 0.0, wig: 0.0, flip: 0.0 });
        }
        g
    }

    fn sfx(&mut self, s: Sfx) {
        self.ev.push(Ev::Sfx(s));
    }

    fn biomes_in_hand(&self) -> Vec<Biome> {
        self.hand.iter().filter_map(|s| match s.card {
            Some(Card::Biome(b)) => Some(b),
            _ => None,
        }).collect()
    }

    pub fn draw_card(&mut self, first: bool) -> Card {
        // now and then a whole new environment turns up
        if !first && self.t > 16.0 && self.biomes_dealt < MAX_BIOMES {
            let held = self.biomes_in_hand();
            let p = if self.biomes_dealt == 0 && self.t > 35.0 { 0.45 } else { 0.1 };
            if held.is_empty() && self.rng.f() < p {
                let placed: Vec<Biome> = self.isl[0].biomes.iter().map(|b| b.0).collect();
                let fresh: Vec<Biome> = crate::eco::BIOMES.iter().copied().filter(|b| !placed.contains(b)).collect();
                let b = if fresh.is_empty() { *self.rng.pick(&crate::eco::BIOMES) } else { *self.rng.pick(&fresh) };
                self.biomes_dealt += 1;
                return Card::Biome(b);
            }
        }
        let mut bag: Vec<(Sp, f32)> = vec![];
        for sp in ALL {
            if !self.isl[0].has_spot(sp) {
                continue;
            }
            let mut w = if def(sp).plant { 1.15 } else { 1.0 };
            if first && matches!(sp, Sp::Fox | Sp::Mushroom) {
                w *= 0.3;
            }
            // natives of a biome that exists show up more
            if self.isl[0].biomes.iter().any(|b| b.0.natives().contains(&sp)) {
                w *= 1.6;
            }
            bag.push((sp, w));
        }
        if bag.is_empty() {
            // the island is full: offer something to dig room for
            return Card::Nature(*self.rng.pick(&[Sp::Flower, Sp::Tree, Sp::Bird, Sp::Bee]));
        }
        let total: f32 = bag.iter().map(|b| b.1).sum();
        let mut x = self.rng.f() * total;
        for &(sp, w) in &bag {
            x -= w;
            if x <= 0.0 {
                return Card::Nature(sp);
            }
        }
        Card::Nature(bag[0].0)
    }

    fn use_slot(&mut self, si: usize) {
        let s = &mut self.hand[si];
        s.card = None;
        s.cd = CARD_CD;
        s.flip = 0.0;
    }

    /// Play the card in slot `si` on `tile`. Returns false if it cannot go there.
    pub fn play(&mut self, si: usize, tile: usize) -> bool {
        match self.hand[si].card {
            Some(Card::Nature(_)) => self.place(si, tile),
            Some(Card::Biome(b)) => {
                if !self.isl[0].can_biome(tile) {
                    return false;
                }
                self.use_slot(si);
                let Game { isl, rng, ev, .. } = self;
                isl[0].raise_biome(0, tile, b, rng, ev);
                self.biomes_placed += 1;
                self.shake = 0.6;
                true
            }
            None => false,
        }
    }

    pub fn place(&mut self, si: usize, tile: usize) -> bool {
        let Some(Card::Nature(sp)) = self.hand[si].card else { return false };
        if !self.isl[0].can_place(tile, sp) {
            return false;
        }
        let (value, _) = self.isl[0].place_value(tile, sp);
        let Game { isl, rng, ev, .. } = self;
        isl[0].add_ent(0, tile, sp, false, rng, ev);
        self.use_slot(si);
        self.placed += 1;
        self.sfx(Sfx::Place);
        // mega bloom: a lucky plant sprouts copies all around it
        if def(sp).plant && self.rng.f() < 0.07 {
            let Game { isl, rng, ev, .. } = self;
            let mut n = 0;
            for u in isl[0].map.tiles[tile].n1.clone() {
                if n < 3 && isl[0].can_place(u, sp) {
                    isl[0].add_ent(0, u, sp, true, rng, ev);
                    n += 1;
                }
            }
            if n > 0 {
                let t = &self.isl[0].map.tiles[tile];
                let (x, y) = (t.x, t.y);
                self.ev.push(Ev::Jackpot { isl: 0, x, y });
                self.ev.push(Ev::Burst { isl: 0, x, y: y - 10.0, kind: PKind::Gold, n: 18 });
                self.sfx(Sfx::Jackpot);
            }
        }
        if value >= 2.0 {
            self.combo = if self.t - self.last_place < COMBO_T { self.combo + 1 } else { 1 };
            self.last_place = self.t;
        } else {
            self.combo = 0;
        }
        if self.combo >= 2 {
            let t = &self.isl[0].map.tiles[tile];
            let (x, y) = (t.x, t.y - 40.0);
            self.ev.push(Ev::Combo { n: self.combo, x, y });
            self.sfx(Sfx::Combo(self.combo.min(8) as u8));
            if self.combo >= 3 {
                for s in &mut self.hand {
                    if s.card.is_none() {
                        s.cd *= 0.4;
                    }
                }
            }
            if self.combo == 5 || self.combo == 8 {
                self.sfx(Sfx::CrowdWow);
            }
        }
        true
    }

    pub fn reroll(&mut self) -> bool {
        if self.rr > 0.0 {
            return false;
        }
        for si in 0..4 {
            // biome cards are rare: keep them
            if matches!(self.hand[si].card, Some(Card::Nature(_))) {
                let c = self.draw_card(false);
                let s = &mut self.hand[si];
                s.card = Some(c);
                s.flip = 0.0;
            }
        }
        self.rr = REROLL_CD;
        self.sel = None;
        self.sfx(Sfx::Reroll);
        true
    }

    pub fn dig(&mut self, tile: usize) {
        if let Some((id, _)) = self.isl[0].map.tiles[tile].occ {
            let Game { isl, ev, .. } = self;
            isl[0].kill(0, id, Death::Dig, ev);
            self.sfx(Sfx::Dig);
        }
    }

    /// Nature's own trouble: a beetle swarm, or a thundercloud when it rains.
    fn hazard(&mut self) {
        let i = &mut self.isl[0];
        if i.ents.len() < 4 {
            return;
        }
        if self.weather == Weather::Rain || self.rng.chance(0.3) {
            let mut best = 0;
            let mut bn = -1.0;
            for (ti, t) in i.map.tiles.iter().enumerate() {
                let mut n = if t.occ.is_some() { 1.2 } else { 0.0 };
                n += t.n1.iter().filter(|&&u| i.map.tiles[u].occ.is_some()).count() as f32;
                n += self.rng.f() * 2.0;
                if n > bn {
                    bn = n;
                    best = ti;
                }
            }
            let (x, y) = (i.map.tiles[best].x, i.map.tiles[best].y);
            let id = i.new_id();
            i.storms.push(Storm { id, tile: best, x, y, t: 0.0, taps: 0, need: 5, struck: false, blown: false, st: 0.0, bt: 0.0, cx: x + 240.0, cy: y - 190.0, sc: 1.0, pulse: 0.0, bot: None, done: false });
        } else {
            for _ in 0..3 {
                let e = *self.rng.pick(&i.map.edge);
                let t = &i.map.tiles[e];
                let (x, y) = (t.x + (self.rng.f() - 0.5) * 20.0, t.y);
                let dir = self.rng.f() * std::f32::consts::TAU;
                let id = i.new_id();
                i.beetles.push(Beetle { id, x, y, tgt: None, eat: 0.0, life: 26.0, dead: 0.0, dir, leave: false, lift: 1.0, bot: None, done: false });
            }
        }
        self.alert = 1.6;
        self.sfx(Sfx::Alarm);
    }

    /// Tap a storm cloud (by id).
    pub fn tap_storm(&mut self, id: u32) -> bool {
        let Some(s) = self.isl[0].storms.iter_mut().find(|s| s.id == id) else { return false };
        if !s.active() || s.t < 0.5 {
            return false;
        }
        s.taps += 1;
        s.pulse = 1.0;
        let (cx, cy) = (s.cx, s.cy);
        if s.taps >= s.need {
            s.blown = true;
            s.bt = 0.0;
        }
        let blown = s.blown;
        self.ev.push(Ev::Burst { isl: 0, x: cx, y: cy, kind: PKind::Puff, n: 5 });
        if blown {
            self.defends += 1;
        }
        self.sfx(if blown { Sfx::Blown } else { Sfx::Puff });
        true
    }

    pub fn tap_beetle(&mut self, id: u32) -> bool {
        let Some(b) = self.isl[0].beetles.iter_mut().find(|b| b.id == id) else { return false };
        if !b.active() {
            return false;
        }
        b.dead = 0.001;
        let (x, y) = (b.x, b.y);
        self.ev.push(Ev::Burst { isl: 0, x, y: y - 4.0, kind: PKind::Splat, n: 7 });
        self.defends += 1;
        self.sfx(Sfx::Squish);
        true
    }

    /// Clear skies alternate with rain, drought or wind.
    fn update_weather(&mut self, dt: f32) {
        self.weather_t -= dt;
        if self.weather_t > 0.0 {
            return;
        }
        self.weather = self.forecast;
        self.weather_t = match self.weather {
            Weather::Clear => 45.0 + self.rng.f() * 25.0,
            Weather::Wind => 25.0 + self.rng.f() * 10.0,
            _ => 32.0 + self.rng.f() * 16.0,
        };
        self.forecast = if self.weather == Weather::Clear { *self.rng.pick(&[Weather::Rain, Weather::Rain, Weather::Drought, Weather::Wind]) } else { Weather::Clear };
        self.ev.push(Ev::Weather(self.weather));
        self.sfx(match self.weather {
            Weather::Clear => Sfx::ClearStart,
            Weather::Rain => Sfx::RainStart,
            Weather::Drought => Sfx::DroughtStart,
            Weather::Wind => Sfx::WindStart,
        });
    }

    pub fn tap_whale(&mut self) -> bool {
        let Game { isl, rng, ev, .. } = self;
        if isl[0].catch_whale(0, rng, ev) {
            self.sfx(Sfx::Coin);
            self.sfx(Sfx::Jackpot);
            true
        } else {
            false
        }
    }

    /// Autopilot: play the best card, raise biomes on quiet ground, swat pests.
    fn autoplay(&mut self, dt: f32) {
        self.think -= dt;
        if self.think > 0.0 {
            return;
        }
        self.think = 2.5 + self.rng.f() * 2.5;
        if let Some(id) = self.isl[0].storms.iter().find(|s| s.active() && s.t > 0.6).map(|s| s.id) {
            for _ in 0..5 {
                self.tap_storm(id);
            }
        }
        let bugs: Vec<u32> = self.isl[0].beetles.iter().filter(|b| b.active()).map(|b| b.id).collect();
        for id in bugs {
            self.tap_beetle(id);
        }
        if self.isl[0].whale.as_ref().map(|w| w.active() && w.t > 2.0).unwrap_or(false) {
            self.tap_whale();
        }
        let mut best: Option<(f32, usize, usize)> = None;
        for si in 0..4 {
            match self.hand[si].card {
                Some(Card::Nature(sp)) => {
                    for t in 0..self.isl[0].map.tiles.len() {
                        if !self.isl[0].can_place(t, sp) {
                            continue;
                        }
                        let v = self.isl[0].place_value(t, sp).0 + self.rng.f() * 1.1;
                        if best.map(|b| v > b.0).unwrap_or(true) {
                            best = Some((v, si, t));
                        }
                    }
                }
                Some(Card::Biome(_)) => {
                    let isl = &self.isl[0];
                    let spot = (0..isl.map.tiles.len()).filter(|&t| isl.can_biome(t)).max_by_key(|&t| {
                        let tl = &isl.map.tiles[t];
                        let busy = tl.n1.iter().chain(std::iter::once(&t)).filter(|&&u| isl.map.tiles[u].occ.is_some()).count() as i32;
                        tl.n1.len() as i32 * 2 - busy * 3 + (t as i32 * 7919) % 5
                    });
                    if let Some(t) = spot {
                        self.play(si, t);
                        return;
                    }
                }
                None => {}
            }
        }
        if let Some((v, si, t)) = best {
            if v > -0.2 {
                self.place(si, t);
                return;
            }
        }
        if let Some(si) = (0..4).find(|&si| matches!(self.hand[si].card, Some(Card::Nature(sp)) if !self.isl[0].has_spot(sp))) {
            self.use_slot(si);
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.over {
            self.end_t += dt;
            self.ceremony();
            let Game { isl, rng, ev, .. } = self;
            isl[0].update(0, dt * 0.5, true, rng, ev);
            return;
        }
        if self.paused {
            return;
        }
        self.t += dt;
        self.update_weather(dt);
        self.fade = (self.fade - dt * 3.0).max(0.0);
        self.shake = (self.shake - dt).max(0.0);
        self.flash = (self.flash - dt).max(0.0);
        self.alert = (self.alert - dt).max(0.0);
        for si in 0..4 {
            let s = &mut self.hand[si];
            s.wig = (s.wig - dt * 3.0).max(0.0);
            s.flip = (s.flip + dt * 3.0).min(1.0);
            if s.card.is_none() {
                s.cd -= dt;
                if s.cd <= 0.0 {
                    let c = self.draw_card(false);
                    let s = &mut self.hand[si];
                    s.card = Some(c);
                    s.flip = 0.0;
                }
            }
        }
        self.rr = (self.rr - dt).max(0.0);
        if self.auto {
            self.autoplay(dt);
        }
        if HAZARD_EVERY[self.diff] > 0.0 {
            self.hazard_t -= dt;
            if self.hazard_t <= 0.0 {
                self.hazard_t = HAZARD_EVERY[self.diff] * (0.8 + self.rng.f() * 0.4);
                if self.t > 30.0 && self.t < DUR - 20.0 {
                    self.hazard();
                }
            }
        }
        let weather = self.weather;
        let Game { isl, rng, ev, .. } = self;
        isl[0].weather = weather;
        isl[0].update(0, dt, true, rng, ev);
        if let Some(si) = self.sel {
            if self.hand[si].card.is_none() {
                self.sel = None;
            }
        }
        if self.t >= DUR {
            self.end();
        }
    }

    fn end(&mut self) {
        self.over = true;
        self.end_t = 0.0;
        self.sel = None;
        self.shovel = false;
        self.isl[0].recalc();
        self.final_score = self.isl[0].score;
        self.stars = STAR_AT.iter().filter(|&&s| self.final_score >= s).count() as u8;
        self.new_best = self.final_score > self.best + 0.5;
        self.cer = 0;
    }

    /// Results show: drumroll while the score counts up, then the stars one by one.
    fn ceremony(&mut self) {
        const AT: [f32; 6] = [0.2, STAR_REVEAL[0], STAR_REVEAL[1], STAR_REVEAL[2], 4.6, 5.0];
        while (self.cer as usize) < AT.len() && self.end_t >= AT[self.cer as usize] {
            match self.cer {
                0 => self.sfx(Sfx::Drumroll),
                k @ 1..=3 => {
                    let n = k as u8;
                    if self.stars >= n {
                        self.sfx(Sfx::Star(n));
                        self.sfx(if n == 3 { Sfx::BigCheer } else { Sfx::Applause });
                        if n == 3 {
                            self.sfx(Sfx::Firework);
                            self.sfx(Sfx::Cymbal);
                        }
                    } else if self.stars + 1 == n {
                        self.sfx(Sfx::Thud);
                        self.sfx(Sfx::Aww);
                    }
                }
                4 => {
                    if self.new_best {
                        self.sfx(Sfx::NewBest);
                        self.sfx(Sfx::CrowdWow);
                    }
                }
                _ => self.sfx(if self.stars >= 2 { Sfx::Win } else { Sfx::Lose }),
            }
            self.cer += 1;
        }
    }

    pub fn selected(&self) -> Option<Card> {
        self.sel.and_then(|s| self.hand[s].card)
    }

    /// Mood of a hypothetical `sp` on every free tile of the island.
    pub fn hints(&self, sp: Sp) -> Vec<(usize, f32, f32)> {
        let i = &self.isl[0];
        (0..i.map.tiles.len()).filter(|&t| i.can_place(t, sp)).map(|t| {
            let (v, h) = i.place_value(t, sp);
            (t, v, h)
        }).collect()
    }
}

/// Is a point (island units) close enough to a storm cloud to tap it?
pub fn storm_hit(s: &Storm, x: f32, y: f32) -> bool {
    (x - s.cx).hypot((y - s.cy) * 1.2) < 44.0 * s.sc
}

pub fn beetle_dist(b: &Beetle, x: f32, y: f32) -> f32 {
    (x - b.x).hypot(y - b.y + 5.0)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::island::Terr;

    pub fn auto_game(diff: usize, seed: u32) -> Game {
        let mut rng = Rng::new(seed);
        let map = crate::island::generate(rng.next_u32());
        let mut g = Game::new(map, diff, 0.0, rng);
        g.auto = true;
        g
    }

    #[test]
    fn solo_games_reach_the_stars() {
        let mut stars = vec![];
        for seed in 1..9 {
            let mut g = auto_game(1, seed * 977);
            let mut steps = 0;
            while !g.over && steps < 20 * (DUR as usize + 60) {
                g.update(0.05);
                g.ev.clear();
                steps += 1;
            }
            assert!(g.over && g.t >= DUR - 0.1, "game should run the full clock");
            let kinds = g.isl[0].seen.iter().filter(|s| **s).count();
            println!("seed {seed}: score={:.0} stars={} kinds_seen={kinds} biomes={:?} ents={} tiles={}", g.final_score, g.stars, g.isl[0].biomes, g.isl[0].ents.len(), g.isl[0].map.tiles.len());
            assert!(g.isl[0].biomes.len() <= MAX_BIOMES as usize);
            assert!(g.final_score > 60.0, "nothing grew: {}", g.final_score);
            stars.push(g.stars);
        }
        assert!(stars.iter().any(|&s| s >= 2), "autopilot should often earn two stars: {stars:?}");
        assert!(stars.iter().any(|&s| s < 3), "three stars should not be automatic: {stars:?}");
    }

    #[test]
    fn animals_hop_and_islands_grow() {
        let mut g = auto_game(1, 4242);
        let mut first_tile: std::collections::HashMap<u32, usize> = Default::default();
        let mut moved = 0;
        let mut grew = 0;
        for _ in 0..(20 * 220) {
            g.update(0.05);
            for e in g.ev.drain(..) {
                if let Ev::Grow { .. } = e {
                    grew += 1;
                }
            }
            for c in &g.isl[0].ents {
                let t0 = *first_tile.entry(c.id).or_insert(c.tile);
                if t0 != c.tile {
                    moved += 1;
                    first_tile.insert(c.id, c.tile);
                }
            }
            if g.over {
                break;
            }
        }
        println!("hops={moved} growths={grew}");
        assert!(moved > 10, "animals should move around, saw {moved}");
        assert!(grew >= 1, "the island should grow, saw {grew}");
        // every occupied tile points back at a living creature on that tile
        let isl = &g.isl[0];
        for (ti, t) in isl.map.tiles.iter().enumerate() {
            if let Some((id, sp)) = t.occ {
                let c = isl.ent(id).expect("occupant exists");
                assert_eq!(c.tile, ti);
                assert_eq!(c.sp, sp);
                assert!(def(sp).terr.contains(&t.t), "{sp:?} stranded on {:?}", t.t);
            }
        }
    }

    #[test]
    fn placement_respects_terrain() {
        let mut rng = Rng::new(7);
        let map = crate::island::generate(42);
        let mut isl = Island::new(map);
        let pond = isl.map.tiles.iter().position(|t| t.t == Terr::Pond).unwrap();
        assert!(!isl.can_place(pond, Sp::Tree));
        assert!(isl.can_place(pond, Sp::Frog));
        let mut ev = vec![];
        isl.add_ent(0, pond, Sp::Frog, false, &mut rng, &mut ev);
        assert!(!isl.can_place(pond, Sp::Lily));
    }
}
