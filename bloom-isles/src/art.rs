//! Every picture in the game is drawn here with vector paths (tiny-skia)
//! and uploaded as a texture at startup, so the game ships as one binary.

use crate::eco::Sp;
use crate::island::{hex_vertex, DecoKind, IslandMap, Terr, EDGE_DIR, HS, SQ};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::f32::consts::{PI, TAU};
use tiny_skia::{
    Color as SkColor, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect as SkRect, Stroke, StrokeDash, Transform as Tf,
};

pub const INK: u32 = 0x4a3a3f;

pub fn col(hex: u32) -> SkColor {
    SkColor::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}
pub fn cola(hex: u32, a: f32) -> SkColor {
    SkColor::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, (a.clamp(0.0, 1.0) * 255.0) as u8)
}
/// Same colour as a Bevy colour.
pub fn bc(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}
pub fn bca(hex: u32, a: f32) -> Color {
    bc(hex).with_alpha(a)
}

/* ---------------- a tiny canvas-like wrapper ---------------- */

pub struct Cv {
    pub pm: Pixmap,
    tf: Tf,
    stack: Vec<Tf>,
    pub lw: f32,
}

impl Cv {
    pub fn new(w: u32, h: u32) -> Self {
        Cv { pm: Pixmap::new(w.max(1), h.max(1)).expect("pixmap"), tf: Tf::identity(), stack: vec![], lw: 1.8 }
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
    /// Fill, then outline in ink.
    pub fn fs(&mut self, p: &Option<Path>, fill: SkColor) {
        self.fill(p, fill);
        let lw = self.lw;
        self.stroke(p, col(INK), lw);
    }
    pub fn blob(&mut self, x: f32, y: f32, r: f32, fill: SkColor) {
        self.fs(&circle(x, y, r), fill);
    }
    pub fn dot(&mut self, x: f32, y: f32, r: f32, fill: SkColor) {
        self.fill(&circle(x, y, r), fill);
    }
    pub fn ell(&mut self, x: f32, y: f32, rx: f32, ry: f32, rot: f32, fill: SkColor, outline: bool) {
        let p = ellipse(x, y, rx, ry, rot);
        if outline {
            self.fs(&p, fill)
        } else {
            self.fill(&p, fill)
        }
    }
    pub fn eyes(&mut self, x1: f32, x2: f32, y: f32, r: f32) {
        self.dot(x1, y, r, col(INK));
        self.dot(x2, y, r, col(INK));
    }
    pub fn line(&mut self, pts: &[(f32, f32)], c: SkColor, w: f32) {
        let mut b = Pb::new();
        for (i, &(x, y)) in pts.iter().enumerate() {
            if i == 0 {
                b.m(x, y)
            } else {
                b.l(x, y)
            }
        }
        self.stroke(&b.done(), c, w);
    }
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
    /// Clockwise arc (screen coordinates) from angle a0 to a1.
    pub fn arc(&mut self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        self.ell_arc(cx, cy, r, r, a0, a1);
    }
    pub fn ell_arc(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32) {
        let mut a1 = a1;
        while a1 < a0 {
            a1 += TAU;
        }
        let n = (((a1 - a0) / TAU) * 48.0).ceil().max(2.0) as usize;
        for i in 0..=n {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            self.l(cx + a.cos() * rx, cy + a.sin() * ry);
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
    pub faces: [Tex; 3],
    pub avatars: Vec<[Tex; 3]>,
    pub hand: Tex,
    pub trophy: Tex,
    pub crown: Tex,
    pub dome: Tex,
    pub rrect_fill: Tex,
    pub rrect_line: Tex,
    pub rrect_mask: Tex,
    pub wave: Tex,
    pub grain: Tex,
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
}

impl Art {
    pub fn sp(&self, sp: Sp, v: u8) -> &Tex {
        &self.sp[sp.idx()][(v % 4) as usize]
    }
}

pub fn build_art(images: &mut Assets<Image>) -> Art {
    let mut sp = vec![];
    for s in crate::eco::ALL {
        let mut vs = vec![];
        for v in 0..4u8 {
            vs.push(sp_tex(images, s, v));
        }
        sp.push(vs);
    }
    let r = SP_RES;
    Art {
        sp,
        rock: [make(images, 44.0, 30.0, 18.0, 24.0, r, |c| draw_rock(c, 0.2)), make(images, 44.0, 30.0, 18.0, 24.0, r, |c| draw_rock(c, 0.8))],
        cloud: make(images, 64.0, 44.0, 32.0, 24.0, r, |c| draw_cloud(c, true)),
        bolt: make(images, 20.0, 32.0, 10.0, 16.0, r, draw_bolt),
        beetle: make(images, 34.0, 22.0, 15.0, 13.0, r, draw_beetle),
        shield: make(images, 40.0, 36.0, 20.0, 22.0, r, draw_shield_icon),
        heart: make(images, 16.0, 16.0, 8.0, 8.0, 4.0, |c| {
            c.lw = 1.2;
            let p = heart_path(0.0, 0.0, 5.0);
            c.fs(&p, col(0xff7f9a));
        }),
        heart_broken: make(images, 16.0, 16.0, 8.0, 8.0, 4.0, |c| {
            c.lw = 1.2;
            let p = heart_path(0.0, 0.0, 5.0);
            c.fs(&p, col(0x9aa0aa));
            c.line(&[(-1.0, -3.0), (1.5, 0.0), (-1.0, 3.0)], col(INK), 1.5);
        }),
        spark: make(images, 12.0, 12.0, 6.0, 6.0, 4.0, |c| {
            let mut b = Pb::new();
            for i in 0..8 {
                let rr = if i % 2 == 1 { 1.4 } else { 4.8 };
                let a = i as f32 / 8.0 * TAU;
                b.l(a.cos() * rr, a.sin() * rr);
            }
            b.close();
            c.fill(&b.done(), col(0xfff6b0));
        }),
        leaf: make(images, 10.0, 6.0, 5.0, 3.0, 4.0, |c| c.ell(0.0, 0.0, 4.0, 2.0, 0.0, col(0xffffff), false)),
        circle: make(images, 32.0, 32.0, 16.0, 16.0, 4.0, |c| c.dot(0.0, 0.0, 15.5, col(0xffffff))),
        ring: make(images, 64.0, 64.0, 32.0, 32.0, 2.0, |c| c.stroke(&circle(0.0, 0.0, 30.0), col(0xffffff), 2.4)),
        drop: make(images, 6.0, 10.0, 3.0, 5.0, 4.0, |c| c.line(&[(1.0, -3.5), (-1.0, 3.5)], col(0xd5ecff), 1.6)),
        shadow: make(images, 32.0, 12.0, 16.0, 6.0, 3.0, |c| c.ell(0.0, 0.0, 15.0, 5.0, 0.0, col(0xffffff), false)),
        hex: make(images, 2.2 * HS, 2.2 * HS * SQ, 1.1 * HS, 1.1 * HS * SQ, 2.0, |c| c.fill(&hex_path(0.0, 0.0, HS), col(0xffffff))),
        hex_line: make(images, 2.2 * HS, 2.2 * HS * SQ, 1.1 * HS, 1.1 * HS * SQ, 2.0, |c| c.stroke(&hex_path(0.0, 0.0, HS * 0.9), col(0xffffff), 3.0)),
        pixel: make(images, 4.0, 4.0, 2.0, 2.0, 1.0, |c| c.fill(&PathBuilder::from_rect(SkRect::from_xywh(-2.0, -2.0, 4.0, 4.0).unwrap()).into(), col(0xffffff))),
        faces: [0u8, 1, 2].map(|st| make(images, 24.0, 24.0, 12.0, 12.0, r, |c| draw_face(c, st))),
        avatars: (0..4).map(|i| [0u8, 1, 2].map(|m| make(images, 48.0, 56.0, 24.0, 30.0, r, |c| draw_avatar(c, i, m)))).collect(),
        hand: make(images, 40.0, 50.0, 14.0, 4.0, r, draw_hand),
        trophy: make(images, 34.0, 34.0, 17.0, 17.0, r, draw_trophy),
        crown: make(images, 42.0, 28.0, 21.0, 14.0, r, draw_crown),
        dome: make(images, 200.0, 200.0, 100.0, 100.0, 2.0, |c| {
            c.fill(&circle(0.0, 0.0, 96.0), cola(0xaae1ff, 0.14));
            c.stroke(&circle(0.0, 0.0, 96.0), cola(0xffffff, 0.75), 3.0);
            let mut b = Pb::new();
            b.arc(0.0, 0.0, 82.0, PI * 1.1, PI * 1.35);
            c.stroke(&b.done(), cola(0xffffff, 0.5), 6.0);
        }),
        rrect_fill: make(images, 64.0, 64.0, 0.0, 0.0, 1.0, |c| c.fill(&rrect(1.0, 1.0, 62.0, 62.0, 14.0), col(0xffffff))),
        rrect_line: make(images, 64.0, 64.0, 0.0, 0.0, 1.0, |c| c.stroke(&rrect(2.0, 2.0, 60.0, 60.0, 13.0), col(0xffffff), 2.6)),
        rrect_mask: make(images, 64.0, 64.0, 0.0, 0.0, 1.0, |c| {
            let mut b = PathBuilder::new();
            b.push_rect(SkRect::from_xywh(0.0, 0.0, 64.0, 64.0).unwrap());
            let outer = b.finish();
            c.fill(&outer, col(0xffffff));
            // punch out the rounded interior
            if let Some(p) = rrect(1.5, 1.5, 61.0, 61.0, 14.0) {
                let mut paint = Paint::default();
                paint.set_color(SkColor::from_rgba8(0, 0, 0, 255));
                paint.anti_alias = true;
                paint.blend_mode = tiny_skia::BlendMode::DestinationOut;
                c.pm.fill_path(&p, &paint, FillRule::Winding, Tf::identity(), None);
            }
        }),
        wave: make(images, 32.0, 10.0, 16.0, 5.0, 2.0, |c| {
            let mut b = Pb::new();
            b.arc(-6.0, 5.0, 7.0, PI * 1.15, PI * 1.85);
            let p1 = b.done();
            let mut b = Pb::new();
            b.arc(5.0, 5.0, 7.0, PI * 1.15, PI * 1.85);
            c.stroke(&p1, col(0xffffff), 2.0);
            c.stroke(&b.done(), col(0xffffff), 2.0);
        }),
        grain: make(images, 160.0, 160.0, 0.0, 0.0, 1.0, |c| {
            let mut rng = crate::rng::Rng::new(99);
            for _ in 0..1400 {
                let (x, y, s) = (rng.f() * 160.0, rng.f() * 160.0, 1.0 + rng.f() * 1.5);
                let colr = if rng.f() < 0.5 { cola(0xffffff, 0.05) } else { cola(0x3c281e, 0.04) };
                c.fill(&Some(PathBuilder::from_rect(SkRect::from_xywh(x, y, s, s).unwrap())), colr);
            }
        }),
        glyph_leaf: make(images, 14.0, 10.0, 7.0, 5.0, r, |c| {
            c.lw = 1.4;
            c.ell(0.0, 0.0, 5.5, 3.0, -0.6, col(0x7fc76f), true)
        }),
        glyph_paw: make(images, 14.0, 14.0, 7.0, 7.0, r, |c| {
            c.dot(0.0, 1.5, 3.2, col(0xd9875a));
            for (a, b) in [(-3.8, -3.0), (0.0, -4.5), (3.8, -3.0)] {
                c.dot(a, b, 1.6, col(0xd9875a));
            }
        }),
        glyph_target: make(images, 20.0, 20.0, 10.0, 10.0, r, |c| {
            c.stroke(&circle(0.0, 0.0, 5.0), col(0xffb3a8), 1.8);
            c.line(&[(-8.0, 0.0), (8.0, 0.0)], col(0xffb3a8), 1.8);
            c.line(&[(0.0, -8.0), (0.0, 8.0)], col(0xffb3a8), 1.8);
        }),
        crosshair: make(images, 64.0, 64.0, 32.0, 32.0, 2.0, |c| {
            let red = col(0xe96b5f);
            c.stroke(&circle(0.0, 0.0, 16.0), red, 3.0);
            for (a, b, x, y) in [(-28.0, 0.0, -10.0, 0.0), (10.0, 0.0, 28.0, 0.0), (0.0, -28.0, 0.0, -10.0), (0.0, 10.0, 0.0, 28.0)] {
                c.line(&[(a, b), (x, y)], red, 3.0);
            }
        }),
        sprout_on: make(images, 16.0, 20.0, 8.0, 10.0, r, |c| draw_sprout(c, true)),
        sprout_off: make(images, 16.0, 20.0, 8.0, 10.0, r, |c| draw_sprout(c, false)),
        dice: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            c.lw = 2.0;
            c.fs(&rrect(-12.0, -12.0, 24.0, 24.0, 5.0), col(0xffffff));
            for (a, b) in [(-5.0, -5.0), (5.0, 5.0), (0.0, 0.0), (5.0, -5.0), (-5.0, 5.0)] {
                c.dot(a, b, 2.0, col(INK));
            }
        }),
        icon_pause: make(images, 20.0, 20.0, 10.0, 10.0, r, |c| {
            c.fill(&rrect(-6.5, -7.0, 4.4, 14.0, 2.0), col(INK));
            c.fill(&rrect(2.1, -7.0, 4.4, 14.0, 2.0), col(INK));
        }),
        icon_play: make(images, 40.0, 40.0, 20.0, 20.0, r, |c| {
            let mut b = Pb::new();
            b.m(-6.0, -10.0);
            b.l(11.0, 0.0);
            b.l(-6.0, 10.0);
            b.close();
            let p = b.done();
            c.fill(&p, col(0xffffff));
            c.stroke(&p, col(INK), 2.2);
        }),
        icon_sound_on: make(images, 24.0, 20.0, 12.0, 10.0, r, |c| draw_sound(c, true)),
        icon_sound_off: make(images, 24.0, 20.0, 12.0, 10.0, r, |c| draw_sound(c, false)),
        icon_reroll: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            for s in [0.0, PI] {
                let mut b = Pb::new();
                b.arc(0.0, 0.0, 9.0, s + 0.3, s + 2.6);
                c.stroke(&b.done(), col(INK), 2.4);
                let a = s + 2.6f32;
                let (px, py) = (a.cos() * 9.0, a.sin() * 9.0);
                c.line(&[(px + (a - 1.2).cos() * 5.0, py + (a - 1.2).sin() * 5.0), (px, py), (px + (a + 0.6).cos() * 4.4, py + (a + 0.6).sin() * 4.4)], col(INK), 2.4);
            }
        }),
        icon_shovel: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            c.rotate(0.7);
            c.lw = 2.0;
            c.fs(&rrect(-1.6, -12.4, 3.2, 14.4, 1.5), col(0xb98763));
            let mut b = Pb::new();
            b.m(-5.6, 1.6);
            b.l(5.6, 1.6);
            b.q(6.0, 10.0, 0.0, 13.2);
            b.q(-6.0, 10.0, -5.6, 1.6);
            b.close();
            c.fs(&b.done(), col(0xc9cfd8));
            c.fs(&rrect(-4.4, -14.4, 8.8, 2.8, 1.4), col(0xb98763));
        }),
        icon_home: make(images, 30.0, 30.0, 15.0, 15.0, r, |c| {
            c.lw = 2.2;
            let mut b = Pb::new();
            b.m(-11.0, -0.4);
            b.l(0.0, -11.0);
            b.l(11.0, -0.4);
            b.close();
            c.fs(&b.done(), col(0xff8f7d));
            c.fs(&rrect(-7.6, -1.0, 15.2, 11.0, 2.0), col(0xfff6e4));
            c.fill(&rrect(-2.0, 3.0, 4.0, 7.0, 1.5), col(INK));
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

/* ---------------- species ---------------- */

/// Box used for every species texture (units): x -32..32, y -(head+16)..10.
fn sp_tex(images: &mut Assets<Image>, sp: Sp, v: u8) -> Tex {
    if sp == Sp::Bee {
        return make(images, 30.0, 26.0, 15.0, 14.0, SP_RES, |c| draw_sp(c, sp, v));
    }
    let head = crate::eco::def(sp).head;
    let top = head + 18.0;
    make(images, 64.0, top + 10.0, 32.0, top, SP_RES, |c| draw_sp(c, sp, v))
}

pub fn draw_sp(c: &mut Cv, sp: Sp, v: u8) {
    c.lw = 1.8;
    let ink = col(INK);
    if !matches!(sp, Sp::Lily | Sp::Frog | Sp::Bee) {
        let rx = if sp == Sp::Tree { 14.0 } else { 10.0 };
        c.ell(0.0, 1.0, rx, 4.0, 0.0, cola(0x3c3228, 0.16), false);
    }
    match sp {
        Sp::Flower => {
            let mut b = Pb::new();
            b.m(0.0, 0.0);
            b.q(2.0, -9.0, 0.0, -17.0);
            c.stroke(&b.done(), ink, 1.8);
            c.ell(-4.5, -7.0, 4.5, 2.2, -0.5, col(0x86cc6a), true);
            let pc = [0xff9fb8, 0xffd35c, 0xc7a4ff, 0xff9a6b][(v % 4) as usize];
            for i in 0..5 {
                let a = i as f32 / 5.0 * TAU - PI / 2.0;
                c.blob(a.cos() * 4.6, -19.0 + a.sin() * 4.6, 3.7, col(pc));
            }
            c.blob(0.0, -19.0, 2.6, col(0xffe58a));
        }
        Sp::Bush => {
            c.blob(-7.0, -8.0, 7.5, col(0x6fb964));
            c.blob(7.0, -8.0, 7.5, col(0x6fb964));
            c.blob(0.0, -14.0, 9.5, col(0x7fc76f));
            c.ell(-3.0, -18.0, 3.5, 2.0, -0.4, col(0xa9df8e), false);
            let bcol = if v % 2 == 1 { 0xe85f6f } else { 0x7c6fe0 };
            c.lw = 1.2;
            for (x, y) in [(-6.0, -9.0), (5.0, -12.0), (8.0, -6.0), (-1.0, -6.0), (2.0, -17.0)] {
                c.blob(x, y, 2.0, col(bcol));
            }
        }
        Sp::Tree => {
            let mut b = Pb::new();
            b.m(-3.5, 0.0);
            b.l(-2.5, -20.0);
            b.l(2.5, -20.0);
            b.l(3.5, 0.0);
            b.close();
            c.fs(&b.done(), col(0xa8764e));
            let (g1, g2) = if v % 2 == 1 { (0x5fae57, 0x74c267) } else { (0x6ab45a, 0x80c868) };
            c.blob(-10.0, -27.0, 10.0, col(g1));
            c.blob(10.0, -28.0, 10.0, col(g1));
            c.blob(0.0, -37.0, 13.5, col(g2));
            c.ell(-5.0, -42.0, 5.0, 3.0, -0.5, cola(0xffffff, 0.35), false);
            if v == 3 {
                c.lw = 1.2;
                c.blob(6.0, -30.0, 2.2, col(0xff8a6b));
                c.blob(-7.0, -33.0, 2.2, col(0xff8a6b));
            }
        }
        Sp::Palm => {
            let mut b = Pb::new();
            b.m(0.0, 0.0);
            b.q(-2.0, -16.0, 5.0, -30.0);
            let trunk = b.done();
            c.stroke(&trunk, ink, 7.5);
            c.stroke(&trunk, col(0xd7a86c), 4.5);
            for i in 0..5 {
                let a = -PI / 2.0 + (i as f32 - 2.0) * 0.75;
                c.save();
                c.translate(5.0, -30.0);
                c.rotate(a);
                let mut b = Pb::new();
                b.m(0.0, 0.0);
                b.q(9.0, -5.0, 17.0, 2.0);
                b.q(9.0, 2.0, 0.0, 0.0);
                b.close();
                c.fs(&b.done(), col(if i % 2 == 1 { 0x6cbb5b } else { 0x7cc96a }));
                c.restore();
            }
            c.lw = 1.3;
            c.blob(3.0, -28.0, 2.6, col(0x8a6040));
            c.blob(7.0, -27.0, 2.6, col(0x8a6040));
        }
        Sp::Lily => {
            let mut b = Pb::new();
            b.ell_arc(0.0, -1.0, 11.0, 6.5, 0.35, TAU - 0.1);
            b.l(0.0, -1.0);
            b.close();
            c.fs(&b.done(), col(0x6dbd67));
            if v % 2 == 0 {
                c.lw = 1.2;
                for i in 0..5 {
                    let a = i as f32 / 5.0 * TAU;
                    c.blob(3.0 + a.cos() * 2.5, -4.0 + a.sin() * 1.6, 2.2, col(0xffc2d6));
                }
                c.lw = 1.0;
                c.blob(3.0, -4.0, 1.3, col(0xffe58a));
            }
        }
        Sp::Mushroom => {
            let shroom = |c: &mut Cv, capc: u32| {
                let mut b = Pb::new();
                b.m(-3.0, 0.0);
                b.l(-2.5, -9.0);
                b.l(2.5, -9.0);
                b.l(3.0, 0.0);
                b.close();
                c.fs(&b.done(), col(0xfff0d6));
                let mut b = Pb::new();
                b.ell_arc(0.0, -9.0, 10.0, 8.5, PI, TAU);
                b.close();
                c.fs(&b.done(), col(capc));
            };
            let capc = if v % 2 == 1 { 0xf06c5b } else { 0xc98ae0 };
            shroom(c, capc);
            for (a, b, r) in [(-4.0, -13.0, 1.8), (3.0, -15.0, 1.5), (5.0, -11.0, 1.2)] {
                c.dot(a, b, r, col(0xffffff));
            }
            if v == 2 {
                c.save();
                c.translate(11.0, 1.0);
                c.scale(0.6, 0.6);
                shroom(c, 0xf06c5b);
                c.restore();
            }
        }
        Sp::Bee => {
            c.lw = 1.3;
            c.ell(-1.0, -6.0, 3.5, 4.0, -0.4, cola(0xffffff, 0.85), true);
            c.ell(3.0, -6.0, 3.0, 3.4, 0.4, cola(0xffffff, 0.85), true);
            c.lw = 1.8;
            c.fill(&ellipse(0.0, 0.0, 7.5, 5.5, 0.0), col(0xffd24d));
            c.fill(&Some(PathBuilder::from_rect(SkRect::from_xywh(-3.0, -5.2, 2.2, 10.4).unwrap())), ink);
            c.fill(&Some(PathBuilder::from_rect(SkRect::from_xywh(1.5, -5.4, 2.2, 10.8).unwrap())), ink);
            c.stroke(&ellipse(0.0, 0.0, 7.5, 5.5, 0.0), ink, 1.8);
            c.dot(5.0, -1.0, 1.0, ink);
            c.line(&[(-7.5, 0.0), (-10.0, 0.5)], ink, 1.8);
        }
        Sp::Rabbit => {
            let fur = if v == 3 { col(0xd9b996) } else { col(0xf4eee8) };
            c.blob(-8.0, -8.0, 3.0, col(0xffffff));
            c.ell(0.0, -8.0, 9.0, 7.0, 0.0, fur, true);
            c.save();
            c.translate(6.0, -20.0);
            c.rotate(-0.15);
            c.ell(0.0, -6.0, 2.4, 7.0, 0.0, fur, true);
            c.ell(0.0, -6.0, 1.0, 5.0, 0.0, col(0xffb3c1), false);
            c.restore();
            c.save();
            c.translate(9.5, -19.0);
            c.rotate(0.35);
            c.ell(0.0, -6.0, 2.4, 7.0, 0.0, fur, true);
            c.restore();
            c.blob(6.5, -15.0, 5.8, fur);
            c.eyes(8.5, 8.5, -16.0, 1.1);
            c.dot(11.8, -14.0, 1.1, col(0xff9fb3));
        }
        Sp::Fox => {
            let orange = col(0xf39a4c);
            c.save();
            c.translate(-6.0, -5.0);
            c.rotate(-0.5);
            c.ell(-6.0, 0.0, 8.0, 4.0, 0.0, orange, true);
            c.ell(-12.0, 0.0, 3.0, 2.8, 0.0, col(0xffffff), true);
            c.restore();
            let mut b = Pb::new();
            b.m(-7.0, 0.0);
            b.q(-8.0, -15.0, 0.0, -16.0);
            b.q(8.0, -15.0, 7.0, 0.0);
            b.close();
            c.fs(&b.done(), orange);
            c.ell(1.0, -7.0, 3.5, 5.0, 0.0, col(0xfff4e6), false);
            for (x0, x1, x2) in [(-5.0, -6.0, -1.0), (5.0, 7.0, 2.0)] {
                let mut b = Pb::new();
                b.m(x0, -22.0);
                b.l(x1, -31.0);
                b.l(x2, -25.0);
                b.close();
                c.fs(&b.done(), orange);
            }
            c.blob(0.0, -21.0, 7.0, orange);
            c.ell(2.5, -18.5, 4.0, 2.8, 0.0, col(0xfff4e6), true);
            c.eyes(-2.5, 3.0, -22.0, 1.1);
            c.dot(6.0, -19.0, 1.2, ink);
        }
        Sp::Bird => {
            let (body, wing) = if v == 2 { (0xf59c9c, 0xe87f7f) } else { (0x7fb8f0, 0x5f97d8) };
            c.line(&[(-2.0, -2.0), (-2.0, 0.0)], ink, 1.8);
            c.line(&[(2.0, -2.0), (2.0, 0.0)], ink, 1.8);
            let mut b = Pb::new();
            b.m(-6.0, -10.0);
            b.l(-13.0, -13.0);
            b.l(-12.0, -7.0);
            b.close();
            c.fs(&b.done(), col(wing));
            c.blob(0.0, -10.0, 7.5, col(body));
            c.ell(1.5, -7.0, 4.5, 3.5, 0.0, col(0xfff4df), false);
            c.ell(-2.0, -10.0, 4.5, 3.0, -0.3, col(wing), true);
            let mut b = Pb::new();
            b.m(7.0, -12.0);
            b.l(11.0, -11.0);
            b.l(7.0, -9.5);
            b.close();
            c.fs(&b.done(), col(0xffb347));
            c.dot(3.8, -13.0, 1.1, ink);
        }
        Sp::Frog => {
            c.stroke(&ellipse(0.0, 1.0, 13.0, 4.5, 0.0), cola(0xffffff, 0.6), 1.2);
            let body = if v % 2 == 1 { 0x8fd16f } else { 0xa6d96a };
            c.ell(0.0, -5.0, 10.0, 6.5, 0.0, col(body), true);
            c.blob(-4.5, -11.0, 3.6, col(0xa6d96a));
            c.blob(4.5, -11.0, 3.6, col(0xa6d96a));
            c.lw = 1.0;
            c.blob(-4.5, -11.5, 2.0, col(0xffffff));
            c.blob(4.5, -11.5, 2.0, col(0xffffff));
            c.eyes(-4.2, 4.8, -11.5, 0.9);
            let mut b = Pb::new();
            b.arc(0.0, -6.0, 3.5, 0.3, PI - 0.3);
            c.stroke(&b.done(), ink, 1.4);
            c.dot(-6.0, -5.0, 1.6, cola(0xff8ca0, 0.5));
            c.dot(6.0, -5.0, 1.6, cola(0xff8ca0, 0.5));
        }
        Sp::Crab => {
            for s in [-1.0f32, 1.0] {
                for i in 0..3 {
                    let i = i as f32;
                    c.line(&[(s * 5.0, -4.0 + i), (s * (10.0 + i), -1.0 + i * 1.5)], ink, 1.4);
                }
            }
            c.blob(-11.0, -10.0, 3.8, col(0xf47c5c));
            c.blob(11.0, -10.0, 3.8, col(0xf47c5c));
            c.ell(0.0, -6.0, 9.5, 6.0, 0.0, col(0xf47c5c), true);
            c.line(&[(-2.5, -11.0), (-3.0, -15.0)], ink, 1.8);
            c.line(&[(2.5, -11.0), (3.0, -15.0)], ink, 1.8);
            c.lw = 1.0;
            c.blob(-3.0, -15.5, 1.8, col(0xffffff));
            c.blob(3.0, -15.5, 1.8, col(0xffffff));
            c.eyes(-3.0, 3.0, -15.5, 0.8);
            let mut b = Pb::new();
            b.arc(0.0, -6.0, 2.5, 0.3, PI - 0.3);
            c.stroke(&b.done(), ink, 1.2);
        }
    }
}

fn draw_rock(c: &mut Cv, s: f32) {
    c.ell(0.0, 1.0, 15.0, 5.0, 0.0, cola(0x3c3228, 0.16), false);
    let mut b = Pb::new();
    b.m(-14.0, 0.0);
    b.q(-15.0, -12.0, -5.0, -15.0);
    b.q(4.0, -21.0, 11.0, -12.0);
    b.q(16.0, -5.0, 13.0, 0.0);
    b.close();
    c.fs(&b.done(), col(0xc4bdb4));
    c.ell(-5.0, -11.0, 4.0, 2.0, -0.4, cola(0xffffff, 0.4), false);
    if s > 0.5 {
        let mut b = Pb::new();
        b.m(8.0, 0.0);
        b.q(10.0, -7.0, 16.0, -5.0);
        b.q(20.0, -2.0, 18.0, 1.0);
        b.close();
        c.fs(&b.done(), col(0xb0a89f));
    }
    c.ell(-8.0, -2.0, 4.0, 2.0, 0.0, col(0x8fc870), false);
}

pub fn draw_cloud(c: &mut Cv, dark: bool) {
    c.lw = 2.0;
    let (f, f2) = if dark { (0x8f8a9c, 0xaaa5b8) } else { (0xf5f4fa, 0xffffff) };
    let mut b = Pb::new();
    b.arc(-18.0, 2.0, 13.0, PI * 0.5, PI * 1.5);
    b.arc(-6.0, -10.0, 15.0, PI, PI * 1.85);
    b.arc(13.0, -6.0, 13.0, PI * 1.25, PI * 2.2);
    b.arc(4.0, 6.0, 10.0, 0.0, PI * 0.5);
    b.close();
    c.fs(&b.done(), col(f));
    c.ell(-8.0, -12.0, 7.0, 4.0, -0.3, col(f2), false);
    if dark {
        c.dot(-8.0, 0.0, 1.6, col(INK));
        c.dot(4.0, 0.0, 1.6, col(INK));
        c.line(&[(-11.0, -5.0), (-6.0, -3.0)], col(INK), 1.6);
        c.line(&[(7.0, -5.0), (2.0, -3.0)], col(INK), 1.6);
    }
}

fn draw_bolt(c: &mut Cv) {
    c.lw = 2.0;
    let mut b = Pb::new();
    for (x, y) in [(2.0, -12.0), (-6.0, 2.0), (0.0, 2.0), (-3.0, 14.0), (7.0, -2.0), (1.0, -2.0), (5.0, -12.0)] {
        b.l(x, y);
    }
    b.close();
    c.fs(&b.done(), col(0xffd84a));
}

fn draw_beetle(c: &mut Cv) {
    c.ell(0.0, 2.0, 8.0, 3.0, 0.0, cola(0x281e28, 0.18), false);
    c.translate(0.0, -5.0);
    for sx in [-4.0, 0.0, 4.0] {
        c.line(&[(sx, 0.0), (sx + 1.5, 6.0)], col(INK), 1.3);
        c.line(&[(sx, 0.0), (sx - 1.5, -6.0)], col(INK), 1.3);
    }
    c.lw = 1.8;
    c.ell(0.0, 0.0, 7.5, 5.5, 0.0, col(0x6a4f8a), true);
    c.line(&[(-7.0, 0.0), (5.0, 0.0)], col(INK), 1.8);
    c.blob(8.0, 0.0, 3.4, col(0x45355a));
    c.line(&[(10.0, -1.5), (13.0, -4.0)], col(INK), 1.2);
    c.line(&[(10.0, 1.5), (13.0, 4.0)], col(INK), 1.2);
    c.ell(-2.0, -2.0, 3.0, 1.4, 0.0, cola(0xffffff, 0.35), false);
    c.dot(9.0, -1.0, 0.9, col(0xff6b6b));
    c.dot(9.0, 1.0, 0.9, col(0xff6b6b));
}

fn draw_shield_icon(c: &mut Cv) {
    c.lw = 2.0;
    c.ell(0.0, 6.0, 17.0, 5.0, 0.0, col(0xe7c98f), true);
    let mut b = Pb::new();
    b.m(-15.0, 5.0);
    b.c(-15.0, -22.0, 15.0, -22.0, 15.0, 5.0);
    c.fs(&b.done(), cola(0xaae1ff, 0.55));
    c.ell(-7.0, -8.0, 3.0, 6.0, 0.5, cola(0xffffff, 0.8), false);
    let mut b = Pb::new();
    b.m(0.0, 5.0);
    b.q(1.0, -2.0, 0.0, -5.0);
    c.stroke(&b.done(), col(INK), 1.6);
    c.lw = 1.6;
    c.ell(-3.0, -3.0, 3.0, 1.6, -0.5, col(0x86cc6a), true);
    c.ell(3.0, -5.0, 3.0, 1.6, 0.5, col(0x86cc6a), true);
}

fn draw_face(c: &mut Cv, st: u8) {
    let r = 10.0;
    c.lw = 1.8;
    let f = match st {
        2 => 0x6cc26a,
        1 => 0xf4c64e,
        _ => 0xe96b5f,
    };
    c.blob(0.0, 0.0, r, col(f));
    c.eyes(-r * 0.33, r * 0.33, -r * 0.18, r * 0.12);
    let mut b = Pb::new();
    match st {
        2 => b.arc(0.0, r * 0.05, r * 0.42, 0.25, PI - 0.25),
        0 => b.arc(0.0, r * 0.62, r * 0.38, PI + 0.45, TAU - 0.45),
        _ => {
            b.m(-r * 0.3, r * 0.3);
            b.l(r * 0.3, r * 0.3);
        }
    }
    c.stroke(&b.done(), col(INK), 1.6);
}

pub const TEAM: [(u32, u32); 4] = [(0xff8f7d, 0xd65f4e), (0x72a9f5, 0x4a7fcc), (0xffc94f, 0xcf9719), (0xb893ea, 0x8a63c2)];

fn draw_avatar(c: &mut Cv, i: usize, mood: u8) {
    c.lw = 2.2;
    let ink = col(INK);
    let tc = col(TEAM[i].0);
    match i {
        0 => {
            c.dot(0.0, -13.0, 7.0, ink);
            let mut b = Pb::new();
            b.m(-3.0, -18.0);
            b.q(-6.0, -25.0, -9.0, -24.0);
            b.m(3.0, -18.0);
            b.q(6.0, -25.0, 9.0, -24.0);
            c.stroke(&b.done(), ink, 2.2);
            c.blob(0.0, 2.0, 17.0, tc);
            c.line(&[(0.0, -15.0), (0.0, 19.0)], ink, 2.2);
            for (a, b, r) in [(-8.0, -3.0, 3.0), (7.0, -4.0, 2.6), (-7.0, 9.0, 2.6), (8.0, 8.0, 3.0)] {
                c.dot(a, b, r, ink);
            }
            c.dot(-3.0, -14.0, 2.2, col(0xffffff));
            c.dot(3.0, -14.0, 2.2, col(0xffffff));
            c.dot(-2.6, -13.6, 1.1, ink);
            c.dot(3.4, -13.6, 1.1, ink);
            return;
        }
        1 => {
            let mut b = Pb::new();
            b.m(-5.0, -14.0);
            b.q(-9.0, -24.0, -14.0, -23.0);
            b.m(5.0, -14.0);
            b.q(9.0, -24.0, 14.0, -23.0);
            c.stroke(&b.done(), ink, 2.2);
            c.blob(0.0, 1.0, 18.0, tc);
            c.ell(-8.0, -7.0, 5.0, 3.0, -0.6, cola(0xffffff, 0.45), false);
        }
        2 => {
            c.lw = 1.8;
            c.ell(-13.0, -14.0, 6.0, 8.0, -0.5, cola(0xffffff, 0.9), true);
            c.ell(13.0, -14.0, 6.0, 8.0, 0.5, cola(0xffffff, 0.9), true);
            c.lw = 2.2;
            c.fill(&circle(0.0, 1.0, 18.0), tc);
            // stripes, clipped by drawing thin arcs of the body instead of a clip mask
            for (y, h) in [(11.0f32, 4.0f32), (-12.0, 3.0)] {
                let half = (18.0f32 * 18.0 - (y + h / 2.0 - 1.0).powi(2)).max(0.0).sqrt();
                c.fill(&Some(PathBuilder::from_rect(SkRect::from_xywh(-half, y, half * 2.0, h).unwrap())), ink);
            }
            c.stroke(&circle(0.0, 1.0, 18.0), ink, 2.2);
        }
        _ => {
            c.line(&[(-5.0, -14.0), (-8.0, -24.0)], ink, 2.2);
            c.line(&[(5.0, -14.0), (8.0, -24.0)], ink, 2.2);
            c.dot(-8.0, -25.0, 2.4, ink);
            c.dot(8.0, -25.0, 2.4, ink);
            c.blob(0.0, 1.0, 18.0, tc);
            let mut b = Pb::new();
            let mut a = 0.0f32;
            while a < 10.0 {
                let rr = 1.0 + a * 1.35;
                b.l(9.0 + a.cos() * rr * 0.6, 6.0 + a.sin() * rr * 0.6);
                a += 0.2;
            }
            c.stroke(&b.done(), cola(INK, 0.45), 1.8);
        }
    }
    c.lw = 1.4;
    c.blob(-6.0, -2.0, 4.0, col(0xffffff));
    c.blob(6.0, -2.0, 4.0, col(0xffffff));
    c.eyes(-6.0, 6.0, -2.0, 2.0);
    let mut b = Pb::new();
    match mood {
        2 => b.arc(0.0, 5.0, 5.0, 0.2, PI - 0.2),
        0 => b.arc(0.0, 12.0, 5.0, PI + 0.4, TAU - 0.4),
        _ => {
            b.m(-3.0, 8.0);
            b.l(3.0, 8.0);
        }
    }
    c.stroke(&b.done(), ink, 2.0);
}

fn draw_hand(c: &mut Cv) {
    c.rotate(-0.35);
    c.lw = 2.2;
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
    c.fs(&b.done(), col(0xffffff));
}

fn draw_trophy(c: &mut Cv) {
    c.lw = 2.0;
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU;
        c.blob(a.cos() * 8.0, a.sin() * 8.0, 6.0, col(if i % 2 == 1 { 0xffb3c7 } else { 0xff9fb8 }));
    }
    c.blob(0.0, 0.0, 6.0, col(0xffe58a));
}

fn draw_crown(c: &mut Cv) {
    c.lw = 2.2;
    let mut b = Pb::new();
    for (x, y) in [(-16.0, 8.0), (-18.0, -8.0), (-8.0, 0.0), (0.0, -12.0), (8.0, 0.0), (18.0, -8.0), (16.0, 8.0)] {
        b.l(x, y);
    }
    b.close();
    c.fs(&b.done(), col(0xffd84a));
    c.dot(0.0, 2.0, 2.5, col(0xff8f7d));
}

fn draw_sprout(c: &mut Cv, on: bool) {
    c.lw = 1.8;
    c.line(&[(0.0, 8.0), (0.0, -2.0)], col(INK), 1.8);
    let g = if on { 0x7fc76f } else { 0xc9d6b5 };
    c.ell(-4.0, -4.0, 4.5, 2.4, -0.5, col(g), true);
    c.ell(4.0, -6.0, 4.5, 2.4, 0.5, col(g), true);
}

fn draw_sound(c: &mut Cv, on: bool) {
    let ink = col(INK);
    let mut b = Pb::new();
    for (x, y) in [(-8.5, -3.0), (-4.2, -3.0), (0.8, -7.6), (0.8, 7.6), (-4.2, 3.0), (-8.5, 3.0)] {
        b.l(x, y);
    }
    b.close();
    c.fill(&b.done(), ink);
    if on {
        for r in [5.0, 8.8] {
            let mut b = Pb::new();
            b.arc(0.8, 0.0, r, -0.8, 0.8);
            c.stroke(&b.done(), ink, 2.0);
        }
    } else {
        c.line(&[(3.4, -3.7), (10.2, 3.7)], ink, 2.0);
        c.line(&[(10.2, -3.7), (3.4, 3.7)], ink, 2.0);
    }
}

/* ---------------- island base ---------------- */

fn h2r(h: u32) -> [f32; 3] {
    [((h >> 16) & 255) as f32, ((h >> 8) & 255) as f32, (h & 255) as f32]
}

fn tile_col(t: Terr, lush: f32, shade: f32) -> SkColor {
    let (a, b) = match t {
        Terr::Grass | Terr::Rock => (0xcadb8a, 0x79c261),
        Terr::Mead => (0xdde6a0, 0x9fd86b),
        Terr::Sand => (0xf5e0a6, 0xf5e0a6),
        Terr::Pond => (0x86cde6, 0x86cde6),
    };
    let (a, b) = (h2r(a), h2r(b));
    let f = 1.0 + ((shade * 4.0).round() - 2.0) * 0.018;
    let ch = |i: usize| ((a[i] + (b[i] - a[i]) * lush) * f).clamp(0.0, 255.0) as u8;
    SkColor::from_rgba8(ch(0), ch(1), ch(2), 255)
}

pub const ISLAND_RES: f32 = 2.5;
pub const ISLAND_PAD: f32 = 70.0;

/// Texture box for an island: bounds plus padding.
pub fn island_box(m: &IslandMap) -> (f32, f32, f32, f32) {
    let b = m.b;
    (b.x1 - b.x0 + ISLAND_PAD * 2.0, b.y1 - b.y0 + ISLAND_PAD * 2.0, -b.x0 + ISLAND_PAD, -b.y0 + ISLAND_PAD)
}

/// Shallow water and foam around the island (drawn under it, gently pulsing).
pub fn island_sea_tex(images: &mut Assets<Image>, m: &IslandMap) -> Tex {
    let (w, h, ox, oy) = island_box(m);
    make(images, w, h, ox, oy, ISLAND_RES, |c| {
        for &i in &m.edge {
            let t = &m.tiles[i];
            c.fill(&hex_path(t.x, t.y + 12.0, HS * 1.85), cola(0xc4f0e4, 0.6));
        }
        for &i in &m.edge {
            let t = &m.tiles[i];
            c.fill(&hex_path(t.x, t.y + 12.0, HS * 1.3 + (t.shade - 0.5) * 4.0), cola(0xfffff8, 0.8));
        }
    })
}

/// The island itself: cliffs, terrain, pond shores, little tufts and outline.
pub fn island_tex(images: &mut Assets<Image>, m: &IslandMap, lush: f32) -> Tex {
    let (w, h, ox, oy) = island_box(m);
    make(images, w, h, ox, oy, ISLAND_RES, |c| {
        let ink = col(INK);
        for t in &m.tiles {
            c.fill(&hex_path(t.x, t.y + 18.0, HS * 1.04), col(0xb88664));
        }
        for t in &m.tiles {
            c.fill(&hex_path(t.x, t.y + 9.0, HS * 1.04), col(0xe0b183));
        }
        let edge_seg = |pb: &mut Pb, t: &crate::island::Tile, i: usize, r: f32, dy: f32| {
            let (a, b) = (hex_vertex(i), hex_vertex((i + 1) % 6));
            pb.m(t.x + a.0 * r, t.y + dy + a.1 * r);
            pb.l(t.x + b.0 * r, t.y + dy + b.1 * r);
        };
        let mut pb = Pb::new();
        for t in &m.tiles {
            for i in 0..3 {
                let d = EDGE_DIR[i];
                if !m.has(t.q + d.0, t.r + d.1) {
                    edge_seg(&mut pb, t, i, HS * 1.04, 18.0);
                }
            }
        }
        c.stroke(&pb.done(), ink, 2.0);
        for t in &m.tiles {
            c.fill(&hex_path(t.x, t.y, HS * 1.04), tile_col(t.t, lush, t.shade));
        }
        let mut pb = Pb::new();
        for t in m.tiles.iter().filter(|t| t.t == Terr::Pond) {
            for i in 0..6 {
                let d = EDGE_DIR[i];
                if m.get(t.q + d.0, t.r + d.1).map(|n| n.t != Terr::Pond).unwrap_or(true) {
                    edge_seg(&mut pb, t, i, HS * 0.98, 0.0);
                }
            }
        }
        c.stroke(&pb.done(), col(0xe9d9a2), 5.0);
        for t in m.tiles.iter().filter(|t| t.t == Terr::Pond) {
            let k = t.shade;
            c.stroke(&ellipse(t.x + (t.shade - 0.5) * 14.0, t.y + 3.0, 5.0 + k * 10.0, (5.0 + k * 10.0) * 0.4, 0.0), cola(0xb3e3f2, 0.8), 1.6);
        }
        for t in &m.tiles {
            for d in &t.deco {
                let (x, y) = (t.x + d.x, t.y + d.y);
                match d.kind {
                    DecoKind::Tuft => {
                        let tuft = if lush > 0.5 { cola(0x3c7a2a, 0.45) } else { cola(0x46783a, 0.45) };
                        c.line(&[(x - 3.0 * d.s, y - 3.0 * d.s), (x, y), (x, y - 4.0 * d.s)], tuft, 1.3);
                        c.line(&[(x, y), (x + 3.0 * d.s, y - 3.0 * d.s)], tuft, 1.3);
                    }
                    DecoKind::Dot => c.dot(x, y, 1.6 * d.s, col(d.col)),
                    DecoKind::Pebble => c.fill(&ellipse(x, y, 2.2 * d.s, 1.3 * d.s, 0.0), cola(0xbea06e, 0.5)),
                    DecoKind::Shell => {
                        let mut b = Pb::new();
                        b.arc(x, y, 2.4 * d.s, PI, TAU);
                        b.close();
                        c.stroke(&b.done(), cola(0xe68c82, 0.8), 1.3);
                    }
                }
            }
        }
        let mut pb = Pb::new();
        for t in &m.tiles {
            for i in 0..6 {
                let d = EDGE_DIR[i];
                if !m.has(t.q + d.0, t.r + d.1) {
                    edge_seg(&mut pb, t, i, HS * 1.04, 0.0);
                }
            }
        }
        c.stroke(&pb.done(), ink, 2.0);
    })
}
