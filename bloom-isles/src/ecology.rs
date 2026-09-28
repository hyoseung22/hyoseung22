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

use crate::eco::{def, Sp};
use crate::island::{soil, Terr};
use crate::rng::Rng;
use crate::sim::{Creature, Death, Ev, Island, Link, PKind, Sfx, Weather, SAD, THRIVE};

pub const TICK: f32 = 0.5;

/// Happiness from the tile's soil, shade and the weather, for species `sp`.
/// `virt` is a creature about to be placed (it may add shade).
impl Island {
    pub fn shade_at(&self, tile: usize, virt: Option<(usize, Sp)>) -> f32 {
        let tall = |u: usize| matches!(self.map.tiles[u].occ, Some((_, Sp::Tree | Sp::Palm))) || matches!(virt, Some((v, Sp::Tree | Sp::Palm)) if v == u);
        let n = self.map.tiles[tile].n1.iter().filter(|&&u| tall(u)).count();
        (n as f32 * 0.45).min(1.0)
    }

    pub fn env_mod(&self, tile: usize, sp: Sp, virt: Option<(usize, Sp)>) -> f32 {
        let t = &self.map.tiles[tile];
        let (m, f) = (t.moist, t.fert);
        let shade = self.shade_at(tile, virt);
        let w = self.weather;
        let rain = w == Weather::Rain;
        let drought = w == Weather::Drought;
        match sp {
            Sp::Flower => {
                let wet = if m < 0.25 { -0.14 } else if m > 0.85 { -0.05 } else { 0.05 };
                wet + (f - 0.4) * 0.2 - shade * 0.12
            }
            Sp::Bush => (f - 0.4) * 0.16 + (if m > 0.3 { 0.03 } else { -0.06 }),
            Sp::Tree => (m - 0.4) * 0.16 + (f - 0.4) * 0.1,
            Sp::Palm => (if m < 0.45 { 0.06 } else { -0.04 }) + (if drought { 0.05 } else { 0.0 }),
            Sp::Lily => if rain { 0.06 } else if drought { -0.1 } else { 0.0 },
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
        e.move_cd = 3.0 + rng.f() * 3.0;
        self.ver += 1;
    }

    pub fn ecology(&mut self, isl: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        self.soil_tick();
        self.terrain_tick(isl, viewed, rng, ev);
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
            if t.t == Terr::Pond {
                continue;
            }
            let p1 = t.n1.iter().filter(|&&u| self.map.tiles[u].t == Terr::Pond).count().min(2) as f32;
            let p2 = t.n2.iter().filter(|&&u| self.map.tiles[u].t == Terr::Pond).count().min(3) as f32 - p1.min(3.0);
            let shade = self.shade_at(i, None);
            let (base_m, base_f) = soil(t.t);
            let mut target = base_m + 0.12 * p1 + 0.04 * p2.max(0.0) + 0.1 * shade;
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
                Some((_, Sp::Tree)) => 0.004,
                Some((_, s)) if def(s).plant => 0.0028,
                _ => 0.0,
            };
            let mut df = 0.006 * animals + 0.004 * mushrooms - draw + 0.003 * (base_f - t.fert);
            if rain {
                df += 0.002;
            }
            let t = &mut self.map.tiles[i];
            t.moist += (target - t.moist) * 0.06;
            t.fert = (t.fert + df).clamp(0.0, 1.0);
            after = after.wrapping_mul(31).wrapping_add((t.moist * 4.0) as u32 * 5 + (t.fert * 4.0) as u32);
        }
        if before != after {
            self.soil_ver += 1;
        }
    }

    /// Slow terrain changes driven by soil and weather. At most one per tick.
    fn terrain_tick(&mut self, isl: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let rain = self.weather == Weather::Rain;
        let drought = self.weather == Weather::Drought;
        let ponds = self.map.tiles.iter().filter(|t| t.t == Terr::Pond).count();
        let mut change: Option<(usize, Terr)> = None;
        for i in 0..self.map.tiles.len() {
            let t = &self.map.tiles[i];
            let near = |kind: Sp, need_thrive: bool| {
                t.n1.iter().chain(std::iter::once(&i)).filter(|&&u| match self.map.tiles[u].occ {
                    Some((id, s)) if s == kind => !need_thrive || self.ent(id).map(|e| e.h >= THRIVE).unwrap_or(false),
                    _ => false,
                }).count()
            };
            let empty = t.occ.is_none();
            let pond_n = t.n1.iter().filter(|&&u| self.map.tiles[u].t == Terr::Pond).count();
            let rule = match t.t {
                // rich soil with happy flowers turns into a flowering meadow
                Terr::Grass if t.fert > 0.68 && near(Sp::Flower, true) > 0 => Some(Terr::Mead),
                // overgrazed, exhausted meadow thins back to grass
                Terr::Mead if t.fert < 0.22 && near(Sp::Rabbit, false) >= 2 => Some(Terr::Grass),
                // soaked ground beside a pond floods during rain
                Terr::Grass | Terr::Mead if rain && empty && pond_n > 0 && t.moist > 0.9 && ponds < 14 => Some(Terr::Pond),
                // a pond edge dries out during drought
                Terr::Pond if drought && empty && pond_n < 3 && ponds > 2 => Some(Terr::Grass),
                _ => None,
            };
            let t = &mut self.map.tiles[i];
            match rule {
                Some(to) => {
                    t.ctr += crate::ecology::TICK;
                    let need = match to {
                        Terr::Mead => 10.0,
                        Terr::Pond | Terr::Grass if rain || drought => 7.0,
                        _ => 12.0,
                    };
                    if t.ctr > need && change.is_none() {
                        change = Some((i, to));
                    }
                }
                None => t.ctr = (t.ctr - crate::ecology::TICK).max(0.0),
            }
        }
        if let Some((i, to)) = change {
            let from = self.map.tiles[i].t;
            self.map.set_terrain(i, to, rng);
            self.map_ver += 1;
            self.ver += 1;
            let (x, y) = (self.map.tiles[i].x, self.map.tiles[i].y);
            let kind = if to == Terr::Pond || from == Terr::Pond { PKind::Splash } else { PKind::Gold };
            ev.push(Ev::Burst { isl, x, y, kind, n: 10 });
            if viewed {
                ev.push(Ev::Sfx(if kind == PKind::Splash { Sfx::Splash } else { Sfx::Bloom }));
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
                let on_sand = self.map.tiles[self.ents[i].tile].t == Terr::Sand;
                let e = &mut self.ents[i];
                if def(sp).plant {
                    e.polli = (e.polli - dt).max(0.0);
                    e.hp = (e.hp + 0.01).min(1.0);
                    e.scorch = (e.scorch - dt).max(0.0);
                } else {
                    let burn = match sp {
                        Sp::Bee => 0.03,
                        Sp::Fox | Sp::Frog => 0.015,
                        Sp::Crab => 0.008,
                        _ => 0.02,
                    };
                    e.fed = (e.fed - burn * dt).max(0.0);
                    if sp == Sp::Frog && rain {
                        e.fed = (e.fed + 0.02).min(1.0);
                    }
                    if sp == Sp::Crab && on_sand {
                        e.fed = (e.fed + 0.004).min(1.0);
                    }
                    if e.fed <= 0.0 {
                        e.starve += dt;
                    } else {
                        e.starve = 0.0;
                    }
                }
            }
            if self.ents[i].starve > 20.0 {
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
            self.ents[i].act = 1.5 + rng.f() * 2.0;
            match sp {
                Sp::Rabbit => self.graze(isl, i, viewed, ev),
                Sp::Fox => self.hunt(isl, i, viewed, rng, ev),
                Sp::Bee => self.pollinate(isl, i, viewed, ev),
                Sp::Bird => self.bird(isl, i, viewed, rng, ev),
                Sp::Frog => self.frog(isl, i, viewed, ev),
                Sp::Crab => self.crab(isl, i, viewed, ev),
                _ => {}
            }
            i += 1;
        }
    }

    fn graze(&mut self, isl: usize, i: usize, viewed: bool, ev: &mut Vec<Ev>) {
        if self.ents[i].fed > 0.75 {
            return;
        }
        let n1 = self.map.tiles[self.ents[i].tile].n1.clone();
        // berries first, then flowers
        let fruit = n1.iter().find_map(|&u| match self.map.tiles[u].occ {
            Some((id, Sp::Bush)) => self.idx(id).filter(|&j| self.ents[j].polli > 0.0),
            _ => None,
        });
        let target = fruit.or_else(|| self.find(&n1, &[Sp::Flower]).and_then(|(_, id)| self.idx(id)));
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
        if let Some((rt, rid)) = self.find(&n1, &[Sp::Rabbit]) {
            let Some(j) = self.idx(rid) else { return };
            let to = self.pos(j);
            // a rabbit near a tree, or well fed, gets away more often
            let cover = self.map.tiles[rt].n1.iter().any(|&u| matches!(self.map.tiles[u].occ, Some((_, Sp::Tree | Sp::Bush))));
            let flee_p = 0.4 + if cover { 0.25 } else { 0.0 } + self.ents[j].fed * 0.15;
            let escape = self.map.tiles[rt].n1.clone().into_iter().filter(|&u| self.can_place(u, Sp::Rabbit) && !n1.contains(&u) && u != tile).max_by_key(|&u| (self.map.tiles[u].n1.len() as i32) - 0);
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
        // stalk: move next to a rabbit two tiles away
        let n2 = self.map.tiles[tile].n2.clone();
        if let Some((rt, rid)) = self.find(&n2, &[Sp::Rabbit]) {
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
            Some((id, Sp::Flower | Sp::Bush)) => self.idx(id).filter(|&j| self.ents[j].polli <= 0.0),
            _ => None,
        });
        let Some(j) = target else { return };
        self.ents[j].polli = if self.ents[j].sp == Sp::Bush { 30.0 } else { 22.0 };
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
        let fruit = n1.iter().find_map(|&u| match self.map.tiles[u].occ {
            Some((id, Sp::Bush)) => self.idx(id).filter(|&j| self.ents[j].polli > 0.0),
            _ => None,
        });
        if let Some(j) = fruit {
            self.ents[j].polli = 0.0;
            self.ents[i].fed = (self.ents[i].fed + 0.5).min(1.0);
            self.ents[i].seed = 4.0 + rng.f() * 4.0;
            let to = self.pos(j);
            self.link(isl, Link::Peck, i, to, true, ev);
            if viewed {
                ev.push(Ev::Sfx(Sfx::Peck));
            }
        }
    }

    fn drop_seed(&mut self, isl: usize, i: usize, viewed: bool, rng: &mut Rng, ev: &mut Vec<Ev>) {
        let tile = self.ents[i].tile;
        let sp = if rng.f() < 0.5 { Sp::Bush } else { Sp::Tree };
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
            let mut g = Game::new(map, 1, rng);
            g.players[0].bot = true;
            let vers: Vec<u32> = g.isl.iter().map(|i| i.map_ver).collect();
            let mut steps = 0;
            while !g.over && steps < 20 * 320 {
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
        for k in [Link::Pollinate, Link::Graze, Link::Hunt, Link::Peck] {
            assert!(kinds.get(&format!("{k:?}")).copied().unwrap_or(0) > 0, "no {k:?} interactions: {kinds:?}");
        }
        assert!(terrain_changes > 0, "the land never changed");
        assert!(weathers.len() >= 3, "weather barely changed: {weathers:?}");
    }
}
