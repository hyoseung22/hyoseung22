//! Game rules, independent of rendering: islands, cards, threats and bots.

use crate::eco::{self, def, Sp, ALL};
use crate::island::{IslandMap, SQ};
use crate::rng::Rng;

pub const THRIVE: f32 = 0.72;
pub const SAD: f32 = 0.32;
pub const DUR: f32 = 300.0;
pub const GOAL: f32 = 250.0;
pub const CARD_CD: f32 = 3.6;
pub const STORM_T: f32 = 7.0;
pub const SHIELD_T: f32 = 25.0;
pub const WITHER_T: f32 = 16.0;
pub const REROLL_CD: f32 = 7.0;
pub const PLAYERS: usize = 4;
/// Island index used for the title-screen island.
pub const TITLE: usize = 9;

pub struct Diff {
    pub cd: f32,
    pub think: f32,
    pub miss: f32,
    pub react: f32,
    pub tap: f32,
    pub aggr: f32,
    pub noise: f32,
    pub bias: f32,
}

pub const DIFFS: [Diff; 3] = [
    Diff { cd: 1.5, think: 1.7, miss: 0.5, react: 2.6, tap: 0.9, aggr: 0.22, noise: 2.4, bias: 0.8 },
    Diff { cd: 1.12, think: 1.2, miss: 0.28, react: 1.7, tap: 0.6, aggr: 0.36, noise: 1.1, bias: 1.1 },
    Diff { cd: 0.9, think: 0.85, miss: 0.1, react: 1.05, tap: 0.42, aggr: 0.5, noise: 0.45, bias: 1.3 },
];

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
    Storm,
    Beetle,
    Shield,
}

impl Card {
    pub fn is_attack(self) -> bool {
        matches!(self, Card::Storm | Card::Beetle)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Death {
    Wither,
    Zap,
    Eaten,
    Dig,
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
}

/// Things the renderer should react to.
#[derive(Clone, Debug)]
pub enum Ev {
    Burst { isl: usize, x: f32, y: f32, kind: PKind, n: u8 },
    Float { isl: usize, x: f32, y: f32, kind: PKind },
    Sfx(Sfx),
    Shake,
    Flash,
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
    pub shield: f32,
    pub hit_t: f32,
    pub hit_by: usize,
    pub recalc_t: f32,
    pub next_id: u32,
}

impl Island {
    pub fn new(map: IslandMap) -> Self {
        Island { map, ents: vec![], storms: vec![], beetles: vec![], score: 0.0, ds: 0.0, lush: 0.0, kinds: 0, ver: 1, shield: 0.0, hit_t: 0.0, hit_by: 0, recalc_t: 0.0, next_id: 1 }
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

    fn new_id(&mut self) -> u32 {
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
        h.clamp(0.0, 1.0)
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
        let mut kinds = [false; 12];
        let hs: Vec<f32> = self.ents.iter().map(|e| if e.dying > 0.0 { e.h } else { self.happ(e.tile, e.sp, None) }).collect();
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
            next: 6.0 + rng.f() * 6.0,
            born: 0.0,
            dying: 0.0,
            how: Death::Dig,
            v: rng.below(4) as u8,
            ph: rng.f() * std::f32::consts::TAU,
        };
        let (x, y) = (e.x, e.y);
        self.map.tiles[tile].occ = Some((id, sp));
        self.ents.push(e);
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
        self.ver += 1;
        let kind = match how {
            Death::Zap => PKind::Soot,
            Death::Wither => PKind::LeafDead,
            _ => PKind::Puff,
        };
        ev.push(Ev::Burst { isl, x, y: y - 10.0, kind, n: 8 });
        self.recalc();
    }

    fn try_spawn(&mut self, isl: usize, idx: usize, rng: &mut Rng, ev: &mut Vec<Ev>) -> bool {
        let (sp, tile) = (self.ents[idx].sp, self.ents[idx].tile);
        let Some(rule) = eco::spawn_rule(sp) else { return false };
        if rng.f() > rule.p {
            return false;
        }
        let t = &self.map.tiles[tile];
        if rule.needs_pair && !t.n1.iter().any(|&u| matches!(self.map.tiles[u].occ, Some((_, s)) if s == sp)) {
            return false;
        }
        let make = *rng.pick(rule.makes);
        let list = if rule.rad == 1 { t.n1.clone() } else { t.n2.clone() };
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
        self.shield = 0.0;
        self.hit_t = 0.0;
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
        if self.shield > 0.0 {
            self.shield -= dt;
        }
        if self.hit_t > 0.0 {
            self.hit_t -= dt;
        }

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
            if st == 2 {
                e.thr_t += dt;
                let (x, y, head, plant) = (e.x, e.y, def(e.sp).head, def(e.sp).plant);
                if e.thr_t > e.next {
                    e.thr_t = 0.0;
                    e.next = 7.0 + rng.f() * 7.0;
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
                for u in victims {
                    if let Some((id, _)) = self.map.tiles[u].occ {
                        if u == tile || rng.f() < 0.75 {
                            self.kill(isl, id, Death::Zap, ev);
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
            b.life -= dt;
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
                        b.x += dx / d * 38.0 * dt;
                        b.y += dy / d * 38.0 * dt;
                    } else {
                        b.eat += dt;
                        if b.eat >= 3.0 {
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
                    b.x += b.dir.cos() * 20.0 * dt;
                    b.y += b.dir.sin() * 14.0 * dt;
                }
            }
        }
        self.beetles.retain(|b| !b.done);
    }
}

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

#[derive(Clone, Debug)]
pub struct Player {
    pub bot: bool,
    pub hand: Vec<Slot>,
    pub think: f32,
    pub rr: f32,
}

#[derive(Clone, Debug)]
pub struct Fly {
    pub card: Card,
    pub from: usize,
    pub to: usize,
    pub t: f32,
    pub dur: f32,
    pub tile: Option<usize>,
    pub done: bool,
}

pub struct Game {
    pub players: Vec<Player>,
    pub isl: Vec<Island>,
    pub t: f32,
    pub view: usize,
    pub sel: Option<usize>,
    pub shovel: bool,
    pub fly: Vec<Fly>,
    pub shake: f32,
    pub flash: f32,
    pub alert: f32,
    pub fade: f32,
    pub placed: u32,
    pub attacks: u32,
    pub defends: u32,
    pub inspect: Option<usize>,
    pub diff: usize,
    pub paused: bool,
    pub over: bool,
    pub end_t: f32,
    pub ranks: Vec<usize>,
    pub rng: Rng,
    pub ev: Vec<Ev>,
}

impl Game {
    pub fn new(player_map: IslandMap, diff: usize, mut rng: Rng) -> Self {
        let mut isl = vec![Island::new(player_map)];
        for _ in 1..PLAYERS {
            let seed = rng.next_u32();
            isl.push(Island::new(crate::island::generate(seed)));
        }
        let players = (0..PLAYERS)
            .map(|i| Player { bot: i > 0, hand: vec![], think: 1.5 + rng.f() * 2.0, rr: 0.0 })
            .collect();
        let mut g = Game {
            players,
            isl,
            t: 0.0,
            view: 0,
            sel: None,
            shovel: false,
            fly: vec![],
            shake: 0.0,
            flash: 0.0,
            alert: 0.0,
            fade: 1.0,
            placed: 0,
            attacks: 0,
            defends: 0,
            inspect: None,
            diff,
            paused: false,
            over: false,
            end_t: 0.0,
            ranks: vec![],
            rng,
            ev: vec![],
        };
        for p in 0..PLAYERS {
            for _ in 0..4 {
                let c = g.draw_card(p, true);
                g.players[p].hand.push(Slot { card: Some(c), cd: 0.0, wig: 0.0, flip: 0.0 });
            }
        }
        g
    }

    fn sfx(&mut self, s: Sfx) {
        self.ev.push(Ev::Sfx(s));
    }

    pub fn draw_card(&mut self, p: usize, first: bool) -> Card {
        let r = self.rng.f();
        if !first && self.t > 14.0 {
            if r < 0.105 {
                return if self.rng.chance(0.5) { Card::Storm } else { Card::Beetle };
            }
            if r < 0.145 {
                return Card::Shield;
            }
        }
        let mut bag: Vec<(Sp, f32)> = vec![];
        for sp in ALL {
            if !self.isl[p].has_spot(sp) {
                continue;
            }
            let mut w = if def(sp).plant { 1.15 } else { 1.0 };
            if first && matches!(sp, Sp::Fox | Sp::Mushroom) {
                w *= 0.3;
            }
            bag.push((sp, w));
        }
        if bag.is_empty() {
            return if self.rng.chance(0.5) { Card::Storm } else { Card::Beetle };
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

    fn card_cd(&self, p: usize) -> f32 {
        CARD_CD * if self.players[p].bot { DIFFS[self.diff].cd } else { 1.0 }
    }

    fn use_slot(&mut self, p: usize, si: usize) {
        let cd = self.card_cd(p);
        let s = &mut self.players[p].hand[si];
        s.card = None;
        s.cd = cd;
        s.flip = 0.0;
    }

    pub fn place(&mut self, p: usize, si: usize, tile: usize) -> bool {
        let Some(Card::Nature(sp)) = self.players[p].hand[si].card else { return false };
        if !self.isl[p].can_place(tile, sp) {
            return false;
        }
        let Game { isl, rng, ev, .. } = self;
        isl[p].add_ent(p, tile, sp, false, rng, ev);
        self.use_slot(p, si);
        if !self.players[p].bot {
            self.placed += 1;
            self.sfx(Sfx::Place);
        } else if self.view == p {
            self.sfx(Sfx::PlaceSoft);
        }
        true
    }

    pub fn attack(&mut self, p: usize, si: usize, to: usize, tile: Option<usize>) {
        let Some(card) = self.players[p].hand[si].card else { return };
        if !card.is_attack() || to == p {
            return;
        }
        self.use_slot(p, si);
        self.fly.push(Fly { card, from: p, to, t: 0.0, dur: 1.15, tile, done: false });
        if !self.players[p].bot {
            self.attacks += 1;
            self.sfx(Sfx::Whoosh);
        } else if to == 0 {
            self.sfx(Sfx::Whoosh);
        }
    }

    pub fn use_shield(&mut self, p: usize, si: usize) {
        if self.players[p].hand[si].card != Some(Card::Shield) {
            return;
        }
        self.use_slot(p, si);
        let i = &mut self.isl[p];
        i.shield = SHIELD_T;
        let (x, y) = (i.map.b.cx(), i.map.b.cy());
        self.ev.push(Ev::Burst { isl: p, x, y, kind: PKind::Spark, n: 16 });
        for b in &mut self.isl[p].beetles {
            if b.active() {
                b.leave = true;
            }
        }
        if !self.players[p].bot || self.view == p {
            self.sfx(Sfx::Shield);
        }
    }

    pub fn reroll(&mut self) -> bool {
        if self.players[0].rr > 0.0 {
            return false;
        }
        for si in 0..4 {
            if self.players[0].hand[si].card.is_some() {
                let c = self.draw_card(0, false);
                let s = &mut self.players[0].hand[si];
                s.card = Some(c);
                s.flip = 0.0;
            }
        }
        self.players[0].rr = REROLL_CD;
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

    fn arrive(&mut self, f: &Fly) {
        let to = f.to;
        let i = &mut self.isl[to];
        i.hit_by = f.from;
        i.hit_t = 3.0;
        if i.shield > 0.0 {
            let (x, y) = (i.map.b.cx(), i.map.b.y0 - 10.0);
            self.ev.push(Ev::Burst { isl: to, x, y, kind: PKind::Spark, n: 14 });
            if to == 0 || self.view == to {
                self.sfx(Sfx::Bounce);
            }
            return;
        }
        match f.card {
            Card::Storm => {
                let tile = f.tile.unwrap_or_else(|| {
                    let mut best = 0;
                    let mut bn = -1.0;
                    for (ti, t) in i.map.tiles.iter().enumerate() {
                        let mut n = if t.occ.is_some() { 1.2 } else { 0.0 };
                        n += t.n1.iter().filter(|&&u| i.map.tiles[u].occ.is_some()).count() as f32;
                        n += self.rng.f() * 0.5;
                        if n > bn {
                            bn = n;
                            best = ti;
                        }
                    }
                    best
                });
                let (x, y) = (i.map.tiles[tile].x, i.map.tiles[tile].y);
                let id = i.new_id();
                i.storms.push(Storm { id, tile, x, y, t: 0.0, taps: 0, need: 6, struck: false, blown: false, st: 0.0, bt: 0.0, cx: x + 240.0, cy: y - 190.0, sc: 1.0, pulse: 0.0, bot: None, done: false });
            }
            Card::Beetle => {
                for _ in 0..3 {
                    let e = *self.rng.pick(&i.map.edge);
                    let t = &i.map.tiles[e];
                    let (x, y) = (t.x + (self.rng.f() - 0.5) * 20.0, t.y);
                    let dir = self.rng.f() * std::f32::consts::TAU;
                    let id = i.new_id();
                    i.beetles.push(Beetle { id, x, y, tgt: None, eat: 0.0, life: 30.0, dead: 0.0, dir, leave: false, lift: 1.0, bot: None, done: false });
                }
            }
            _ => {}
        }
        if to == 0 {
            self.alert = 2.2;
            self.sfx(Sfx::Alarm);
        }
    }

    /// Tap a storm (by id) or beetle (by id) on island `ii`.
    pub fn tap_storm(&mut self, ii: usize, id: u32, by_player: bool) -> bool {
        let view = self.view;
        let Some(s) = self.isl[ii].storms.iter_mut().find(|s| s.id == id) else { return false };
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
        self.ev.push(Ev::Burst { isl: ii, x: cx, y: cy, kind: PKind::Puff, n: 5 });
        if blown && by_player {
            self.defends += 1;
        }
        if by_player || view == ii {
            self.sfx(if blown { Sfx::Blown } else { Sfx::Puff });
        }
        true
    }

    pub fn tap_beetle(&mut self, ii: usize, id: u32, by_player: bool) -> bool {
        let view = self.view;
        let Some(b) = self.isl[ii].beetles.iter_mut().find(|b| b.id == id) else { return false };
        if !b.active() {
            return false;
        }
        b.dead = 0.001;
        let (x, y) = (b.x, b.y);
        self.ev.push(Ev::Burst { isl: ii, x, y: y - 4.0, kind: PKind::Splat, n: 7 });
        if by_player {
            self.defends += 1;
        }
        if by_player || view == ii {
            self.sfx(Sfx::Squish);
        }
        true
    }

    fn bot_think(&mut self, p: usize) {
        let d = &DIFFS[self.diff];
        let threatened = self.isl[p].threatened();
        if let Some(si) = self.players[p].hand.iter().position(|s| s.card == Some(Card::Shield)) {
            if threatened && self.isl[p].shield <= 0.0 {
                self.use_shield(p, si);
                return;
            }
        }
        if let Some(si) = self.players[p].hand.iter().position(|s| s.card.map(|c| c.is_attack()).unwrap_or(false)) {
            if self.rng.f() < d.aggr + 0.2 {
                let others: Vec<usize> = (0..PLAYERS).filter(|&o| o != p).collect();
                let ws: Vec<f32> = others.iter().map(|&o| (self.isl[o].score + 12.0).powf(1.6) * if o == 0 { d.bias } else { 1.0 }).collect();
                let mut x = self.rng.f() * ws.iter().sum::<f32>();
                let mut tgt = others[0];
                for (k, &o) in others.iter().enumerate() {
                    x -= ws[k];
                    if x <= 0.0 {
                        tgt = o;
                        break;
                    }
                }
                self.attack(p, si, tgt, None);
                return;
            }
        }
        let mut best: Option<(f32, usize, usize)> = None;
        for si in 0..4 {
            let Some(Card::Nature(sp)) = self.players[p].hand[si].card else { continue };
            for t in 0..self.isl[p].map.tiles.len() {
                if !self.isl[p].can_place(t, sp) {
                    continue;
                }
                let v = self.isl[p].place_value(t, sp).0 + self.rng.f() * d.noise;
                if best.map(|b| v > b.0).unwrap_or(true) {
                    best = Some((v, si, t));
                }
            }
        }
        if let Some((v, si, t)) = best {
            if v > -0.2 {
                self.place(p, si, t);
                return;
            }
        }
        let dead = (0..4).find(|&si| match self.players[p].hand[si].card {
            Some(Card::Nature(sp)) => !self.isl[p].has_spot(sp),
            Some(Card::Shield) => self.rng.f() < 0.15,
            _ => false,
        });
        if let Some(si) = dead {
            self.use_slot(p, si);
        }
    }

    fn bot_defend(&mut self, p: usize, dt: f32) {
        let d = &DIFFS[self.diff];
        let mut taps: Vec<(bool, u32)> = vec![];
        let (miss, react, tap) = (d.miss, d.react, d.tap);
        let rng = &mut self.rng;
        for s in &mut self.isl[p].storms {
            if !s.active() {
                continue;
            }
            let b = s.bot.get_or_insert_with(|| if rng.f() < miss { 1e9 } else { react * (0.6 + rng.f() * 0.8) });
            *b -= dt;
            if *b <= 0.0 {
                *b = tap * (0.7 + rng.f() * 0.6);
                taps.push((true, s.id));
            }
        }
        for s in &mut self.isl[p].beetles {
            if !s.active() {
                continue;
            }
            let b = s.bot.get_or_insert_with(|| if rng.f() < miss { 1e9 } else { react * (0.6 + rng.f() * 0.8) });
            *b -= dt;
            if *b <= 0.0 {
                *b = tap * (0.7 + rng.f() * 0.6);
                taps.push((false, s.id));
            }
        }
        for (storm, id) in taps {
            if storm {
                self.tap_storm(p, id, false);
            } else {
                self.tap_beetle(p, id, false);
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.over {
            self.end_t += dt;
            let Game { isl, rng, ev, .. } = self;
            for (i, s) in isl.iter_mut().enumerate() {
                s.update(i, dt * 0.5, false, rng, ev);
            }
            return;
        }
        if self.paused {
            return;
        }
        self.t += dt;
        self.fade = (self.fade - dt * 3.0).max(0.0);
        self.shake = (self.shake - dt).max(0.0);
        self.flash = (self.flash - dt).max(0.0);
        self.alert = (self.alert - dt).max(0.0);
        for p in 0..PLAYERS {
            for si in 0..4 {
                let s = &mut self.players[p].hand[si];
                s.wig = (s.wig - dt * 3.0).max(0.0);
                s.flip = (s.flip + dt * 3.0).min(1.0);
                if s.card.is_none() {
                    s.cd -= dt;
                    if s.cd <= 0.0 {
                        let c = self.draw_card(p, false);
                        let s = &mut self.players[p].hand[si];
                        s.card = Some(c);
                        s.flip = 0.0;
                    }
                }
            }
            let pl = &mut self.players[p];
            pl.rr = (pl.rr - dt).max(0.0);
            if pl.bot {
                pl.think -= dt;
                if pl.think <= 0.0 {
                    pl.think = (1.0 + self.rng.f() * 1.3) * DIFFS[self.diff].think;
                    self.bot_think(p);
                }
                self.bot_defend(p, dt);
            }
            let viewed = self.view == p;
            let Game { isl, rng, ev, .. } = self;
            isl[p].update(p, dt, viewed, rng, ev);
        }
        let mut arrived = vec![];
        for f in &mut self.fly {
            f.t += dt;
            if f.t >= f.dur && !f.done {
                f.done = true;
                arrived.push(f.clone());
            }
        }
        for f in arrived {
            self.arrive(&f);
        }
        self.fly.retain(|f| !f.done);
        if let Some(si) = self.sel {
            if self.players[0].hand[si].card.is_none() {
                self.sel = None;
            }
        }
        if self.t >= DUR || self.isl.iter().any(|i| i.score >= GOAL) {
            self.end();
        }
    }

    fn end(&mut self) {
        self.over = true;
        self.end_t = 0.0;
        self.sel = None;
        for i in &mut self.isl {
            i.recalc();
        }
        let mut ranks: Vec<usize> = (0..PLAYERS).collect();
        ranks.sort_by(|a, b| self.isl[*b].score.partial_cmp(&self.isl[*a].score).unwrap_or(std::cmp::Ordering::Equal));
        let win = ranks[0] == 0;
        self.ranks = ranks;
        self.sfx(if win { Sfx::Win } else { Sfx::Lose });
    }

    pub fn selected(&self) -> Option<Card> {
        self.sel.and_then(|s| self.players[0].hand[s].card)
    }

    /// Mood of a hypothetical `sp` on every free tile of the player's island.
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

    fn run(diff: usize, seed: u32) -> (f32, Vec<f32>) {
        let mut rng = Rng::new(seed);
        let map = crate::island::generate(rng.next_u32());
        let mut g = Game::new(map, diff, rng);
        g.players[0].bot = true;
        let mut steps = 0;
        while !g.over && steps < 20 * 400 {
            g.update(0.05);
            g.ev.clear();
            steps += 1;
        }
        (g.t, g.isl.iter().map(|i| i.score).collect())
    }

    #[test]
    fn games_finish_in_a_sensible_time() {
        for seed in 1..6 {
            let (t, scores) = run(1, seed * 977);
            assert!(t > 150.0, "game ended too fast: {t} {scores:?}");
            assert!(scores.iter().any(|s| *s > 60.0), "nobody grew anything: {scores:?}");
            println!("seed {seed}: t={t:.0} scores={scores:?}");
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
