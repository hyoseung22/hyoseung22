//! Screen-space interface: layout, HUD, title, pause and end screens, tutorial hand.
//! Drawn immediate-mode: every frame spawns short-lived sprites.

use crate::art::{bc, bca, Art, Tex, TEAM};
use crate::eco::{def, Sp};
use crate::input::{BtnId, Pointer};
use crate::island::{Terr, HS};
use crate::rng::Rng;
use crate::sim::{self, ease_back, ease_io, lerp, Card, Game, DUR, GOAL, REROLL_CD};
use crate::world::MainView;
use crate::{Ephemeral, Fonts, Scene, Session, Settings, LAYER_HUD};
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
    let rx0 = 70.0;
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

    // end screen panels, indexed by player
    l.end_panels = vec![None; 4];
    if let (Scene::End, Some(g)) = (session.scene, session.game.as_ref()) {
        let gap = 22.0;
        let pw = if port { ((w - 60.0) / 2.0).clamp(110.0, 210.0) } else { (w * 0.18).clamp(130.0, 240.0).min((w - 60.0 - gap * 3.0) / 4.0) };
        let ph = pw * 0.72;
        for (rank, &p) in g.ranks.iter().enumerate() {
            let (sx, sy) = if port {
                let x0 = w / 2.0 - pw - gap / 2.0;
                (x0 + (rank % 2) as f32 * (pw + gap), h * 0.2 + (rank / 2) as f32 * (ph + 100.0) + (rank % 2) as f32 * 18.0)
            } else {
                let x0 = w / 2.0 - (pw * 4.0 + gap * 3.0) / 2.0;
                (x0 + rank as f32 * (pw + gap), h * 0.28 + rank as f32 * 20.0)
            };
            let d = ((g.end_t - 0.3 - rank as f32 * 0.25) / 0.5).clamp(0.0, 1.0);
            if d > 0.0 {
                l.end_panels[p] = Some(R::new(sx, sy + (1.0 - ease_back(d)) * 60.0, pw, ph));
            }
        }
        let by = h - 70.0;
        l.end_again = Btn { x: w / 2.0 + 40.0, y: by, r: 36.0 };
        l.end_home = Btn { x: w / 2.0 - 50.0, y: by, r: 26.0 };
    }
}

/* ---------------- painter ---------------- */

pub struct Pen<'a, 'w, 's> {
    pub cmd: &'a mut Commands<'w, 's>,
    pub w: f32,
    pub h: f32,
    pub z: f32,
    pub layer: usize,
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
    pub fn button(&mut self, art: &Art, b: Btn, fill: Color, hot: bool) {
        let r = b.r * if hot { 1.06 } else { 1.0 };
        self.circle(art, b.x, b.y + 3.0, b.r, Color::srgba(0.16, 0.12, 0.12, 0.18));
        self.circle(art, b.x, b.y, r, fill);
        self.ring(art, b.x, b.y, r, bc(crate::art::INK));
    }
}

fn to_screen(w: f32, h: f32, p: Vec2) -> Vec2 {
    Vec2::new(p.x - w / 2.0, h / 2.0 - p.y)
}

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

/* ---------------- state for confetti and the logo ---------------- */

pub struct Confetto {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r: f32,
    vr: f32,
    c: u32,
}

#[derive(Resource, Default)]
pub struct HudState {
    confetti: Vec<Confetto>,
    pending_win: Option<bool>,
}

impl HudState {
    pub fn start_confetti(&mut self, win: bool) {
        self.pending_win = Some(win);
    }
}

#[derive(Component)]
pub struct Logo {
    dx: f32,
    dy: f32,
}

pub fn setup_title_text(mut commands: Commands, fonts: Res<Fonts>) {
    let size = 80.0;
    let colors = [0xff9fb8, 0xffd35c, 0x9fd86b, 0x72a9f5, 0xb893ea, 0xfff6e4, 0xfff6e4, 0xfff6e4, 0xfff6e4, 0xfff6e4, 0xfff6e4];
    let word = "Bloom Isles";
    let mut offsets: Vec<(f32, f32, bool)> = (0..12).map(|i| {
        let a = i as f32 / 12.0 * TAU;
        (a.cos() * 6.0, a.sin() * 6.0 + 5.0, false)
    }).collect();
    offsets.extend((0..12).map(|i| {
        let a = i as f32 / 12.0 * TAU;
        (a.cos() * 6.0, a.sin() * 6.0, false)
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
                    let c = if main { bc(colors[i]) } else { bc(crate::art::INK) };
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
    layout: Res<Layout>,
    view: Res<MainView>,
    pointer: Res<Pointer>,
    time: Res<Time>,
    mut state: ResMut<HudState>,
    mut g: Gizmos<HudGizmos>,
    mut logo: Query<(&Logo, &mut Transform, &mut Visibility)>,
    mut rng: Local<Option<Rng>>,
) {
    let rng = rng.get_or_insert_with(Rng::from_time);
    let l = &*layout;
    let mut pen = Pen { cmd: &mut commands, w: l.w, h: l.h, z: 10.0, layer: LAYER_HUD };
    let t = session.t;
    let title = session.scene == Scene::Title;
    for (lg, mut tf, mut vis) in &mut logo {
        *vis = if title { Visibility::Visible } else { Visibility::Hidden };
        if title {
            let s = l.logo_size / 80.0;
            let p = to_screen(l.w, l.h, Vec2::new(l.w / 2.0 + lg.dx * s, l.logo_y + lg.dy * s + (t * 2.0).sin() * 3.0));
            tf.translation.x = p.x;
            tf.translation.y = p.y;
            tf.scale = Vec3::splat(s);
        }
    }
    if let Some(win) = state.pending_win.take() {
        state.confetti.clear();
        if win {
            for _ in 0..140 {
                state.confetti.push(Confetto {
                    x: rng.f() * l.w,
                    y: -rng.f() * l.h * 0.6,
                    vx: (rng.f() - 0.5) * 60.0,
                    vy: 60.0 + rng.f() * 120.0,
                    r: rng.f() * TAU,
                    vr: (rng.f() - 0.5) * 8.0,
                    c: *rng.pick(&[0xff8f7d, 0xffc94f, 0x8fd070, 0x72a9f5, 0xb893ea, 0xffffff]),
                });
            }
        }
    }
    match session.scene {
        Scene::Title => draw_title(&mut pen, &art, l, &settings, &pointer, t, session.dice_t),
        Scene::Play | Scene::End => {
            let Some(gm) = session.game.as_ref() else { return };
            if session.scene == Scene::Play {
                draw_play(&mut pen, &mut g, &art, l, gm, &view, &pointer, &settings, t);
            }
            if session.scene == Scene::End {
                let dt = time.delta_secs().min(0.05);
                for c in &mut state.confetti {
                    c.x += c.vx * dt;
                    c.y += c.vy * dt;
                    c.r += c.vr * dt;
                    if c.y > l.h + 20.0 {
                        c.y = -10.0;
                        c.x = rng.f() * l.w;
                    }
                }
                draw_end(&mut pen, &art, l, gm, &pointer, &state, t);
            } else if gm.paused {
                pen.layer = LAYER_TOP;
                pen.rect(0.0, 0.0, l.w, l.h, &art, Color::srgba(0.24, 0.27, 0.31, 0.45));
                pen.button(&art, l.pause_resume, bc(TEAM[0].0), pointer.hover == Some(BtnId::Resume));
                pen.tex(&art.icon_play, l.pause_resume.x + 3.0, l.pause_resume.y, 2.2, Color::WHITE);
                pen.button(&art, l.pause_home, bc(0xfff6e4), pointer.hover == Some(BtnId::PauseHome));
                pen.tex(&art.icon_home, l.pause_home.x, l.pause_home.y, 1.7, Color::WHITE);
            }
        }
    }
    // paper grain over everything
    pen.layer = LAYER_TOP;
    pen.z = 90.0;
    let grain = Sprite {
        image: art.grain.h.clone(),
        custom_size: Some(Vec2::new(l.w, l.h)),
        image_mode: SpriteImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: 1.0 },
        ..default()
    };
    pen.spawn(grain, l.w / 2.0, l.h / 2.0, 0.0, Vec2::ONE);
    // mute button lives on every screen
    pen.button(&art, l.mute, bc(0xfff6e4), pointer.hover == Some(BtnId::Mute));
    pen.tex(if settings.muted { &art.icon_sound_off } else { &art.icon_sound_on }, l.mute.x, l.mute.y, 1.3, Color::WHITE);
}

fn draw_title(pen: &mut Pen, art: &Art, l: &Layout, settings: &Settings, pointer: &Pointer, t: f32, dice_t: f32) {
    let ay = l.logo_y + l.logo_size * 0.66;
    for i in 0..4 {
        let s = if i == 0 { 16.0 } else { 13.0 } / 20.0;
        pen.tex(&art.avatars[i][2], l.w / 2.0 + (i as f32 - 1.5) * 44.0, ay + (t * 3.0 + i as f32).sin() * 2.0, s, Color::WHITE);
    }
    let p = l.title_play;
    let pulse = Btn { r: p.r * (1.0 + (t * 3.0).sin() * 0.03), ..p };
    pen.button(art, pulse, bc(TEAM[0].0), pointer.hover == Some(BtnId::Play));
    pen.tex(&art.icon_play, p.x + 4.0, p.y, p.r / 16.0, Color::WHITE);
    let d = l.title_dice;
    pen.button(art, d, bc(0xfff6e4), pointer.hover == Some(BtnId::Dice));
    pen.tex_full(&art.dice, d.x, d.y, Vec2::ONE, ((dice_t * 8.0).min(TAU)).sin() * 0.3, false, Color::WHITE);
    for (i, r) in l.title_diff.iter().enumerate() {
        let on = settings.diff == i;
        let fill = if on { bc(0xfff3c7) } else { bca(0xfff6e4, 0.6) };
        let line = if on { bc(crate::art::INK) } else { bca(crate::art::INK, 0.4) };
        pen.rrect(art, r.x, r.y, r.w, r.h, 14.0, Some(fill), Some(line));
        for k in 0..=i {
            let x = r.x + r.w / 2.0 + (k as f32 - i as f32 / 2.0) * 12.0;
            pen.tex(if on { &art.sprout_on } else { &art.sprout_off }, x, r.y + r.h / 2.0, 0.9, Color::WHITE);
        }
    }
    // quit
    let q = l.title_quit;
    pen.button(art, q, bc(0xfff6e4), pointer.hover == Some(BtnId::Quit));
    let s = q.r * 0.55;
    pen.rect(q.x - 1.2, q.y - s, 2.4, s * 0.9, art, bc(crate::art::INK));
    for i in 0..14 {
        let a = -PI / 2.0 + 0.55 + i as f32 / 13.0 * (TAU - 1.1);
        pen.circle(art, q.x + a.cos() * s * 0.8, q.y + a.sin() * s * 0.8, 1.3, bc(crate::art::INK));
    }
}

fn card_icon(pen: &mut Pen, art: &Art, card: Card, x: f32, y: f32, s: f32, alpha: f32) {
    let wc = Color::WHITE.with_alpha(alpha);
    match card {
        Card::Nature(sp) => {
            let k = s * icon_k(sp);
            let head = def(sp).head;
            let gy = y + head * k * 0.42;
            let ground = match def(sp).terr[0] {
                Terr::Pond => 0x86cde6,
                Terr::Sand => 0xf5e0a6,
                _ => 0xa8d77a,
            };
            pen.tex(&art.shadow, x, gy, k, bca(ground, alpha));
            let by = if sp == Sp::Bee { gy - 24.0 * k } else { gy };
            pen.tex(art.sp(sp, 1), x, by, k, wc);
        }
        Card::Storm => {
            pen.tex(&art.cloud, x, y - 4.0 * s, s * 0.9, wc);
            pen.tex(&art.bolt, x + 2.0 * s, y + 14.0 * s, s * 0.8, wc);
        }
        Card::Beetle => {
            pen.tex(&art.beetle, x - 8.0 * s, y + 8.0 * s, s * 1.2, wc);
            pen.tex_full(&art.beetle, x + 9.0 * s, y - 2.0 * s, Vec2::splat(s * 1.05), 0.0, true, wc);
            pen.tex(&art.beetle, x - 2.0 * s, y - 12.0 * s, s * 0.9, wc);
        }
        Card::Shield => pen.tex(&art.shield, x, y + 4.0 * s, s * 1.2, wc),
    }
}

fn icon_k(sp: Sp) -> f32 {
    match sp {
        Sp::Bee => 1.75,
        Sp::Bird => 1.65,
        Sp::Crab => 1.7,
        Sp::Frog => 1.6,
        Sp::Lily => 1.6,
        Sp::Mushroom => 1.5,
        Sp::Flower => 1.4,
        Sp::Rabbit | Sp::Fox => 1.3,
        Sp::Bush => 1.35,
        Sp::Tree => 0.95,
        Sp::Palm => 1.05,
    }
}

fn portrait_center(l: &Layout, i: usize) -> Vec2 {
    l.p[i].center()
}

#[allow(clippy::too_many_arguments)]
fn draw_play(pen: &mut Pen, g: &mut Gizmos<HudGizmos>, art: &Art, l: &Layout, gm: &Game, view: &MainView, pointer: &Pointer, settings: &Settings, t: f32) {
    let (w, h) = (l.w, l.h);
    let ink = bc(crate::art::INK);
    let paper = bc(0xfff6e4);
    let me = &gm.players[0];

    if gm.view != 0 {
        let a = l.isle;
        pen.rrect(art, a.x + 4.0, a.y + 4.0, a.w - 8.0, a.h - 8.0, 22.0, None, Some(bca(TEAM[gm.view].0, 0.6)));
        pen.rrect(art, a.x + 6.0, a.y + 6.0, a.w - 12.0, a.h - 12.0, 20.0, None, Some(bca(TEAM[gm.view].0, 0.6)));
        pen.tex(&art.avatars[gm.view][1], a.x + 34.0, a.y + 34.0, 0.9, Color::WHITE);
    }
    if gm.fade > 0.0 {
        let a = l.isle;
        pen.rect(a.x, a.y, a.w, a.h, art, bca(0x8ed5d2, gm.fade));
    }
    if gm.flash > 0.0 {
        pen.rect(0.0, 0.0, w, h, art, Color::srgba(1.0, 1.0, 0.9, (gm.flash * 1.6).min(1.0)));
    }
    if gm.alert > 0.0 {
        let a = gm.alert * if gm.view != 0 { 0.35 } else { 0.22 };
        let c = bca(0xe96b5f, a);
        let b = 14.0;
        pen.rect(0.0, 0.0, w, b, art, c);
        pen.rect(0.0, h - b, w, b, art, c);
        pen.rect(0.0, b, b, h - 2.0 * b, art, c);
        pen.rect(w - b, b, b, h - 2.0 * b, art, c);
    }

    // timer
    let tm = l.timer;
    let left = (1.0 - gm.t / DUR).clamp(0.0, 1.0);
    pen.button(art, tm, paper, false);
    let warm = if left < 0.12 && (t * 8.0).sin() > 0.0 { bc(0xffb070) } else { bc(0xffd36b) };
    arc(g, w, h, tm.c(), tm.r - 7.0, PI / 2.0, left, warm);
    arc(g, w, h, tm.c(), tm.r - 11.0, PI / 2.0, left, warm);
    let sa = -PI / 2.0 - left * TAU;
    let sun = tm.c() + Vec2::new(sa.cos(), -sa.sin()) * (tm.r - 5.0);
    pen.circle(art, sun.x, sun.y, 5.5, ink);
    pen.circle(art, sun.x, sun.y, 4.3, bc(0xffb347));

    // race track
    let rc = l.race;
    pen.rrect(art, rc.x, rc.y - 5.0, rc.w, 14.0, 7.0, Some(Color::srgba(0.16, 0.12, 0.12, 0.15)), None);
    pen.rrect(art, rc.x, rc.y - 7.0, rc.w, 14.0, 7.0, Some(paper), Some(ink));
    for i in 1..4 {
        pen.circle(art, rc.x + rc.w * i as f32 / 4.0, rc.y, 2.0, bca(crate::art::INK, 0.2));
    }
    pen.tex_full(&art.trophy, rc.x + rc.w + 14.0, rc.y, Vec2::ONE, (t * 1.5).sin() * 0.08, false, Color::WHITE);
    for i in [1usize, 2, 3, 0] {
        let k = (gm.isl[i].ds / GOAL).clamp(0.0, 1.0);
        let x = rc.x + 8.0 + (rc.w - 16.0) * k;
        let y = rc.y + if i == 0 { 0.0 } else { (i as f32 - 2.0) * 5.0 };
        if i == 0 {
            pen.circle(art, x, y, 17.0 + (t * 3.0).sin() * 1.5, Color::WHITE.with_alpha(0.5));
        }
        pen.tex(&art.avatars[i][1], x, y, if i == 0 { 13.0 } else { 10.5 } / 20.0, Color::WHITE);
    }
    // pause
    pen.button(art, l.pause, paper, pointer.hover == Some(BtnId::Pause));
    pen.tex(&art.icon_pause, l.pause.x, l.pause.y, 1.3, Color::WHITE);

    // portraits: the islands themselves come from live cameras; decorate on the top layer
    let sel = gm.selected();
    let aiming = sel.map(|c| c.is_attack()).unwrap_or(false);
    let old = pen.layer;
    pen.layer = LAYER_TOP;
    for i in 0..4 {
        let r = l.p[i];
        let viewing = gm.view == i;
        let hot = aiming && i != 0;
        pen.rrect(art, r.x, r.y, r.w, r.h, 14.0, None, None);
        pen.sliced(&art.rrect_mask, r.x, r.y, r.w, r.h, 14.0, bc(0x8ed5d2));
        if hot {
            pen.rrect(art, r.x, r.y, r.w, r.h, 14.0, Some(bca(0xe96b5f, 0.12 + (t * 6.0).sin() * 0.08)), None);
        }
        let k = (gm.isl[i].ds / GOAL).clamp(0.0, 1.0);
        pen.rect(r.x + 6.0, r.y + r.h - 9.0, r.w - 12.0, 5.0, art, Color::WHITE.with_alpha(0.75));
        pen.rect(r.x + 6.0, r.y + r.h - 9.0, (r.w - 12.0) * k, 5.0, art, bc(TEAM[i].0));
        let threatened = gm.isl[i].threatened();
        let edge = if threatened && (t * 10.0).sin() > 0.0 { bc(0xe96b5f) } else if viewing { bc(TEAM[i].1) } else { ink };
        pen.rrect(art, r.x, r.y, r.w, r.h, 14.0, None, Some(edge));
        if viewing || threatened {
            pen.rrect(art, r.x + 1.5, r.y + 1.5, r.w - 3.0, r.h - 3.0, 12.5, None, Some(edge));
        }
        let ar = (r.h * 0.2).clamp(9.0, 16.0);
        let mood = if gm.isl[i].hit_t > 0.0 { 0 } else { 1 };
        pen.tex(&art.avatars[i][mood], r.x + ar + 5.0, r.y + ar + 6.0, ar / 20.0, Color::WHITE);
        if gm.isl[i].shield > 0.0 {
            pen.tex(&art.shield, r.x + r.w - ar - 4.0, r.y + ar + 6.0, ar / 20.0, Color::WHITE);
        }
        if hot {
            let c = r.center();
            let lift = if pointer.hover_portrait == Some(i) { 1.15 } else { 1.0 };
            pen.tex(&art.crosshair, c.x, c.y, r.h * 0.4 / 32.0 * (1.0 + (t * 6.0).sin() * 0.08) * lift, Color::WHITE);
        }
        if i == 0 {
            pen.tex(&art.heart, r.x + r.w - 14.0, r.y + r.h - 20.0, 1.0, Color::WHITE);
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
        for j in 0..12 {
            if j % 2 == 1 {
                continue;
            }
            let (k0, k1) = (k * j as f32 / 12.0, k * (j as f32 + 1.0) / 12.0);
            g.line_2d(to_screen(w, h, path(k0)), to_screen(w, h, path(k1)), Color::WHITE.with_alpha(0.6));
        }
        let p = path(k);
        card_icon(pen, art, f.card, p.x, p.y, 0.9, 1.0);
        pen.circle(art, p.x - 16.0, p.y - 16.0, 6.0, ink);
        pen.circle(art, p.x - 16.0, p.y - 16.0, 4.8, bc(TEAM[f.from].0));
    }
    pen.layer = old;

    // hand
    for i in 0..4 {
        draw_card(pen, g, art, l, gm, pointer, i);
    }
    let rb = l.reroll;
    pen.button(art, rb, paper, pointer.hover == Some(BtnId::Reroll));
    pen.tex(&art.icon_reroll, rb.x, rb.y, rb.r / 15.0 * 1.1, Color::WHITE);
    if me.rr > 0.0 {
        arc(g, w, h, rb.c(), rb.r + 5.0, PI / 2.0, me.rr / REROLL_CD, bca(crate::art::INK, 0.45));
    }
    let sb = l.shovel;
    pen.button(art, sb, if gm.shovel { bc(0xffd36b) } else { paper }, pointer.hover == Some(BtnId::Shovel));
    pen.tex(&art.icon_shovel, sb.x, sb.y, sb.r / 15.0 * 1.1, Color::WHITE);
    if gm.view != 0 {
        let hb = l.home;
        let urgent = gm.alert > 0.0 || gm.isl[0].has_threats();
        let pul = if urgent { 1.0 + (t * 8.0).sin() * 0.08 } else { 1.0 };
        pen.button(art, Btn { r: hb.r * pul, ..hb }, if gm.isl[0].has_threats() { bc(0xffc0b5) } else { paper }, pointer.hover == Some(BtnId::Home));
        pen.tex(&art.icon_home, hb.x, hb.y, hb.r / 15.0, Color::WHITE);
    }
    // the card being dragged
    if let Some(d) = pointer.drag.as_ref() {
        if d.moved {
            if let Some(c) = me.hand[d.si].card {
                pen.layer = LAYER_TOP;
                card_icon(pen, art, c, pointer.pos.x, pointer.pos.y - 26.0, 1.1, 0.9);
                pen.layer = old;
            }
        }
    }
    draw_tutorial(pen, art, l, gm, view, pointer, settings, t);
}

#[allow(clippy::too_many_arguments)]
fn draw_card(pen: &mut Pen, g: &mut Gizmos<HudGizmos>, art: &Art, l: &Layout, gm: &Game, pointer: &Pointer, i: usize) {
    let s = &gm.players[0].hand[i];
    let r = l.cards[i];
    let ink = bc(crate::art::INK);
    let sel = gm.sel == Some(i);
    let hov = pointer.hover_card == Some(i) && s.card.is_some();
    let lift = if sel { -14.0 } else if hov { -5.0 } else { 0.0 };
    let wig = if s.wig > 0.0 { (s.wig * 30.0).sin() * 5.0 * s.wig } else { 0.0 };
    let (cx, cy) = (r.x + r.w / 2.0 + wig, r.y + r.h / 2.0 + lift);
    let Some(card) = s.card else {
        pen.rrect(art, r.x, r.y, r.w, r.h, 12.0, Some(bca(0xfff6e4, 0.45)), Some(bca(crate::art::INK, 0.35)));
        let k = 1.0 - (s.cd / sim::CARD_CD).clamp(0.0, 1.0);
        arc(g, l.w, l.h, Vec2::new(cx, cy), r.w * 0.2, -PI / 2.0 - k * TAU + TAU, k, bc(0x7fc76f));
        pen.tex(&art.sprout_on, cx, cy + 2.0, 0.4 + k * 0.8, Color::WHITE.with_alpha(0.4 + k * 0.6));
        return;
    };
    let flip = ease_back(s.flip.clamp(0.0, 1.0)).clamp(0.05, 1.2);
    let (w, h) = (r.w * flip, r.h);
    let x0 = cx - w / 2.0;
    let y0 = cy - h / 2.0;
    pen.rrect(art, x0 + 2.0, y0 + 5.0 - lift * 0.4, w, h, 12.0, Some(Color::srgba(0.16, 0.12, 0.12, 0.2)), None);
    let fill = match card {
        Card::Storm | Card::Beetle => bc(0x6b5880),
        Card::Shield => bc(0xdff2ff),
        _ => bc(0xfff6e4),
    };
    pen.rrect(art, x0, y0, w, h, 12.0, Some(fill), Some(if sel { bc(TEAM[0].1) } else { ink }));
    if sel {
        pen.rrect(art, x0 + 1.5, y0 + 1.5, w - 3.0, h - 3.0, 10.5, None, Some(bc(TEAM[0].1)));
    }
    if let Card::Nature(sp) = card {
        let band = if def(sp).plant { bca(0x7fc76f, 0.22) } else { bca(0xffaa6e, 0.22) };
        pen.rect(x0 + 2.5, y0 + h * 0.62, (w - 5.0).max(0.0), h * 0.26, art, band);
    }
    if flip > 0.3 {
        card_icon(pen, art, card, cx, cy - h * 0.06, r.w / 64.0 * flip.min(1.0), 1.0);
        let (gx, gy) = (x0 + 12.0, y0 + 12.0);
        match card {
            Card::Nature(sp) => {
                pen.tex(if def(sp).plant { &art.glyph_leaf } else { &art.glyph_paw }, gx, gy, 1.0, Color::WHITE);
                let terr = def(sp).terr;
                let chips: Vec<u32> = if terr.contains(&Terr::Sand) && terr.len() > 1 {
                    vec![0xa8d77a, 0xf5e0a6]
                } else {
                    vec![match terr[0] {
                        Terr::Pond => 0x86cde6,
                        Terr::Sand => 0xf5e0a6,
                        _ => 0xa8d77a,
                    }]
                };
                let n = chips.len() as f32;
                for (j, c) in chips.iter().enumerate() {
                    let x = cx + (j as f32 - (n - 1.0) / 2.0) * 13.0;
                    let y = y0 + h - 11.0;
                    pen.tex(&art.hex, x, y, 6.5 / HS, ink);
                    pen.tex(&art.hex, x, y, 5.2 / HS, bc(*c));
                }
            }
            Card::Storm | Card::Beetle => pen.tex(&art.glyph_target, gx, gy, 1.0, Color::WHITE),
            Card::Shield => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tutorial(pen: &mut Pen, art: &Art, l: &Layout, gm: &Game, view: &MainView, pointer: &Pointer, settings: &Settings, t: f32) {
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
            pen.ring(art, p.x, p.y, 10.0 + press * 12.0, Color::WHITE.with_alpha(0.9));
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
                        card_icon(pen, art, Card::Nature(sp), p.x, p.y - 22.0, 1.0, 0.7);
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

fn draw_end(pen: &mut Pen, art: &Art, l: &Layout, gm: &Game, pointer: &Pointer, state: &HudState, t: f32) {
    let k = (gm.end_t / 0.6).clamp(0.0, 1.0);
    // dim sits under the island panels (which are live cameras)
    pen.rect(0.0, 0.0, l.w, l.h, art, Color::srgba(0.24, 0.27, 0.31, 0.55 * k));
    let old = pen.layer;
    pen.layer = LAYER_TOP;
    let ink = bc(crate::art::INK);
    for (rank, &p) in gm.ranks.iter().enumerate() {
        let Some(r) = l.end_panels.get(p).copied().flatten() else { continue };
        let edge = if p == 0 { bc(TEAM[0].1) } else { ink };
        pen.rrect(art, r.x, r.y, r.w, r.h, 16.0, None, Some(edge));
        if p == 0 {
            pen.rrect(art, r.x + 1.5, r.y + 1.5, r.w - 3.0, r.h - 3.0, 14.5, None, Some(edge));
        }
        let pl = [46.0, 34.0, 26.0, 20.0][rank];
        let plc = [0xffd96b, 0xe2e6ee, 0xf0c29a, 0xd9d2c6][rank];
        pen.rrect(art, r.x + r.w * 0.15, r.y + r.h + 8.0, r.w * 0.7, pl, 8.0, Some(bc(plc)), Some(ink));
        for j in 0..=rank {
            pen.circle(art, r.x + r.w / 2.0 + (j as f32 - rank as f32 / 2.0) * 10.0, r.y + r.h + 8.0 + pl / 2.0, 3.0, ink);
        }
        let mood = if rank == 0 { 2 } else if rank == 3 { 0 } else { 1 };
        pen.tex(&art.avatars[p][mood], r.x + r.w / 2.0, r.y - 4.0, if rank == 0 { 22.0 } else { 17.0 } / 20.0, Color::WHITE);
        if rank == 0 {
            pen.tex(&art.crown, r.x + r.w / 2.0, r.y - 34.0 + (t * 3.0).sin() * 2.0, 1.0, Color::WHITE);
        }
        let kk = (gm.isl[p].score / GOAL).clamp(0.0, 1.0);
        pen.rrect(art, r.x + 10.0, r.y + r.h - 16.0, r.w - 20.0, 8.0, 4.0, Some(Color::WHITE.with_alpha(0.8)), None);
        pen.rrect(art, r.x + 10.0, r.y + r.h - 16.0, ((r.w - 20.0) * kk).max(8.0), 8.0, 4.0, Some(bc(TEAM[p].0)), None);
        if kk >= 1.0 {
            pen.tex(&art.trophy, r.x + r.w - 12.0, r.y + r.h - 12.0, 0.7, Color::WHITE);
        }
    }
    for c in &state.confetti {
        let spr = Sprite { image: art.pixel.h.clone(), custom_size: Some(Vec2::new(8.0, 5.0)), color: bc(c.c), ..default() };
        pen.spawn(spr, c.x, c.y, c.r, Vec2::ONE);
    }
    if gm.end_t > 1.2 {
        pen.button(art, l.end_again, bc(TEAM[0].0), pointer.hover == Some(BtnId::Again));
        pen.tex(&art.icon_reroll, l.end_again.x, l.end_again.y, 1.6, Color::WHITE);
        pen.button(art, l.end_home, bc(0xfff6e4), pointer.hover == Some(BtnId::EndHome));
        pen.tex(&art.icon_home, l.end_home.x, l.end_home.y, 1.5, Color::WHITE);
    }
    pen.layer = old;
}
