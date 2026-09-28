//! Every picture in the game is drawn here with vector paths (tiny-skia)
//! and uploaded as a texture at startup, so the game ships as one binary.
//!
//! Style: minimalist low-poly. Shapes are flat-shaded facets lit from the
//! upper left, with no ink outlines. Creatures are board-game figurines on
//! wooden bases; the island is made of bevelled hex tiles.

use crate::island::{hex_vertex, HS, SQ};
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
}

impl Cv {
    pub fn new(w: u32, h: u32) -> Self {
        Cv { pm: Pixmap::new(w.max(1), h.max(1)).expect("pixmap"), tf: Tf::identity() }
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
    pub bolt: Tex,
    /// Five-pointed star: filled (white, tinted when drawn) and outline.
    pub star: Tex,
    pub star_line: Tex,
    pub heart: Tex,
    pub heart_broken: Tex,
    pub spark: Tex,
    pub circle: Tex,
    pub ring: Tex,
    pub hex: Tex,
    pub hex_line: Tex,
    pub pixel: Tex,
    /// Mood badges: sad, fine, thriving.
    pub faces: [Tex; 3],
    pub hand: Tex,
    pub trophy: Tex,
    pub crown: Tex,
    pub rrect_fill: Tex,
    pub rrect_line: Tex,
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
    /// Weather: clear, rain, drought, wind.
    pub weather: [Tex; 4],
    pub alert: Tex,
    pub food: Tex,
    pub drop_icon: Tex,
    pub shade_icon: Tex,
}


const ICON: u32 = pal::CREAM;

pub fn build_art(images: &mut Assets<Image>) -> Art {
    let r = SP_RES;
    let rect = |x: f32, y: f32, w: f32, h: f32| Some(PathBuilder::from_rect(SkRect::from_xywh(x, y, w, h).unwrap()));
    Art {
        bolt: make(images, 20.0, 32.0, 10.0, 16.0, r, |c| {
            let pts = [(2.0, -12.0), (-6.0, 2.0), (0.0, 2.0), (-3.0, 14.0), (7.0, -2.0), (1.0, -2.0), (5.0, -12.0)];
            c.poly(&pts, 0xf5c542);
            c.poly(&clip_left(&pts, 0.5), 0xfbe08a);
        }),
        star: make(images, 40.0, 40.0, 20.0, 21.0, r, |c| {
            c.fill(&star_path(15.5), col(0xffffff));
        }),
        star_line: make(images, 40.0, 40.0, 20.0, 21.0, r, |c| {
            c.stroke(&star_path(14.0), col(0xffffff), 2.6);
        }),
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
        circle: make(images, 32.0, 32.0, 16.0, 16.0, 4.0, |c| c.dot(0.0, 0.0, 15.5, col(0xffffff))),
        ring: make(images, 64.0, 64.0, 32.0, 32.0, 2.0, |c| c.stroke(&circle(0.0, 0.0, 30.0), col(0xffffff), 2.4)),
        hex: make(images, 2.2 * HS, 2.2 * HS * SQ, 1.1 * HS, 1.1 * HS * SQ, 2.0, |c| c.fill(&hex_path(0.0, 0.0, HS), col(0xffffff))),
        hex_line: make(images, 2.2 * HS, 2.2 * HS * SQ, 1.1 * HS, 1.1 * HS * SQ, 2.0, |c| c.stroke(&hex_path(0.0, 0.0, HS * 0.9), col(0xffffff), 3.0)),
        pixel: make(images, 4.0, 4.0, 2.0, 2.0, 1.0, |c| c.fill(&rect(-2.0, -2.0, 4.0, 4.0), col(0xffffff))),
        faces: [0u8, 1, 2].map(|st| make(images, 24.0, 24.0, 12.0, 12.0, r, |c| draw_mood(c, st))),
        hand: make(images, 40.0, 50.0, 14.0, 4.0, r, draw_hand),
        trophy: make(images, 34.0, 34.0, 12.0, 20.0, r, draw_flag),
        crown: make(images, 42.0, 28.0, 21.0, 14.0, r, |c| {
            let pts = [(-16.0, 8.0), (-18.0, -8.0), (-8.0, 0.0), (0.0, -12.0), (8.0, 0.0), (18.0, -8.0), (16.0, 8.0)];
            c.two_tone(&pts, pal::GOLD, 0.0);
            c.dot(0.0, 2.0, 2.5, col(0xd9483b));
        }),
        rrect_fill: make(images, 64.0, 64.0, 0.0, 0.0, 1.0, |c| c.fill(&rrect(1.0, 1.0, 62.0, 62.0, 14.0), col(0xffffff))),
        rrect_line: make(images, 64.0, 64.0, 0.0, 0.0, 1.0, |c| c.stroke(&rrect(2.0, 2.0, 60.0, 60.0, 13.0), col(0xffffff), 2.2)),
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
        weather: [
            make(images, 36.0, 36.0, 18.0, 18.0, r, |c| {
                for k in 0..8 {
                    let a = k as f32 / 8.0 * TAU;
                    c.line(&[(a.cos() * 10.5, a.sin() * 10.5), (a.cos() * 15.0, a.sin() * 15.0)], col(0xf5c542), 2.4);
                }
                c.dot(0.0, 0.0, 8.0, col(0xf5c542));
                c.dot(-2.0, -2.0, 3.0, col(0xfbe08a));
            }),
            make(images, 36.0, 36.0, 18.0, 18.0, r, |c| {
                for (x, y, rr) in [(-7.0, -2.0, 7.0), (2.0, -6.0, 9.0), (9.0, -1.0, 6.5)] {
                    c.dot(x, y, rr, col(0xcfd6de));
                }
                c.fill(&Some(PathBuilder::from_rect(SkRect::from_xywh(-14.0, -2.0, 29.0, 7.0).unwrap())), col(0xcfd6de));
                for x in [-8.0, -1.0, 6.0] {
                    c.line(&[(x, 9.0), (x - 2.0, 15.0)], col(0x6fc3ff), 2.4);
                }
            }),
            make(images, 36.0, 36.0, 18.0, 18.0, r, |c| {
                c.dot(0.0, -3.0, 9.0, col(0xf08a3a));
                c.dot(-2.0, -5.0, 3.0, col(0xf6b36a));
                for k in 0..3 {
                    let y = 10.0 + k as f32 * 3.5;
                    c.line(&[(-10.0, y), (-5.0, y - 1.5), (0.0, y), (5.0, y - 1.5), (10.0, y)], col(0xf08a3a), 1.6);
                }
            }),
            make(images, 36.0, 36.0, 18.0, 18.0, r, |c| {
                let w = col(0xdfeaf0);
                c.line(&[(-14.0, -6.0), (4.0, -6.0), (8.0, -9.0), (5.0, -12.0)], w, 2.4);
                c.line(&[(-14.0, 1.0), (10.0, 1.0), (14.0, -2.0), (11.0, -5.0)], w, 2.4);
                c.line(&[(-10.0, 8.0), (4.0, 8.0), (7.0, 11.0), (4.0, 13.0)], w, 2.4);
            }),
        ],
        alert: make(images, 22.0, 22.0, 11.0, 11.0, r, |c| {
            c.dot(0.0, 0.0, 10.0, col(pal::RED));
            c.fill(&rrect(-1.6, -6.5, 3.2, 8.5, 1.4), col(0xffffff));
            c.dot(0.0, 5.0, 1.8, col(0xffffff));
        }),
        food: make(images, 22.0, 22.0, 11.0, 11.0, r, |c| {
            c.dot(-2.5, 1.5, 6.5, col(0xd9483b));
            c.dot(2.5, 1.5, 6.5, col(0xc93e32));
            c.dot(-3.5, -1.0, 2.0, cola(0xffffff, 0.5));
            c.line(&[(0.0, -4.5), (1.0, -9.0)], col(0x6b4a1f), 1.6);
            c.poly(&[(1.0, -7.5), (6.5, -9.5), (3.0, -5.5)], 0x5fb04a);
        }),
        drop_icon: make(images, 16.0, 20.0, 8.0, 10.0, r, |c| {
            let mut b = Pb::new();
            b.m(0.0, -8.0);
            b.q(6.5, 0.0, 5.0, 4.0);
            b.q(3.5, 8.0, 0.0, 8.0);
            b.q(-3.5, 8.0, -5.0, 4.0);
            b.q(-6.5, 0.0, 0.0, -8.0);
            b.close();
            c.fill(&b.done(), col(0xffffff));
        }),
        shade_icon: make(images, 20.0, 22.0, 10.0, 11.0, r, |c| {
            c.fill(&rrect(-1.5, 2.0, 3.0, 8.0, 1.0), col(0x8a6440));
            c.dot(0.0, -3.0, 7.5, col(0x3f8a36));
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


pub fn heart_path(x: f32, y: f32, s: f32) -> Option<Path> {
    let mut b = Pb::new();
    b.m(x, y + s * 0.9);
    b.c(x - s * 1.4, y, x - s * 0.9, y - s * 1.1, x, y - s * 0.35);
    b.c(x + s * 0.9, y - s * 1.1, x + s * 1.4, y, x, y + s * 0.9);
    b.close();
    b.done()
}

fn star_path(r: f32) -> Option<tiny_skia::Path> {
    let mut b = Pb::new();
    for i in 0..10 {
        let rr = if i % 2 == 0 { r } else { r * 0.45 };
        let a = i as f32 / 10.0 * TAU - std::f32::consts::FRAC_PI_2;
        b.l(a.cos() * rr, a.sin() * rr);
    }
    b.close();
    b.done()
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
