//! Low-poly 3D models, built from primitives at startup.
//!
//! Every face gets one flat colour, brightened or darkened by which way it
//! faces (baked into vertex colours, drawn unlit), and every part gets a bold
//! dark outline from an inflated back-face shell. Models stand on y = 0 and
//! face +X; one unit is one island unit (a hex tile has radius 30).

use crate::eco::Sp;
use crate::island::{IslandMap, Terr, HS, SQ};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Outline colour and default thickness.
pub const INK: u32 = 0x1a1d22;
pub const OUT: f32 = 1.05;

/// Camera pitch that makes the ground foreshorten by SQ, like the 2D layout.
pub fn pitch() -> f32 {
    SQ.asin()
}

/// Raw triangle soup.
#[derive(Clone, Default)]
pub struct Geo {
    pub pos: Vec<Vec3>,
    pub idx: Vec<u32>,
}

impl Geo {
    pub fn from_mesh(m: Mesh) -> Geo {
        let pos: Vec<Vec3> = match m.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|a| Vec3::from(*a)).collect(),
            _ => vec![],
        };
        let idx = match m.indices() {
            Some(Indices::U16(v)) => v.iter().map(|&i| i as u32).collect(),
            Some(Indices::U32(v)) => v.clone(),
            None => (0..pos.len() as u32).collect(),
        };
        Geo { pos, idx }
    }
    pub fn transformed(mut self, t: Transform) -> Geo {
        for p in &mut self.pos {
            *p = t.transform_point(*p);
        }
        // a mirroring scale flips winding
        if t.scale.x * t.scale.y * t.scale.z < 0.0 {
            for tri in self.idx.chunks_mut(3) {
                tri.swap(1, 2);
            }
        }
        self
    }
}

/* ---------- primitives ---------- */

pub fn ico(r: f32, sub: u32) -> Geo {
    Geo::from_mesh(Sphere::new(r).mesh().ico(sub).expect("ico"))
}
pub fn cyl(r: f32, h: f32, n: u32) -> Geo {
    Geo::from_mesh(Cylinder::new(r, h).mesh().resolution(n).build())
}
pub fn cone(r: f32, h: f32, n: u32) -> Geo {
    Geo::from_mesh(Cone { radius: r, height: h }.mesh().resolution(n).build())
}
pub fn frustum(rt: f32, rb: f32, h: f32, n: u32) -> Geo {
    Geo::from_mesh(ConicalFrustum { radius_top: rt, radius_bottom: rb, height: h }.mesh().resolution(n).build())
}
pub fn cuboid(x: f32, y: f32, z: f32) -> Geo {
    Geo::from_mesh(Cuboid::new(x, y, z).mesh().build())
}

pub fn at(x: f32, y: f32, z: f32) -> Transform {
    Transform::from_xyz(x, y, z)
}

/* ---------- colour ---------- */

pub fn srgb(hex: u32) -> [f32; 3] {
    [((hex >> 16) & 255) as f32 / 255.0, ((hex >> 8) & 255) as f32 / 255.0, (hex & 255) as f32 / 255.0]
}

/// Brightness of a face: lit from the upper left, towards the camera.
pub fn face_light(n: Vec3) -> f32 {
    let l = Vec3::new(-0.45, 0.85, 0.5).normalize();
    (0.56 + 0.5 * n.dot(l).max(0.0) + 0.1 * n.y).clamp(0.4, 1.12)
}

fn lit(hex: u32, k: f32) -> [f32; 4] {
    let c = srgb(hex);
    let f = |v: f32| {
        let o = if k <= 1.0 { v * k } else { v + (1.0 - v) * (k - 1.0) };
        o.clamp(0.0, 1.0)
    };
    let s = Color::srgb(f(c[0]), f(c[1]), f(c[2])).to_linear();
    [s.red, s.green, s.blue, 1.0]
}

#[derive(Clone, Copy)]
pub enum Paint {
    Solid(u32),
    /// Up-facing faces get `top`, the rest `side`; faces below `strata` get a darker side.
    TopSide { top: u32, side: u32, strata: f32 },
}

impl Paint {
    fn color(&self, n: Vec3, centroid: Vec3) -> u32 {
        match *self {
            Paint::Solid(c) => c,
            Paint::TopSide { top, side, strata } => {
                if n.y > 0.6 {
                    top
                } else if centroid.y < strata {
                    crate::art::shade(side, 0.8)
                } else {
                    side
                }
            }
        }
    }
}

pub struct Part {
    pub geo: Geo,
    pub paint: Paint,
    pub out: f32,
}

#[derive(Default)]
pub struct Model {
    pub parts: Vec<Part>,
}

impl Model {
    pub fn add(&mut self, geo: Geo, t: Transform, color: u32) -> &mut Self {
        self.add_paint(geo, t, Paint::Solid(color), OUT)
    }
    pub fn add_thin(&mut self, geo: Geo, t: Transform, color: u32, out: f32) -> &mut Self {
        self.add_paint(geo, t, Paint::Solid(color), out)
    }
    pub fn add_paint(&mut self, geo: Geo, t: Transform, paint: Paint, out: f32) -> &mut Self {
        self.parts.push(Part { geo: geo.transformed(t), paint, out });
        self
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for p in &self.parts {
            for v in &p.geo.pos {
                lo = lo.min(*v);
                hi = hi.max(*v);
            }
        }
        (lo, hi)
    }

    /// Flat-shaded mesh: one normal and one baked colour per face.
    pub fn body(&self) -> Mesh {
        let mut pos = vec![];
        let mut nor = vec![];
        let mut col = vec![];
        for part in &self.parts {
            let g = &part.geo;
            for tri in g.idx.chunks(3) {
                if tri.len() < 3 {
                    continue;
                }
                let (a, b, c) = (g.pos[tri[0] as usize], g.pos[tri[1] as usize], g.pos[tri[2] as usize]);
                let n = (b - a).cross(c - a);
                if n.length_squared() < 1e-12 {
                    continue;
                }
                let n = n.normalize();
                let color = lit(part.paint.color(n, (a + b + c) / 3.0), face_light(n));
                for p in [a, b, c] {
                    pos.push(p.to_array());
                    nor.push(n.to_array());
                    col.push(color);
                }
            }
        }
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        m
    }

    /// Inflated shell of every outlined part, to be drawn with front faces culled.
    pub fn outline(&self) -> Option<Mesh> {
        let mut pos: Vec<[f32; 3]> = vec![];
        let mut nor: Vec<[f32; 3]> = vec![];
        let mut idx: Vec<u32> = vec![];
        for part in &self.parts {
            if part.out <= 0.0 {
                continue;
            }
            let g = &part.geo;
            // weld vertices that share a position so the shell has no cracks
            let mut weld: HashMap<(i32, i32, i32), usize> = HashMap::new();
            let mut ids = vec![0usize; g.pos.len()];
            let mut uniq: Vec<Vec3> = vec![];
            for (i, p) in g.pos.iter().enumerate() {
                let k = ((p.x * 200.0).round() as i32, (p.y * 200.0).round() as i32, (p.z * 200.0).round() as i32);
                let id = *weld.entry(k).or_insert_with(|| {
                    uniq.push(*p);
                    uniq.len() - 1
                });
                ids[i] = id;
            }
            let mut acc = vec![Vec3::ZERO; uniq.len()];
            for tri in g.idx.chunks(3) {
                if tri.len() < 3 {
                    continue;
                }
                let (a, b, c) = (ids[tri[0] as usize], ids[tri[1] as usize], ids[tri[2] as usize]);
                let n = (uniq[b] - uniq[a]).cross(uniq[c] - uniq[a]);
                acc[a] += n;
                acc[b] += n;
                acc[c] += n;
            }
            let base = pos.len() as u32;
            for (i, p) in uniq.iter().enumerate() {
                let n = acc[i].normalize_or_zero();
                pos.push((*p + n * part.out).to_array());
                nor.push(n.to_array());
            }
            for tri in g.idx.chunks(3) {
                if tri.len() < 3 {
                    continue;
                }
                idx.extend([base + ids[tri[0] as usize] as u32, base + ids[tri[1] as usize] as u32, base + ids[tri[2] as usize] as u32]);
            }
        }
        if idx.is_empty() {
            return None;
        }
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        m.insert_indices(Indices::U32(idx));
        Some(m)
    }

    /// Wavefront OBJ with per-vertex colours (the flat body plus the outline shell).
    pub fn to_obj(&self, name: &str) -> String {
        let mut s = format!("# Bloom Isles low-poly model: {name}\n# vertex colours are sRGB after 'v x y z'\no {name}\n");
        let mut n = 0u32;
        for part in &self.parts {
            let g = &part.geo;
            for tri in g.idx.chunks(3) {
                if tri.len() < 3 {
                    continue;
                }
                let (a, b, c) = (g.pos[tri[0] as usize], g.pos[tri[1] as usize], g.pos[tri[2] as usize]);
                let nn = (b - a).cross(c - a);
                if nn.length_squared() < 1e-12 {
                    continue;
                }
                let k = face_light(nn.normalize());
                let rgb = srgb(crate::art::shade(part.paint.color(nn.normalize(), (a + b + c) / 3.0), k));
                for p in [a, b, c] {
                    s += &format!("v {:.3} {:.3} {:.3} {:.3} {:.3} {:.3}\n", p.x, p.y, p.z, rgb[0], rgb[1], rgb[2]);
                }
                s += &format!("f {} {} {}\n", n + 1, n + 2, n + 3);
                n += 3;
            }
        }
        s
    }
}

/* ---------- the cast ---------- */

/// Wooden figurine base under every animal.
fn base(m: &mut Model, r: f32) {
    m.add_paint(cyl(r, 3.0, 12), at(0.0, 1.5, 0.0), Paint::TopSide { top: 0xc79b62, side: 0x8a6440, strata: -99.0 }, OUT);
}

fn rot_z(t: Transform, a: f32) -> Transform {
    t.with_rotation(Quat::from_rotation_z(a))
}

pub fn species(sp: Sp, v: u8) -> Model {
    let mut m = Model::default();
    let green = 0x4f9a3a;
    match sp {
        Sp::Flower => {
            m.add_thin(cyl(0.7, 15.0, 5), at(0.0, 7.5, 0.0), 0x3f7a2e, 0.5);
            m.add_thin(cuboid(6.0, 0.8, 2.6), rot_z(at(-2.6, 6.0, 0.0), 0.5), green, 0.6);
            m.add_thin(cuboid(5.0, 0.8, 2.4), rot_z(at(2.4, 9.0, 0.0), -0.5), green, 0.6);
            let pc = [0xd9434f, 0xf2b632, 0x8e5bd1, 0xefe9dc][(v % 4) as usize];
            for i in 0..5 {
                let a = i as f32 / 5.0 * TAU;
                m.add_thin(ico(2.6, 0), at(a.cos() * 3.0, 16.5, a.sin() * 3.0).with_scale(Vec3::new(1.2, 0.55, 1.2)), pc, 0.7);
            }
            m.add_thin(ico(1.7, 0), at(0.0, 17.4, 0.0), 0x6b4a1f, 0.5);
        }
        Sp::Bush => {
            let g = if v % 2 == 1 { 0x4a8a3a } else { 0x58963f };
            m.add(ico(8.5, 1), at(0.0, 8.5, 0.0), g);
            m.add(ico(6.5, 0), at(-7.0, 5.5, 2.5), crate::art::shade(g, 0.95));
            m.add(ico(6.5, 0), at(7.0, 5.5, -1.5), crate::art::shade(g, 0.9));
            let b = if v % 2 == 1 { 0xc7343f } else { 0x4a4fb5 };
            for (x, y, z) in [(-4.0, 11.0, 6.0), (4.0, 13.0, 5.0), (8.0, 6.0, 4.5), (-8.0, 5.0, 7.0), (1.0, 8.0, 8.2)] {
                m.add_thin(ico(1.5, 0), at(x, y, z), b, 0.5);
            }
        }
        Sp::Tree => {
            if v == 2 {
                m.add(cyl(2.2, 9.0, 6), at(0.0, 4.5, 0.0), 0x7a5436);
                let g = 0x2f7040;
                m.add(cone(12.5, 18.0, 7), at(0.0, 15.0, 0.0), g);
                m.add(cone(10.0, 16.0, 7), at(0.0, 25.0, 0.0), crate::art::shade(g, 1.05));
                m.add(cone(7.0, 14.0, 7), at(0.0, 35.0, 0.0), crate::art::shade(g, 1.1));
            } else {
                m.add(frustum(2.0, 3.0, 20.0, 6), at(0.0, 10.0, 0.0), 0x7a5436);
                let g = if v % 2 == 1 { 0x3f8a36 } else { 0x4c9638 };
                m.add(ico(12.5, 1), at(0.0, 31.0, 0.0), g);
                m.add(ico(8.0, 0), at(-8.0, 24.0, 3.0), crate::art::shade(g, 0.92));
                m.add(ico(8.0, 0), at(8.0, 25.0, -2.0), crate::art::shade(g, 0.88));
                if v == 3 {
                    for (x, y, z) in [(5.0, 30.0, 10.0), (-6.0, 34.0, 9.0), (1.0, 38.0, 9.5), (9.0, 27.0, 5.0)] {
                        m.add_thin(ico(1.8, 0), at(x, y, z), 0xd9483b, 0.5);
                    }
                }
            }
        }
        Sp::Palm => {
            let mut p = Vec3::ZERO;
            for i in 0..5 {
                let k = i as f32 / 5.0;
                let next = Vec3::new(5.0 * (k + 0.2).powi(2), 6.2 * (i + 1) as f32, 0.0);
                let mid = (p + next) / 2.0;
                let dir = (next - p).normalize();
                let rot = Quat::from_rotation_arc(Vec3::Y, dir);
                m.add(frustum(2.2 - k * 0.8, 2.6 - k * 0.8, (next - p).length() + 0.4, 6), Transform::from_translation(mid).with_rotation(rot), if i % 2 == 0 { 0xa47a4a } else { 0x8e6a40 });
                p = next;
            }
            let top = p + Vec3::Y * 1.0;
            for i in 0..6 {
                let a = i as f32 / 6.0 * TAU + 0.3;
                let dir = Vec3::new(a.cos(), -0.35, a.sin()).normalize();
                let rot = Quat::from_rotation_arc(Vec3::X, dir);
                let g = if i % 2 == 0 { 0x4a9a3a } else { 0x3f8a33 };
                m.add(cuboid(17.0, 0.9, 5.0), Transform::from_translation(top + dir * 8.0).with_rotation(rot), g);
            }
            m.add_thin(ico(2.2, 0), at(top.x + 1.5, top.y - 2.5, 2.0), 0x5b3d22, 0.6);
            m.add_thin(ico(2.2, 0), at(top.x - 1.0, top.y - 2.5, -1.5), 0x4a3019, 0.6);
        }
        Sp::Lily => {
            m.add(cyl(10.0, 1.2, 9), at(0.0, 0.6, 0.0), 0x3f8f4a);
            if v % 2 == 0 {
                for i in 0..5 {
                    let a = i as f32 / 5.0 * TAU;
                    m.add_thin(ico(1.8, 0), at(2.0 + a.cos() * 2.2, 2.8, a.sin() * 2.2).with_scale(Vec3::new(1.0, 0.7, 1.0)), 0xe98aa6, 0.5);
                }
                m.add_thin(ico(1.1, 0), at(2.0, 3.6, 0.0), 0xf2c14e, 0.4);
            }
        }
        Sp::Mushroom => {
            let capc = if v % 2 == 1 { 0xc8413b } else { 0x8a4fa8 };
            let shroom = |m: &mut Model, x: f32, z: f32, s: f32, capc: u32| {
                m.add(frustum(2.2 * s, 2.8 * s, 11.0 * s, 6), at(x, 5.5 * s, z), 0xe9dcc2);
                m.add(ico(7.5 * s, 1), at(x, 12.5 * s, z).with_scale(Vec3::new(1.0, 0.55, 1.0)), capc);
                for (dx, dy, dz) in [(-3.0, 15.0, 3.2), (3.0, 14.8, 3.6), (0.0, 16.4, -1.0)] {
                    m.add_thin(ico(1.2 * s, 0), at(x + dx * s, dy * s, z + dz * s), 0xf1e9d8, 0.3);
                }
            };
            shroom(&mut m, 0.0, 0.0, 1.0, capc);
            if v == 2 {
                shroom(&mut m, 10.0, 5.0, 0.6, 0xc8413b);
            }
        }
        Sp::Bee => {
            m.add_paint(cyl(5.0, 2.0, 10), at(0.0, 1.0, 0.0), Paint::TopSide { top: 0xc79b62, side: 0x8a6440, strata: -99.0 }, OUT);
            m.add_thin(cyl(0.45, 18.0, 5), at(0.0, 11.0, 0.0), 0xd7e6ea, 0.0);
            m.add(ico(5.0, 1), at(0.0, 23.0, 0.0).with_scale(Vec3::new(1.35, 1.0, 1.0)), 0xe8b52e);
            for x in [-2.0, 2.2] {
                m.add_thin(cyl(4.3, 1.4, 10), rot_z(at(x, 23.0, 0.0), FRAC_PI_2).with_scale(Vec3::new(1.0, 1.0, 1.08)), 0x2b2620, 0.0);
            }
            m.add(ico(3.0, 0), at(7.5, 23.5, 0.0), 0x2b2620);
            for z in [-3.2, 3.2] {
                m.add_thin(ico(3.4, 0), at(-1.0, 28.5, z).with_scale(Vec3::new(1.2, 0.3, 0.7)), 0xeef6fa, 0.7);
            }
        }
        Sp::Rabbit => {
            base(&mut m, 9.0);
            let fur = if v == 3 { 0xb08a64 } else { 0xe4dfd4 };
            m.add(ico(6.5, 1), at(-1.0, 9.5, 0.0).with_scale(Vec3::new(1.25, 0.95, 0.95)), fur);
            m.add(ico(4.4, 1), at(5.5, 15.0, 0.0), fur);
            m.add(cuboid(1.8, 9.0, 2.6), rot_z(at(3.8, 22.0, -1.6), 0.18), fur);
            m.add(cuboid(1.8, 9.0, 2.6), rot_z(at(5.6, 21.5, 1.6), -0.12), fur);
            m.add_thin(ico(2.2, 0), at(-8.5, 10.0, 0.0), 0xffffff, 0.7);
            for z in [-2.6, 2.6] {
                m.add_thin(ico(0.8, 0), at(8.6, 16.0, z), 0x1c1a18, 0.0);
            }
        }
        Sp::Fox => {
            base(&mut m, 9.0);
            let o = 0xd66a28;
            m.add(ico(6.2, 1), at(-0.5, 10.0, 0.0).with_scale(Vec3::new(1.2, 1.0, 0.9)), o);
            m.add(ico(3.2, 0), at(4.8, 11.5, 0.0), 0xefe6d8);
            m.add(ico(4.8, 1), at(4.0, 18.5, 0.0), o);
            m.add(cone(2.2, 6.0, 5), rot_z(at(9.5, 17.5, 0.0), -FRAC_PI_2), 0xefe6d8);
            m.add_thin(ico(0.9, 0), at(12.5, 17.6, 0.0), 0x1c1a18, 0.3);
            for z in [-2.4, 2.4] {
                m.add(cone(1.9, 5.0, 4), at(2.5, 24.5, z), o);
                m.add_thin(ico(0.7, 0), at(6.8, 19.5, z * 1.3), 0x1c1a18, 0.0);
                m.add_thin(cyl(0.9, 4.0, 5), at(3.0, 5.0, z * 0.8), 0x2a1e18, 0.5);
            }
            m.add(cone(3.2, 13.0, 6), rot_z(at(-9.0, 10.5, 0.0), 1.2), o);
            m.add_thin(ico(2.3, 0), at(-14.5, 13.0, 0.0), 0xefe6d8, 0.7);
        }
        Sp::Bird => {
            base(&mut m, 8.0);
            let (body, dark) = if v == 2 { (0xc94a3a, 0x8f2f24) } else { (0x3f7cc4, 0x28558f) };
            for z in [-1.6, 1.6] {
                m.add_thin(cyl(0.5, 4.0, 4), at(0.0, 5.0, z), 0x3a3a3a, 0.3);
            }
            m.add(ico(6.2, 1), at(0.0, 11.0, 0.0).with_scale(Vec3::new(1.2, 1.0, 1.0)), body);
            m.add(ico(3.6, 0), at(3.0, 9.5, 0.0), 0xe9dcc2);
            m.add(ico(4.0, 1), at(5.0, 15.5, 0.0), body);
            m.add(cone(1.4, 4.5, 4), rot_z(at(9.8, 15.2, 0.0), -FRAC_PI_2), 0xe7a634);
            for z in [-5.2, 5.2] {
                m.add(ico(3.8, 0), at(-1.0, 12.0, z).with_scale(Vec3::new(1.4, 0.55, 0.35)), dark);
                m.add_thin(ico(0.7, 0), at(8.0, 16.5, z * 0.4), 0x1c1a18, 0.0);
            }
            m.add(cuboid(7.0, 1.2, 4.5), rot_z(at(-8.0, 12.5, 0.0), 0.35), dark);
        }
        Sp::Frog => {
            base(&mut m, 9.0);
            let body = if v % 2 == 1 { 0x4e9e45 } else { 0x62a84a };
            m.add(ico(7.0, 1), at(0.0, 8.0, 0.0).with_scale(Vec3::new(1.15, 0.72, 1.05)), body);
            for z in [-3.3, 3.3] {
                m.add(ico(2.6, 0), at(2.6, 12.0, z), crate::art::shade(body, 1.05));
                m.add_thin(ico(1.0, 0), at(4.6, 12.6, z * 1.1), 0x1c1a18, 0.0);
                m.add(ico(2.6, 0), at(-3.5, 5.0, z * 1.7).with_scale(Vec3::new(1.4, 0.6, 1.0)), crate::art::shade(body, 0.9));
            }
        }
        Sp::Crab => {
            base(&mut m, 9.5);
            let r = 0xd2482e;
            for z in [-1.0f32, 1.0] {
                for i in 0..3 {
                    let x = -3.0 + i as f32 * 3.0;
                    let t = Transform::from_xyz(x, 5.0, z * 7.5).with_rotation(Quat::from_rotation_x(z * 0.9));
                    m.add_thin(cuboid(1.0, 1.0, 6.0), t, 0x8a2a18, 0.5);
                }
                m.add(ico(3.0, 0), at(6.5, 8.5, z * 8.0), r);
                m.add_thin(cyl(0.5, 4.0, 4), at(3.5, 11.5, z * 2.0), 0x8a2a18, 0.3);
                m.add_thin(ico(1.1, 0), at(3.5, 13.8, z * 2.0), 0x1c1a18, 0.3);
            }
            m.add(ico(7.0, 1), at(0.0, 7.5, 0.0).with_scale(Vec3::new(1.05, 0.55, 1.25)), r);
        }
    }
    m
}

pub fn rock(big: bool) -> Model {
    let mut m = Model::default();
    m.add(ico(10.0, 0), at(0.0, 5.0, 0.0).with_scale(Vec3::new(1.4, 0.8, 1.1)), 0x9aa0a3);
    if big {
        m.add(ico(6.0, 0), at(11.0, 3.0, 5.0).with_scale(Vec3::new(1.2, 0.8, 1.0)), 0x878d90);
    }
    m
}

pub fn cloud() -> Model {
    let mut m = Model::default();
    let g = 0x6d7280;
    m.add(ico(12.0, 1), at(-14.0, 0.0, 0.0), crate::art::shade(g, 0.95));
    m.add(ico(16.0, 1), at(0.0, 5.0, 0.0), g);
    m.add(ico(12.0, 1), at(14.0, 1.0, 2.0), crate::art::shade(g, 0.9));
    m
}

/// A zigzag bolt one unit tall (scaled to reach the ground), hanging down from y = 0.
pub fn bolt() -> Model {
    let mut m = Model::default();
    let pts = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(-4.0, -0.35, 0.0), Vec3::new(3.0, -0.6, 0.0), Vec3::new(-1.0, -1.0, 0.0)];
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mid = (a + b) / 2.0;
        m.add_thin(cuboid(2.2, 1.0, 2.2), Transform::from_translation(mid).with_scale(Vec3::new(1.0, (b - a).length(), 1.0)), 0xf5c542, 0.0);
    }
    m
}

pub fn beetle() -> Model {
    let mut m = Model::default();
    for z in [-1.0f32, 1.0] {
        for i in 0..3 {
            let t = Transform::from_xyz(-3.0 + i as f32 * 3.0, 2.0, z * 5.0).with_rotation(Quat::from_rotation_x(z * 0.8));
            m.add_thin(cuboid(0.8, 0.8, 5.0), t, 0x1e1a24, 0.3);
        }
    }
    m.add(ico(5.5, 1), at(0.0, 4.5, 0.0).with_scale(Vec3::new(1.3, 0.7, 1.0)), 0x3b2f4a);
    m.add(ico(2.8, 0), at(7.5, 4.0, 0.0), 0x2a2233);
    m
}

pub fn whale() -> Model {
    let mut m = Model::default();
    let g = 0xe0ad3a;
    m.add(ico(13.0, 1), at(0.0, 0.0, 0.0).with_scale(Vec3::new(2.0, 1.0, 1.1)), g);
    m.add(ico(9.0, 1), at(4.0, -5.0, 0.0).with_scale(Vec3::new(2.0, 0.7, 1.0)), 0xf3dca0);
    m.add(cone(7.0, 16.0, 6), Transform::from_xyz(-28.0, 2.0, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2 + 0.25)), crate::art::shade(g, 0.95));
    m.add(cuboid(7.0, 2.0, 22.0), Transform::from_xyz(-36.0, 5.5, 0.0).with_rotation(Quat::from_rotation_z(0.45)), crate::art::shade(g, 0.88));
    for z in [-9.0, 9.0] {
        m.add_thin(ico(1.4, 0), at(18.0, 3.0, z * 0.9), 0x1c1a18, 0.0);
    }
    m
}

/// Board-game pawn in a team colour. mood 0: slumped and dim, 2: with a gold star.
pub fn pawn(color: u32, mood: u8) -> Model {
    let c = if mood == 0 { crate::art::shade(color, 0.7) } else { color };
    let mut m = Model::default();
    m.add(cyl(9.0, 3.0, 14), at(0.0, 1.5, 0.0), crate::art::shade(c, 0.75));
    m.add(frustum(5.0, 8.0, 9.0, 12), at(0.0, 7.5, 0.0), c);
    m.add(cyl(6.2, 2.0, 12), at(0.0, 12.5, 0.0), crate::art::shade(c, 0.9));
    m.add(frustum(3.4, 4.6, 5.0, 10), at(0.0, 16.0, 0.0), c);
    m.add(ico(6.0, 1), at(0.0, 23.0, 0.0), c);
    if mood == 2 {
        m.add(ico(2.6, 0), at(0.0, 32.0, 0.0).with_scale(Vec3::new(1.0, 1.3, 1.0)), crate::art::pal::GOLD);
    }
    m
}

/* ---------- island ---------- */

pub fn terrain_h(t: Terr) -> f32 {
    match t {
        Terr::Sand => 0.0,
        Terr::Grass | Terr::Rock => 2.5,
        Terr::Mead => 3.2,
        Terr::Pond => -2.0,
    }
}

pub const DEPTH: f32 = 18.0;

fn lerp_col(a: u32, b: u32, k: f32) -> u32 {
    let ch = |s: u32| -> u32 {
        let (x, y) = (((a >> s) & 255) as f32, ((b >> s) & 255) as f32);
        ((x + (y - x) * k).clamp(0.0, 255.0) as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

pub fn terrain_col(t: Terr, lush: f32, shade_k: f32) -> u32 {
    let base = match t {
        Terr::Grass | Terr::Rock => lerp_col(0xa6b04f, 0x57a33a, lush),
        Terr::Mead => lerp_col(0xc2b75a, 0x80b640, lush),
        Terr::Sand => 0xe8c784,
        Terr::Pond => 0x3fa3c8,
    };
    crate::art::shade(base, 1.0 + (shade_k - 0.5) * 0.08)
}

/// A hexagonal board tile: top at `h`, sides down to -DEPTH with a darker lower band. Centre (x, z).
pub fn hex_prism(x: f32, z: f32, h: f32, r: f32) -> Geo {
    let mut g = Geo::default();
    let corner = |i: usize, y: f32| {
        let a = (60.0 * i as f32 - 30.0).to_radians();
        Vec3::new(x + a.cos() * r, y, z + a.sin() * r)
    };
    let mid = STRATA;
    g.pos.push(Vec3::new(x, h, z));
    for y in [h, mid, -DEPTH] {
        for i in 0..6 {
            g.pos.push(corner(i, y));
        }
    }
    for i in 0..6u32 {
        let j = (i + 1) % 6;
        g.idx.extend([0, 1 + j, 1 + i]);
        for band in 0..2u32 {
            let (u, d) = (1 + band * 6, 7 + band * 6);
            g.idx.extend([u + i, u + j, d + j]);
            g.idx.extend([u + i, d + j, d + i]);
        }
    }
    fix_winding(&mut g, Vec3::new(x, (h - DEPTH) / 2.0, z));
    g
}

pub const STRATA: f32 = -7.0;

/// Make every triangle face away from `centre` (for convex solids).
fn fix_winding(g: &mut Geo, centre: Vec3) {
    for tri in g.idx.chunks_mut(3) {
        let (a, b, c) = (g.pos[tri[0] as usize], g.pos[tri[1] as usize], g.pos[tri[2] as usize]);
        let n = (b - a).cross(c - a);
        if n.dot((a + b + c) / 3.0 - centre) < 0.0 {
            tri.swap(1, 2);
        }
    }
}

/// The whole island: bevel-less board tiles with bold seams.
pub fn island(map: &IslandMap, lush: f32) -> Model {
    let mut m = Model::default();
    for t in &map.tiles {
        let top = terrain_col(t.t, lush, t.shade);
        let side = if t.t == Terr::Pond { 0x2f7fa0 } else { 0x9a6843 };
        m.parts.push(Part { geo: hex_prism(t.x, t.y / SQ, terrain_h(t.t), HS * 0.985), paint: Paint::TopSide { top, side, strata: STRATA }, out: 1.5 });
    }
    m
}

/// Shallow water and surf around the coast (flat, no outlines).
pub fn shallows(map: &IslandMap) -> Model {
    let mut m = Model::default();
    let disc = |r: f32| cyl(r, 0.5, 6);
    for &i in &map.edge {
        let t = &map.tiles[i];
        m.parts.push(Part { geo: disc(HS * 1.95).transformed(at(t.x, -12.0, t.y / SQ).with_rotation(Quat::from_rotation_y(PI / 6.0))), paint: Paint::Solid(0x3fb0bf), out: 0.0 });
    }
    for &i in &map.edge {
        let t = &map.tiles[i];
        m.parts.push(Part { geo: disc(HS * 1.3).transformed(at(t.x, -11.0, t.y / SQ).with_rotation(Quat::from_rotation_y(PI / 6.0))), paint: Paint::Solid(0xdff3ef), out: 0.0 });
    }
    m
}

pub fn species_name(sp: Sp) -> &'static str {
    match sp {
        Sp::Flower => "flower",
        Sp::Bush => "bush",
        Sp::Tree => "tree",
        Sp::Palm => "palm",
        Sp::Lily => "lily",
        Sp::Mushroom => "mushroom",
        Sp::Bee => "bee",
        Sp::Rabbit => "rabbit",
        Sp::Fox => "fox",
        Sp::Bird => "bird",
        Sp::Frog => "frog",
        Sp::Crab => "crab",
    }
}
