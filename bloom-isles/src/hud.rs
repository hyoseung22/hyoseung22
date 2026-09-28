//! Screen-space interface: layout, HUD, title, pause and end screens, tutorial hand.
//! Drawn immediate-mode: every frame spawns short-lived sprites.

use crate::art::{bc, bca, pal, Art, Tex, TEAM};
use crate::eco::def;
use crate::input::{BtnId, Pointer};
use crate::island::{Terr, HS, SQ};
use crate::rng::Rng;
use crate::sim::{self, ease_back, ease_io, lerp, state_of, Card, Ev, Game, Link, Sfx, Weather, DUR, REROLL_CD, WITHER_T};
use crate::eco::need_w;
use crate::world::{ground_h, Icons, MainView};
use crate::{Ephemeral, Fonts, Pending, Scene, Session, Settings, LAYER_HUD};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::{Anchor, BorderRect, SliceScaleMode, SpriteImageMode, TextureSlicer};
use bevy::window::PrimaryWindow;
use std::f32::consts::{PI, TAU};

/// Layer drawn above the island portraits (end-screen podium, flying attacks, tutorial hand).
pub const LAYER_TOP: usize = 4;

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct HudGizmos;

#[derive(Clone, Copy, Default, Debug)]
pub struct R {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl R {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        R { x, y, w, h }
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Btn {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

impl Btn {
    pub fn hit(&self, p: Vec2) -> bool {
        self.r > 0.0 && p.distance(Vec2::new(self.x, self.y)) <= self.r + 4.0
    }
    pub fn c(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

#[derive(Resource, Default, Clone)]
pub struct Layout {
    pub w: f32,
    pub h: f32,
    pub port: bool,
    pub p: [R; 4],
    pub play: R,
    pub cards: [R; 4],
    pub reroll: Btn,
    pub shovel: Btn,
    pub home: Btn,
    pub isle: R,
    pub timer: Btn,
    pub pause: Btn,
    pub mute: Btn,
    pub race: R,
    pub title_isle: R,
    pub logo_y: f32,
    pub logo_size: f32,
    pub title_play: Btn,
    pub title_dice: Btn,
    pub title_diff: [R; 3],
    pub title_quit: Btn,
    pub end_panels: Vec<Option<R>>,
    pub end_again: Btn,
    pub end_home: Btn,
    pub pause_resume: Btn,
    pub pause_home: Btn,
}

pub fn update_layout(window: Single<&Window, With<PrimaryWindow>>, session: Res<Session>, mut l: ResMut<Layout>) {
    let (w, h) = (window.width().max(200.0), window.height().max(200.0));
    let port = w < h * 0.95;
    let top = 58.0;
    l.w = w;
    l.h = h;
    l.port = port;
    if !port {
        let mut pw = (w * 0.17).clamp(118.0, 200.0);
        let mut ph = pw * 0.72;
        let room = h - top - 18.0;
        if ph * 4.0 + 30.0 > room {
            ph = (room - 30.0) / 4.0;
            pw = ph / 0.72;
        }
        for i in 0..4 {
            l.p[i] = R::new(w - pw - 14.0, top + 4.0 + i as f32 * (ph + 10.0), pw, ph);
        }
        l.play = R::new(0.0, top, w - pw - 28.0, h - top);
    } else {
        let pw = (w - 28.0 - 24.0) / 4.0;
        let ph = pw * 0.72;
        for i in 0..4 {
            l.p[i] = R::new(14.0 + i as f32 * (pw + 8.0), top + 2.0, pw, ph);
        }
        l.play = R::new(0.0, top + ph + 10.0, w, h - (top + ph + 10.0));
    }
    let pl = l.play;
    let cw = (pl.w * 0.15).min(h * 0.11).clamp(56.0, 94.0);
    let ch = cw * 1.3;
    let gap = cw * 0.12;
    let hand_w = cw * 4.0 + gap * 3.0;
    let cy = h - ch - 14.0;
    for i in 0..4 {
        l.cards[i] = R::new(pl.x + pl.w / 2.0 - hand_w / 2.0 + i as f32 * (cw + gap), cy, cw, ch);
    }
    let br = cw * 0.34;
    l.reroll = Btn { x: l.cards[0].x - br - gap * 1.6, y: cy + ch * 0.5, r: br };
    l.shovel = Btn { x: l.cards[3].x + cw + br + gap * 1.6, y: cy + ch * 0.5, r: br };
    l.home = Btn { x: l.reroll.x, y: cy - br * 1.4 - 10.0, r: br * 1.05 };
    l.isle = R::new(pl.x + 10.0, pl.y + 6.0, pl.w - 20.0, cy - pl.y - 8.0);
    l.timer = Btn { x: 34.0, y: 30.0, r: 19.0 };
    l.pause = Btn { x: w - 30.0, y: 30.0, r: 17.0 };
    l.mute = Btn { x: w - 70.0, y: 30.0, r: 17.0 };
    let rx0 = 112.0;
    let rx1 = if port { w - 100.0 } else { (pl.x + pl.w - 20.0).min(w - 110.0) };
    l.race = R::new(rx0, 30.0, (rx1 - rx0 - 28.0).max(80.0), 14.0);

    // title
    l.logo_y = if port { h * 0.13 } else { h * 0.15 };
    l.logo_size = (w * 0.085).clamp(38.0, 82.0);
    let tb = if port { 170.0 } else { 150.0 };
    l.title_isle = R::new(w * 0.08, l.logo_y + 60.0, w * 0.84, h - l.logo_y - 60.0 - tb);
    let by = h - if port { 110.0 } else { 90.0 };
    let pr = if port { 42.0 } else { 44.0 };
    l.title_play = Btn { x: w / 2.0, y: by, r: pr };
    l.title_dice = Btn { x: w / 2.0 - pr - 56.0, y: by, r: 28.0 };
    for i in 0..3 {
        l.title_diff[i] = R::new(w / 2.0 + pr + 30.0, by - 48.0 + i as f32 * 34.0, 62.0, 28.0);
    }
    l.title_quit = Btn { x: 30.0, y: 30.0, r: 17.0 };

    l.pause_resume = Btn { x: w / 2.0, y: h / 2.0, r: 46.0 };
    l.pause_home = Btn { x: w / 2.0 - 100.0, y: h / 2.0, r: 28.0 };

    // end screen: a podium. 2nd | 1st | 3rd, and 4th off to the side.
    l.end_panels = vec![None; 4];
    if let (Scene::End, Some(g)) = (session.scene, session.game.as_ref()) {
        let gap = 24.0;
        let pw = if port { ((w - 60.0) / 2.0).clamp(110.0, 210.0) } else { (w * 0.17).clamp(120.0, 230.0).min((w - 80.0 - gap * 3.0) / 4.4) };
        let ph = pw * 0.72;
        for (rank, &p) in g.ranks.iter().enumerate() {
            let big = if rank == 0 { 1.2 } else { 1.0 };
            let (bw, bh) = (pw * big, ph * big);
            let (sx, sy) = if port {
                match rank {
                    0 => (w / 2.0 - bw / 2.0, h * 0.14),
                    1 => (w / 2.0 - pw - gap / 2.0, h * 0.14 + ph * 1.2 + 90.0),
                    2 => (w / 2.0 + gap / 2.0, h * 0.14 + ph * 1.2 + 110.0),
                    _ => (w / 2.0 - pw / 2.0, h * 0.14 + ph * 2.2 + 200.0),
                }
            } else {
                let total = pw * 4.2 + gap * 3.0;
                let x0 = w / 2.0 - total / 2.0;
                match rank {
                    0 => (x0 + pw + gap, h * 0.2),
                    1 => (x0, h * 0.27),
                    2 => (x0 + pw * 2.2 + gap * 2.0, h * 0.3),
                    _ => (x0 + pw * 3.2 + gap * 3.0, h * 0.34),
                }
            };
            let d = ((g.end_t - sim::REVEAL_AT[rank]) / 0.45).clamp(0.0, 1.0);
            if d > 0.0 {
                l.end_panels[p] = Some(R::new(sx, sy + (1.0 - ease_back(d)) * 80.0, bw, bh));
            }
        }
        let by = h - 60.0;
        l.end_again = Btn { x: w / 2.0 + 44.0, y: by, r: 34.0 };
        l.end_home = Btn { x: w / 2.0 - 44.0, y: by, r: 26.0 };
    }
}

/* ---------------- painter ---------------- */

pub struct Pen<'a, 'w, 's> {
    pub cmd: &'a mut Commands<'w, 's>,
    pub w: f32,
    pub h: f32,
    pub z: f32,
    pub layer: usize,
    pub font: Handle<Font>,
}

impl Pen<'_, '_, '_> {
    fn spawn(&mut self, sprite: Sprite, x: f32, y: f32, rot: f32, scale: Vec2) {
        self.z += 0.01;
        self.cmd.spawn((
            sprite,
            Transform::from_xyz(x - self.w / 2.0, self.h / 2.0 - y, self.z).with_rotation(Quat::from_rotation_z(-rot)).with_scale(scale.extend(1.0)),
            RenderLayers::layer(self.layer),
            Ephemeral,
        ));
    }
    /// Texture placed by its own anchor point.
    pub fn tex(&mut self, t: &Tex, x: f32, y: f32, s: f32, color: Color) {
        self.tex_full(t, x, y, Vec2::splat(s), 0.0, false, color);
    }
    #[allow(clippy::too_many_arguments)]
    pub fn tex_full(&mut self, t: &Tex, x: f32, y: f32, s: Vec2, rot: f32, flip: bool, color: Color) {
        let spr = Sprite { image: t.h.clone(), custom_size: Some(t.size), anchor: Anchor::Custom(t.anchor), color, flip_x: flip, ..default() };
        self.spawn(spr, x, y, rot, s);
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, art: &Art, color: Color) {
        let spr = Sprite { image: art.pixel.h.clone(), custom_size: Some(Vec2::new(w, h)), anchor: Anchor::TopLeft, color, ..default() };
        self.spawn(spr, x, y, 0.0, Vec2::ONE);
    }
    #[allow(clippy::too_many_arguments)]
    fn sliced(&mut self, tex: &Tex, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
        let k = 14.0 / radius.max(1.0);
        let spr = Sprite {
            image: tex.h.clone(),
            custom_size: Some(Vec2::new(w * k, h * k)),
            anchor: Anchor::TopLeft,
            color,
            image_mode: SpriteImageMode::Sliced(TextureSlicer {
                border: BorderRect { left: 16.0, right: 16.0, top: 16.0, bottom: 16.0 },
                center_scale_mode: SliceScaleMode::Stretch,
                sides_scale_mode: SliceScaleMode::Stretch,
                max_corner_scale: 1.0,
            }),
            ..default()
        };
        self.spawn(spr, x, y, 0.0, Vec2::splat(1.0 / k));
    }
    #[allow(clippy::too_many_arguments)]
    pub fn rrect(&mut self, art: &Art, x: f32, y: f32, w: f32, h: f32, radius: f32, fill: Option<Color>, line: Option<Color>) {
        if let Some(f) = fill {
            self.sliced(&art.rrect_fill, x, y, w, h, radius, f);
        }
        if let Some(l) = line {
            self.sliced(&art.rrect_line, x, y, w, h, radius, l);
        }
    }
    pub fn circle(&mut self, art: &Art, x: f32, y: f32, r: f32, color: Color) {
        self.tex(&art.circle, x, y, r / 15.5, color);
    }
    pub fn ring(&mut self, art: &Art, x: f32, y: f32, r: f32, color: Color) {
        self.tex(&art.ring, x, y, r / 30.0, color);
    }
    /// Round button: soft shadow, fill, faint rim.
    pub fn button(&mut self, art: &Art, b: Btn, fill: Color, hot: bool) {
        let r = b.r * if hot { 1.07 } else { 1.0 };
        self.circle(art, b.x, b.y + 3.0, b.r, Color::srgba(0.02, 0.06, 0.08, 0.35));
        self.circle(art, b.x, b.y, r, fill);
        self.ring(art, b.x, b.y, r - 1.0, bca(pal::CREAM, if hot { 0.5 } else { 0.18 }));
    }
    /// Centred text with a dark drop shadow.
    pub fn text(&mut self, x: f32, y: f32, size: f32, color: Color, s: &str) {
        for (dx, dy, c) in [(0.0, size * 0.08, bca(pal::SLATE, 0.8 * color.alpha())), (0.0, 0.0, color)] {
            self.z += 0.01;
            self.cmd.spawn((
                Text2d::new(s),
                TextFont { font: self.font.clone(), font_size: 64.0, ..default() },
                TextColor(c),
                Transform::from_xyz(x + dx - self.w / 2.0, self.h / 2.0 - y - dy, self.z).with_scale(Vec3::splat(size / 64.0)),
                RenderLayers::layer(self.layer),
                Ephemeral,
            ));
        }
    }
}

fn to_screen(w: f32, h: f32, p: Vec2) -> Vec2 {
    Vec2::new(p.x - w / 2.0, h / 2.0 - p.y)
}

#[allow(clippy::too_many_arguments)]
fn arc(g: &mut Gizmos<HudGizmos>, w: f32, h: f32, c: Vec2, r: f32, a0: f32, frac: f32, color: Color) {
    if frac <= 0.001 {
        return;
    }
    let n = (48.0 * frac).ceil().max(2.0) as usize;
    let pts: Vec<Vec2> = (0..=n)
        .map(|i| {
            let a = a0 + frac * TAU * i as f32 / n as f32;
            to_screen(w, h, c + Vec2::new(a.cos(), a.sin()) * r)
        })
        .collect();
    g.linestrip_2d(pts, color);
}

/* ---------------- screen-space effects ---------------- */

pub struct Confetto {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r: f32,
    vr: f32,
    c: u32,
}

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max: f32,
    c: u32,
}

struct Rocket {
    x: f32,
    y: f32,
    ty: f32,
    vy: f32,
    c: u32,
}

#[derive(Resource, Default)]
pub struct HudState {
    confetti: Vec<Confetto>,
    pending_win: Option<bool>,
    sparks: Vec<Spark>,
    rockets: Vec<Rocket>,
    next_rocket: f32,
    /// (moved up?, seconds left)
    rank_flash: Option<(bool, f32)>,
    crowd_seed: u32,
    popups: Vec<(crate::world::PopupReq, f32)>,
    /// (island, kind, from, to, ok, age)
    links: Vec<(usize, Link, Vec2, Vec2, bool, f32)>,
    /// (island, where, age)
    fears: Vec<(usize, Vec2, f32)>,
    /// A weather change being announced: (weather, age)
    banner: Option<(Weather, f32)>,
}

impl HudState {
    pub fn start_confetti(&mut self, win: bool) {
        self.pending_win = Some(win);
        self.next_rocket = 0.0;
        self.rockets.clear();
        self.sparks.clear();
    }
}

#[derive(Component)]
pub struct Logo {
    dx: f32,
    dy: f32,
}

pub fn setup_title_text(mut commands: Commands, fonts: Res<Fonts>) {
    let size = 80.0;
    let word = "Bloom Isles";
    let mut offsets: Vec<(f32, f32, bool)> = (0..16)
        .map(|i| {
            let a = i as f32 / 16.0 * TAU;
            (a.cos() * 5.0, a.sin() * 5.0 + 7.0, false)
        })
        .collect();
    offsets.extend((0..16).map(|i| {
        let a = i as f32 / 16.0 * TAU;
        (a.cos() * 5.0, a.sin() * 5.0, false)
    }));
    offsets.push((0.0, 0.0, true));
    for (k, (dx, dy, main)) in offsets.into_iter().enumerate() {
        let font = TextFont { font: fonts.title.clone(), font_size: size, ..default() };
        commands
            .spawn((
                Text2d::new(""),
                font.clone(),
                TextLayout::new_with_justify(JustifyText::Center),
                Transform::from_xyz(0.0, 0.0, 50.0 + k as f32 * 0.01),
                RenderLayers::layer(LAYER_HUD),
                Visibility::Hidden,
                Logo { dx, dy },
            ))
            .with_children(|p| {
                for (i, ch) in word.chars().enumerate() {
                    let c = if !main { bc(pal::SLATE) } else if i < 5 { bc(pal::GOLD) } else { bc(pal::CREAM) };
                    p.spawn((TextSpan::new(ch.to_string()), font.clone(), TextColor(c)));
                }
            });
    }
}

/* ---------------- the big draw ---------------- */

#[allow(clippy::too_many_arguments)]
pub fn draw_hud(
    mut commands: Commands,
    session: Res<Session>,
    settings: Res<Settings>,
    art: Res<Art>,
    icons: Res<Icons>,
    fonts: Res<Fonts>,
    layout: Res<Layout>,
    view: Res<MainView>,
    pointer: Res<Pointer>,
    time: Res<Time>,
    mut pending: ResMut<Pending>,
    mut state: ResMut<HudState>,
    mut g: Gizmos<HudGizmos>,
    mut logo: Query<(&Logo, &mut Transform, &mut Visibility)>,
    mut rng: Local<Option<Rng>>,
    gallery: Option<Res<crate::shots::Gallery>>,
) {
    let rng = rng.get_or_insert_with(Rng::from_time);
    let dt = time.delta_secs().min(0.05);
    if gallery.is_some() {
        let mut pen = Pen { cmd: &mut commands, w: layout.w, h: layout.h, z: 30.0, layer: LAYER_TOP, font: fonts.title.clone() };
        draw_gallery(&mut pen, &art, &icons, &layout);
        for (_, _, mut vis) in &mut logo {
            *vis = Visibility::Hidden;
        }
        return;
    }
    let l = &*layout;
    let mut pen = Pen { cmd: &mut commands, w: l.w, h: l.h, z: 10.0, layer: LAYER_HUD, font: fonts.title.clone() };
    let t = session.t;
    let title = session.scene == Scene::Title;
    for (lg, mut tf, mut vis) in &mut logo {
        *vis = if title { Visibility::Visible } else { Visibility::Hidden };
        if title {
            let s = l.logo_size / 80.0;
            let p = to_screen(l.w, l.h, Vec2::new(l.w / 2.0 + lg.dx * s, l.logo_y + lg.dy * s + (t * 1.6).sin() * 2.0));
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            tf.scale = Vec3::splat(s);
        }
    }
    // rank changes flash the player's marker
    for e in pending.ev.iter() {
        if let Ev::Rank { up } = e {
            state.rank_flash = Some((*up, 1.2));
        }
    }
    pending.ev.retain(|e| !matches!(e, Ev::Rank { .. }));
    for e in pending.ev.iter() {
        match e {
            Ev::Link { isl, kind, ax, ay, bx, by, ok } => state.links.push((*isl, *kind, Vec2::new(*ax, *ay), Vec2::new(*bx, *by), *ok, 0.0)),
            Ev::Fear { isl, x, y } => state.fears.push((*isl, Vec2::new(*x, *y), 0.0)),
            Ev::Weather(w) => state.banner = Some((*w, 0.0)),
            _ => {}
        }
    }
    pending.ev.retain(|e| !matches!(e, Ev::Link { .. } | Ev::Fear { .. } | Ev::Weather(_)));
    for l in &mut state.links {
        l.5 += dt;
    }
    state.links.retain(|l| l.5 < 1.3);
    if state.links.len() > 120 {
        let n = state.links.len() - 120;
        state.links.drain(..n);
    }
    for f in &mut state.fears {
        f.2 += dt;
    }
    state.fears.retain(|f| f.2 < 1.4);
    if let Some(b) = state.banner.as_mut() {
        b.1 += dt;
    }
    if state.banner.map(|b| b.1 > 2.6).unwrap_or(false) {
        state.banner = None;
    }
    let new_pops: Vec<_> = pending.popups.drain(..).map(|p| (p, 0.0)).collect();
    state.popups.extend(new_pops);
    for p in &mut state.popups {
        p.1 += dt;
    }
    state.popups.retain(|p| p.1 < p.0.max);
    if let Some((_, left)) = state.rank_flash.as_mut() {
        *left -= dt;
    }
    if state.rank_flash.map(|f| f.1 <= 0.0).unwrap_or(false) {
        state.rank_flash = None;
    }
    if let Some(win) = state.pending_win.take() {
        state.confetti.clear();
        state.crowd_seed = rng.next_u32();
        if win {
            for _ in 0..160 {
                state.confetti.push(Confetto {
                    x: rng.f() * l.w,
                    y: -rng.f() * l.h * 0.8,
                    vx: (rng.f() - 0.5) * 60.0,
                    vy: 70.0 + rng.f() * 120.0,
                    r: rng.f() * TAU,
                    vr: (rng.f() - 0.5) * 8.0,
                    c: *rng.pick(&[TEAM[0].0, pal::GOLD, 0x5fb04a, TEAM[1].0, TEAM[3].0, pal::CREAM]),
                });
            }
        }
    }
    match session.scene {
        Scene::Title => draw_title(&mut pen, &art, &icons, l, &settings, &pointer, t, session.dice_t),
        Scene::Play | Scene::End => {
            let Some(gm) = session.game.as_ref() else { return };
            if session.scene == Scene::Play {
                draw_play(&mut pen, &mut g, &art, &icons, l, gm, &view, &pointer, &settings, &state, t);
                if gm.paused {
                    pen.layer = LAYER_TOP;
                    pen.rect(0.0, 0.0, l.w, l.h, &art, bca(pal::SLATE, 0.62));
                    pen.button(&art, l.pause_resume, bc(TEAM[0].0), pointer.hover == Some(BtnId::Resume));
                    pen.tex(&art.icon_play, l.pause_resume.x + 3.0, l.pause_resume.y, 2.2, Color::WHITE);
                    pen.button(&art, l.pause_home, bc(pal::SLATE2), pointer.hover == Some(BtnId::PauseHome));
                    pen.tex(&art.icon_home, l.pause_home.x, l.pause_home.y, 1.6, Color::WHITE);
                }
            } else {
                update_show(&mut state, gm, l, rng, dt, &mut pending);
                draw_end(&mut pen, &art, &icons, l, gm, &pointer, &state, t);
            }
        }
    }
    pen.layer = LAYER_TOP;
    pen.z = 90.0;
    pen.button(&art, l.mute, bc(pal::SLATE), pointer.hover == Some(BtnId::Mute));
    pen.tex(if settings.muted { &art.icon_sound_off } else { &art.icon_sound_on }, l.mute.x, l.mute.y, 1.25, Color::WHITE);
}

#[allow(clippy::too_many_arguments)]
fn draw_title(pen: &mut Pen, art: &Art, icons: &Icons, l: &Layout, settings: &Settings, pointer: &Pointer, t: f32, dice_t: f32) {
    let ay = l.logo_y + l.logo_size * 0.72;
    for i in 0..4 {
        let hop = ((t * 2.2 + i as f32 * 0.7).sin()).max(0.0) * 6.0;
        pen.tex(&icons.pawns[i][1], l.w / 2.0 + (i as f32 - 1.5) * 42.0, ay - hop, if i == 0 { 0.95 } else { 0.8 }, Color::WHITE);
    }
    let p = l.title_play;
    let pulse = Btn { r: p.r * (1.0 + (t * 3.0).sin() * 0.025), ..p };
    pen.button(art, pulse, bc(TEAM[0].0), pointer.hover == Some(BtnId::Play));
    pen.tex(&art.icon_play, p.x + 4.0, p.y, p.r / 16.0, Color::WHITE);
    let d = l.title_dice;
    pen.button(art, d, bc(pal::SLATE), pointer.hover == Some(BtnId::Dice));
    pen.tex_full(&art.dice, d.x, d.y, Vec2::ONE, ((dice_t * 8.0).min(TAU)).sin() * 0.3, false, Color::WHITE);
    for (i, r) in l.title_diff.iter().enumerate() {
        let on = settings.diff == i;
        pen.rrect(art, r.x, r.y, r.w, r.h, 14.0, Some(if on { bc(pal::SLATE) } else { bca(pal::SLATE, 0.55) }), Some(if on { bc(pal::GOLD) } else { bca(pal::CREAM, 0.15) }));
        for k in 0..=i {
            let x = r.x + r.w / 2.0 + (k as f32 - i as f32 / 2.0) * 12.0;
            pen.tex(if on { &art.sprout_on } else { &art.sprout_off }, x, r.y + r.h / 2.0, 0.9, Color::WHITE);
        }
    }
    let q = l.title_quit;
    pen.button(art, q, bc(pal::SLATE), pointer.hover == Some(BtnId::Quit));
    pen.tex(&art.icon_quit, q.x, q.y, 1.0, Color::WHITE);
}

#[allow(clippy::too_many_arguments)]
fn card_icon(pen: &mut Pen, art: &Art, icons: &Icons, card: Card, x: f32, y: f32, s: f32, alpha: f32) {
    let wc = Color::WHITE.with_alpha(alpha);
    let fit = |t: &Tex, w: f32| w / t.size.x.max(1.0);
    match card {
        Card::Nature(sp) => {
            let t = &icons.sp[sp.idx()];
            pen.tex(t, x, y, fit(t, 52.0 * s), wc);
        }
        Card::Storm => {
            pen.tex(&icons.storm, x, y - 6.0 * s, fit(&icons.storm, 52.0 * s), wc);
            pen.tex(&art.bolt, x + 2.0 * s, y + 14.0 * s, s * 0.8, wc);
        }
        Card::Beetle => {
            let t = &icons.beetle;
            pen.tex(t, x - 9.0 * s, y + 8.0 * s, fit(t, 30.0 * s), wc);
            pen.tex(t, x + 10.0 * s, y - 1.0 * s, fit(t, 26.0 * s), wc);
            pen.tex(t, x - 2.0 * s, y - 12.0 * s, fit(t, 22.0 * s), wc);
        }
        Card::Shield => pen.tex(&art.shield, x, y + 4.0 * s, s * 1.2, wc),
    }
}

fn portrait_center(l: &Layout, i: usize) -> Vec2 {
    l.p[i].center()
}

#[allow(clippy::too_many_arguments)]
fn draw_play(pen: &mut Pen, g: &mut Gizmos<HudGizmos>, art: &Art, icons: &Icons, l: &Layout, gm: &Game, view: &MainView, pointer: &Pointer, settings: &Settings, state: &HudState, t: f32) {
    let (w, h) = (l.w, l.h);
    let slate = bc(pal::SLATE);
    let me = &gm.players[0];
    draw_weather_fx(pen, g, art, l, gm, t);
    draw_world_overlays(pen, g, art, icons, l, gm, view, pointer, state, t);

    if gm.view != 0 {
        let a = l.isle;
        pen.rrect(art, a.x + 4.0, a.y + 4.0, a.w - 8.0, a.h - 8.0, 22.0, None, Some(bca(TEAM[gm.view].0, 0.8)));
        pen.rrect(art, a.x + 6.0, a.y + 6.0, a.w - 12.0, a.h - 12.0, 20.0, None, Some(bca(TEAM[gm.view].0, 0.8)));
        pen.tex(&icons.pawns[gm.view][1], a.x + 34.0, a.y + 38.0, 1.0, Color::WHITE);
    }
    if gm.fade > 0.0 {
        let a = l.isle;
        pen.rect(a.x, a.y, a.w, a.h, art, bca(pal::SEA, gm.fade));
    }
    if gm.flash > 0.0 {
        pen.rect(0.0, 0.0, w, h, art, Color::srgba(1.0, 1.0, 0.92, (gm.flash * 1.6).min(1.0)));
    }
    if gm.alert > 0.0 {
        let c = bca(pal::RED, gm.alert * if gm.view != 0 { 0.4 } else { 0.28 });
        let b = 14.0;
        pen.rect(0.0, 0.0, w, b, art, c);
        pen.rect(0.0, h - b, w, b, art, c);
        pen.rect(0.0, b, b, h - 2.0 * b, art, c);
        pen.rect(w - b, b, b, h - 2.0 * b, art, c);
    }

    // sun timer
    let tm = l.timer;
    let left = (1.0 - gm.t / DUR).clamp(0.0, 1.0);
    pen.button(art, tm, slate, false);
    let warm = if left < 0.12 && (t * 8.0).sin() > 0.0 { bc(pal::RED) } else { bc(pal::GOLD) };
    arc(g, w, h, tm.c(), tm.r - 7.0, PI / 2.0, left, warm);
    arc(g, w, h, tm.c(), tm.r - 10.0, PI / 2.0, left, warm);
    let sa = -PI / 2.0 - left * TAU;
    let sun = tm.c() + Vec2::new(sa.cos(), -sa.sin()) * (tm.r - 5.0);
    pen.circle(art, sun.x, sun.y, 4.5, bc(0xffe08a));

    // weather now, and what comes next when it is close
    let wi = |w: Weather| match w {
        Weather::Clear => 0,
        Weather::Rain => 1,
        Weather::Drought => 2,
        Weather::Wind => 3,
    };
    let wb = Btn { x: tm.x + 40.0, y: tm.y, r: 16.0 };
    pen.button(art, wb, slate, false);
    pen.tex(&art.weather[wi(gm.weather)], wb.x, wb.y, 0.8, Color::WHITE);
    if gm.weather_t < 8.0 {
        let blink = if (t * 4.0).sin() > 0.0 { 1.0 } else { 0.5 };
        pen.circle(art, wb.x + 14.0, wb.y + 14.0, 10.0, bca(pal::SLATE, 0.95));
        pen.tex(&art.weather[wi(gm.forecast)], wb.x + 14.0, wb.y + 14.0, 0.45, Color::WHITE.with_alpha(blink));
    }
    if let Some((w, age)) = state.banner {
        let pop = if age < 0.3 { ease_back(age / 0.3) } else { 1.0 };
        let a = if age > 2.0 { (2.6 - age) / 0.6 } else { 1.0 };
        let c = Vec2::new(l.isle.x + l.isle.w / 2.0, l.isle.y + 60.0);
        pen.circle(art, c.x, c.y, 34.0 * pop, bca(pal::SLATE, 0.85 * a));
        pen.tex(&art.weather[wi(w)], c.x, c.y, 1.7 * pop, Color::WHITE.with_alpha(a));
    }

    // race to the flag
    let rc = l.race;
    pen.rrect(art, rc.x, rc.y - 4.0, rc.w, 14.0, 7.0, Some(Color::srgba(0.02, 0.06, 0.08, 0.3)), None);
    pen.rrect(art, rc.x, rc.y - 7.0, rc.w, 14.0, 7.0, Some(slate), Some(bca(pal::CREAM, 0.15)));
    for i in 1..4 {
        pen.circle(art, rc.x + rc.w * i as f32 / 4.0, rc.y, 2.0, bca(pal::CREAM, 0.25));
    }
    let kme = (gm.isl[0].ds / sim::GOAL).clamp(0.0, 1.0);
    pen.rrect(art, rc.x + 3.0, rc.y - 3.0, ((rc.w - 6.0) * kme).max(8.0), 6.0, 3.0, Some(bca(TEAM[0].0, 0.7)), None);
    pen.tex(&art.trophy, rc.x + rc.w + 12.0, rc.y + 4.0, 1.0, Color::WHITE);
    for i in [1usize, 2, 3, 0] {
        let k = (gm.isl[i].ds / sim::GOAL).clamp(0.0, 1.0);
        let x = rc.x + 8.0 + (rc.w - 16.0) * k;
        let y = rc.y + if i == 0 { -2.0 } else { (i as f32 - 2.0) * 4.0 - 2.0 };
        if i == 0 {
            if let Some((up, left)) = state.rank_flash {
                let k = 1.0 - left / 1.2;
                pen.ring(art, x, y, 12.0 + k * 26.0, bca(if up { pal::GREEN } else { pal::RED }, 1.0 - k));
                pen.ring(art, x, y, 10.0 + k * 16.0, bca(if up { pal::GREEN } else { pal::RED }, 1.0 - k));
            }
            pen.circle(art, x, y, 15.0 + (t * 3.0).sin(), bca(pal::GOLD, 0.35));
        }
        pen.tex(&icons.pawns[i][1], x, y, if i == 0 { 0.62 } else { 0.5 }, Color::WHITE);
    }
    pen.button(art, l.pause, slate, pointer.hover == Some(BtnId::Pause));
    pen.tex(&art.icon_pause, l.pause.x, l.pause.y, 1.2, Color::WHITE);

    // portraits: live cameras show the islands; decorate them on the top layer
    let aiming = gm.selected().map(|c| c.is_attack()).unwrap_or(false);
    let old = pen.layer;
    pen.layer = LAYER_TOP;
    for i in 0..4 {
        let r = l.p[i];
        let viewing = gm.view == i;
        let hot = aiming && i != 0;
        pen.sliced(&art.rrect_mask, r.x, r.y, r.w, r.h, 12.0, bc(pal::SEA));
        if hot {
            pen.rrect(art, r.x, r.y, r.w, r.h, 12.0, Some(bca(pal::RED, 0.14 + (t * 6.0).sin() * 0.08)), None);
        }
        let k = (gm.isl[i].ds / sim::GOAL).clamp(0.0, 1.0);
        pen.rect(r.x + 8.0, r.y + r.h - 10.0, r.w - 16.0, 5.0, art, bca(pal::SLATE, 0.7));
        pen.rect(r.x + 8.0, r.y + r.h - 10.0, (r.w - 16.0) * k, 5.0, art, bc(TEAM[i].0));
        let threatened = gm.isl[i].threatened();
        let edge = if threatened && (t * 10.0).sin() > 0.0 { bc(pal::RED) } else if viewing { bc(pal::GOLD) } else { slate };
        pen.rrect(art, r.x, r.y, r.w, r.h, 12.0, None, Some(edge));
        pen.rrect(art, r.x + 1.5, r.y + 1.5, r.w - 3.0, r.h - 3.0, 10.5, None, Some(edge));
        let mood = if gm.isl[i].hit_t > 0.0 { 0 } else { 1 };
        let ar = (r.h * 0.22).clamp(10.0, 18.0);
        pen.tex(&icons.pawns[i][mood], r.x + ar + 4.0, r.y + ar + 6.0, ar / 20.0, Color::WHITE);
        if gm.isl[i].shield > 0.0 {
            pen.tex(&art.shield, r.x + r.w - ar - 4.0, r.y + ar + 6.0, ar / 20.0, Color::WHITE);
        }
        if gm.isl[i].whale.as_ref().map(|w| w.active()).unwrap_or(false) {
            pen.tex(&icons.whale, r.x + r.w - 26.0, r.y + r.h - 20.0, 0.34, Color::WHITE);
        }
        if hot {
            let c = r.center();
            let lift = if pointer.hover_portrait == Some(i) { 1.15 } else { 1.0 };
            pen.tex(&art.crosshair, c.x, c.y, r.h * 0.4 / 32.0 * (1.0 + (t * 6.0).sin() * 0.08) * lift, Color::WHITE);
        }
    }
    // attacks in flight
    for f in &gm.fly {
        let k = ease_io((f.t / f.dur).clamp(0.0, 1.0));
        let a = portrait_center(l, f.from);
        let b = if gm.view == f.to {
            match f.tile {
                Some(tl) => {
                    let tile = &gm.isl[f.to].map.tiles[tl];
                    view.to_screen(tile.x, tile.y - 60.0)
                }
                None => Vec2::new(l.isle.x + l.isle.w / 2.0, l.isle.y + l.isle.h * 0.35),
            }
        } else {
            portrait_center(l, f.to)
        };
        let path = |kk: f32| Vec2::new(lerp(a.x, b.x, kk), lerp(a.y, b.y, kk) - (kk * PI).sin() * 90.0);
        for j in (0..12).step_by(2) {
            let (k0, k1) = (k * j as f32 / 12.0, k * (j as f32 + 1.0) / 12.0);
            g.line_2d(to_screen(w, h, path(k0)), to_screen(w, h, path(k1)), bca(pal::CREAM, 0.6));
        }
        let p = path(k);
        card_icon(pen, art, icons, f.card, p.x, p.y, 0.9, 1.0);
        pen.tex(&icons.pawns[f.from][1], p.x - 18.0, p.y - 14.0, 0.5, Color::WHITE);
    }
    pen.layer = old;

    // hand
    for i in 0..4 {
        draw_card(pen, g, art, icons, l, gm, pointer, i);
    }
    // combo meter
    if gm.combo >= 2 && gm.t - gm.last_place < 6.0 {
        let c = l.cards[3];
        let (x, y) = (c.x + c.w + 12.0, c.y - 22.0);
        let pul = 1.0 + (t * 10.0).sin() * 0.06;
        let col = [pal::CREAM, pal::GOLD, 0xff9a3c, 0xff5a5a, 0xe86bff][((gm.combo as usize).saturating_sub(2)).min(4)];
        pen.circle(art, x, y, 20.0 * pul, bca(pal::SLATE, 0.9));
        arc(g, w, h, Vec2::new(x, y), 18.0, -PI / 2.0, 1.0 - (gm.t - gm.last_place) / 6.0, bc(col));
        pen.text(x, y + 1.0, 18.0 * pul, bc(col), &format!("x{}", gm.combo));
    }
    let rb = l.reroll;
    pen.button(art, rb, slate, pointer.hover == Some(BtnId::Reroll));
    pen.tex(&art.icon_reroll, rb.x, rb.y, rb.r / 15.0 * 1.05, Color::WHITE);
    if me.rr > 0.0 {
        arc(g, w, h, rb.c(), rb.r + 4.0, PI / 2.0, me.rr / REROLL_CD, bca(pal::CREAM, 0.5));
    }
    let sb = l.shovel;
    pen.button(art, sb, if gm.shovel { bc(pal::GOLD) } else { slate }, pointer.hover == Some(BtnId::Shovel));
    pen.tex(&art.icon_shovel, sb.x, sb.y, sb.r / 15.0 * 1.05, Color::WHITE);
    if gm.view != 0 {
        let hb = l.home;
        let urgent = gm.alert > 0.0 || gm.isl[0].has_threats();
        let pul = if urgent { 1.0 + (t * 8.0).sin() * 0.08 } else { 1.0 };
        pen.button(art, Btn { r: hb.r * pul, ..hb }, if gm.isl[0].has_threats() { bc(pal::RED) } else { slate }, pointer.hover == Some(BtnId::Home));
        pen.tex(&art.icon_home, hb.x, hb.y, hb.r / 15.0, Color::WHITE);
    }
    if let Some(d) = pointer.drag.as_ref() {
        if d.moved {
            if let Some(c) = me.hand[d.si].card {
                pen.layer = LAYER_TOP;
                card_icon(pen, art, icons, c, pointer.pos.x, pointer.pos.y - 26.0, 1.1, 0.9);
                pen.layer = old;
            }
        }
    }
    draw_tutorial(pen, art, icons, l, gm, view, pointer, settings, t);
}

#[allow(clippy::too_many_arguments)]
fn draw_card(pen: &mut Pen, g: &mut Gizmos<HudGizmos>, art: &Art, icons: &Icons, l: &Layout, gm: &Game, pointer: &Pointer, i: usize) {
    let s = &gm.players[0].hand[i];
    let r = l.cards[i];
    let sel = gm.sel == Some(i);
    let hov = pointer.hover_card == Some(i) && s.card.is_some();
    let lift = if sel { -14.0 } else if hov { -5.0 } else { 0.0 };
    let wig = if s.wig > 0.0 { (s.wig * 30.0).sin() * 5.0 * s.wig } else { 0.0 };
    let (cx, cy) = (r.x + r.w / 2.0 + wig, r.y + r.h / 2.0 + lift);
    let Some(card) = s.card else {
        pen.rrect(art, r.x, r.y, r.w, r.h, 10.0, Some(bca(pal::SLATE, 0.4)), Some(bca(pal::CREAM, 0.2)));
        let k = 1.0 - (s.cd / sim::CARD_CD).clamp(0.0, 1.0);
        arc(g, l.w, l.h, Vec2::new(cx, cy), r.w * 0.22, -PI / 2.0 - k * TAU + TAU, k, bc(pal::GOLD));
        pen.tex(&art.sprout_on, cx, cy + 2.0, 0.4 + k * 0.8, Color::WHITE.with_alpha(0.4 + k * 0.6));
        return;
    };
    let flip = ease_back(s.flip.clamp(0.0, 1.0)).clamp(0.05, 1.2);
    let (w, h) = (r.w * flip, r.h);
    let x0 = cx - w / 2.0;
    let y0 = cy - h / 2.0;
    pen.rrect(art, x0 + 2.0, y0 + 6.0 - lift * 0.4, w, h, 10.0, Some(Color::srgba(0.02, 0.06, 0.08, 0.35)), None);
    let fill = match card {
        Card::Storm | Card::Beetle => bc(0x3a2f45),
        Card::Shield => bc(0xcfe3ea),
        _ => bc(0xefe6d2),
    };
    pen.rrect(art, x0, y0, w, h, 10.0, Some(fill), Some(if sel { bc(pal::GOLD) } else { bc(pal::SLATE) }));
    if sel {
        pen.rrect(art, x0 + 1.5, y0 + 1.5, w - 3.0, h - 3.0, 8.5, None, Some(bc(pal::GOLD)));
    }
    if let Card::Nature(sp) = card {
        let band = if def(sp).plant { bca(0x5a9a3c, 0.28) } else { bca(0xd9853a, 0.28) };
        pen.rect(x0 + 2.5, y0 + h * 0.6, (w - 5.0).max(0.0), h * 0.28, art, band);
    }
    if flip > 0.3 {
        card_icon(pen, art, icons, card, cx, cy - h * 0.06, r.w / 64.0 * flip.min(1.0), 1.0);
        let (gx, gy) = (x0 + 12.0, y0 + 12.0);
        match card {
            Card::Nature(sp) => {
                pen.tex(if def(sp).plant { &art.glyph_leaf } else { &art.glyph_paw }, gx, gy, 1.0, Color::WHITE);
                let terr = def(sp).terr;
                let chips: Vec<u32> = if terr.contains(&Terr::Sand) && terr.len() > 1 {
                    vec![0x5f9a3a, 0xe3c07e]
                } else {
                    vec![match terr[0] {
                        Terr::Pond => 0x3d9ec2,
                        Terr::Sand => 0xe3c07e,
                        _ => 0x5f9a3a,
                    }]
                };
                let n = chips.len() as f32;
                for (j, c) in chips.iter().enumerate() {
                    let x = cx + (j as f32 - (n - 1.0) / 2.0) * 13.0;
                    let y = y0 + h - 10.0;
                    pen.tex(&art.hex, x, y, 6.5 / HS, bc(pal::SLATE));
                    pen.tex(&art.hex, x, y, 5.3 / HS, bc(*c));
                }
            }
            Card::Storm | Card::Beetle => pen.tex(&art.glyph_target, gx, gy, 1.0, Color::WHITE),
            Card::Shield => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tutorial(pen: &mut Pen, art: &Art, icons: &Icons, l: &Layout, gm: &Game, view: &MainView, pointer: &Pointer, settings: &Settings, t: f32) {
    if gm.paused || pointer.drag.is_some() {
        return;
    }
    let cyc = (t % 2.2) / 2.2;
    let press = if cyc < 0.15 { cyc / 0.15 } else if cyc > 0.8 { (cyc - 0.8) / 0.2 } else { 0.0 };
    let old = pen.layer;
    pen.layer = LAYER_TOP;
    let me = &gm.players[0];
    let hand = |pen: &mut Pen, p: Vec2, press: f32| {
        if press > 0.0 {
            pen.ring(art, p.x, p.y, 10.0 + press * 12.0, bca(pal::CREAM, 0.9));
        }
        pen.tex(&art.hand, p.x, p.y, 1.0, Color::WHITE);
    };
    let card_pt = |si: usize| Vec2::new(l.cards[si].x + l.cards[si].w / 2.0, l.cards[si].y + l.cards[si].h * 0.45);
    if !settings.tut_place && gm.placed == 0 && gm.view == 0 && gm.sel.is_none() {
        if let Some(si) = me.hand.iter().position(|s| matches!(s.card, Some(Card::Nature(_)))) {
            if let Some(Card::Nature(sp)) = me.hand[si].card {
                if let Some(best) = gm.hints(sp).into_iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)) {
                    let tl = &gm.isl[0].map.tiles[best.0];
                    let a = card_pt(si);
                    let b = view.to_screen(tl.x, tl.y);
                    let k = ease_io(((cyc - 0.2) / 0.55).clamp(0.0, 1.0));
                    let p = a.lerp(b, k);
                    if cyc > 0.15 && cyc < 0.8 {
                        card_icon(pen, art, icons, Card::Nature(sp), p.x, p.y - 22.0, 1.0, 0.7);
                    }
                    hand(pen, p, press);
                }
            }
        }
    } else if !settings.tut_attack && gm.attacks == 0 {
        if let Some(si) = me.hand.iter().position(|s| s.card.map(|c| c.is_attack()).unwrap_or(false)) {
            let tgt = (1..4).max_by(|&a, &b| gm.isl[a].score.partial_cmp(&gm.isl[b].score).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(1);
            let a = card_pt(si);
            let b = portrait_center(l, tgt);
            hand(pen, a.lerp(b, ease_io(((cyc - 0.2) / 0.55).clamp(0.0, 1.0))), press);
        }
    } else if !settings.tut_defend && gm.isl[0].has_threats() {
        if gm.view != 0 {
            hand(pen, l.home.c() + Vec2::new(6.0, 6.0), (t * 2.0) % 1.0);
        } else {
            let isl = &gm.isl[0];
            let p = if let Some(s) = isl.storms.iter().find(|s| s.active() && s.t > 0.8) {
                Some(view.to_screen(s.cx, s.cy))
            } else {
                isl.beetles.iter().find(|b| b.active()).map(|b| view.to_screen(b.x, b.y - 5.0))
            };
            if let Some(p) = p {
                hand(pen, p + Vec2::new(4.0, 6.0), (t * 3.0) % 1.0);
            }
        }
    }
    pen.layer = old;
}

/* ---------------- results show ---------------- */

/// How excited the crowd is at this point of the show (0..1).
fn excitement(end_t: f32) -> f32 {
    let mut e: f32 = 0.15;
    for (rank, &at) in sim::REVEAL_AT.iter().enumerate() {
        let since = end_t - at;
        if since >= 0.0 {
            let peak = if rank == 0 { 1.0 } else { 0.55 };
            let sustain = if rank == 0 { 0.65 } else { 0.25 };
            e = e.max(sustain + (peak - sustain) * (-since * 1.5).exp());
        }
    }
    if end_t > 0.2 && end_t < sim::REVEAL_AT[3] {
        e = e.max(0.3);
    }
    e
}

fn update_show(state: &mut HudState, gm: &Game, l: &Layout, rng: &mut Rng, dt: f32, pending: &mut Pending) {
    for c in &mut state.confetti {
        c.x += c.vx * dt;
        c.y += c.vy * dt;
        c.r += c.vr * dt;
        if c.y > l.h + 20.0 {
            c.y = -10.0;
            c.x = rng.f() * l.w;
        }
    }
    // fireworks after the winner is revealed
    let since = gm.end_t - sim::REVEAL_AT[0];
    let win = gm.ranks.first() == Some(&0);
    if since > 0.0 && since < if win { 14.0 } else { 6.0 } {
        state.next_rocket -= dt;
        if state.next_rocket <= 0.0 {
            state.next_rocket = if win { 0.35 } else { 0.8 } + rng.f() * 0.4;
            state.rockets.push(Rocket { x: l.w * (0.12 + rng.f() * 0.76), y: l.h + 10.0, ty: l.h * (0.1 + rng.f() * 0.3), vy: -l.h * 1.1, c: *rng.pick(&[pal::GOLD, TEAM[0].0, TEAM[1].0, 0x5fb04a, TEAM[3].0, 0xffffff]) });
        }
    }
    let mut bursts = vec![];
    for r in &mut state.rockets {
        r.y += r.vy * dt;
        if r.y <= r.ty {
            bursts.push((r.x, r.y, r.c));
        }
    }
    state.rockets.retain(|r| r.y > r.ty);
    for (x, y, c) in bursts {
        if rng.chance(0.6) {
            pending.ev.push(Ev::Sfx(Sfx::Firework));
        }
        for i in 0..44 {
            let a = i as f32 / 44.0 * TAU + rng.f() * 0.1;
            let sp = 120.0 + rng.f() * 110.0;
            state.sparks.push(Spark { x, y, vx: a.cos() * sp, vy: a.sin() * sp, life: 0.0, max: 1.0 + rng.f() * 0.6, c: if rng.chance(0.3) { 0xffffff } else { c } });
        }
    }
    for s in &mut state.sparks {
        s.life += dt;
        s.x += s.vx * dt;
        s.y += s.vy * dt;
        s.vy += 120.0 * dt;
        s.vx *= 0.985;
        s.vy *= 0.985;
    }
    state.sparks.retain(|s| s.life < s.max);
}

#[allow(clippy::too_many_arguments)]
fn draw_end(pen: &mut Pen, art: &Art, icons: &Icons, l: &Layout, gm: &Game, pointer: &Pointer, state: &HudState, t: f32) {
    let et = gm.end_t;
    let k = (et / 0.6).clamp(0.0, 1.0);
    // the dim sits under the island panels (they are live cameras)
    pen.rect(0.0, 0.0, l.w, l.h, art, bca(0x0d161c, 0.72 * k));
    let old = pen.layer;
    pen.layer = LAYER_TOP;
    // fireworks
    for r in &state.rockets {
        pen.circle(art, r.x, r.y, 2.5, bc(0xfff1c0));
        pen.circle(art, r.x, r.y + 10.0, 1.6, bca(0xfff1c0, 0.5));
    }
    for s in &state.sparks {
        let a = 1.0 - s.life / s.max;
        pen.tex(&art.spark, s.x, s.y, 0.9, bca(s.c, a));
    }
    // drumroll: a pulsing question over the empty stage
    if et > 0.2 && et < sim::REVEAL_AT[3] {
        let pul = 1.0 + (et * 18.0).sin() * 0.06;
        pen.text(l.w / 2.0, l.h * 0.4, 70.0 * pul, bca(pal::CREAM, 0.85), "?");
    }
    let slate = bc(pal::SLATE);
    for (rank, &p) in gm.ranks.iter().enumerate() {
        let Some(r) = l.end_panels.get(p).copied().flatten() else { continue };
        pen.sliced(&art.rrect_mask, r.x, r.y, r.w, r.h, 14.0, bc(0x0d161c).mix(&bc(pal::SEA), 1.0 - 0.72 * k));
        let edge = if rank == 0 { bc(pal::GOLD) } else { slate };
        pen.rrect(art, r.x, r.y, r.w, r.h, 14.0, None, Some(edge));
        pen.rrect(art, r.x + 1.5, r.y + 1.5, r.w - 3.0, r.h - 3.0, 12.5, None, Some(edge));
        let pl = [52.0, 38.0, 28.0, 20.0][rank];
        let plc = [pal::GOLD, 0xc9ced6, 0xc8875a, 0x6b7a80][rank];
        pen.rrect(art, r.x + r.w * 0.12, r.y + r.h + 8.0, r.w * 0.76, pl, 6.0, Some(bc(plc)), None);
        pen.rect(r.x + r.w * 0.12 + 3.0, r.y + r.h + 8.0 + pl - 6.0, r.w * 0.76 - 6.0, 4.0, art, bca(pal::SLATE, 0.25));
        // score counts up after the reveal
        let since = (et - sim::REVEAL_AT[rank]).max(0.0);
        let shown = gm.isl[p].score * (since / 0.9).clamp(0.0, 1.0);
        pen.text(r.x + r.w / 2.0, r.y + r.h + 8.0 + pl / 2.0, (pl * 0.55).clamp(14.0, 26.0), if rank == 0 { slate } else { bc(pal::CREAM) }, &format!("{shown:.0}"));
        let mood = if rank == 0 { 2 } else if rank == 3 { 0 } else { 1 };
        let hop = if rank == 0 { ((t * 5.0).sin()).max(0.0) * 8.0 } else { 0.0 };
        pen.tex(&icons.pawns[p][mood], r.x + r.w / 2.0, r.y - 12.0 - hop, if rank == 0 { 1.25 } else { 0.95 }, Color::WHITE);
        if rank == 0 {
            pen.tex(&art.crown, r.x + r.w / 2.0, r.y - 50.0 - hop + (t * 3.0).sin() * 2.0, 1.0, Color::WHITE);
        }
        let kk = (gm.isl[p].score / sim::GOAL).clamp(0.0, 1.0) * (since / 0.9).clamp(0.0, 1.0);
        pen.rrect(art, r.x + 10.0, r.y + r.h - 16.0, r.w - 20.0, 8.0, 4.0, Some(bca(pal::SLATE, 0.8)), None);
        pen.rrect(art, r.x + 10.0, r.y + r.h - 16.0, ((r.w - 20.0) * kk).max(8.0), 8.0, 4.0, Some(bc(TEAM[p].0)), None);
    }
    // the crowd: rows of pawns bouncing with the mood of the show
    let ex = excitement(et);
    let mut crng = Rng::new(state.crowd_seed | 1);
    let spacing = 26.0;
    let cols = (l.w / spacing) as usize + 2;
    for row in 0..2 {
        let yb = l.h - 8.0 - row as f32 * 22.0;
        for i in 0..cols {
            let x = i as f32 * spacing - 8.0 + if row == 1 { spacing / 2.0 } else { 0.0 };
            let c = *crng.pick(&[TEAM[0].0, TEAM[1].0, TEAM[2].0, TEAM[3].0, 0x8a9aa6, 0xc7b89a, 0x5f9a5a]);
            let ph = crng.f() * TAU;
            let speed = 7.0 + crng.f() * 5.0;
            let jump = ((t * speed + ph).sin()).max(0.0) * 16.0 * ex;
            let s = if row == 1 { 0.62 } else { 0.72 };
            let shade_k = if row == 1 { 0.62 } else { 0.85 };
            let col = crate::art::shade(c, shade_k);
            pen.tex_full(&icons.pawn_white, x, yb - jump - 12.0, Vec2::splat(s * 1.15), ((t * speed * 0.5 + ph).sin()) * 0.12 * ex, false, bc(col));
        }
    }
    for c in &state.confetti {
        let spr = Sprite { image: art.pixel.h.clone(), custom_size: Some(Vec2::new(8.0, 5.0)), color: bc(c.c), ..default() };
        pen.spawn(spr, c.x, c.y, c.r, Vec2::ONE);
    }
    if et > sim::END_BUTTONS_AT {
        pen.button(art, l.end_again, bc(TEAM[0].0), pointer.hover == Some(BtnId::Again));
        pen.tex(&art.icon_reroll, l.end_again.x, l.end_again.y, 1.5, Color::WHITE);
        pen.button(art, l.end_home, slate, pointer.hover == Some(BtnId::EndHome));
        pen.tex(&art.icon_home, l.end_home.x, l.end_home.y, 1.4, Color::WHITE);
    }
    pen.layer = old;
}

/* ---------------- world overlays (screen space over the 3D view) ---------------- */

fn state_color(st: u8) -> Color {
    match st {
        2 => bc(pal::GREEN),
        1 => bc(pal::AMBER),
        _ => bc(pal::RED),
    }
}

fn dashed_ellipse(g: &mut Gizmos<HudGizmos>, w: f32, h: f32, c: Vec2, rx: f32, ry: f32, color: Color) {
    let n = 40;
    for i in (0..n).step_by(2) {
        let (a0, a1) = (i as f32 / n as f32 * TAU, (i as f32 + 1.0) / n as f32 * TAU);
        let p = |a: f32| to_screen(w, h, c + Vec2::new(a.cos() * rx, a.sin() * ry));
        g.line_2d(p(a0), p(a1), color);
    }
}

/// Screen point of a creature's middle and top.
fn ent_points(isl: &crate::sim::Island, view: &MainView, e: &crate::sim::Creature) -> (Vec2, Vec2) {
    let gh = ground_h(isl, e.x, e.y);
    let head = def(e.sp).head * 0.9 * crate::world::PIECE;
    (view.to_screen_h(e.x, e.y, gh + head * 0.5), view.to_screen_h(e.x, e.y, gh + head))
}

#[allow(clippy::too_many_arguments)]
fn relation_lines(pen: &mut Pen, g: &mut Gizmos<HudGizmos>, art: &Art, l: &Layout, isl: &crate::sim::Island, view: &MainView, tile: usize, sp: crate::eco::Sp, from: Vec2, t: f32) {
    for &u in &isl.map.tiles[tile].n2 {
        let Some((id, usp)) = isl.map.tiles[u].occ else { continue };
        if u == tile {
            continue;
        }
        let d = isl.map.dist(tile, u);
        let w = need_w(sp, usp, d) + need_w(usp, sp, d);
        if w == 0.0 {
            continue;
        }
        let Some(e) = isl.ent(id) else { continue };
        let to = ent_points(isl, view, e).0;
        let mid = Vec2::new((from.x + to.x) / 2.0, from.y.min(to.y) - 22.0);
        let good = w > 0.0;
        let col = if good { bca(pal::GREEN, 0.95) } else { bca(pal::RED, 0.95) };
        let pt = |k: f32| from * (1.0 - k) * (1.0 - k) + mid * 2.0 * k * (1.0 - k) + to * k * k;
        let off = ((t * 4.0) as i32).rem_euclid(2) as usize;
        for i in 0..24usize {
            if (i + off) % 2 == 1 {
                continue;
            }
            g.line_2d(to_screen(l.w, l.h, pt(i as f32 / 24.0)), to_screen(l.w, l.h, pt((i as f32 + 0.8) / 24.0)), col);
        }
        let hp = pt(0.5);
        let s = (4.5 + (w.abs() * 12.0).min(3.0)) / 5.0 * 1.3;
        pen.tex(if good { &art.heart } else { &art.heart_broken }, hp.x, hp.y, s, Color::WHITE);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_world_overlays(pen: &mut Pen, g: &mut Gizmos<HudGizmos>, art: &Art, icons: &Icons, l: &Layout, gm: &Game, view: &MainView, pointer: &Pointer, state: &HudState, t: f32) {
    let slot = gm.view;
    let isl = &gm.isl[slot];
    let (w, h) = (l.w, l.h);
    let vs = view.s;
    // badges: sad, hungry, pollinated or fruiting, scorched
    for e in &isl.ents {
        if e.dying > 0.0 {
            continue;
        }
        let top = ent_points(isl, view, e).1;
        let bob = (t * 3.0 + e.ph).sin() * 1.5;
        if crate::ecology::hungry(e) {
            pen.tex(&art.food, top.x + 9.0, top.y - 12.0 + bob, 0.75, Color::srgba(0.75, 0.75, 0.75, 0.95));
        }
        if state_of(e.hd) == 0 {
            pen.tex(&art.faces[0], top.x - 4.0, top.y - 12.0 + bob, 0.6, Color::WHITE.with_alpha(0.92));
        }
        if e.polli > 0.0 {
            let tw = ((t * 5.0 + e.ph).sin() * 0.5 + 0.5) * 0.8 + 0.2;
            pen.tex(&art.spark, top.x + 6.0, top.y - 4.0, 1.1, bca(pal::GOLD, tw));
        }
        if e.scorch > 0.0 {
            for k in 0..3 {
                let q = (t * 0.8 + k as f32 / 3.0 + e.ph) % 1.0;
                pen.circle(art, top.x + (k as f32 - 1.0) * 5.0, top.y - q * 30.0, 3.0 + q * 4.0, bca(0x4a4f5a, 0.6 * (1.0 - q)));
            }
        }
    }
    // interactions: a line from actor to target, coloured by kind
    for (sl, kind, a, b, ok, age) in &state.links {
        if *sl != slot {
            continue;
        }
        let k = (age / 1.3).clamp(0.0, 1.0);
        let fade = 1.0 - k;
        let (col, icon): (u32, Option<&Tex>) = match kind {
            Link::Pollinate => (pal::GOLD, Some(&art.spark)),
            Link::Graze => (0x7fc76f, Some(&art.glyph_leaf)),
            Link::Hunt => (if *ok { pal::RED } else { 0xf0a0a0 }, Some(&art.glyph_paw)),
            Link::Tongue => (0xff7fb0, None),
            Link::Peck => (0xf2a33a, None),
            Link::Seed => (0xb8864a, Some(&art.sprout_on)),
            Link::Rod => (0xffe066, Some(&art.bolt)),
            Link::Pinch => (0xe0533d, None),
            Link::Compost => (0xa27ad0, Some(&art.sprout_on)),
        };
        let pa = view.to_screen_h(a.x, a.y, 14.0);
        let pb = view.to_screen_h(b.x, b.y, 10.0);
        let lift = if matches!(kind, Link::Seed | Link::Compost | Link::Pollinate) { 30.0 } else { 12.0 };
        let mid = (pa + pb) / 2.0 - Vec2::new(0.0, lift);
        let grow = (age / 0.25).clamp(0.0, 1.0);
        let pt = |q: f32| pa * (1.0 - q) * (1.0 - q) + mid * 2.0 * q * (1.0 - q) + pb * q * q;
        let n = 16;
        for i in 0..n {
            let (q0, q1) = (i as f32 / n as f32 * grow, (i as f32 + 0.7) / n as f32 * grow);
            if matches!(kind, Link::Pollinate | Link::Seed | Link::Compost) && i % 2 == 1 {
                continue;
            }
            g.line_2d(to_screen(w, h, pt(q0)), to_screen(w, h, pt(q1)), bca(col, fade));
        }
        if let Some(icon) = icon {
            let p = pt(0.5 * grow);
            pen.tex(icon, p.x, p.y, 0.9, Color::WHITE.with_alpha(fade));
        }
        if matches!(kind, Link::Hunt) && !*ok {
            pen.tex(&art.heart_broken, pb.x, pb.y - 16.0, 0.9, Color::WHITE.with_alpha(fade));
        }
    }
    for (sl, p, age) in &state.fears {
        if *sl != slot {
            continue;
        }
        let gh = ground_h(isl, p.x, p.y);
        let q = view.to_screen_h(p.x, p.y, gh + 40.0);
        let pop = if *age < 0.2 { ease_back(age / 0.2) } else { 1.0 };
        let shake = (age * 40.0).sin() * 2.0 * (1.0 - age / 1.4);
        pen.tex(&art.alert, q.x + shake, q.y, pop * 1.0, Color::WHITE.with_alpha((1.4 - age).min(1.0)));
    }
    if !gm.paused {
        for s in &isl.storms {
            if !s.active() {
                continue;
            }
            let warn = (s.t / sim::STORM_T).clamp(0.0, 1.0);
            let c = view.to_screen(s.cx, s.cy);
            arc(g, w, h, c + Vec2::new(40.0, -26.0) * vs * s.sc, 8.0, -PI / 2.0, 1.0 - warn, bc(pal::RED));
            for i in 0..s.need {
                let x = c.x + (i as f32 - (s.need as f32 - 1.0) / 2.0) * 11.0;
                let col = if i < s.taps { Color::WHITE } else { Color::WHITE.with_alpha(0.35) };
                pen.circle(art, x, c.y + 36.0 * vs * s.sc, 3.4, col);
            }
            let o = view.to_screen(s.x, s.y);
            dashed_ellipse(g, w, h, o, HS * 2.2 * vs, HS * 2.2 * SQ * vs, bca(pal::RED, 0.5 + (t * 10.0).sin() * 0.3));
        }
        for b in &isl.beetles {
            if b.active() && b.eat > 0.0 {
                arc(g, w, h, view.to_screen(b.x, b.y - 22.0), 7.0, -PI / 2.0, b.eat / 3.0, bc(pal::RED));
            }
        }
        if let Some(wh) = &isl.whale {
            if wh.active() && wh.t > 0.4 {
                let c = view.to_screen_h(wh.x, wh.y, -4.0);
                for i in 0..6 {
                    let k = (t * 1.4 + i as f32 / 6.0) % 1.0;
                    let x = c.x - 14.0 * vs + (i as f32 - 2.5) * 3.0 * k * vs;
                    let y = c.y - (26.0 + k * 34.0) * vs;
                    pen.circle(art, x, y, (2.0 + k * 1.5) * vs, bca(0xe6f6fb, 0.85 * (1.0 - k)));
                }
                if slot == 0 {
                    let pul = 1.0 + (t * 6.0).sin() * 0.08;
                    pen.tex_full(&art.ring, c.x, c.y, Vec2::new(1.6 * pul * vs, 0.95 * pul * vs), 0.0, false, bca(pal::GOLD, 0.9));
                }
            }
        }
    }

    let sel = gm.selected();
    let hover = pointer.hover_tile;
    match sel {
        Some(Card::Nature(sp)) if slot == 0 && !gm.paused => {
            let pul = 1.0 + (t * 4.0).sin() * 0.12;
            for (tile, _v, hh) in gm.hints(sp) {
                let tl = &isl.map.tiles[tile];
                let p = view.to_screen_h(tl.x, tl.y, crate::models::terrain_h(tl.t));
                let st = state_of(hh);
                // good spots glow, poor spots stay quiet
                let (r, ring_a, hex_a) = match st {
                    2 => (5.5 * pul, 1.0, 0.28),
                    1 => (3.8, 0.8, 0.14),
                    _ => (2.4, 0.0, 0.05),
                };
                pen.tex(&art.hex, p.x, p.y, 0.9 * vs, Color::WHITE.with_alpha(hex_a));
                if ring_a > 0.0 {
                    pen.circle(art, p.x, p.y, r + 1.8, Color::WHITE.with_alpha(ring_a));
                }
                pen.circle(art, p.x, p.y, r, state_color(st).with_alpha(if st == 0 { 0.5 } else { 1.0 }));
            }
            if let Some(tile) = hover {
                let tl = &isl.map.tiles[tile];
                let gh = crate::models::terrain_h(tl.t);
                let head = def(sp).head * 0.9 * crate::world::PIECE;
                if isl.can_place(tile, sp) {
                    let (_, hh) = isl.place_value(tile, sp);
                    let mid = view.to_screen_h(tl.x, tl.y, gh + head * 0.5);
                    relation_lines(pen, g, art, l, isl, view, tile, sp, mid, t);
                    let icon = &icons.sp[sp.idx()];
                    pen.tex(icon, mid.x, mid.y, vs * crate::world::PIECE, Color::WHITE.with_alpha(0.8));
                    let top = view.to_screen_h(tl.x, tl.y, gh + head);
                    pen.tex(&art.faces[state_of(hh) as usize], top.x, top.y - 18.0, 1.0, Color::WHITE);
                    let foot = view.to_screen_h(tl.x, tl.y, gh);
                    env_pill(pen, art, isl, tile, None, foot + Vec2::new(0.0, 26.0));
                } else {
                    let o = view.to_screen_h(tl.x, tl.y, gh);
                    g.line_2d(to_screen(w, h, o + Vec2::new(-8.0, -6.0)), to_screen(w, h, o + Vec2::new(8.0, 6.0)), bc(pal::RED));
                    g.line_2d(to_screen(w, h, o + Vec2::new(8.0, -6.0)), to_screen(w, h, o + Vec2::new(-8.0, 6.0)), bc(pal::RED));
                }
            }
        }
        Some(Card::Storm) if slot != 0 => {
            if let Some(tile) = hover {
                let tl = &isl.map.tiles[tile];
                let o = view.to_screen(tl.x, tl.y);
                dashed_ellipse(g, w, h, o, HS * 2.2 * vs, HS * 2.2 * SQ * vs, bc(pal::RED));
                pen.tex(&icons.storm, o.x, o.y - 80.0 * vs, 1.1 * vs, Color::WHITE.with_alpha(0.75));
            }
        }
        None if gm.shovel && slot == 0 => {
            for tl in isl.map.tiles.iter().filter(|t| t.occ.is_some()) {
                let p = view.to_screen_h(tl.x, tl.y, crate::models::terrain_h(tl.t));
                pen.tex(&art.hex_line, p.x, p.y, 0.9 * vs, Color::WHITE.with_alpha(0.55));
            }
            if let Some(tile) = hover {
                let tl = &isl.map.tiles[tile];
                if tl.occ.is_some() {
                    let p = view.to_screen_h(tl.x, tl.y, crate::models::terrain_h(tl.t));
                    pen.tex(&art.hex_line, p.x, p.y, 0.95 * vs, bc(pal::RED));
                }
            }
        }
        None => {
            if let Some((id, sp)) = gm.inspect.and_then(|tile| isl.map.tiles.get(tile)).and_then(|t| t.occ) {
                if let Some(e) = isl.ent(id) {
                    let (mid, top) = ent_points(isl, view, e);
                    relation_lines(pen, g, art, l, isl, view, e.tile, sp, mid, t);
                    let st = state_of(e.h);
                    pen.tex(&art.faces[st as usize], top.x, top.y - 18.0, 1.0, Color::WHITE);
                    let foot = view.to_screen_h(e.x, e.y, ground_h(isl, e.x, e.y));
                    env_pill(pen, art, isl, e.tile, Some(e), foot + Vec2::new(0.0, 26.0));
                    if st == 0 && e.sad_t > 0.0 {
                        arc(g, w, h, top - Vec2::new(0.0, 18.0), 14.0, -PI / 2.0, 1.0 - e.sad_t / WITHER_T, bc(pal::RED));
                    }
                }
            }
        }
        _ => {}
    }
    // floating labels: +3, x4, MEGA!, WOW!
    for (p, age) in &state.popups {
        if p.slot != slot {
            continue;
        }
        let k = age / p.max;
        let pop = if *age < 0.25 { ease_back(age / 0.25) } else { 1.0 };
        let base = view.to_screen(p.x, p.y);
        let a = if k > 0.7 { (1.0 - k) / 0.3 } else { 1.0 };
        pen.text(base.x, base.y - 34.0 * k.sqrt(), p.size * pop.max(0.05), bca(p.color, a), &p.text);
    }
}

/// Every 3D model render on one sheet (used for sharing the resources).
fn draw_gallery(pen: &mut Pen, art: &Art, icons: &Icons, l: &Layout) {
    pen.rect(0.0, 0.0, l.w, l.h, art, bc(0xe9e2d2));
    let mut items: Vec<&Tex> = icons.sp.iter().collect();
    items.extend([&icons.storm, &icons.beetle, &icons.whale]);
    for p in &icons.pawns {
        items.extend(p.iter());
    }
    let cols = 8;
    let cell = (l.w - 40.0) / cols as f32;
    for (i, t) in items.iter().enumerate() {
        let (cx, cy) = (20.0 + (i % cols) as f32 * cell + cell / 2.0, 20.0 + (i / cols) as f32 * cell * 0.95 + cell / 2.0);
        pen.rrect(art, cx - cell * 0.46, cy - cell * 0.44, cell * 0.92, cell * 0.88, 12.0, Some(bc(0xf6f1e6)), Some(bca(pal::SLATE, 0.25)));
        pen.tex(t, cx, cy, cell * 0.8 / t.size.x.max(1.0), Color::WHITE);
    }
}

/// Little dark pill showing a tile's moisture and fertility (three pips each),
/// shade, and for animals how fed they are.
fn env_pill(pen: &mut Pen, art: &Art, isl: &crate::sim::Island, tile: usize, e: Option<&crate::sim::Creature>, at: Vec2) {
    let t = &isl.map.tiles[tile];
    let shade = isl.shade_at(tile, None);
    let animal = e.map(|e| !def(e.sp).plant).unwrap_or(false);
    let wdt = 118.0 + if shade > 0.0 { 20.0 } else { 0.0 } + if animal { 52.0 } else { 0.0 };
    let (x0, y0) = (at.x - wdt / 2.0, at.y - 12.0);
    pen.rrect(art, x0, y0, wdt, 24.0, 12.0, Some(bca(pal::SLATE, 0.88)), Some(bca(pal::CREAM, 0.2)));
    let mut x = x0 + 14.0;
    let lvl = |v: f32| ((v * 3.0).round() as i32).clamp(0, 3);
    let m = if t.t == crate::island::Terr::Pond { 3 } else { lvl(t.moist) };
    for k in 0..3 {
        pen.tex(&art.drop_icon, x, at.y, 0.7, if k < m { bc(0x6fc3ff) } else { bca(pal::CREAM, 0.2) });
        x += 12.0;
    }
    x += 6.0;
    let f = lvl(t.fert);
    for k in 0..3 {
        pen.tex(if k < f { &art.sprout_on } else { &art.sprout_off }, x, at.y, 0.62, Color::WHITE.with_alpha(if k < f { 1.0 } else { 0.35 }));
        x += 13.0;
    }
    if shade > 0.0 {
        x += 4.0;
        pen.tex(&art.shade_icon, x, at.y, 0.75, Color::WHITE);
        x += 16.0;
    }
    if let Some(e) = e.filter(|_| animal) {
        x += 6.0;
        pen.tex(&art.food, x, at.y, 0.7, Color::WHITE);
        let bw = 28.0;
        pen.rect(x + 9.0, at.y - 3.0, bw, 6.0, art, bca(pal::CREAM, 0.2));
        let col = if e.fed < 0.2 { pal::RED } else if e.fed < 0.5 { pal::AMBER } else { pal::GREEN };
        pen.rect(x + 9.0, at.y - 3.0, bw * e.fed.clamp(0.0, 1.0), 6.0, art, bc(col));
    }
}

/// Rain streaks, wind gusts, heat haze.
fn draw_weather_fx(pen: &mut Pen, g: &mut Gizmos<HudGizmos>, art: &Art, l: &Layout, gm: &Game, t: f32) {
    let (w, h) = (l.w, l.h);
    let a = l.isle;
    let hash = |i: u32, k: u32| ((i.wrapping_mul(2654435761).wrapping_add(k.wrapping_mul(40503))) % 10007) as f32 / 10007.0;
    // ease weather in and out over its first and last two seconds
    let strength = (gm.weather_t / 2.0).clamp(0.0, 1.0);
    match gm.weather {
        Weather::Rain => {
            for i in 0..110u32 {
                let x = a.x + ((hash(i, 1) * a.w + t * 70.0) % a.w);
                let y = a.y + ((hash(i, 2) * a.h + t * (700.0 + hash(i, 3) * 300.0)) % a.h);
                g.line_2d(to_screen(w, h, Vec2::new(x, y)), to_screen(w, h, Vec2::new(x - 5.0, y + 15.0)), bca(0xcfe8ff, 0.45 * strength));
            }
        }
        Weather::Wind => {
            for i in 0..22u32 {
                let y = a.y + hash(i, 4) * a.h;
                let x = a.x + ((hash(i, 5) * a.w + t * (420.0 + hash(i, 6) * 200.0)) % (a.w + 200.0)) - 100.0;
                let len = 50.0 + hash(i, 7) * 60.0;
                let pts: Vec<Vec2> = (0..=8).map(|k| {
                    let q = k as f32 / 8.0;
                    to_screen(w, h, Vec2::new(x + q * len, y + (q * 6.0 + t * 3.0 + i as f32).sin() * 3.0))
                }).collect();
                g.linestrip_2d(pts, bca(0xffffff, 0.35 * strength));
            }
            for i in 0..10u32 {
                let x = a.x + ((hash(i, 8) * a.w + t * 260.0) % a.w);
                let y = a.y + hash(i, 9) * a.h + (t * 2.0 + i as f32).sin() * 20.0;
                pen.tex_full(&art.glyph_leaf, x, y, Vec2::splat(0.9), t * 4.0 + i as f32, false, Color::WHITE.with_alpha(0.8 * strength));
            }
        }
        Weather::Drought => {
            pen.rect(0.0, 0.0, w, h, art, bca(0xff9a3c, 0.07 * strength));
            for i in 0..6u32 {
                let y = a.y + a.h * (0.15 + i as f32 * 0.13) + (t * 0.7 + i as f32).sin() * 6.0;
                let pts: Vec<Vec2> = (0..=24).map(|k| {
                    let q = k as f32 / 24.0;
                    to_screen(w, h, Vec2::new(a.x + q * a.w, y + (q * 30.0 + t * 2.0 + i as f32).sin() * 2.5))
                }).collect();
                g.linestrip_2d(pts, bca(0xffe2b0, 0.12 * strength));
            }
        }
        Weather::Clear => {}
    }
}
