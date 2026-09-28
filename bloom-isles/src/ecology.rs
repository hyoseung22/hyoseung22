//! The living part of an island: soil, weather and what creatures do to each other.
//!
//! Every half second each island:
//! - moves soil moisture toward what ponds, shade and weather allow, and
//!   fertility up where animals live and things die, down where plants feed;
//! - lets rich flowered grass bloom into meadow, overgrazed meadow thin back to
//!   grass, soaked ground next to ponds flood into wetland, and dry ponds shrink;
//! - lets creatures act on each other: rabbits graze, foxes hunt (rabbits may
//!   flee), bees pollinate (flowers seed faster, bushes fruit), birds eat
//!   fruit and later drop seeds, frogs, birds and crabs eat beetles, hungry
//!   frogs eat bees, the dead feed crabs and grow mushrooms;
//! - makes animals hungry. Fed animals are happier; starving ones leave.
//!
//! Biome cards raise whole environments (volcano, lake, snowy peak, desert).
//! When ground changes under a creature, a plant either adapts into a local
//! cousin or withers, and an animal migrates to the nearest place it can live
//! (or leaves the island). Volcanoes erupt, snow comes and goes with the
//! weather, deserts creep in droughts and green up by water in the rain.

use crate::eco::{adapt_to, def, Biome, Sp};
use crate::island::{hdist, soil, Terr};
use crate::rng::Rng;
use crate::sim::{Creature, Death, Ev, Island, Link, PKind, Sfx, Weather, SAD, THRIVE};

pub const TICK: f32 = 0.5;

/// Happiness from the tile's soil, shade and the weather, for species `sp`.
/// `virt` is a creature about to be placed (it may add shade).
impl Island {
    pub fn shade_at(&self, tile: usize, virt: Option<(usize, Sp)>) -> f32 {
        let tall = |u: usize| matches!(self.map.tiles[u].occ, Some((_, Sp::Tree | Sp::Palm | Sp::Pine))) || matches!(virt, Some((v, Sp::Tree | Sp::Palm | Sp::Pine)) if v == u);
        let n = self.map.tiles[tile].n1.iter().filter(|&&u| tall(u)).count();
        (n as f32 * 0.45).min(1.0)
    }

    /// Adjacent volcanoes (heat) and snow (cold) around a tile.
    pub fn climate(&self, tile: usize) -> (f32, f32) {
        let t = &self.map.tiles[tile];
        let mut heat: f32 = if t.t == Terr::Ash { 0.5 } else { 0.0 };
        let mut cold: f32 = if t.t == Terr::Snow { 1.0 } else { 0.0 };
        for &u in &t.n1 {
            match self.map.tiles[u].t {
                Terr::Volcano => heat += 1.0,
                Terr::Peak => cold += 0.6,
                Terr::Snow => cold += 0.25,
                _ => {}
            }
        }
        (heat.min(2.0), cold.min(2.0))
    }

    pub fn env_mod(&self, tile: usize, sp: Sp, virt: Option<(usize, Sp)>) -> f32 {
        let t = &self.map.tiles[tile];
        let (m, f) = (t.moist, t.fert);
        let shade = self.shade_at(tile, virt);
        let w = self.weather;
        let rain = w == Weather::Rain;
        let drought = w == Weather::Drought;
        let (heat, cold) = self.climate(tile);
        let heat_lover = matches!(sp, Sp::Fireweed | Sp::Lizard | Sp::Cactus | Sp::Camel | Sp::Palm);
        let cold_lover = matches!(sp, Sp::Pine | Sp::Goat | Sp::Fox | Sp::Rabbit | Sp::Deer);
        let wet_life = matches!(sp, Sp::Lily | Sp::Reed | Sp::Fish | Sp::Duck | Sp::Frog);
        let climate = if heat_lover {
            heat * 0.06 - cold * 0.12
        } else if cold_lover {
            cold * 0.05 - heat * 0.14
        } else if wet_life {
            -heat * 0.06 - cold * 0.02
        } else {
            -heat * 0.14 - cold * 0.04
        };
        climate + match sp {
            Sp::Flower => {
                let wet = if m < 0.25 { -0.14 } else if m > 0.85 { -0.05 } else { 0.05 };
                wet + (f - 0.4) * 0.2 - shade * 0.12
            }
            Sp::Bush => (f - 0.4) * 0.16 + (if m > 0.3 { 0.03 } else { -0.06 }),
            Sp::Tree => (m - 0.4) * 0.16 + (f - 0.4) * 0.1,
            Sp::Palm => (if m < 0.45 { 0.06 } else { -0.04 }) + (if drought { 0.05 } else { 0.0 }),
            Sp::Lily | Sp::Reed => if rain { 0.06 } else if drought { -0.1 } else { 0.0 },
            Sp::Mushroom => shade * 0.22 + (if m > 0.55 { 0.1 } else if m < 0.3 { -0.15 } else { 0.0 }) + (if drought { -0.1 } else { 0.0 }),
            Sp::Bee => match w {
                Weather::Rain => -0.18,
                Weather::Wind => -0.08,
                Weather::Clear => 0.04,
                Weather::Drought => -0.04,
            },
            Sp::Rabbit => (f - 0.4) * 0.1 + (if rain { -0.05 } else { 0.0 }),
            Sp::Fox => if rain { -0.04 } else { 0.0 },
            Sp::Bird => (if w == Weather::Wind { -0.06 } else { 0.0 }) + shade * 0.06,
            Sp::Frog => if rain { 0.15 } else if drought { -0.2 } else { 0.0 },
            Sp::Crab => if drought { 0.05 } else if rain { -0.04 } else { 0.0 },
            Sp::Fern => shade * 0.25 + (if m > 0.5 { 0.08 } else { -0.1 }) + (if drought { -0.08 } else { 0.0 }),
            Sp::Pine => (if drought { -0.05 } else { 0.0 }) + (f - 0.3) * 0.08,
            Sp::Cactus => (if m < 0.35 { 0.1 } else if m > 0.6 { -0.15 } else { 0.0 }) + (if rain { -0.08 } else if drought { 0.06 } else { 0.0 }),
            Sp::Fireweed => (f - 0.4) * 0.2 + (if rain { -0.04 } else { 0.0 }),
            Sp::Deer => shade * 0.08 + (if rain { -0.03 } else { 0.0 }),
            Sp::Goat => if drought { -0.04 } else { 0.0 },
            Sp::Duck => if rain { 0.06 } else if drought { -0.06 } else { 0.0 },
            Sp::Fish => if drought { -0.1 } else if rain { 0.04 } else { 0.0 },
            Sp::Lizard => if rain { -0.1 } else if drought { 0.06 } else { 0.02 },
            Sp::Camel => if rain { -0.04 } else if drought { 0.04 } else { 0.0 },
        }
    }

    /// Nearest creature of one of `kinds` among the tiles `list`.
    fn find(&self, list: &[usize], kinds: &[Sp]) -> Option<(usize, u32)> {
        list.iter().find_map(|&u| match self.map.tiles[u].occ {
            Some((id, s)) if kinds.contains(&s) => Some((u, id)),
            _ => None,
        })
    }

    fn idx(&self, id: u32) -> Option<usize> {
        self.ents.iter().position(|e| e.id == id && e.dying == 0.0)
    }

    fn pos(&self, i: usize) -> (f32, f32) {
        (self.ents[i].x, self.ents[i].y)
    }

    fn link(&self, isl: usize, kind: Link, a: usize, b: (f32, f32), ok: bool, ev: &mut Vec<Ev>) {
        let (ax, ay) = self.pos(a);
        ev.push(Ev::Link { isl, kind, ax, ay, bx: b.0, by: b.1, ok });
    }

    /// Start a hop from creature `i` to `tile` (tile must be free).
    fn hop_to(&mut self, i: usize, tile: usize, rng: &mut Rng) {
        let (id, sp, from) = (self.ents[i].id, self.ents[i].sp, self.ents[i].tile);
        self.map.tiles[from].occ = None;
        self.map.tiles[tile].occ = Some((id, sp));
        let t = &self.map.tiles[tile];
        let (tx, ty) = (t.x + (rng.f() - 0.5) * 5.0, t.y + (rng.f() - 0.5) * 3.0);
        let e = &mut self.ents[i];
        e.tile = tile;
        e.fx = e.x;
        e.fy = e.y;
        e.tx = tx;
        e.ty = ty;
        e.hop = 0.001;
        e.hop_dur = crate::sim::HOP_T;
        e.move_cd = 6.0 + rng.f() * 6.0;
        self.ver += 1;
    }

    pub fn ecology(&mut self, isl: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        self.soil_tick();
        self.terrain_tick(isl, viewed, rng, ev);
        self.volcano_tick(isl, viewed, rng, ev);
        self.compost_tick(isl, viewed, rng, ev);
        self.creature_tick(isl, viewed, rng, ev);
    }

    fn soil_tick(&mut self) {
        let rain = self.weather == Weather::Rain;
        let drought = self.weather == Weather::Drought;
        let n = self.map.tiles.len();
        let mut before = 0u32;
        let mut after = 0u32;
        for i in 0..n {
            let t = &self.map.tiles[i];
            before = before.wrapping_mul(31).wrapping_add((t.moist * 4.0) as u32 * 5 + (t.fert * 4.0) as u32);
            if t.t.water() || t.t.summit() {
                continue;
            }
            let p1 = t.n1.iter().filter(|&&u| self.map.tiles[u].t.water()).count().min(2) as f32;
            let p2 = t.n2.iter().filter(|&&u| self.map.tiles[u].t.water()).count().min(3) as f32 - p1.min(3.0);
            let shade = self.shade_at(i, None);
            let (base_m, base_f) = soil(t.t);
            let (heat, cold) = self.climate(i);
            let mut target = base_m + 0.12 * p1 + 0.04 * p2.max(0.0) + 0.1 * shade - 0.12 * heat + 0.05 * cold;
            if rain {
                target += 0.4;
            }
            if drought {
                target -= 0.3;
            }
            let target = target.clamp(0.0, 1.0);
            let animals = t.n1.iter().chain(std::iter::once(&i)).filter(|&&u| matches!(self.map.tiles[u].occ, Some((_, s)) if !def(s).plant)).count().min(3) as f32;
            let mushrooms = t.n1.iter().filter(|&&u| matches!(self.map.tiles[u].occ, Some((_, Sp::Mushroom)))).count().min(2) as f32;
            let draw = match t.occ {
                Some((_, Sp::Tree | Sp::Pine)) => 0.004,
                Some((_, s)) if def(s).plant => 0.0028,
                _ => 0.0,
            };
            let mut df = 0.006 * animals + 0.004 * mushrooms - draw + 0.003 * (base_f - t.fert);
            if rain {
                df += 0.002;
            }
            let t = &mut self.map.tiles[i];
            t.moist += (target - t.moist) * 0.035;
            t.fert = (t.fert + df * 0.6).clamp(0.0, 1.0);
            after = after.wrapping_mul(31).wrapping_add((t.moist * 4.0) as u32 * 5 + (t.fert * 4.0) as u32);
        }
        if before != after {
            self.soil_ver += 1;
        }
    }

    /// Slow terrain changes driven by soil, weather and the biomes. At most one per tick.
    fn terrain_tick(&mut self, isl: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let rain = self.weather == Weather::Rain;
        let drought = self.weather == Weather::Drought;
        let count = |t: Terr| self.map.tiles.iter().filter(|x| x.t == t).count();
        let (ponds, lakes, dunes) = (count(Terr::Pond), count(Terr::Lake), count(Terr::Dune));
        let mut change: Option<(usize, Terr)> = None;
        for i in 0..self.map.tiles.len() {
            let t = &self.map.tiles[i];
            let near = |kind: Sp, need_thrive: bool| {
                t.n1.iter().chain(std::iter::once(&i)).filter(|&&u| match self.map.tiles[u].occ {
                    Some((id, s)) if s == kind => !need_thrive || self.ent(id).map(|e| e.h >= THRIVE).unwrap_or(false),
                    _ => false,
                }).count()
            };
            let n1_of = |k: Terr| t.n1.iter().filter(|&&u| self.map.tiles[u].t == k).count();
            let n2_of = |k: Terr| t.n2.iter().filter(|&&u| self.map.tiles[u].t == k).count();
            let empty = t.occ.is_none();
            let water_n = t.n1.iter().filter(|&&u| self.map.tiles[u].t.water()).count();
            let rule = match t.t {
                // rich soil with happy flowers turns into a flowering meadow
                Terr::Grass if t.fert > 0.68 && near(Sp::Flower, true) > 0 => Some(Terr::Mead),
                // overgrazed, exhausted meadow thins back to grass
                Terr::Mead if t.fert < 0.22 && near(Sp::Rabbit, false) + near(Sp::Goat, false) >= 2 => Some(Terr::Grass),
                // snow falls around a peak when it rains, and melts in a drought
                Terr::Grass | Terr::Mead if rain && n2_of(Terr::Peak) > 0 => Some(Terr::Snow),
                Terr::Snow if drought && n1_of(Terr::Peak) == 0 => Some(Terr::Grass),
                // the desert creeps in during droughts...
                Terr::Grass | Terr::Mead | Terr::Sand if drought && n1_of(Terr::Dune) >= 2 && dunes < 18 => Some(Terr::Dune),
                // ...and greens up beside water in the rain
                Terr::Dune if rain && water_n > 0 => Some(Terr::Grass),
                // ash far from any volcano cools into rich grass
                Terr::Ash if n2_of(Terr::Volcano) == 0 => Some(Terr::Grass),
                // soaked ground beside water floods during rain
                Terr::Grass | Terr::Mead if rain && empty && water_n > 0 && t.moist > 0.9 && ponds + lakes < 16 => Some(Terr::Pond),
                // a pond next to a lake joins it in the rain; a lake edge shrinks in a drought
                Terr::Pond if rain && n1_of(Terr::Lake) > 0 => Some(Terr::Lake),
                Terr::Lake if drought && n1_of(Terr::Lake) <= 2 && lakes > 3 => Some(Terr::Pond),
                // a pond edge dries out during drought
                Terr::Pond if drought && empty && water_n < 3 && ponds > 2 => Some(Terr::Grass),
                _ => None,
            };
            let t = &mut self.map.tiles[i];
            match rule {
                Some(to) => {
                    t.ctr += TICK;
                    let need = match to {
                        Terr::Mead => 20.0,
                        Terr::Snow | Terr::Dune => 18.0,
                        Terr::Pond | Terr::Grass | Terr::Lake if rain || drought => 14.0,
                        _ => 28.0,
                    };
                    if t.ctr > need && change.is_none() {
                        change = Some((i, to));
                    }
                }
                None => t.ctr = (t.ctr - TICK).max(0.0),
            }
        }
        if let Some((i, to)) = change {
            let from = self.map.tiles[i].t;
            self.retile(isl, i, to, viewed, rng, ev);
            let (x, y) = (self.map.tiles[i].x, self.map.tiles[i].y);
            let kind = match to {
                _ if to.water() || from.water() => PKind::Splash,
                Terr::Snow => PKind::Snow,
                Terr::Dune => PKind::Dust,
                _ => PKind::Gold,
            };
            ev.push(Ev::Burst { isl, x, y, kind, n: 10 });
            if viewed {
                ev.push(Ev::Sfx(if kind == PKind::Splash { Sfx::Splash } else { Sfx::Bloom }));
            }
        }
    }

    /// Change a tile's ground. Whoever lives there adapts, withers, or moves away.
    pub fn retile(&mut self, isl: usize, i: usize, to: Terr, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        self.map.set_terrain(i, to, rng);
        self.map_ver += 1;
        self.ver += 1;
        let Some((id, sp)) = self.map.tiles[i].occ else { return };
        if def(sp).terr.contains(&to) {
            return;
        }
        let Some(k) = self.idx(id) else { return };
        let (x, y) = self.pos(k);
        if def(sp).plant {
            // a healthy plant may turn into a cousin that suits the new ground
            let h = self.ents[k].h;
            let chance = 0.3 + h * 0.55;
            match adapt_to(sp, to) {
                Some(cousin) if def(cousin).terr.contains(&to) && rng.f() < chance => {
                    let e = &mut self.ents[k];
                    e.sp = cousin;
                    e.born = 0.2;
                    e.sad_t = 0.0;
                    e.hp = 1.0;
                    e.polli = 0.0;
                    self.map.tiles[i].occ = Some((id, cousin));
                    if !self.seen[cousin.idx()] {
                        self.seen[cousin.idx()] = true;
                        ev.push(Ev::Discover { isl, sp: cousin, x, y });
                    }
                    ev.push(Ev::Adapt { isl, x, y });
                    ev.push(Ev::Burst { isl, x, y: y - 12.0, kind: PKind::Gold, n: 10 });
                    if viewed {
                        ev.push(Ev::Sfx(Sfx::Adapt));
                    }
                    self.recalc();
                }
                _ => {
                    self.kill(isl, id, Death::Wither, ev);
                    if viewed {
                        ev.push(Ev::Sfx(Sfx::Wither));
                    }
                }
            }
        } else {
            self.migrate(isl, k, viewed, rng, ev);
        }
    }

    /// Send animal `k` to the nearest tile it can live on; if there is none it leaves the island.
    fn migrate(&mut self, isl: usize, k: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let (sp, from, id) = (self.ents[k].sp, self.ents[k].tile, self.ents[k].id);
        let (x, y) = self.pos(k);
        let (q, r) = (self.map.tiles[from].q, self.map.tiles[from].r);
        let mut best: Option<(f32, usize, i32)> = None;
        for u in 0..self.map.tiles.len() {
            if u == from || !self.can_place(u, sp) {
                continue;
            }
            let d = hdist((q, r), (self.map.tiles[u].q, self.map.tiles[u].r));
            let v = d as f32 - self.happ(u, sp, None) * 3.0;
            if best.map(|b| v < b.0).unwrap_or(true) {
                best = Some((v, u, d));
            }
        }
        ev.push(Ev::Fear { isl, x, y });
        if viewed {
            ev.push(Ev::Sfx(Sfx::Fear));
        }
        match best {
            Some((_, u, d)) => {
                if self.ents[k].hop > 0.0 {
                    let e = &mut self.ents[k];
                    e.x = e.tx;
                    e.y = e.ty;
                    e.hop = 0.0;
                }
                self.hop_to(k, u, rng);
                self.ents[k].hop_dur = crate::sim::HOP_T * (1.0 + 0.3 * (d - 1).max(0) as f32);
            }
            None => self.kill(isl, id, Death::Leave, ev),
        }
    }

    /// Can a biome be raised around `tile`?
    pub fn can_biome(&self, tile: usize) -> bool {
        let t = &self.map.tiles[tile];
        !t.t.summit() && !t.n1.iter().any(|&u| self.map.tiles[u].t.summit()) && !self.storms.iter().any(|s| s.active() && s.tile == tile)
    }

    /// The tiles a biome card changes: the centre and the ring around it (never another summit).
    pub fn biome_tiles(&self, tile: usize) -> Vec<usize> {
        let mut v = vec![tile];
        v.extend(self.map.tiles[tile].n1.iter().copied().filter(|&u| !self.map.tiles[u].t.summit()));
        v
    }

    /// What happens to each creature in the footprint: 0 fine, 1 may adapt, 2 withers, 3 moves away.
    pub fn biome_preview(&self, tile: usize, b: Biome) -> Vec<(usize, u8)> {
        let (c, ring) = b.terrain();
        self.biome_tiles(tile)
            .into_iter()
            .filter_map(|u| {
                let (_, sp) = self.map.tiles[u].occ?;
                let to = if u == tile { c } else { ring };
                let fate = if def(sp).terr.contains(&to) {
                    0
                } else if !def(sp).plant {
                    3
                } else if adapt_to(sp, to).map(|s| def(s).terr.contains(&to)).unwrap_or(false) {
                    1
                } else {
                    2
                };
                Some((u, fate))
            })
            .collect()
    }

    /// Raise a whole environment around `tile` and invite its natives.
    pub fn raise_biome(&mut self, isl: usize, tile: usize, b: Biome, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let (c, ring) = b.terrain();
        let tiles = self.biome_tiles(tile);
        for &u in &tiles {
            let to = if u == tile { c } else { ring };
            self.retile(isl, u, to, true, rng, ev);
            let (x, y) = (self.map.tiles[u].x, self.map.tiles[u].y);
            let kind = match b {
                Biome::Volcano => PKind::Ember,
                Biome::Lake => PKind::Splash,
                Biome::Peak => PKind::Snow,
                Biome::Desert => PKind::Dust,
            };
            ev.push(Ev::Burst { isl, x, y, kind, n: 8 });
        }
        if b == Biome::Volcano {
            self.erupt_t = 20.0;
        }
        self.biomes.push((b, tile));
        for (k, &sp) in b.natives().iter().enumerate() {
            self.arrivals.push((2.5 + k as f32 * 3.0, sp, tile));
        }
        let (x, y) = (self.map.tiles[tile].x, self.map.tiles[tile].y);
        ev.push(Ev::Biome { isl, biome: b, x, y });
        ev.push(Ev::Shake);
        ev.push(Ev::Sfx(Sfx::Biome(b.idx() as u8)));
        ev.push(Ev::Sfx(Sfx::CrowdWow));
        self.recalc();
    }

    /// Natives of a new biome turn up a moment after it rises.
    pub fn update_arrivals(&mut self, isl: usize, dt: f32, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        if self.arrivals.is_empty() {
            return;
        }
        for a in &mut self.arrivals {
            a.0 -= dt;
        }
        let due: Vec<(f32, Sp, usize)> = self.arrivals.iter().copied().filter(|a| a.0 <= 0.0).collect();
        self.arrivals.retain(|a| a.0 > 0.0);
        for (_, sp, near) in due {
            let (q, r) = (self.map.tiles[near].q, self.map.tiles[near].r);
            let mut best = None;
            let mut bv = f32::MIN;
            for u in 0..self.map.tiles.len() {
                if !self.can_place(u, sp) {
                    continue;
                }
                let d = hdist((q, r), (self.map.tiles[u].q, self.map.tiles[u].r));
                if d > 3 {
                    continue;
                }
                let v = self.place_value(u, sp).0 - d as f32 * 0.8 + rng.f();
                if v > bv {
                    bv = v;
                    best = Some(u);
                }
            }
            if let Some(u) = best {
                self.add_ent(isl, u, sp, true, rng, ev);
                let (x, y) = (self.map.tiles[u].x, self.map.tiles[u].y);
                ev.push(Ev::Burst { isl, x, y: y - 10.0, kind: PKind::Gold, n: 8 });
                if viewed {
                    ev.push(Ev::Sfx(Sfx::Sprout));
                }
            }
        }
    }

    /// Volcanoes rumble and erupt: ash spreads, soil gets richer, the heat drives life away.
    fn volcano_tick(&mut self, isl: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let vents: Vec<usize> = (0..self.map.tiles.len()).filter(|&i| self.map.tiles[i].t == Terr::Volcano).collect();
        if vents.is_empty() {
            return;
        }
        self.erupt_t -= TICK;
        if self.erupt_t > 0.0 {
            return;
        }
        self.erupt_t = 50.0 + rng.f() * 25.0;
        let v = *rng.pick(&vents);
        let (x, y) = (self.map.tiles[v].x, self.map.tiles[v].y);
        ev.push(Ev::Erupt { isl, x, y });
        ev.push(Ev::Shake);
        if viewed {
            ev.push(Ev::Sfx(Sfx::Erupt));
        }
        for u in self.map.tiles[v].n1.clone() {
            let t = &mut self.map.tiles[u];
            t.fert = (t.fert + 0.15).min(1.0);
            t.moist = (t.moist - 0.2).max(0.0);
            let Some((id, sp)) = t.occ else { continue };
            let Some(k) = self.idx(id) else { continue };
            if matches!(sp, Sp::Fireweed | Sp::Lizard | Sp::Cactus) {
                self.ents[k].jump = 1.0;
            } else if def(sp).plant {
                self.ents[k].scorch = 10.0;
                self.ents[k].hp -= 0.3;
            } else {
                self.migrate(isl, k, viewed, rng, ev);
            }
        }
        // a lava bomb turns a tile nearby to ash
        if rng.chance(0.6) {
            let ring2: Vec<usize> = self.map.tiles[v].n2.iter().copied().filter(|&u| {
                let t = &self.map.tiles[u];
                !t.n1.contains(&v) && matches!(t.t, Terr::Grass | Terr::Mead | Terr::Sand | Terr::Dune | Terr::Snow)
            }).collect();
            if !ring2.is_empty() {
                let u = *rng.pick(&ring2);
                self.retile(isl, u, Terr::Ash, viewed, rng, ev);
                let (bx, by) = (self.map.tiles[u].x, self.map.tiles[u].y);
                ev.push(Ev::Burst { isl, x: bx, y: by, kind: PKind::Ember, n: 12 });
            }
        }
    }

    /// The dead feed nearby crabs and sometimes sprout mushrooms.
    fn compost_tick(&mut self, isl: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let spots: Vec<usize> = self.compost.drain(..).collect();
        for tile in spots {
            let (x, y) = (self.map.tiles[tile].x, self.map.tiles[tile].y);
            for u in self.map.tiles[tile].n2.clone() {
                if let Some((id, Sp::Crab)) = self.map.tiles[u].occ {
                    if let Some(i) = self.idx(id) {
                        self.ents[i].fed = (self.ents[i].fed + 0.3).min(1.0);
                    }
                }
            }
            if rng.f() > 0.35 {
                continue;
            }
            let mut spot = None;
            let mut cand = vec![tile];
            cand.extend(self.map.tiles[tile].n1.iter().copied());
            for u in cand {
                if self.can_place(u, Sp::Mushroom) && self.happ(u, Sp::Mushroom, None) >= SAD {
                    spot = Some(u);
                    break;
                }
            }
            if let Some(u) = spot {
                let id = self.add_ent(isl, u, Sp::Mushroom, true, rng, ev);
                if let Some(i) = self.idx(id) {
                    ev.push(Ev::Link { isl, kind: Link::Compost, ax: x, ay: y, bx: self.ents[i].x, by: self.ents[i].y, ok: true });
                }
                if viewed {
                    ev.push(Ev::Sfx(Sfx::Sprout));
                }
            }
        }
    }

    fn creature_tick(&mut self, isl: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let dt = TICK;
        let rain = self.weather == Weather::Rain;
        let mut i = 0;
        while i < self.ents.len() {
            if self.ents[i].dying > 0.0 {
                i += 1;
                continue;
            }
            let sp = self.ents[i].sp;
            // passive state
            {
                let here = self.map.tiles[self.ents[i].tile].t;
                let on_sand = here == Terr::Sand;
                let sunny = matches!(self.weather, Weather::Clear | Weather::Drought);
                let warm_rock = matches!(here, Terr::Ash | Terr::Dune | Terr::Rock | Terr::Sand);
                let weeds = self.map.tiles[self.ents[i].tile].n1.iter().filter(|&&u| matches!(self.map.tiles[u].occ, Some((_, Sp::Lily | Sp::Reed)))).count().min(2) as f32;
                let e = &mut self.ents[i];
                if def(sp).plant {
                    e.polli = (e.polli - dt).max(0.0);
                    e.hp = (e.hp + 0.01).min(1.0);
                    e.scorch = (e.scorch - dt).max(0.0);
                } else {
                    let burn = match sp {
                        Sp::Bee => 0.03,
                        Sp::Fox | Sp::Frog | Sp::Goat => 0.015,
                        Sp::Crab | Sp::Fish | Sp::Lizard => 0.01,
                        Sp::Camel => 0.007,
                        _ => 0.02,
                    };
                    e.fed = (e.fed - burn * 0.5 * dt).max(0.0);
                    if sp == Sp::Frog && rain {
                        e.fed = (e.fed + 0.02).min(1.0);
                    }
                    if sp == Sp::Crab && on_sand {
                        e.fed = (e.fed + 0.004).min(1.0);
                    }
                    // fish nibble water plants; lizards run on sunshine
                    if sp == Sp::Fish {
                        e.fed = (e.fed + 0.004 + 0.006 * weeds).min(1.0);
                    }
                    if sp == Sp::Lizard && sunny && warm_rock {
                        e.fed = (e.fed + 0.012).min(1.0);
                    }
                    if e.fed <= 0.0 {
                        e.starve += dt;
                    } else {
                        e.starve = 0.0;
                    }
                }
            }
            if self.ents[i].starve > 40.0 {
                let id = self.ents[i].id;
                self.kill(isl, id, Death::Wither, ev);
                i += 1;
                continue;
            }
            // a bird drops the seed it carries
            if sp == Sp::Bird && self.ents[i].seed > 0.0 {
                self.ents[i].seed -= dt;
                if self.ents[i].seed <= 0.0 {
                    self.drop_seed(isl, i, viewed, rng, ev);
                }
            }
            self.ents[i].act -= dt;
            if self.ents[i].act > 0.0 || self.ents[i].hop > 0.0 || self.ents[i].born < 1.0 {
                i += 1;
                continue;
            }
            self.ents[i].act = 3.5 + rng.f() * 3.5;
            match sp {
                Sp::Rabbit => self.graze(isl, i, &[Sp::Flower], viewed, ev),
                Sp::Deer => self.graze(isl, i, &[Sp::Fern, Sp::Flower], viewed, ev),
                // goats eat anything, spines included
                Sp::Goat => self.graze(isl, i, &[Sp::Flower, Sp::Fern, Sp::Cactus, Sp::Pine], viewed, ev),
                Sp::Duck => {
                    if !self.snap_beetle(isl, i, 60.0, Link::Peck, viewed, ev) {
                        self.graze(isl, i, &[Sp::Lily, Sp::Reed], viewed, ev);
                    }
                }
                Sp::Camel => self.camel(isl, i, viewed, ev),
                Sp::Fox => self.hunt(isl, i, viewed, rng, ev),
                Sp::Bee => self.pollinate(isl, i, viewed, ev),
                Sp::Bird => self.bird(isl, i, viewed, rng, ev),
                Sp::Frog => self.frog(isl, i, viewed, ev),
                Sp::Lizard => {
                    if !self.snap_beetle(isl, i, 70.0, Link::Tongue, viewed, ev) && self.ents[i].fed < 0.3 {
                        self.eat_bee(isl, i, viewed, ev);
                    }
                }
                Sp::Crab => self.crab(isl, i, viewed, ev),
                _ => {}
            }
            i += 1;
        }
    }

    /// Eat berries off a bush if there are any, otherwise nibble one of `food`.
    fn graze(&mut self, isl: usize, i: usize, food: &[Sp], viewed: bool, ev: &mut Vec<Ev>) {
        if self.ents[i].fed > 0.75 {
            return;
        }
        let n1 = self.map.tiles[self.ents[i].tile].n1.clone();
        let fruit = n1.iter().find_map(|&u| match self.map.tiles[u].occ {
            Some((id, Sp::Bush)) => self.idx(id).filter(|&j| self.ents[j].polli > 0.0),
            _ => None,
        });
        let fruit = fruit.filter(|_| !matches!(self.ents[i].sp, Sp::Duck));
        let target = fruit.or_else(|| self.find(&n1, food).and_then(|(_, id)| self.idx(id)));
        let Some(j) = target else { return };
        let bush = self.ents[j].sp == Sp::Bush;
        if bush {
            self.ents[j].polli = 0.0;
            self.ents[i].fed = (self.ents[i].fed + 0.45).min(1.0);
        } else {
            self.ents[j].hp -= 0.34;
            self.ents[i].fed = (self.ents[i].fed + 0.3).min(1.0);
        }
        let to = self.pos(j);
        self.link(isl, Link::Graze, i, to, true, ev);
        ev.push(Ev::Burst { isl, x: to.0, y: to.1 - 8.0, kind: PKind::Leaf, n: 3 });
        if viewed {
            ev.push(Ev::Sfx(Sfx::Munch));
        }
        if self.ents[j].hp <= 0.0 {
            let id = self.ents[j].id;
            self.kill(isl, id, Death::Eaten, ev);
        }
    }

    fn hunt(&mut self, isl: usize, i: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        if self.ents[i].fed > 0.6 {
            return;
        }
        let tile = self.ents[i].tile;
        let n1 = self.map.tiles[tile].n1.clone();
        const PREY: &[Sp] = &[Sp::Rabbit, Sp::Duck];
        if let Some((rt, rid)) = self.find(&n1, PREY) {
            let Some(j) = self.idx(rid) else { return };
            let prey = self.ents[j].sp;
            let to = self.pos(j);
            // prey near cover, or well fed, gets away more often
            let cover = self.map.tiles[rt].n1.iter().any(|&u| matches!(self.map.tiles[u].occ, Some((_, Sp::Tree | Sp::Bush | Sp::Pine | Sp::Reed))));
            let flee_p = 0.4 + if cover { 0.25 } else { 0.0 } + self.ents[j].fed * 0.15;
            let escape = self.map.tiles[rt].n1.clone().into_iter().filter(|&u| self.can_place(u, prey) && !n1.contains(&u) && u != tile).max_by_key(|&u| self.map.tiles[u].n1.len() as i32);
            if rng.f() < flee_p && self.ents[j].hop == 0.0 {
                if let Some(u) = escape {
                    self.hop_to(j, u, rng);
                    ev.push(Ev::Fear { isl, x: to.0, y: to.1 });
                    self.link(isl, Link::Hunt, i, to, false, ev);
                    if viewed {
                        ev.push(Ev::Sfx(Sfx::Fear));
                    }
                    return;
                }
            }
            self.link(isl, Link::Hunt, i, to, true, ev);
            self.ents[i].fed = 1.0;
            self.ents[i].jump = 0.6;
            self.kill(isl, rid, Death::Eaten, ev);
            if viewed {
                ev.push(Ev::Sfx(Sfx::Chomp));
            }
            return;
        }
        // stalk: move next to prey two tiles away
        let n2 = self.map.tiles[tile].n2.clone();
        if let Some((rt, rid)) = self.find(&n2, PREY) {
            let step = self.map.tiles[rt].n1.iter().copied().find(|&u| n1.contains(&u) && self.can_place(u, Sp::Fox));
            if let (Some(u), Some(j)) = (step, self.idx(rid)) {
                let to = self.pos(j);
                self.hop_to(i, u, rng);
                ev.push(Ev::Fear { isl, x: to.0, y: to.1 });
                if viewed {
                    ev.push(Ev::Sfx(Sfx::Fear));
                }
            }
        }
    }

    fn pollinate(&mut self, isl: usize, i: usize, viewed: bool, ev: &mut Vec<Ev>) {
        let n1 = self.map.tiles[self.ents[i].tile].n1.clone();
        let target = n1.iter().find_map(|&u| match self.map.tiles[u].occ {
            Some((id, Sp::Flower | Sp::Bush | Sp::Fireweed | Sp::Cactus)) => self.idx(id).filter(|&j| self.ents[j].polli <= 0.0),
            _ => None,
        });
        let Some(j) = target else { return };
        self.ents[j].polli = if self.ents[j].sp == Sp::Bush { 55.0 } else { 40.0 };
        self.ents[i].fed = (self.ents[i].fed + 0.4).min(1.0);
        let to = self.pos(j);
        self.link(isl, Link::Pollinate, i, to, true, ev);
        ev.push(Ev::Burst { isl, x: to.0, y: to.1 - 16.0, kind: PKind::Spark, n: 4 });
        if viewed {
            ev.push(Ev::Sfx(Sfx::Pollinate));
        }
    }

    fn bird(&mut self, isl: usize, i: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let tile = self.ents[i].tile;
        let n1 = self.map.tiles[tile].n1.clone();
        // pest control first
        let (bx, by) = self.pos(i);
        if let Some(b) = self.beetles.iter_mut().find(|b| b.active() && (b.x - bx).hypot((b.y - by) / crate::island::SQ) < 70.0) {
            b.dead = 0.001;
            let to = (b.x, b.y);
            self.ents[i].fed = (self.ents[i].fed + 0.4).min(1.0);
            self.link(isl, Link::Peck, i, to, true, ev);
            ev.push(Ev::Burst { isl, x: to.0, y: to.1 - 4.0, kind: PKind::Splat, n: 6 });
            if viewed {
                ev.push(Ev::Sfx(Sfx::Peck));
            }
            return;
        }
        if self.ents[i].fed > 0.8 || self.ents[i].seed > 0.0 {
            return;
        }
        // a hungry bird dives for a fish
        if self.ents[i].fed < 0.35 {
            if let Some((_, fid)) = self.find(&n1, &[Sp::Fish]) {
                if let Some(j) = self.idx(fid) {
                    let to = self.pos(j);
                    self.link(isl, Link::Peck, i, to, true, ev);
                    self.ents[i].fed = 1.0;
                    self.kill(isl, fid, Death::Eaten, ev);
                    ev.push(Ev::Burst { isl, x: to.0, y: to.1, kind: PKind::Splash, n: 6 });
                    if viewed {
                        ev.push(Ev::Sfx(Sfx::Splash));
                    }
                    return;
                }
            }
        }
        let fruit = n1.iter().find_map(|&u| match self.map.tiles[u].occ {
            Some((id, Sp::Bush)) => self.idx(id).filter(|&j| self.ents[j].polli > 0.0),
            _ => None,
        });
        if let Some(j) = fruit {
            self.ents[j].polli = 0.0;
            self.ents[i].fed = (self.ents[i].fed + 0.5).min(1.0);
            self.ents[i].seed = 8.0 + rng.f() * 6.0;
            let to = self.pos(j);
            self.link(isl, Link::Peck, i, to, true, ev);
            if viewed {
                ev.push(Ev::Sfx(Sfx::Peck));
            }
        }
    }

    fn drop_seed(&mut self, isl: usize, i: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let tile = self.ents[i].tile;
        let sp = *rng.pick(&[Sp::Bush, Sp::Tree, Sp::Pine]);
        let mut spots: Vec<usize> = self.map.tiles[tile].n2.iter().copied().filter(|&u| self.can_place(u, sp) && self.happ(u, sp, None) >= SAD).collect();
        rng.shuffle(&mut spots);
        let Some(&u) = spots.first() else { return };
        let id = self.add_ent(isl, u, sp, true, rng, ev);
        if let Some(j) = self.idx(id) {
            let to = self.pos(j);
            self.link(isl, Link::Seed, i, to, true, ev);
        }
        if viewed {
            ev.push(Ev::Sfx(Sfx::SeedDrop));
        }
    }

    fn frog(&mut self, isl: usize, i: usize, viewed: bool, ev: &mut Vec<Ev>) {
        let (fx, fy) = self.pos(i);
        if let Some(b) = self.beetles.iter_mut().find(|b| b.active() && (b.x - fx).hypot((b.y - fy) / crate::island::SQ) < 80.0) {
            b.dead = 0.001;
            let to = (b.x, b.y);
            self.ents[i].fed = (self.ents[i].fed + 0.5).min(1.0);
            self.link(isl, Link::Tongue, i, to, true, ev);
            if viewed {
                ev.push(Ev::Sfx(Sfx::Tongue));
            }
            return;
        }
        // a hungry frog snaps up a bee
        if self.ents[i].fed < 0.35 {
            self.eat_bee(isl, i, viewed, ev);
        }
    }

    /// Catch a beetle within `range`. Returns true if one was caught.
    fn snap_beetle(&mut self, isl: usize, i: usize, range: f32, kind: Link, viewed: bool, ev: &mut Vec<Ev>) -> bool {
        let (fx, fy) = self.pos(i);
        let Some(b) = self.beetles.iter_mut().find(|b| b.active() && (b.x - fx).hypot((b.y - fy) / crate::island::SQ) < range) else { return false };
        b.dead = 0.001;
        let to = (b.x, b.y);
        self.ents[i].fed = (self.ents[i].fed + 0.5).min(1.0);
        self.link(isl, kind, i, to, true, ev);
        ev.push(Ev::Burst { isl, x: to.0, y: to.1 - 4.0, kind: PKind::Splat, n: 6 });
        if viewed {
            ev.push(Ev::Sfx(Sfx::Tongue));
        }
        true
    }

    /// Snap up a bee on a neighbouring tile.
    fn eat_bee(&mut self, isl: usize, i: usize, viewed: bool, ev: &mut Vec<Ev>) {
        let n1 = self.map.tiles[self.ents[i].tile].n1.clone();
        if let Some((_, bid)) = self.find(&n1, &[Sp::Bee]) {
            if let Some(j) = self.idx(bid) {
                let to = self.pos(j);
                self.link(isl, Link::Tongue, i, to, true, ev);
                self.ents[i].fed = 1.0;
                self.kill(isl, bid, Death::Eaten, ev);
                if viewed {
                    ev.push(Ev::Sfx(Sfx::Tongue));
                }
            }
        }
    }

    /// Camels drink deep from nearby water, or chew a cactus.
    fn camel(&mut self, isl: usize, i: usize, viewed: bool, ev: &mut Vec<Ev>) {
        if self.ents[i].fed > 0.6 {
            return;
        }
        let tile = self.ents[i].tile;
        let water = self.map.tiles[tile].n2.iter().copied().find(|&u| self.map.tiles[u].t.water());
        if let Some(u) = water {
            let to = (self.map.tiles[u].x, self.map.tiles[u].y);
            self.ents[i].fed = 1.0;
            self.link(isl, Link::Drink, i, to, true, ev);
            ev.push(Ev::Burst { isl, x: to.0, y: to.1, kind: PKind::Splash, n: 5 });
            if viewed {
                ev.push(Ev::Sfx(Sfx::Splash));
            }
            return;
        }
        self.graze(isl, i, &[Sp::Cactus], viewed, ev);
    }

    fn crab(&mut self, isl: usize, i: usize, viewed: bool, ev: &mut Vec<Ev>) {
        let (cx, cy) = self.pos(i);
        if let Some(b) = self.beetles.iter_mut().find(|b| b.active() && (b.x - cx).hypot((b.y - cy) / crate::island::SQ) < 50.0) {
            b.dead = 0.001;
            let to = (b.x, b.y);
            self.ents[i].fed = (self.ents[i].fed + 0.3).min(1.0);
            self.link(isl, Link::Pinch, i, to, true, ev);
            ev.push(Ev::Burst { isl, x: to.0, y: to.1 - 4.0, kind: PKind::Splat, n: 6 });
            if viewed {
                ev.push(Ev::Sfx(Sfx::Squish));
            }
        }
    }
}

/// Happiness from a creature's own condition: hunger, pollination, burns.
pub fn creature_mod(e: &Creature) -> f32 {
    if def(e.sp).plant {
        (if e.polli > 0.0 { 0.08 } else { 0.0 }) - (if e.scorch > 0.0 { 0.3 } else { 0.0 }) - (1.0 - e.hp) * 0.15
    } else {
        (e.fed - 0.5) * 0.24
    }
}

/// True when a creature is visibly hungry (for the hunger badge).
pub fn hungry(e: &Creature) -> bool {
    !def(e.sp).plant && e.fed < 0.2
}

#[cfg(test)]
mod tests {
    use crate::rng::Rng;
    use crate::sim::{Ev, Game, Link};

    /// A full bot game should show every kind of interaction and some terrain change.
    #[test]
    fn interactions_happen_and_the_land_changes() {
        let mut kinds = std::collections::HashMap::new();
        let mut terrain_changes = 0;
        let mut weathers = std::collections::HashSet::new();
        let mut hungry_leave = 0;
        for seed in [11u32, 23, 37] {
            let mut rng = Rng::new(seed);
            let map = crate::island::generate(rng.next_u32());
            let mut g = Game::new(map, 1, 0.0, rng);
            g.auto = true;
            let vers: Vec<u32> = g.isl.iter().map(|i| i.map_ver).collect();
            let mut steps = 0;
            while !g.over && steps < 20 * (crate::sim::DUR as usize + 40) {
                g.update(0.05);
                for e in g.ev.drain(..) {
                    match e {
                        Ev::Link { kind, .. } => *kinds.entry(format!("{kind:?}")).or_insert(0) += 1,
                        Ev::Weather(w) => {
                            weathers.insert(format!("{w:?}"));
                        }
                        _ => {}
                    }
                }
                steps += 1;
            }
            for (i, isl) in g.isl.iter().enumerate() {
                // growth also bumps map_ver; count only beyond the growth steps
                terrain_changes += isl.map_ver.saturating_sub(vers[i] + isl.grow_level as u32);
                hungry_leave += isl.ents.iter().filter(|e| crate::ecology::hungry(e)).count();
            }
            println!("seed {seed}: t={:.0} scores={:?}", g.t, g.isl.iter().map(|i| i.score as i32).collect::<Vec<_>>());
        }
        println!("links={kinds:?} terrain_changes={terrain_changes} weathers={weathers:?} hungry_now={hungry_leave}");
        for k in [Link::Pollinate, Link::Graze, Link::Peck] {
            assert!(kinds.get(&format!("{k:?}")).copied().unwrap_or(0) > 0, "no {k:?} interactions: {kinds:?}");
        }
        assert!(terrain_changes > 0, "the land never changed");
        assert!(weathers.len() >= 3, "weather barely changed: {weathers:?}");
    }

    /// Raising each biome changes the land, and the residents adapt, wither or move; natives arrive.
    #[test]
    fn biomes_reshape_the_island() {
        use crate::eco::{Biome, Sp, BIOMES};
        use crate::island::Terr;
        use crate::sim::{Death, Island};
        for (k, b) in BIOMES.iter().enumerate() {
            let mut rng = Rng::new(100 + k as u32);
            let mut isl = Island::new(crate::island::generate(900 + k as u32));
            let mut ev = vec![];
            // a centre tile with a full ring of land
            let c = (0..isl.map.tiles.len())
                .find(|&i| {
                    let t = &isl.map.tiles[i];
                    t.n1.len() == 6 && t.n1.iter().chain(std::iter::once(&i)).all(|&u| matches!(isl.map.tiles[u].t, Terr::Grass | Terr::Mead))
                })
                .expect("inland meadow");
            let ring = isl.map.tiles[c].n1.clone();
            // residents: healthy trees and flowers, and some animals
            for (j, &u) in ring.iter().enumerate() {
                let sp = [Sp::Tree, Sp::Flower, Sp::Rabbit, Sp::Bush, Sp::Deer, Sp::Fern][j % 6];
                isl.add_ent(0, u, sp, false, &mut rng, &mut ev);
            }
            isl.add_ent(0, c, Sp::Tree, false, &mut rng, &mut ev);
            for e in &mut isl.ents {
                e.h = 1.0;
                e.born = 5.0;
            }
            ev.clear();
            isl.raise_biome(0, c, *b, &mut rng, &mut ev);
            let (ct, rt) = b.terrain();
            assert_eq!(isl.map.tiles[c].t, ct);
            assert!(ring.iter().all(|&u| isl.map.tiles[u].t == rt));
            let adapted = ev.iter().filter(|e| matches!(e, Ev::Adapt { .. })).count();
            let fled = ev.iter().filter(|e| matches!(e, Ev::Fear { .. })).count();
            let gone = isl.ents.iter().filter(|e| e.dying > 0.0 && e.how == Death::Wither).count();
            // nobody alive is left standing on ground it cannot live on
            for e in isl.ents.iter().filter(|e| e.dying == 0.0) {
                assert!(crate::eco::def(e.sp).terr.contains(&isl.map.tiles[e.tile].t), "{:?} stranded on {:?} after {b:?}", e.sp, isl.map.tiles[e.tile].t);
            }
            // let the natives turn up and the volcano rumble
            let mut erupted = false;
            let mut arrived = 0;
            for _ in 0..(20 * 40) {
                isl.update(0, 0.05, true, &mut rng, &mut ev);
                for e in ev.drain(..) {
                    match e {
                        Ev::Erupt { .. } => erupted = true,
                        Ev::Discover { sp, .. } if b.natives().contains(&sp) => arrived += 1,
                        _ => {}
                    }
                }
            }
            let natives = b.natives().iter().filter(|sp| isl.seen[sp.idx()]).count();
            println!("{b:?}: adapted={adapted} fled={fled} withered={gone} natives_seen={natives} arrived={arrived} erupted={erupted}");
            assert!(adapted + fled + gone > 0, "{b:?} changed nothing for the residents");
            assert!(natives > 0, "{b:?} brought no natives");
            if *b == Biome::Volcano {
                assert!(erupted, "the volcano never erupted");
            }
        }
    }
}
