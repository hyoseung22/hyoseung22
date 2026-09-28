//! Procedural island generation on a squashed, pointy-top hex grid.

use crate::rng::Rng;
use std::collections::HashMap;

/// Hex radius in island units.
pub const HS: f32 = 30.0;
/// Vertical squash that gives the tabletop look.
pub const SQ: f32 = 0.74;
pub const SQ3: f32 = 1.732_050_8;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Terr {
    Sand,
    Grass,
    Mead,
    Pond,
    Rock,
}

pub const DIRS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

pub fn hex_vertex(i: usize) -> (f32, f32) {
    let a = (60.0 * i as f32 - 30.0).to_radians();
    (a.cos(), a.sin() * SQ)
}

pub fn hdist(a: (i32, i32), b: (i32, i32)) -> i32 {
    ((a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.0 + a.1 - b.0 - b.1).abs()) / 2
}

pub fn hx(q: i32, r: i32) -> f32 {
    HS * SQ3 * (q as f32 + r as f32 / 2.0)
}

pub fn hy(r: i32) -> f32 {
    HS * 1.5 * r as f32 * SQ
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DecoKind {
    Tuft,
    Dot,
    Pebble,
    Shell,
}

#[derive(Clone, Debug)]
pub struct Deco {
    pub x: f32,
    pub y: f32,
    pub kind: DecoKind,
    pub col: u32,
    pub s: f32,
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub q: i32,
    pub r: i32,
    pub x: f32,
    pub y: f32,
    pub t: Terr,
    pub shade: f32,
    pub deco: Vec<Deco>,
    pub n1: Vec<usize>,
    pub n2: Vec<usize>,
    /// Occupant: creature id and species.
    pub occ: Option<(u32, crate::eco::Sp)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
}

impl Bounds {
    pub fn cx(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }
    pub fn cy(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }
}

#[derive(Clone, Debug)]
pub struct IslandMap {
    pub seed: u32,
    pub tiles: Vec<Tile>,
    pub index: HashMap<(i32, i32), usize>,
    /// Coastal tiles.
    pub edge: Vec<usize>,
    pub b: Bounds,
}

impl IslandMap {
    pub fn has(&self, q: i32, r: i32) -> bool {
        self.index.contains_key(&(q, r))
    }
    pub fn get(&self, q: i32, r: i32) -> Option<&Tile> {
        self.index.get(&(q, r)).map(|&i| &self.tiles[i])
    }
    pub fn dist(&self, a: usize, b: usize) -> i32 {
        let (ta, tb) = (&self.tiles[a], &self.tiles[b]);
        hdist((ta.q, ta.r), (tb.q, tb.r))
    }
    /// Nearest tile to an island-space point, if the point is on land.
    pub fn tile_at(&self, x: f32, y: f32) -> Option<usize> {
        let mut best = None;
        let mut bd = HS * 0.95;
        for (i, t) in self.tiles.iter().enumerate() {
            let d = (t.x - x).hypot((t.y - y) / SQ);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }
}

pub fn make_deco(t: Terr, rng: &mut Rng) -> Vec<Deco> {
    let count = match t {
        Terr::Grass => 3,
        Terr::Mead => 5,
        Terr::Sand => 2,
        _ => 0,
    };
    (0..count)
        .map(|k| {
            let a = rng.f() * std::f32::consts::TAU;
            let rr = rng.f().sqrt() * HS * 0.72;
            let kind = match t {
                Terr::Sand => {
                    if rng.f() < 0.3 {
                        DecoKind::Shell
                    } else {
                        DecoKind::Pebble
                    }
                }
                Terr::Mead if k > 1 => DecoKind::Dot,
                _ => DecoKind::Tuft,
            };
            let col = *rng.pick(&[0xf4efe0u32, 0xf2c14e, 0xe56b6f]);
            let s = 0.7 + rng.f() * 0.6;
            Deco { x: a.cos() * rr, y: a.sin() * rr * SQ, kind, col, s }
        })
        .collect()
}

impl IslandMap {
    /// Recompute neighbour lists, the coastline and the bounds.
    pub fn link(&mut self) {
        let n = self.tiles.len();
        for i in 0..n {
            let (q, r) = (self.tiles[i].q, self.tiles[i].r);
            let mut n1 = vec![];
            let mut n2 = vec![];
            for j in 0..n {
                let d = hdist((q, r), (self.tiles[j].q, self.tiles[j].r));
                if d == 1 {
                    n1.push(j);
                }
                if (1..=2).contains(&d) {
                    n2.push(j);
                }
            }
            self.tiles[i].n1 = n1;
            self.tiles[i].n2 = n2;
        }
        self.edge = (0..n).filter(|&i| DIRS.iter().any(|&(dq, dr)| !self.has(self.tiles[i].q + dq, self.tiles[i].r + dr))).collect();
        let mut b = Bounds { x0: f32::MAX, x1: f32::MIN, y0: f32::MAX, y1: f32::MIN };
        for t in &self.tiles {
            b.x0 = b.x0.min(t.x - HS);
            b.x1 = b.x1.max(t.x + HS);
            b.y0 = b.y0.min(t.y - HS);
            b.y1 = b.y1.max(t.y + HS);
        }
        self.b = b;
    }

    /// Raise `count` new beach tiles from the sea around the coast. Old tile indices stay valid.
    /// Beaches that end up inland turn to grass unless a beach-only creature lives there.
    pub fn grow(&mut self, count: usize, rng: &mut Rng) -> Vec<usize> {
        let mut added = vec![];
        for _ in 0..count {
            let mut best: Option<((i32, i32), f32)> = None;
            for t in &self.tiles {
                for (dq, dr) in DIRS {
                    let p = (t.q + dq, t.r + dr);
                    if self.has(p.0, p.1) || hdist(p, (0, 0)) > 9 {
                        continue;
                    }
                    let adj = DIRS.iter().filter(|&&(a, b)| self.has(p.0 + a, p.1 + b)).count() as f32;
                    let score = adj + rng.f() * 1.6;
                    if best.map(|b| score > b.1).unwrap_or(true) {
                        best = Some((p, score));
                    }
                }
            }
            let Some(((q, r), _)) = best else { break };
            let tile = Tile { q, r, x: hx(q, r), y: hy(r), t: Terr::Sand, shade: rng.f(), deco: make_deco(Terr::Sand, rng), n1: vec![], n2: vec![], occ: None };
            self.index.insert((q, r), self.tiles.len());
            added.push(self.tiles.len());
            self.tiles.push(tile);
        }
        for i in 0..self.tiles.len() {
            let t = &self.tiles[i];
            if t.t != Terr::Sand {
                continue;
            }
            let inland = DIRS.iter().all(|&(dq, dr)| self.has(t.q + dq, t.r + dr));
            let beach_only = matches!(t.occ, Some((_, crate::eco::Sp::Palm | crate::eco::Sp::Crab)));
            if inland && !beach_only {
                let nt = if rng.f() < 0.35 { Terr::Mead } else { Terr::Grass };
                self.tiles[i].t = nt;
                self.tiles[i].deco = make_deco(nt, rng);
            }
        }
        self.link();
        added
    }
}

pub fn generate(seed: u32) -> IslandMap {
    for attempt in 0..30u32 {
        if let Some(m) = try_gen(seed.wrapping_add(attempt.wrapping_mul(7919)), false) {
            return m;
        }
    }
    try_gen(seed, true).expect("forced generation always succeeds")
}

struct Bump {
    x: f32,
    y: f32,
    r: f32,
    a: f32,
}

fn try_gen(seed: u32, force: bool) -> Option<IslandMap> {
    let mut rng = Rng::new(seed);
    const RAD: i32 = 6;
    let bumps: Vec<Bump> = (0..8)
        .map(|_| Bump { x: (rng.f() - 0.5) * 330.0, y: (rng.f() - 0.5) * 330.0, r: 50.0 + rng.f() * 90.0, a: (rng.f() - 0.38) * 0.9 })
        .collect();
    let mead: Vec<Bump> = (0..5)
        .map(|_| Bump { x: (rng.f() - 0.5) * 300.0, y: (rng.f() - 0.5) * 300.0, r: 50.0 + rng.f() * 70.0, a: 0.0 })
        .collect();

    let mut raw: HashMap<(i32, i32), ()> = HashMap::new();
    for q in -RAD - 1..=RAD + 1 {
        for r in -RAD - 1..=RAD + 1 {
            let d = hdist((q, r), (0, 0));
            if d > RAD {
                continue;
            }
            let (x, y) = (hx(q, r), HS * 1.5 * r as f32);
            let mut v = 1.0 - (d as f32 / RAD as f32).powi(2) * 1.05;
            for b in &bumps {
                v += b.a * (-((x - b.x).powi(2) + (y - b.y).powi(2)) / (b.r * b.r)).exp();
            }
            v += (rng.f() - 0.5) * 0.16;
            if v > 0.28 || d == 0 || (force && d < 4) {
                raw.insert((q, r), ());
            }
        }
    }
    // keep only the piece connected to the centre
    let mut land: Vec<(i32, i32)> = Vec::new();
    let mut seen: HashMap<(i32, i32), ()> = HashMap::new();
    let mut stack = vec![(0, 0)];
    while let Some(p) = stack.pop() {
        if seen.contains_key(&p) || !raw.contains_key(&p) {
            continue;
        }
        seen.insert(p, ());
        land.push(p);
        for (dq, dr) in DIRS {
            stack.push((p.0 + dq, p.1 + dr));
        }
    }
    if !force && !(62..=112).contains(&land.len()) {
        return None;
    }
    land.sort();

    let mut tiles: Vec<Tile> = land
        .iter()
        .map(|&(q, r)| Tile { q, r, x: hx(q, r), y: hy(r), t: Terr::Grass, shade: rng.f(), deco: vec![], n1: vec![], n2: vec![], occ: None })
        .collect();
    let index: HashMap<(i32, i32), usize> = tiles.iter().enumerate().map(|(i, t)| ((t.q, t.r), i)).collect();
    let lookup = |q: i32, r: i32| index.get(&(q, r)).copied();

    let mut edge = vec![];
    for i in 0..tiles.len() {
        let (q, r) = (tiles[i].q, tiles[i].r);
        let coast = DIRS.iter().any(|&(dq, dr)| lookup(q + dq, r + dr).is_none());
        if coast {
            tiles[i].t = Terr::Sand;
            edge.push(i);
        } else {
            let (x, y) = (tiles[i].x, HS * 1.5 * r as f32);
            let m: f32 = mead.iter().map(|b| (-((x - b.x).powi(2) + (y - b.y).powi(2)) / (b.r * b.r)).exp()).sum();
            tiles[i].t = if m > 0.45 { Terr::Mead } else { Terr::Grass };
        }
    }
    // a few coastal tiles stay grassy so the beach is not a perfect ring
    for &i in &edge {
        if rng.f() < 0.18 {
            let (q, r) = (tiles[i].q, tiles[i].r);
            let inner = DIRS.iter().any(|&(dq, dr)| lookup(q + dq, r + dr).map(|n| tiles[n].t != Terr::Sand).unwrap_or(false));
            if inner {
                tiles[i].t = Terr::Grass;
            }
        }
    }
    let interior: Vec<usize> = (0..tiles.len())
        .filter(|&i| {
            let (q, r) = (tiles[i].q, tiles[i].r);
            tiles[i].t != Terr::Sand && DIRS.iter().all(|&(dq, dr)| lookup(q + dq, r + dr).map(|n| tiles[n].t != Terr::Sand).unwrap_or(false))
        })
        .collect();
    if !force && interior.len() < 8 {
        return None;
    }
    // ponds
    let ponds = if interior.len() > 30 { 2 } else { 1 };
    for _ in 0..ponds {
        if interior.is_empty() {
            break;
        }
        let c = interior[rng.below(interior.len())];
        if tiles[c].t == Terr::Pond {
            continue;
        }
        tiles[c].t = Terr::Pond;
        let (q, r) = (tiles[c].q, tiles[c].r);
        let mut around: Vec<usize> = DIRS
            .iter()
            .filter_map(|&(dq, dr)| lookup(q + dq, r + dr))
            .filter(|&n| tiles[n].t != Terr::Sand && tiles[n].t != Terr::Pond)
            .collect();
        rng.shuffle(&mut around);
        let grow = 2 + rng.below(2);
        for n in around.into_iter().take(grow) {
            tiles[n].t = Terr::Pond;
        }
    }
    // rocks
    let grassy: Vec<usize> = (0..tiles.len()).filter(|&i| matches!(tiles[i].t, Terr::Grass | Terr::Mead)).collect();
    for _ in 0..2 + rng.below(3) {
        if grassy.is_empty() {
            break;
        }
        let t = grassy[rng.below(grassy.len())];
        tiles[t].t = Terr::Rock;
    }
    if !force && tiles.iter().filter(|t| t.t == Terr::Sand).count() < 8 {
        return None;
    }

    for t in &mut tiles {
        t.deco = make_deco(t.t, &mut rng);
    }
    let mut m = IslandMap { seed, tiles, index, edge, b: Bounds { x0: 0.0, x1: 0.0, y0: 0.0, y1: 0.0 } };
    m.link();
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn islands_have_every_terrain_needed() {
        for seed in 0..200u32 {
            let m = generate(seed.wrapping_mul(2_654_435_761));
            assert!(m.tiles.len() >= 40, "seed {seed} too small");
            assert!(m.tiles.iter().any(|t| t.t == Terr::Sand));
            assert!(m.tiles.iter().any(|t| t.t == Terr::Pond), "seed {seed} lacks a pond");
            assert!(m.tiles.iter().any(|t| matches!(t.t, Terr::Grass | Terr::Mead)));
        }
    }
}
