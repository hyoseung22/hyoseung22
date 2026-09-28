//! Every picture in the game is drawn here with vector paths (tiny-skia)
//! and uploaded as a texture at startup, so the game ships as one binary.
//!
//! Style: minimalist low-poly. Shapes are flat-shaded facets lit from the
//! upper left, with no ink outlines. Creatures are board-game figurines on
//! wooden bases; the island is made of bevelled hex tiles.

use crate::eco::Sp;
use crate::island::{hex_vertex, DecoKind, IslandMap, Terr, EDGE_DIR, HS, SQ};
use crate::rng::Rng;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::f32::consts::{PI, TAU};
use tiny_skia::{Color as SkColor, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect as SkRect, Stroke, StrokeDash, Transform as Tf};

/// Interface palette.
pub mod pal {
    pub const SLATE: u32 = 0x1f2b33;
    pub const SLATE2: u32 = 0x2e3e48;
    pub const CREAM: u32 = 0xf1e9d8;
    pub const GOLD: u32 = 0xe8b23a;
    pub const RED: u32 = 0xe04848;
    pub const GREEN: u32 = 0x5fb04a;
    pub const AMBER: u32 = 0xe8a21b;
    pub const SEA: u32 = 0x2a88a4;
    pub const WAVE: u32 = 0x7cc6d6;
    pub const PORTRAIT: u32 = 0x2f8ba6;
    pub const PORTRAIT_VIEW: u32 = 0x3fa3bd;
}


/// Team colours (main, dark).
pub const TEAM: [(u32, u32); 4] = [(0xd9483b, 0x9e2f25), (0x3a78d6, 0x24549e), (0xe0a21b, 0xa8760d), (0x8a55d0, 0x5f3597)];

pub fn col(hex: u32) -> SkColor {
    SkColor::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}
pub fn cola(hex: u32, a: f32) -> SkColor {
    SkColor::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, (a.clamp(0.0, 1.0) * 255.0) as u8)
}
pub fn bc(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}
pub fn bca(hex: u32, a: f32) -> Color {
    bc(hex).with_alpha(a)
}

/// Darken (k < 1) or lighten (k > 1) a colour.
pub fn shade(hex: u32, k: f32) -> u32 {
    let ch = |v: u32| -> u32 {
        let v = v as f32;
        let o = if k <= 1.0 { v * k } else { v + (255.0 - v) * (k - 1.0) };
        o.clamp(0.0, 255.0) as u32
    };
    (ch((hex >> 16) & 255) << 16) | (ch((hex >> 8) & 255) << 8) | ch(hex & 255)
}

/* ---------------- a tiny canvas-like wrapper ---------------- */

pub struct Cv {
    pub pm: Pixmap,
    tf: Tf,
    stack: Vec<Tf>,
}

impl Cv {
    pub fn new(w: u32, h: u32) -> Self {
        Cv { pm: Pixmap::new(w.max(1), h.max(1)).expect("pixmap"), tf: Tf::identity(), stack: vec![] }
    }
    pub fn save(&mut self) {
        self.stack.push(self.tf);
    }
    pub fn restore(&mut self) {
        if let Some(t) = self.stack.pop() {
            self.tf = t;
        }
    }
    pub fn translate(&mut self, x: f32, y: f32) {
        self.tf = self.tf.pre_translate(x, y);
    }
    pub fn rotate(&mut self, rad: f32) {
        self.tf = self.tf.pre_rotate(rad.to_degrees());
    }
    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.tf = self.tf.pre_scale(sx, sy);
    }
    pub fn fill(&mut self, p: &Option<Path>, c: SkColor) {
        if let Some(p) = p {
            let mut paint = Paint::default();
            paint.set_color(c);
            paint.anti_alias = true;
            self.pm.fill_path(p, &paint, FillRule::Winding, self.tf, None);
        }
    }
    pub fn stroke(&mut self, p: &Option<Path>, c: SkColor, w: f32) {
        self.stroke_dash(p, c, w, None);
    }
    pub fn stroke_dash(&mut self, p: &Option<Path>, c: SkColor, w: f32, dash: Option<(f32, f32)>) {
        if let Some(p) = p {
            let mut paint = Paint::default();
            paint.set_color(c);
            paint.anti_alias = true;
            let mut st = Stroke { width: w, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
            if let Some((a, b)) = dash {
                st.dash = StrokeDash::new(vec![a, b], 0.0);
            }
            self.pm.stroke_path(p, &paint, &st, self.tf, None);
        }
    }
    pub fn dot(&mut self, x: f32, y: f32, r: f32, fill: SkColor) {
        self.fill(&circle(x, y, r), fill);
    }
    pub fn ell(&mut self, x: f32, y: f32, rx: f32, ry: f32, rot: f32, fill: SkColor) {
        self.fill(&ellipse(x, y, rx, ry, rot), fill);
    }
    pub fn line(&mut self, pts: &[(f32, f32)], c: SkColor, w: f32) {
        let mut b = Pb::new();
        for &(x, y) in pts {
            b.l(x, y);
        }
        self.stroke(&b.done(), c, w);
    }
    /// Solid polygon; a hairline of the same colour hides seams between facets.
    pub fn poly(&mut self, pts: &[(f32, f32)], hex: u32) {
        if pts.len() < 3 {
            return;
        }
        let mut b = Pb::new();
        for &(x, y) in pts {
            b.l(x, y);
        }
        b.close();
        let p = b.done();
        self.fill(&p, col(hex));
        self.stroke(&p, col(hex), 0.45);
    }
    /// Polygon in two tones: lit left part, shaded right part, split at x = `cut`.
    pub fn two_tone(&mut self, pts: &[(f32, f32)], hex: u32, cut: f32) {
        self.poly(pts, shade(hex, 0.8));
        let left = clip_left(pts, cut);
        self.poly(&left, shade(hex, 1.06));
    }
    /// Low-poly blob: a fan of triangles around an off-centre apex, each shaded by its facing.
    #[allow(clippy::too_many_arguments)]
    pub fn facets(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, n: usize, hex: u32, seed: u32, rot: f32) {
        let mut rng = Rng::new(seed.wrapping_mul(2_654_435_761).wrapping_add(17));
        let pts: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let a = rot + i as f32 / n as f32 * TAU;
                let r = 1.0 + (rng.f() - 0.5) * 0.16;
                (cx + a.cos() * rx * r, cy + a.sin() * ry * r)
            })
            .collect();
        let apex = (cx - rx * 0.2, cy - ry * 0.25);
        for i in 0..n {
            let (p, q) = (pts[i], pts[(i + 1) % n]);
            let (mx, my) = ((p.0 + q.0) / 2.0 - cx, (p.1 + q.1) / 2.0 - cy);
            self.poly(&[apex, p, q], shade(hex, lum(mx, my)));
        }
    }
}

const LIGHT: (f32, f32) = (-0.55, -0.83);

/// Brightness of a facet facing direction (dx, dy).
fn lum(dx: f32, dy: f32) -> f32 {
    let l = dx.hypot(dy).max(1e-3);
    let d = (dx * LIGHT.0 + dy * LIGHT.1) / l;
    0.9 + 0.2 * d
}

/// Part of a polygon with x <= cut (Sutherland-Hodgman against one edge).
fn clip_left(pts: &[(f32, f32)], cut: f32) -> Vec<(f32, f32)> {
    let mut out = vec![];
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let (ain, bin) = (a.0 <= cut, b.0 <= cut);
        if ain {
            out.push(a);
        }
        if ain != bin {
            let t = (cut - a.0) / (b.0 - a.0);
            out.push((cut, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

pub fn circle(x: f32, y: f32, r: f32) -> Option<Path> {
    PathBuilder::from_circle(x, y, r.max(0.01))
}

pub fn ellipse(x: f32, y: f32, rx: f32, ry: f32, rot: f32) -> Option<Path> {
    let r = SkRect::from_xywh(-rx, -ry, rx * 2.0, ry * 2.0)?;
    PathBuilder::from_oval(r)?.transform(Tf::from_rotate(rot.to_degrees()).post_translate(x, y))
}

fn ell_pts(cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32, n: usize) -> Vec<(f32, f32)> {
    (0..=n).map(|i| {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        (cx + a.cos() * rx, cy + a.sin() * ry)
    }).collect()
}

/// Path builder with canvas-style arcs.
pub struct Pb {
    b: PathBuilder,
    started: bool,
}

impl Pb {
    pub fn new() -> Self {
        Pb { b: PathBuilder::new(), started: false }
    }
    pub fn m(&mut self, x: f32, y: f32) {
        self.b.move_to(x, y);
        self.started = true;
    }
    pub fn l(&mut self, x: f32, y: f32) {
        if self.started {
            self.b.line_to(x, y)
        } else {
            self.m(x, y)
        }
    }
    pub fn q(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.b.quad_to(cx, cy, x, y);
    }
    pub fn c(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.b.cubic_to(a, b, c, d, x, y);
    }
    pub fn close(&mut self) {
        self.b.close();
        self.started = false;
    }
    pub fn arc(&mut self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let mut a1 = a1;
        while a1 < a0 {
            a1 += TAU;
        }
        let n = (((a1 - a0) / TAU) * 48.0).ceil().max(2.0) as usize;
        for i in 0..=n {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            self.l(cx + a.cos() * r, cy + a.sin() * r);
        }
    }
    pub fn done(self) -> Option<Path> {
        self.b.finish()
    }
}

/* ---------------- textures ---------------- */

#[derive(Clone)]
pub struct Tex {
    pub h: Handle<Image>,
    /// Size in drawing units.
    pub size: Vec2,
    /// Sprite anchor (Bevy convention: -0.5..0.5, y up).
    pub anchor: Vec2,
}

pub fn to_image(pm: &Pixmap) -> Image {
    let mut data = Vec::with_capacity((pm.width() * pm.height() * 4) as usize);
    for p in pm.pixels() {
        let c = p.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Image::new(
        Extent3d { width: pm.width(), height: pm.height(), depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Draw into a texture `w`×`h` units big whose anchor sits at (`ox`,`oy`) units from the top-left.
pub fn make(images: &mut Assets<Image>, w: f32, h: f32, ox: f32, oy: f32, res: f32, f: impl FnOnce(&mut Cv)) -> Tex {
    let mut cv = Cv::new((w * res).ceil() as u32, (h * res).ceil() as u32);
    cv.scale(res, res);
    cv.translate(ox, oy);
    f(&mut cv);
    Tex { h: images.add(to_image(&cv.pm)), size: Vec2::new(w, h), anchor: Vec2::new(ox / w - 0.5, 0.5 - oy / h) }
}

pub const SP_RES: f32 = 3.0;

#[derive(Resource, Clone)]
pub struct Art {
    pub sp: Vec<Vec<Tex>>,
    pub rock: [Tex; 2],
    pub cloud: Tex,
    pub bolt: Tex,
    pub beetle: Tex,
    pub shield: Tex,
    pub whale: Tex,
    pub heart: Tex,
    pub heart_broken: Tex,
    pub spark: Tex,
    pub leaf: Tex,
    pub circle: Tex,
    pub ring: Tex,
    pub drop: Tex,
    pub shadow: Tex,
    pub hex: Tex,
    pub hex_line: Tex,
    pub pixel: Tex,
    /// Mood badges: sad, fine, thriving.
    pub faces: [Tex; 3],
    /// Team pawns: [team][mood].
    pub avatars: Vec<[Tex; 3]>,
    /// A pale pawn for tinting (the crowd).
    pub pawn: Tex,
    pub hand: Tex,
    pub trophy: Tex,
    pub crown: Tex,
    pub dome: Tex,
    pub rrect_fill: Tex,
    pub rrect_line: Tex,
    pub rrect_mask: Tex,
    pub wave: Tex,
    pub glyph_leaf: Tex,
    pub glyph_paw: Tex,
    pub glyph_target: Tex,
    pub crosshair: Tex,
    pub sprout_on: Tex,
    pub sprout_off: Tex,
    pub dice: Tex,
    pub icon_pause: Tex,
    pub icon_play: Tex,
    pub icon_sound_on: Tex,
    pub icon_sound_off: Tex,
    pub icon_reroll: Tex,
    pub icon_shovel: Tex,
    pub icon_home: Tex,
    pub icon_quit: Tex,
}

impl Art {
    pub fn sp(&self, sp: Sp, v: u8) -> &Tex {
        &self.sp[sp.idx()][(v % 4) as usize]
    }
}

const ICON: u32 = pal::CREAM;

pub fn build_art(images: &mut Assets<Image>) -> Art {
    let mut sp = vec![];
    for s in crate::eco::ALL {
        sp.push((0..4u8).map(|v| sp_tex(images, s, v)).collect());
    }
    let r = SP_RES;
    let rect = |x: f32, y: f32, w: f32, h: f32| Some(PathBuilder::from_rect(SkRect::from_xywh(x, y, w, h).unwrap()));
    Art {
        sp,
        rock: [make(images, 44.0, 30.0, 18.0, 24.0, r, |c| draw_rock(c, 0.2)), make(images, 44.0, 30.0, 18.0, 24.0, r, |c| draw_rock(c, 0.8))],
        cloud: make(images, 70.0, 46.0, 35.0, 24.0, r, draw_cloud),
        bolt: make(images, 20.0, 32.0, 10.0, 16.0, r, |c| {
            let pts = [(2.0, -12.0), (-6.0, 2.0), (0.0, 2.0), (-3.0, 14.0), (7.0, -2.0), (1.0, -2.0), (5.0, -12.0)];
            c.poly(&pts, 0xf5c542);
            c.poly(&clip_left(&pts, 0.5), 0xfbe08a);
        }),
        beetle: make(images, 34.0, 22.0, 15.0, 13.0, r, draw_beetle),
        shield: make(images, 40.0, 36.0, 20.0, 22.0, r, draw_shield_icon),
        whale: make(images, 84.0, 44.0, 42.0, 28.0, r, draw_whale),
        heart: make(images, 16.0, 16.0, 8.0, 8.0, 4.0, |c| {
            c.fill(&heart_path(0.0, 0.0, 5.0), col(0xe0474c));
            c.fill(&heart_path(-1.2, -1.2, 2.0), cola(0xffffff, 0.35));
        }),
        heart_broken: make(images, 16.0, 16.0, 8.0, 8.0, 4.0, |c| {
            c.fill(&heart_path(0.0, 0.0, 5.0), col(0x7d858c));
            c.line(&[(-1.0, -3.0), (1.5, 0.0), (-1.0, 3.0)], col(pal::SLATE), 1.4);
        }),
        spark: make(images, 12.0, 12.0, 6.0, 6.0, 4.0, |c| {
            let mut b = Pb::new();
            for i in 0..8 {
                let rr = if i % 2 == 1 { 1.3 } else { 4.8 };
                let a = i as f32 / 8.0 * TAU;
                b.l(a.cos() * rr, a.sin() * rr);
            }
            b.close();
            c.fill(&b.done(), col(0xfff1b8));
        }),
        leaf: make(images, 10.0, 6.0, 5.0, 3.0, 4.0, |c| c.poly(&[(-4.0, 0.0), (0.0, -2.2), (4.0, 0.0), (0.0, 2.2)], 0xffffff)),
        circle: make(images, 32.0, 32.0, 16.0, 16.0, 4.0, |c| c.dot(0.0, 0.0, 15.5, col(0xffffff))),
        ring: make(images, 64.0, 64.0, 32.0, 32.0, 2.0, |c| c.stroke(&circle(0.0, 0.0, 30.0), col(0xffffff), 2.4)),
        drop: make(images, 6.0, 10.0, 3.0, 5.0, 4.0, |c| c.line(&[(1.0, -3.5), (-1.0, 3.5)], col(0xd5ecff), 1.6)),
        shadow: make(images, 32.0, 12.0, 16.0, 6.0, 3.0, |c| c.ell(0.0, 0.0, 15.0, 5.0, 0.0, col(0xffffff))),
        hex: make(images, 2.2 * HS, 2.2 * HS * SQ, 1.1 * HS, 1.1 * HS * SQ, 2.0, |c| c.fill(&hex_path(0.0, 0.0, HS), col(0xffffff))),
        hex_line: make(images, 2.2 * HS, 2.2 * HS * SQ, 1.1 * HS, 1.1 * HS * SQ, 2.0, |c| c.stroke(&hex_path(0.0, 0.0, HS * 0.9), col(0xffffff), 3.0)),
        pixel: make(images, 4.0, 4.0, 2.0, 2.0, 1.0, |c| c.fill(&rect(-2.0, -2.0, 4.0, 4.0), col(0xffffff))),
        faces: [0u8, 1, 2].map(|st| make(images, 24.0, 24.0, 12.0, 12.0, r, |c| draw_mood(c, st))),
        avatars: (0..4).map(|i| [0u8, 1, 2].map(|m| make(images, 48.0, 56.0, 24.0, 30.0, r, |c| draw_pawn(c, TEAM[i].0, m)))).collect(),
        pawn: make(images, 48.0, 56.0, 24.0, 30.0, 2.0, |c| draw_pawn(c, 0xe8e8e8, 1)),
        hand: make(images, 40.0, 50.0, 14.0, 4.0, r, draw_hand),
        trophy: make(images, 34.0, 34.0, 12.0, 20.0, r, draw_flag),
        crown: make(images, 42.0, 28.0, 21.0, 14.0, r, |c| {
            let pts = [(-16.0, 8.0), (-18.0, -8.0), (-8.0, 0.0), (0.0, -12.0), (8.0, 0.0), (18.0, -8.0), (16.0, 8.0)];
            c.two_tone(&pts, pal::GOLD, 0.0);
            c.dot(0.0, 2.0, 2.5, col(0xd9483b));
        }),
        dome: make(images, 200.0, 200.0, 100.0, 100.0, 2.0, |c| {
            c.fill(&circle(0.0, 0.0, 96.0), cola(0x9fe0ff, 0.12));
            c.stroke(&circle(0.0, 0.0, 96.0), cola(0xd8f4ff, 0.7), 2.5);
            let mut b = Pb::new();
            b.arc(0.0, 0.0, 84.0, PI * 1.1, PI * 1.35);
            c.stroke(&b.done(), cola(0xffffff, 0.45), 5.0);
        }),
        rrect_fill: make(images, 64.0, 64.0, 0.0, 0.0, 1.0, |c| c.fill(&rrect(1.0, 1.0, 62.0, 62.0, 14.0), col(0xffffff))),
        rrect_line: make(images, 64.0, 64.0, 0.0, 0.0, 1.0, |c| c.stroke(&rrect(2.0, 2.0, 60.0, 60.0, 13.0), col(0xffffff), 2.2)),
        rrect_mask: make(images, 64.0, 64.0, 0.0, 0.0, 1.0, |c| {
            c.fill(&rect(0.0, 0.0, 64.0, 64.0), col(0xffffff));
            if let Some(p) = rrect(1.5, 1.5, 61.0, 61.0, 14.0) {
                let mut paint = Paint::default();
                paint.set_color(SkColor::from_rgba8(0, 0, 0, 255));
                paint.anti_alias = true;
                paint.blend_mode = tiny_skia::BlendMode::DestinationOut;
                c.pm.fill_path(&p, &paint, FillRule::Winding, Tf::identity(), None);
            }
        }),
        wave: make(images, 32.0, 10.0, 16.0, 5.0, 2.0, |c| c.line(&[(-12.0, 3.0), (-6.0, -1.0), (0.0, 3.0), (6.0, -1.0), (12.0, 3.0)], col(0xffffff), 1.6)),
        glyph_leaf: make(images, 14.0, 10.0, 7.0, 5.0, r, |c| c.two_tone(&[(-5.5, 2.0), (0.0, -3.0), (5.5, -2.0), (0.0, 3.0)], 0x5a9a3c, 0.0)),
        glyph_paw: make(images, 14.0, 14.0, 7.0, 7.0, r, |c| {
            c.dot(0.0, 1.5, 3.2, col(0xc96f2e));
            for (a, b) in [(-3.8, -3.0), (0.0, -4.5), (3.8, -3.0)] {
                c.dot(a, b, 1.6, col(0xc96f2e));
            }
        }),
        glyph_target: make(images, 20.0, 20.0, 10.0, 10.0, r, |c| {
            c.stroke(&circle(0.0, 0.0, 5.0), col(0xff8a80), 1.8);
            c.line(&[(-8.0, 0.0), (8.0, 0.0)], col(0xff8a80), 1.8);
            c.line(&[(0.0, -8.0), (0.0, 8.0)], col(0xff8a80), 1.8);
        }),
        crosshair: make(images, 64.0, 64.0, 32.0, 32.0, 2.0, |c| {
            let red = col(pal::RED);
            c.stroke(&circle(0.0, 0.0, 16.0), red, 3.0);
            for (a, b, x, y) in [(-28.0, 0.0, -10.0, 0.0), (10.0, 0.0, 28.0, 0.0), (0.0, -28.0, 0.0, -10.0), (0.0, 10.0, 0.0, 28.0)] {
                c.line(&[(a, b), (x, y)], red, 3.0);
            }
        }),
        sprout_on: make(images, 16.0, 20.0, 8.0, 10.0, r, |c| draw_sprout(c, 0x6cbf4f)),
        sprout_off: make(images, 16.0, 20.0, 8.0, 10.0, r, |c| draw_sprout(c, 0x6b7a80)),
        dice: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            // isometric cube
            c.poly(&[(0.0, -12.0), (11.0, -6.0), (0.0, 0.0), (-11.0, -6.0)], 0xf6f1e6);
            c.poly(&[(-11.0, -6.0), (0.0, 0.0), (0.0, 12.0), (-11.0, 6.0)], 0xd9d0bf);
            c.poly(&[(0.0, 0.0), (11.0, -6.0), (11.0, 6.0), (0.0, 12.0)], 0xb9ae99);
            let d = col(pal::SLATE);
            c.dot(0.0, -6.0, 1.5, d);
            c.dot(-7.0, -1.5, 1.3, d);
            c.dot(-4.0, 5.0, 1.3, d);
            c.dot(4.0, 2.0, 1.3, d);
            c.dot(7.5, -1.0, 1.3, d);
            c.dot(5.5, 5.5, 1.3, d);
        }),
        icon_pause: make(images, 20.0, 20.0, 10.0, 10.0, r, |c| {
            c.fill(&rrect(-6.0, -7.0, 4.0, 14.0, 1.2), col(ICON));
            c.fill(&rrect(2.0, -7.0, 4.0, 14.0, 1.2), col(ICON));
        }),
        icon_play: make(images, 40.0, 40.0, 20.0, 20.0, r, |c| c.poly(&[(-6.0, -10.0), (11.0, 0.0), (-6.0, 10.0)], ICON)),
        icon_sound_on: make(images, 24.0, 20.0, 12.0, 10.0, r, |c| draw_sound(c, true)),
        icon_sound_off: make(images, 24.0, 20.0, 12.0, 10.0, r, |c| draw_sound(c, false)),
        icon_reroll: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            for s in [0.0, PI] {
                let mut b = Pb::new();
                b.arc(0.0, 0.0, 8.5, s + 0.3, s + 2.6);
                c.stroke(&b.done(), col(ICON), 2.4);
                let a = s + 2.6f32;
                let (px, py) = (a.cos() * 8.5, a.sin() * 8.5);
                c.line(&[(px + (a - 1.2).cos() * 5.0, py + (a - 1.2).sin() * 5.0), (px, py), (px + (a + 0.6).cos() * 4.4, py + (a + 0.6).sin() * 4.4)], col(ICON), 2.4);
            }
        }),
        icon_shovel: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            c.rotate(0.7);
            c.fill(&rrect(-1.5, -12.4, 3.0, 14.4, 1.2), col(ICON));
            c.poly(&[(-5.6, 1.6), (5.6, 1.6), (4.0, 10.0), (0.0, 13.2), (-4.0, 10.0)], ICON);
            c.fill(&rrect(-4.4, -14.4, 8.8, 2.6, 1.2), col(ICON));
        }),
        icon_home: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            c.poly(&[(-11.0, -0.5), (0.0, -11.0), (11.0, -0.5)], ICON);
            c.poly(&[(-7.5, -1.0), (7.5, -1.0), (7.5, 10.0), (2.0, 10.0), (2.0, 4.0), (-2.0, 4.0), (-2.0, 10.0), (-7.5, 10.0)], ICON);
        }),
        icon_quit: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            let mut b = Pb::new();
            b.arc(0.0, 1.0, 8.5, -PI / 2.0 + 0.6, PI * 1.5 - 0.6);
            c.stroke(&b.done(), col(ICON), 2.4);
            c.line(&[(0.0, -9.0), (0.0, 0.5)], col(ICON), 2.4);
        }),
    }
}

pub fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut b = Pb::new();
    b.m(x + r, y);
    b.l(x + w - r, y);
    b.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    b.l(x + w, y + h - r);
    b.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    b.l(x + r, y + h);
    b.arc(x + r, y + h - r, r, PI / 2.0, PI);
    b.l(x, y + r);
    b.arc(x + r, y + r, r, PI, PI * 1.5);
    b.close();
    b.done()
}

pub fn hex_path(x: f32, y: f32, r: f32) -> Option<Path> {
    let mut b = Pb::new();
    for i in 0..6 {
        let (vx, vy) = hex_vertex(i);
        b.l(x + vx * r, y + vy * r);
    }
    b.close();
    b.done()
}

fn hex_pts(x: f32, y: f32, r: f32) -> Vec<(f32, f32)> {
    (0..6).map(|i| {
        let (vx, vy) = hex_vertex(i);
        (x + vx * r, y + vy * r)
    }).collect()
}

pub fn heart_path(x: f32, y: f32, s: f32) -> Option<Path> {
    let mut b = Pb::new();
    b.m(x, y + s * 0.9);
    b.c(x - s * 1.4, y, x - s * 0.9, y - s * 1.1, x, y - s * 0.35);
    b.c(x + s * 0.9, y - s * 1.1, x + s * 1.4, y, x, y + s * 0.9);
    b.close();
    b.done()
}

/* ---------------- creatures ---------------- */

fn sp_tex(images: &mut Assets<Image>, sp: Sp, v: u8) -> Tex {
    let head = crate::eco::def(sp).head;
    let top = head + 18.0;
    make(images, 64.0, top + 10.0, 32.0, top, SP_RES, |c| draw_sp(c, sp, v))
}

fn ground_shadow(c: &mut Cv, rx: f32) {
    c.ell(0.0, 1.5, rx, rx * 0.36, 0.0, cola(0x10202a, 0.28));
}

/// Wooden figurine base shared by every animal piece.
fn token_base(c: &mut Cv, r: f32) {
    ground_shadow(c, r + 2.0);
    c.ell(0.0, 0.3, r, r * 0.38, 0.0, col(0x6e5132));
    c.fill(&Some(PathBuilder::from_rect(SkRect::from_xywh(-r, -2.0, r * 2.0, 2.3).unwrap())), col(0x6e5132));
    c.ell(0.0, -2.0, r, r * 0.38, 0.0, col(0xb4895a));
    c.ell(-r * 0.25, -2.4, r * 0.55, r * 0.17, 0.0, cola(0xffffff, 0.18));
}

fn eye(c: &mut Cv, x: f32, y: f32) {
    c.dot(x, y, 0.95, col(0x1c1a18));
}

pub fn draw_sp(c: &mut Cv, sp: Sp, v: u8) {
    match sp {
        Sp::Flower => {
            ground_shadow(c, 7.0);
            c.two_tone(&[(-7.0, 0.0), (-1.0, -3.0), (0.0, 0.0)], 0x4f8f3a, -3.0);
            c.two_tone(&[(7.0, 0.0), (1.0, -3.5), (0.0, 0.0)], 0x4f8f3a, 3.0);
            c.line(&[(0.0, 0.0), (0.5, -9.0), (0.0, -16.0)], col(0x3f7a2e), 1.6);
            c.two_tone(&[(0.3, -8.0), (-6.0, -12.0), (0.2, -10.5)], 0x5a9a3c, -2.0);
            let pc = [0xd9434f, 0xf2b632, 0x8e5bd1, 0xefe9dc][(v % 4) as usize];
            let (cx, cy) = (0.0, -19.0);
            for i in 0..5 {
                let a = i as f32 / 5.0 * TAU - PI / 2.0;
                let tip = (cx + a.cos() * 7.0, cy + a.sin() * 6.0);
                let l = (cx + (a - 0.5).cos() * 3.6, cy + (a - 0.5).sin() * 3.2);
                let r = (cx + (a + 0.5).cos() * 3.6, cy + (a + 0.5).sin() * 3.2);
                c.poly(&[(cx, cy), l, tip], shade(pc, lum((a - 0.25).cos(), (a - 0.25).sin()) + 0.04));
                c.poly(&[(cx, cy), tip, r], shade(pc, lum((a + 0.25).cos(), (a + 0.25).sin()) - 0.06));
            }
            c.dot(cx, cy, 1.9, col(0x6b4a1f));
            c.dot(cx - 0.6, cy - 0.6, 0.7, col(0xe9c46a));
        }
        Sp::Bush => {
            ground_shadow(c, 13.0);
            let g = if v % 2 == 1 { 0x4a8a3a } else { 0x55953e };
            c.facets(0.0, -13.0, 10.5, 8.5, 7, g, 11 + v as u32, 0.2);
            c.facets(-7.0, -7.0, 7.5, 6.0, 6, shade(g, 0.95), 21 + v as u32, 0.5);
            c.facets(7.5, -7.0, 7.5, 6.0, 6, shade(g, 0.92), 31 + v as u32, 0.1);
            let bcol = if v % 2 == 1 { 0xc7343f } else { 0x4a4fb5 };
            for (x, y) in [(-6.0, -9.0), (5.0, -12.0), (8.0, -6.0), (-1.0, -6.0), (2.0, -17.0)] {
                c.dot(x, y, 1.7, col(bcol));
                c.dot(x - 0.5, y - 0.5, 0.6, cola(0xffffff, 0.6));
            }
        }
        Sp::Tree => {
            ground_shadow(c, 15.0);
            if v == 2 {
                // conifer
                c.two_tone(&[(-2.5, 0.0), (-2.0, -10.0), (2.0, -10.0), (2.5, 0.0)], 0x7a5436, 0.0);
                let g = 0x2f6f3c;
                for (i, (w, y0, y1)) in [(13.0, -6.0, -26.0), (10.5, -17.0, -38.0), (7.5, -28.0, -50.0)].iter().enumerate() {
                    let pts = [(-w, *y0), (0.0, *y1), (*w, *y0)];
                    c.two_tone(&pts, shade(g, 1.0 + i as f32 * 0.05), 0.0);
                }
            } else {
                c.two_tone(&[(-3.2, 0.0), (-2.4, -20.0), (2.4, -20.0), (3.2, 0.0)], 0x7a5436, 0.0);
                let g = if v % 2 == 1 { 0x3f8a36 } else { 0x4c9638 };
                c.facets(-9.0, -26.0, 10.0, 8.5, 7, shade(g, 0.92), 41 + v as u32, 0.3);
                c.facets(9.5, -27.0, 10.0, 8.5, 7, shade(g, 0.88), 51 + v as u32, 0.0);
                c.facets(0.0, -36.0, 13.0, 11.0, 8, g, 61 + v as u32, 0.2);
                if v == 3 {
                    for (x, y) in [(6.0, -30.0), (-7.0, -33.0), (2.0, -41.0)] {
                        c.dot(x, y, 2.0, col(0xd9483b));
                        c.dot(x - 0.6, y - 0.6, 0.7, cola(0xffffff, 0.6));
                    }
                }
            }
        }
        Sp::Palm => {
            ground_shadow(c, 11.0);
            let segs = 6;
            for i in 0..segs {
                let k0 = i as f32 / segs as f32;
                let k1 = (i + 1) as f32 / segs as f32;
                let p = |k: f32| (-1.5 * (k * PI).sin() + 5.0 * k * k, -31.0 * k);
                let (a, b) = (p(k0), p(k1));
                let (w0, w1) = (3.0 - k0 * 1.2, 3.0 - k1 * 1.2);
                let pts = [(a.0 - w0, a.1), (b.0 - w1, b.1 + 0.4), (b.0 + w1, b.1 + 0.4), (a.0 + w0, a.1)];
                c.two_tone(&pts, if i % 2 == 0 { 0xa47a4a } else { 0x8e6a40 }, (a.0 + b.0) / 2.0);
            }
            let top = (5.0, -31.0);
            let angles: [f32; 6] = [-2.9, -2.35, -1.8, -1.25, -0.7, -0.2];
            for (i, &a) in angles.iter().enumerate() {
                let len = 17.0;
                let (dx, dy) = (a.cos(), a.sin());
                let (px, py) = (-dy, dx);
                let mid = (top.0 + dx * len * 0.5, top.1 + dy * len * 0.5 - 2.0);
                let tip = (top.0 + dx * len, top.1 + dy * len + 5.0);
                let g = if i % 2 == 0 { 0x4a9a3a } else { 0x3f8a33 };
                c.poly(&[top, (mid.0 + px * 3.2, mid.1 + py * 3.2), tip], shade(g, 1.08));
                c.poly(&[top, tip, (mid.0 - px * 1.6, mid.1 - py * 1.6)], shade(g, 0.78));
            }
            c.dot(3.0, -29.0, 2.3, col(0x5b3d22));
            c.dot(7.0, -28.5, 2.3, col(0x4a3019));
        }
        Sp::Lily => {
            let pad = ell_pts(0.0, -1.0, 11.0, 6.2, 0.35, TAU - 0.1, 20);
            let mut pts = pad.clone();
            pts.push((0.0, -1.0));
            c.two_tone(&pts, 0x3f8f4a, -1.0);
            c.line(&[(0.0, -1.0), (-7.0, -3.5)], cola(0x2e6b37, 0.6), 0.8);
            if v % 2 == 0 {
                let (cx, cy) = (3.0, -4.5);
                for i in 0..6 {
                    let a = i as f32 / 6.0 * TAU - PI / 2.0;
                    let tip = (cx + a.cos() * 4.0, cy + a.sin() * 3.0);
                    c.poly(&[(cx, cy), (cx + (a - 0.6).cos() * 1.8, cy + (a - 0.6).sin() * 1.4), tip], shade(0xe98aa6, lum(a.cos(), a.sin())));
                    c.poly(&[(cx, cy), tip, (cx + (a + 0.6).cos() * 1.8, cy + (a + 0.6).sin() * 1.4)], shade(0xe98aa6, lum(a.cos(), a.sin()) - 0.1));
                }
                c.dot(cx, cy, 1.1, col(0xf2c14e));
            }
        }
        Sp::Mushroom => {
            let shroom = |c: &mut Cv, capc: u32| {
                c.two_tone(&[(-2.8, 0.0), (-2.2, -9.0), (2.2, -9.0), (2.8, 0.0)], 0xe9dcc2, 0.0);
                let (cx, cy) = (0.0, -9.0);
                let pts = ell_pts(cx, cy, 10.0, 8.5, PI, TAU, 6);
                for i in 0..6 {
                    let (p, q) = (pts[i], pts[i + 1]);
                    let (mx, my) = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0 - cy);
                    c.poly(&[(cx - 1.5, cy - 3.0), p, q], shade(capc, lum(mx, my)));
                }
                c.poly(&[(-10.0, cy), (10.0, cy), (8.0, cy + 1.2), (-8.0, cy + 1.2)], shade(capc, 0.62));
            };
            ground_shadow(c, 9.0);
            let capc = if v % 2 == 1 { 0xc8413b } else { 0x8a4fa8 };
            shroom(c, capc);
            for (a, b, r) in [(-4.0, -13.5, 1.5), (3.0, -15.0, 1.3), (5.5, -11.5, 1.0)] {
                c.dot(a, b, r, col(0xf1e9d8));
            }
            if v == 2 {
                c.save();
                c.translate(11.0, 1.0);
                c.scale(0.6, 0.6);
                shroom(c, 0xc8413b);
                c.restore();
            }
        }
        Sp::Bee => {
            // a flying piece on a clear stand
            token_base(c, 6.0);
            c.line(&[(0.0, -2.5), (0.0, -20.0)], cola(0xe8f4f8, 0.6), 1.3);
            let (bx, by) = (0.0, -25.0);
            c.ell(bx - 2.0, by - 6.0, 3.6, 4.2, -0.4, cola(0xf4fbff, 0.7));
            c.ell(bx + 2.5, by - 6.0, 3.0, 3.6, 0.4, cola(0xf4fbff, 0.6));
            c.facets(bx, by, 7.0, 5.0, 7, 0xe8b52e, 7, 0.0);
            c.ell(bx - 2.2, by, 1.3, 4.6, 0.0, col(0x2b2620));
            c.ell(bx + 1.8, by, 1.3, 4.8, 0.0, col(0x2b2620));
            c.dot(bx + 7.0, by - 0.5, 3.0, col(0x2b2620));
            c.poly(&[(bx - 7.0, by - 1.0), (bx - 10.0, by + 0.3), (bx - 7.0, by + 1.2)], 0x2b2620);
        }
        Sp::Rabbit => {
            token_base(c, 10.0);
            let fur = if v == 3 { 0xb08a64 } else { 0xe4dfd4 };
            c.dot(-8.5, -10.0, 2.6, col(shade(fur, 1.1)));
            c.facets(-1.0, -10.0, 8.5, 6.5, 7, fur, 70 + v as u32, 0.2);
            c.two_tone(&[(3.5, -19.0), (2.5, -30.0), (5.5, -20.0)], fur, 3.8);
            c.two_tone(&[(6.5, -19.0), (8.5, -29.0), (8.8, -18.5)], shade(fur, 0.95), 7.6);
            c.poly(&[(3.8, -20.5), (3.2, -27.0), (4.6, -21.0)], 0xd99aa0);
            c.facets(6.0, -16.0, 5.2, 4.5, 6, fur, 80 + v as u32, 0.0);
            eye(c, 8.2, -17.0);
            c.dot(11.0, -15.0, 0.9, col(0xc07a80));
        }
        Sp::Fox => {
            token_base(c, 10.0);
            let o = 0xd66a28;
            c.facets(-9.0, -9.0, 7.5, 3.6, 6, o, 91, -0.4);
            c.facets(-15.0, -11.5, 2.8, 2.4, 5, 0xefe6d8, 92, 0.0);
            c.facets(0.0, -10.0, 6.5, 8.0, 7, o, 93, 0.0);
            c.poly(&[(-0.5, -16.0), (4.0, -15.0), (1.2, -5.0)], 0xefe6d8);
            c.facets(1.0, -21.0, 5.5, 5.0, 6, o, 94, 0.3);
            c.poly(&[(4.5, -21.0), (10.0, -19.0), (4.5, -17.0)], 0xefe6d8);
            c.dot(10.0, -19.0, 0.9, col(0x1c1a18));
            c.two_tone(&[(-3.5, -24.5), (-4.5, -31.0), (0.0, -25.5)], o, -2.5);
            c.poly(&[(-4.2, -29.0), (-4.5, -31.0), (-3.0, -29.5)], 0x2a1e18);
            c.two_tone(&[(1.5, -25.5), (3.5, -31.5), (5.0, -24.0)], shade(o, 0.9), 3.0);
            c.poly(&[(3.1, -29.5), (3.5, -31.5), (4.2, -29.4)], 0x2a1e18);
            eye(c, 3.8, -22.0);
            c.line(&[(-3.0, -2.0), (-3.0, -4.0)], col(0x2a1e18), 1.4);
            c.line(&[(3.0, -2.0), (3.0, -4.0)], col(0x2a1e18), 1.4);
        }
        Sp::Bird => {
            token_base(c, 8.0);
            let (body, dark) = if v == 2 { (0xc94a3a, 0x8f2f24) } else { (0x3f7cc4, 0x28558f) };
            c.line(&[(-1.5, -2.0), (-1.5, -5.0)], col(0x3a3a3a), 1.2);
            c.line(&[(1.5, -2.0), (1.5, -5.0)], col(0x3a3a3a), 1.2);
            c.poly(&[(-5.0, -12.0), (-13.0, -15.0), (-11.0, -9.0)], dark);
            c.facets(0.0, -12.0, 7.5, 6.8, 7, body, 101 + v as u32, 0.0);
            c.facets(1.5, -9.0, 4.0, 3.0, 5, 0xe9dcc2, 102, 0.0);
            c.poly(&[(-6.0, -13.0), (3.0, -11.0), (-3.0, -7.0)], dark);
            c.poly(&[(7.0, -14.0), (11.5, -12.8), (7.0, -11.5)], 0xe7a634);
            eye(c, 4.2, -15.0);
        }
        Sp::Frog => {
            token_base(c, 10.0);
            let body = if v % 2 == 1 { 0x4e9e45 } else { 0x62a84a };
            c.facets(0.0, -7.0, 9.0, 6.0, 7, body, 111 + v as u32, 0.0);
            c.facets(0.5, -4.5, 5.0, 2.5, 5, 0xd8d49a, 112, 0.0);
            c.facets(-4.3, -12.0, 3.2, 3.0, 5, shade(body, 1.05), 113, 0.0);
            c.facets(4.3, -12.0, 3.2, 3.0, 5, shade(body, 0.95), 114, 0.0);
            eye(c, -4.0, -12.6);
            eye(c, 4.6, -12.6);
        }
        Sp::Crab => {
            token_base(c, 10.0);
            let r = 0xd2482e;
            for s in [-1.0f32, 1.0] {
                for i in 0..3 {
                    let i = i as f32;
                    c.line(&[(s * 5.0, -6.0 + i), (s * (10.0 + i), -2.5 + i * 0.6)], col(0x8a2a18), 1.3);
                }
            }
            c.facets(-11.0, -11.0, 3.8, 3.3, 5, r, 121, 0.0);
            c.facets(11.0, -11.0, 3.8, 3.3, 5, shade(r, 0.9), 122, 0.0);
            c.facets(0.0, -8.0, 9.5, 5.8, 7, r, 123, 0.0);
            c.line(&[(-2.5, -12.5), (-3.0, -16.0)], col(0x8a2a18), 1.3);
            c.line(&[(2.5, -12.5), (3.0, -16.0)], col(0x8a2a18), 1.3);
            c.dot(-3.0, -16.5, 1.3, col(0x1c1a18));
            c.dot(3.0, -16.5, 1.3, col(0x1c1a18));
        }
    }
}

fn draw_rock(c: &mut Cv, s: f32) {
    ground_shadow(c, 15.0);
    c.facets(0.0, -7.0, 14.0, 9.0, 6, 0x9aa0a3, (s * 100.0) as u32, 0.3);
    if s > 0.5 {
        c.facets(12.0, -3.5, 6.0, 4.0, 5, 0x878d90, 7, 0.0);
    }
    c.poly(&[(-11.0, -2.0), (-6.0, -4.0), (-3.0, -1.0)], 0x5f8f3a);
}

fn draw_cloud(c: &mut Cv) {
    let g = 0x6d7280;
    c.facets(-15.0, 1.0, 13.0, 10.0, 7, shade(g, 0.95), 201, 0.0);
    c.facets(14.0, -1.0, 13.0, 10.0, 7, shade(g, 0.9), 202, 0.3);
    c.facets(0.0, -8.0, 17.0, 13.0, 8, g, 203, 0.1);
    c.poly(&[(-26.0, 5.0), (26.0, 5.0), (20.0, 10.0), (-20.0, 10.0)], shade(g, 0.62));
}

fn draw_beetle(c: &mut Cv) {
    ground_shadow(c, 8.0);
    c.translate(0.0, -5.0);
    for sx in [-4.0, 0.0, 4.0] {
        c.line(&[(sx, 0.0), (sx + 1.5, 6.0)], col(0x1e1a24), 1.2);
        c.line(&[(sx, 0.0), (sx - 1.5, -6.0)], col(0x1e1a24), 1.2);
    }
    c.facets(0.0, 0.0, 7.5, 5.5, 7, 0x3b2f4a, 301, 0.0);
    c.line(&[(-7.0, 0.0), (5.0, 0.0)], col(0x1e1a24), 1.0);
    c.facets(8.0, 0.0, 3.2, 3.0, 5, 0x2a2233, 302, 0.0);
    c.line(&[(10.0, -1.5), (13.0, -4.0)], col(0x1e1a24), 1.0);
    c.line(&[(10.0, 1.5), (13.0, 4.0)], col(0x1e1a24), 1.0);
    c.ell(-2.0, -2.2, 3.0, 1.2, 0.0, cola(0xb9a6d8, 0.35));
}

fn draw_shield_icon(c: &mut Cv) {
    c.ell(0.0, 6.0, 17.0, 5.0, 0.0, col(0x8a6a44));
    c.ell(0.0, 5.0, 17.0, 5.0, 0.0, col(0xc49a62));
    let mut b = Pb::new();
    b.m(-15.0, 5.0);
    b.c(-15.0, -22.0, 15.0, -22.0, 15.0, 5.0);
    let p = b.done();
    c.fill(&p, cola(0x9fe0ff, 0.45));
    c.stroke(&p, cola(0xe6f8ff, 0.9), 1.6);
    c.ell(-7.0, -8.0, 2.4, 5.5, 0.5, cola(0xffffff, 0.7));
    c.two_tone(&[(0.0, 5.0), (-4.0, -2.0), (0.0, -6.0), (4.0, -2.0)], 0x5a9a3c, 0.0);
}

fn draw_whale(c: &mut Cv) {
    // golden whale breaching; water line at y = 0
    c.ell(0.0, 1.0, 36.0, 6.0, 0.0, cola(0xffffff, 0.35));
    let g = 0xe0ad3a;
    c.facets(-2.0, -7.0, 27.0, 11.0, 10, g, 401, 0.0);
    c.facets(0.0, -2.5, 20.0, 5.0, 7, 0xf3dca0, 402, 0.0);
    c.two_tone(&[(22.0, -10.0), (36.0, -22.0), (31.0, -9.0), (38.0, -3.0)], shade(g, 0.9), 31.0);
    c.poly(&[(-8.0, -4.0), (-2.0, 2.0), (4.0, -3.0)], shade(g, 0.72));
    c.dot(-18.0, -9.0, 1.4, col(0x1c1a18));
    c.ell(-8.0, -15.0, 6.0, 1.5, -0.2, cola(0xffffff, 0.45));
    c.ell(0.0, 1.5, 34.0, 3.0, 0.0, cola(0x2a88a4, 0.55));
}

fn draw_mood(c: &mut Cv, st: u8) {
    let ring = match st {
        2 => pal::GREEN,
        1 => pal::AMBER,
        _ => pal::RED,
    };
    c.dot(0.0, 0.0, 10.0, col(ring));
    c.dot(0.0, 0.0, 7.8, col(pal::SLATE));
    let w = col(0xf4efe4);
    match st {
        2 => c.line(&[(-4.0, 2.0), (0.0, -2.5), (4.0, 2.0)], w, 2.2),
        1 => c.line(&[(-3.8, 0.0), (3.8, 0.0)], w, 2.2),
        _ => c.line(&[(-4.0, -2.0), (0.0, 2.5), (4.0, -2.0)], w, 2.2),
    }
}

/// A board-game pawn. mood: 0 sad (dimmed, slumped), 1 normal, 2 happy (with a gold star).
fn draw_pawn(c: &mut Cv, color: u32, mood: u8) {
    let base = if mood == 0 { shade(color, 0.7) } else { color };
    if mood == 0 {
        c.rotate(0.12);
    }
    c.ell(0.0, 16.0, 14.0, 4.2, 0.0, cola(0x10202a, 0.3));
    c.ell(0.0, 14.5, 12.0, 4.0, 0.0, col(shade(base, 0.6)));
    c.two_tone(&[(-12.0, 13.5), (-6.5, 2.0), (6.5, 2.0), (12.0, 13.5)], base, -1.5);
    c.ell(0.0, 13.5, 12.0, 3.8, 0.0, col(shade(base, 0.72)));
    c.two_tone(&[(-8.0, 2.5), (-8.0, 0.0), (8.0, 0.0), (8.0, 2.5)], shade(base, 0.9), -1.0);
    c.two_tone(&[(-5.0, 0.0), (-3.8, -7.0), (3.8, -7.0), (5.0, 0.0)], base, -1.0);
    c.facets(0.0, -13.0, 7.5, 7.5, 9, base, color, 0.0);
    if mood == 2 {
        let mut b = Pb::new();
        for i in 0..10 {
            let rr = if i % 2 == 1 { 2.2 } else { 5.0 };
            let a = -PI / 2.0 + i as f32 / 10.0 * TAU;
            b.l(8.0 + a.cos() * rr, -21.0 + a.sin() * rr);
        }
        b.close();
        c.fill(&b.done(), col(pal::GOLD));
    }
}

fn draw_hand(c: &mut Cv) {
    c.rotate(-0.35);
    let mut b = Pb::new();
    b.m(-4.0, 0.0);
    b.l(-4.0, 18.0);
    b.q(-12.0, 16.0, -13.0, 22.0);
    b.q(-10.0, 34.0, -2.0, 40.0);
    b.l(14.0, 40.0);
    b.q(19.0, 30.0, 17.0, 20.0);
    b.l(17.0, 16.0);
    b.q(15.0, 12.0, 11.0, 14.0);
    b.q(10.0, 10.0, 6.0, 11.0);
    b.q(5.0, 8.0, 2.0, 9.0);
    b.l(2.0, 0.0);
    b.q(-1.0, -4.0, -4.0, 0.0);
    b.close();
    let p = b.done();
    c.fill(&p, col(0xf6f1e6));
    c.stroke(&p, col(pal::SLATE), 1.6);
}

fn draw_flag(c: &mut Cv) {
    c.line(&[(0.0, 12.0), (0.0, -15.0)], col(0x5a4630), 2.0);
    c.dot(0.0, -16.0, 1.8, col(pal::GOLD));
    c.poly(&[(1.0, -14.0), (17.0, -9.5), (1.0, -5.0)], pal::GOLD);
    c.poly(&[(1.0, -9.5), (17.0, -9.5), (1.0, -5.0)], shade(pal::GOLD, 0.8));
    c.ell(0.0, 12.5, 5.0, 1.8, 0.0, cola(0x10202a, 0.3));
}

fn draw_sprout(c: &mut Cv, g: u32) {
    c.line(&[(0.0, 8.0), (0.0, -2.0)], col(shade(g, 0.7)), 1.6);
    c.two_tone(&[(0.0, -1.0), (-8.0, -5.0), (-1.0, -6.0)], g, -3.0);
    c.two_tone(&[(0.0, -3.0), (8.0, -8.0), (1.0, -9.0)], shade(g, 0.95), 3.0);
}

fn draw_sound(c: &mut Cv, on: bool) {
    let w = col(ICON);
    let mut b = Pb::new();
    for (x, y) in [(-8.5, -3.0), (-4.2, -3.0), (0.8, -7.6), (0.8, 7.6), (-4.2, 3.0), (-8.5, 3.0)] {
        b.l(x, y);
    }
    b.close();
    c.fill(&b.done(), w);
    if on {
        for r in [5.0, 8.8] {
            let mut b = Pb::new();
            b.arc(0.8, 0.0, r, -0.8, 0.8);
            c.stroke(&b.done(), w, 2.0);
        }
    } else {
        c.line(&[(3.4, -3.7), (10.2, 3.7)], w, 2.0);
        c.line(&[(10.2, -3.7), (3.4, 3.7)], w, 2.0);
    }
}

/* ---------------- island base ---------------- */

fn lerp_col(a: u32, b: u32, k: f32) -> u32 {
    let ch = |s: u32| -> u32 {
        let (x, y) = (((a >> s) & 255) as f32, ((b >> s) & 255) as f32);
        ((x + (y - x) * k).clamp(0.0, 255.0) as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

fn terrain_col(t: Terr, lush: f32, shade_k: f32) -> u32 {
    let base = match t {
        Terr::Grass | Terr::Rock => lerp_col(0xa3ad52, 0x4f9636, lush),
        Terr::Mead => lerp_col(0xbdb35c, 0x79ad3d, lush),
        Terr::Sand => 0xe3c07e,
        Terr::Pond => 0x3d9ec2,
    };
    shade(base, 1.0 + (shade_k - 0.5) * 0.08)
}

pub const ISLAND_RES: f32 = 2.5;
pub const ISLAND_PAD: f32 = 70.0;
const DEPTH: f32 = 17.0;

/// Texture box for an island: bounds plus padding.
pub fn island_box(m: &IslandMap) -> (f32, f32, f32, f32) {
    let b = m.b;
    (b.x1 - b.x0 + ISLAND_PAD * 2.0, b.y1 - b.y0 + ISLAND_PAD * 2.0, -b.x0 + ISLAND_PAD, -b.y0 + ISLAND_PAD)
}

/// Shallow water and surf around the island (drawn under it, gently pulsing).
pub fn island_sea_tex(images: &mut Assets<Image>, m: &IslandMap) -> Tex {
    let (w, h, ox, oy) = island_box(m);
    make(images, w, h, ox, oy, ISLAND_RES, |c| {
        for &i in &m.edge {
            let t = &m.tiles[i];
            c.fill(&hex_path(t.x, t.y + 12.0, HS * 1.9), cola(0x55c2c9, 0.4));
        }
        for &i in &m.edge {
            let t = &m.tiles[i];
            c.fill(&hex_path(t.x, t.y + 13.0, HS * 1.3 + (t.shade - 0.5) * 4.0), cola(0xe9f7f2, 0.55));
        }
    })
}

/// The island: extruded hex tiles with bevelled tops, seams and a few tufts.
pub fn island_tex(images: &mut Assets<Image>, m: &IslandMap, lush: f32) -> Tex {
    let (w, h, ox, oy) = island_box(m);
    make(images, w, h, ox, oy, ISLAND_RES, |c| {
        let r = HS * 1.02;
        // cliff faces: east (dark), south-east (mid), south-west (lit)
        let mut order: Vec<usize> = (0..m.tiles.len()).collect();
        order.sort_by(|a, b| m.tiles[*a].y.partial_cmp(&m.tiles[*b].y).unwrap_or(std::cmp::Ordering::Equal));
        for &i in &order {
            let t = &m.tiles[i];
            for e in 0..3 {
                let d = EDGE_DIR[e];
                if m.has(t.q + d.0, t.r + d.1) {
                    continue;
                }
                let (a, b) = (hex_vertex(e), hex_vertex((e + 1) % 6));
                let (ax, ay) = (t.x + a.0 * r, t.y + a.1 * r);
                let (bx, by) = (t.x + b.0 * r, t.y + b.1 * r);
                let face = [0x7a4f33, 0x93613f, 0xab784e][e];
                c.poly(&[(ax, ay), (bx, by), (bx, by + DEPTH), (ax, ay + DEPTH)], face);
                c.poly(&[(ax, ay + DEPTH * 0.62), (bx, by + DEPTH * 0.62), (bx, by + DEPTH), (ax, ay + DEPTH)], shade(face, 0.78));
            }
        }
        // tiles: seam, top, bevel
        for t in &m.tiles {
            let base = terrain_col(t.t, lush, t.shade);
            c.poly(&hex_pts(t.x, t.y, r), shade(base, 0.8));
            let top = hex_pts(t.x, t.y, HS * 0.93);
            c.poly(&top, base);
            // lit rim along the upper-left edges, shaded rim along the lower-right edges
            c.line(&[top[2], top[3], top[4], top[5]], col(shade(base, 1.14)), 1.4);
            c.line(&[top[5], top[0], top[1], top[2]], col(shade(base, 0.86)), 1.4);
            if t.t == Terr::Pond {
                let k = t.shade;
                c.ell(t.x + (k - 0.5) * 12.0, t.y + 2.0, 5.0 + k * 8.0, (5.0 + k * 8.0) * 0.35, 0.0, cola(0x86d3ea, 0.5));
                c.line(&[(t.x - 12.0, t.y - 3.0), (t.x - 4.0, t.y - 3.0)], cola(0xc8eef8, 0.55), 1.2);
            }
        }
        for t in &m.tiles {
            for d in &t.deco {
                let (x, y) = (t.x + d.x * 0.9, t.y + d.y * 0.9);
                match d.kind {
                    DecoKind::Tuft => {
                        let g = shade(terrain_col(t.t, lush, t.shade), 0.72);
                        c.poly(&[(x - 2.6 * d.s, y), (x - 0.8 * d.s, y - 4.2 * d.s), (x + 0.4 * d.s, y)], g);
                        c.poly(&[(x, y), (x + 1.6 * d.s, y - 3.4 * d.s), (x + 2.8 * d.s, y)], g);
                    }
                    DecoKind::Dot => c.dot(x, y, 1.3 * d.s, col(d.col)),
                    DecoKind::Pebble => c.ell(x, y, 2.0 * d.s, 1.2 * d.s, 0.0, cola(0xa98a58, 0.6)),
                    DecoKind::Shell => c.poly(&[(x - 2.0 * d.s, y), (x, y - 2.2 * d.s), (x + 2.0 * d.s, y)], 0xf1e2d0),
                }
            }
        }
    })
}
