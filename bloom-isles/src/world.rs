//! The world: cameras, island sprites, creatures, threats, particles and in-world overlays.

use crate::art::{self, bc, bca, Art, Tex};
use crate::eco::{def, need_w, Sp};
use crate::hud::Layout;
use crate::input::Pointer;
use crate::island::{Terr, HS, SQ};
use crate::rng::Rng;
use crate::sim::{self, state_of, Card, Ev, Island, PKind, PLAYERS, STORM_T, TITLE, WITHER_T};
use crate::{Ephemeral, Pending, Scene, Session, LAYER_BG, LAYER_OVER, LAYER_WORLD};
use bevy::prelude::*;
use bevy::render::camera::{ClearColorConfig, Viewport};
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;
use std::collections::{HashMap, HashSet};
use std::f32::consts::{PI, TAU};

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct WorldGizmos;

pub fn origin(slot: usize) -> Vec2 {
    if slot == TITLE {
        Vec2::new(-6000.0, 0.0)
    } else {
        Vec2::new(slot as f32 * 6000.0, 0.0)
    }
}

/// Island-space point (y down) to world space (y up).
pub fn wpos(slot: usize, x: f32, y: f32) -> Vec2 {
    origin(slot) + Vec2::new(x, -y)
}

pub fn zfor(y: f32) -> f32 {
    1.0 + (y + 500.0) / 1000.0
}

/// How island space maps onto the screen in the main view: screen = c + p * s.
#[derive(Resource, Default, Clone, Copy)]
pub struct MainView {
    pub s: f32,
    pub cx: f32,
    pub cy: f32,
}

impl MainView {
    pub fn to_screen(&self, x: f32, y: f32) -> Vec2 {
        Vec2::new(self.cx + x * self.s, self.cy + y * self.s)
    }
    pub fn to_local(&self, p: Vec2) -> Vec2 {
        Vec2::new((p.x - self.cx) / self.s, (p.y - self.cy) / self.s)
    }
}

/// Fit an island into a screen rectangle.
pub fn fit(isl: &Island, x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32) {
    let b = isl.map.b;
    let (bw, bh) = (b.x1 - b.x0 + 60.0, b.y1 - b.y0 + 110.0);
    let s = (w / bw).min(h / bh).min(2.4).max(0.05);
    (s, x + w / 2.0 - b.cx() * s, y + h / 2.0 - (b.cy() - 18.0) * s)
}

#[derive(Component)]
pub struct MainCam;
#[derive(Component)]
pub struct PortraitCam(pub usize);
#[derive(Component)]
pub struct BgCam;
#[derive(Component)]
pub struct UiCam;

pub fn spawn_cameras(commands: &mut Commands) {
    commands.spawn((Camera2d, Camera { order: 0, clear_color: ClearColorConfig::Custom(bc(0x8ed5d2)), ..default() }, RenderLayers::layer(LAYER_BG), BgCam));
    commands.spawn((
        Camera2d,
        Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
        RenderLayers::from_layers(&[LAYER_WORLD, LAYER_OVER]),
        MainCam,
    ));
    for i in 0..PLAYERS {
        commands.spawn((
            Camera2d,
            Camera { order: 11 + i as isize, clear_color: ClearColorConfig::Custom(bc(0xa9dfdb)), is_active: false, ..default() },
            RenderLayers::layer(LAYER_WORLD),
            PortraitCam(i),
        ));
    }
    commands.spawn((Camera2d, Camera { order: 10, clear_color: ClearColorConfig::None, ..default() }, RenderLayers::layer(crate::LAYER_HUD), UiCam, IsDefaultUiCamera));
    commands.spawn((Camera2d, Camera { order: 20, clear_color: ClearColorConfig::None, ..default() }, RenderLayers::layer(crate::hud::LAYER_TOP), UiCam));
}

#[derive(Component)]
pub struct SeaGrad;
#[derive(Component)]
pub struct Wave {
    fx: f32,
    fy: f32,
    s: f32,
    p: f32,
}

pub fn spawn_background(commands: &mut Commands, art: &Art) {
    commands.spawn((Sprite { image: art.pixel.h.clone(), color: bc(0x8ed5d2), ..default() }, Transform::from_xyz(0.0, 0.0, 0.0), RenderLayers::layer(LAYER_BG), SeaGrad));
    let mut rng = Rng::new(3);
    for _ in 0..70 {
        commands.spawn((
            Sprite { image: art.wave.h.clone(), color: bca(0xb9e9e2, 0.6), custom_size: Some(art.wave.size), ..default() },
            Transform::from_xyz(0.0, 0.0, 1.0),
            RenderLayers::layer(LAYER_BG),
            Wave { fx: rng.f(), fy: rng.f(), s: 0.6 + rng.f() * 0.8, p: rng.f() * TAU },
        ));
    }
}

fn set_ortho(proj: &mut Projection, scale: f32) {
    if let Projection::Orthographic(o) = proj {
        o.scale = scale;
    }
}

/// Position every camera for this frame.
#[allow(clippy::type_complexity)]
pub fn update_cameras(
    session: Res<Session>,
    layout: Res<Layout>,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut view: ResMut<MainView>,
    mut main: Query<(&mut Transform, &mut Projection), (With<MainCam>, Without<PortraitCam>)>,
    mut portraits: Query<(&PortraitCam, &mut Camera, &mut Transform, &mut Projection), Without<MainCam>>,
    mut bg: Query<(&mut Transform, &mut Sprite), (With<SeaGrad>, Without<MainCam>, Without<PortraitCam>, Without<Wave>)>,
    mut waves: Query<(&Wave, &mut Transform), (Without<MainCam>, Without<PortraitCam>, Without<SeaGrad>)>,
) {
    let (w, h) = (layout.w, layout.h);
    let t = session.t;
    for (mut tf, mut spr) in &mut bg {
        spr.custom_size = Some(Vec2::new(w + 4.0, h + 4.0));
        spr.color = bc(0x8ed5d2);
        tf.translation = Vec3::ZERO;
    }
    for (wv, mut tf) in &mut waves {
        let x = ((wv.fx * w + time.elapsed_secs() * 8.0 * wv.s) % (w + 60.0)) - 30.0;
        let y = wv.fy * h + (t * 0.8 + wv.p).sin() * 3.0;
        tf.translation = Vec3::new(x - w / 2.0, h / 2.0 - y, 1.0);
        tf.scale = Vec3::splat(wv.s);
    }

    let (slot, isl): (usize, &Island) = match (&session.scene, &session.game) {
        (Scene::Title, _) | (_, None) => (TITLE, &session.title),
        (_, Some(g)) => (g.view, &g.isl[g.view]),
    };
    let a = if session.scene == Scene::Title { layout.title_isle } else { layout.isle };
    let (s, cx, cy) = fit(isl, a.x, a.y, a.w, a.h);
    let bob = if session.scene == Scene::Title { (t * 0.9).sin() * 4.0 } else { 0.0 };
    *view = MainView { s, cx, cy: cy + bob };
    let shake = session.game.as_ref().map(|g| g.shake).unwrap_or(0.0);
    let jitter = if shake > 0.0 { Vec2::new((t * 97.0).sin(), (t * 131.0).cos()) * shake * 8.0 } else { Vec2::ZERO };
    if let Ok((mut tf, mut proj)) = main.single_mut() {
        let o = origin(slot);
        let c = Vec2::new(o.x + (w / 2.0 - cx) / s, o.y - (h / 2.0 - (cy + bob)) / s) - jitter / s;
        tf.translation = Vec3::new(c.x, c.y, 100.0);
        set_ortho(&mut proj, 1.0 / s);
    }

    let sf = window.scale_factor();
    let (pw, ph) = (window.physical_width(), window.physical_height());
    for (pc, mut cam, mut tf, mut proj) in &mut portraits {
        let rect = match (session.scene, &session.game) {
            (Scene::Play, Some(_)) => Some(layout.p[pc.0]),
            (Scene::End, Some(_)) => layout.end_panels.get(pc.0).copied().flatten(),
            _ => None,
        };
        let Some(r) = rect else {
            cam.is_active = false;
            continue;
        };
        let x0 = (r.x * sf).max(0.0) as u32;
        let y0 = (r.y * sf).max(0.0) as u32;
        let x1 = ((r.x + r.w) * sf).min(pw as f32) as u32;
        let y1 = ((r.y + r.h) * sf).min(ph as f32) as u32;
        if x1 <= x0 + 2 || y1 <= y0 + 2 {
            cam.is_active = false;
            continue;
        }
        cam.is_active = true;
        cam.viewport = Some(Viewport { physical_position: UVec2::new(x0, y0), physical_size: UVec2::new(x1 - x0, y1 - y0), ..default() });
        let viewing = session.game.as_ref().map(|g| g.view == pc.0 && session.scene == Scene::Play).unwrap_or(false);
        cam.clear_color = ClearColorConfig::Custom(if viewing { bc(0xd6f1ee) } else { bc(0xa9dfdb) });
        let g = session.game.as_ref().unwrap();
        let (vw, vh) = ((x1 - x0) as f32 / sf, (y1 - y0) as f32 / sf);
        let (s, cx, cy) = fit(&g.isl[pc.0], 0.0, 6.0, vw, vh - 6.0);
        let o = origin(pc.0);
        tf.translation = Vec3::new(o.x + (vw / 2.0 - cx) / s, o.y - (vh / 2.0 - cy) / s, 100.0);
        set_ortho(&mut proj, 1.0 / s);
    }
}

/* ---------------- island base sprites ---------------- */

#[derive(Component)]
pub struct IslandPart;
#[derive(Component)]
pub struct LushLayer(pub usize);
#[derive(Component)]
pub struct SeaRing(pub usize);

/// Tracks which island sprites and life sprites exist.
#[derive(Resource, Default)]
pub struct VisIndex {
    gen: u32,
    title: bool,
    map: HashMap<(usize, u32), Entity>,
}

#[derive(Component)]
pub struct LifeVis;

#[allow(clippy::too_many_arguments)]
pub fn sync_islands(
    mut commands: Commands,
    session: Res<Session>,
    mut images: ResMut<Assets<Image>>,
    art: Res<Art>,
    mut index: ResMut<VisIndex>,
    parts: Query<Entity, Or<(With<IslandPart>, With<LifeVis>, With<Particle>)>>,
    mut lush: Query<(&LushLayer, &mut Sprite), Without<SeaRing>>,
    mut sea: Query<(&SeaRing, &mut Transform), Without<LushLayer>>,
) {
    let title = session.scene == Scene::Title || session.game.is_none();
    if index.gen != session.gen || index.title != title {
        for e in &parts {
            commands.entity(e).despawn();
        }
        index.map.clear();
        index.gen = session.gen;
        index.title = title;
        let islands: Vec<(usize, &Island)> = if title { vec![(TITLE, &session.title)] } else { session.game.as_ref().unwrap().isl.iter().enumerate().collect() };
        for (slot, isl) in islands {
            let o = origin(slot);
            let spawn_tex = |commands: &mut Commands, tex: Tex, z: f32, color: Color| {
                commands
                    .spawn((
                        Sprite { image: tex.h.clone(), custom_size: Some(tex.size), anchor: Anchor::Custom(tex.anchor), color, ..default() },
                        Transform::from_xyz(o.x, o.y, z),
                        RenderLayers::layer(LAYER_WORLD),
                        IslandPart,
                    ))
                    .id()
            };
            let sea_t = art::island_sea_tex(&mut images, &isl.map);
            let e = spawn_tex(&mut commands, sea_t, 0.0, Color::WHITE);
            commands.entity(e).insert(SeaRing(slot));
            let dry = art::island_tex(&mut images, &isl.map, 0.0);
            spawn_tex(&mut commands, dry, 0.1, Color::WHITE);
            let lushed = art::island_tex(&mut images, &isl.map, 1.0);
            let e = spawn_tex(&mut commands, lushed, 0.2, Color::WHITE.with_alpha(isl.lush));
            commands.entity(e).insert(LushLayer(slot));
            for t in isl.map.tiles.iter().filter(|t| t.t == Terr::Rock) {
                let tex = &art.rock[if t.shade > 0.5 { 1 } else { 0 }];
                let p = wpos(slot, t.x, t.y + 3.0);
                commands.spawn((
                    Sprite { image: tex.h.clone(), custom_size: Some(tex.size), anchor: Anchor::Custom(tex.anchor), ..default() },
                    Transform::from_xyz(p.x, p.y, zfor(t.y + 3.0)),
                    RenderLayers::layer(LAYER_WORLD),
                    IslandPart,
                ));
            }
        }
    }
    let get = |slot: usize| -> Option<&Island> {
        if slot == TITLE {
            Some(&session.title)
        } else {
            session.game.as_ref().and_then(|g| g.isl.get(slot))
        }
    };
    for (l, mut spr) in &mut lush {
        if let Some(i) = get(l.0) {
            spr.color = Color::WHITE.with_alpha(i.lush.clamp(0.0, 1.0));
        }
    }
    for (s, mut tf) in &mut sea {
        tf.scale = Vec3::splat(1.0 + (session.t * 1.6 + s.0 as f32).sin() * 0.006);
    }
}

/* ---------------- creatures, beetles, storm clouds ---------------- */

const BEETLE_KEY: u32 = 1 << 24;
const STORM_KEY: u32 = 2 << 24;

fn flips(sp: Sp) -> bool {
    matches!(sp, Sp::Rabbit | Sp::Fox | Sp::Bird)
}

/// Transform for a creature this frame: (island-space offset, rotation, scale xy, flip, alpha, tint).
fn creature_pose(e: &sim::Creature, t: f32) -> (Vec2, f32, Vec2, bool, f32, f32) {
    let st = state_of(e.hd);
    let lively = match st {
        2 => 1.6,
        1 => 1.0,
        _ => 0.35,
    };
    let mut off = Vec2::ZERO;
    let mut rot = 0.0;
    let mut sc = Vec2::ONE;
    let mut flip = flips(e.sp) && e.v % 2 == 1;
    let ph = e.ph;
    match e.sp {
        Sp::Flower => rot = (t * 2.0 + ph).sin() * 0.07 * lively,
        Sp::Tree => rot = (t * 1.2 + ph).sin() * 0.025 * lively,
        Sp::Palm => rot = (t * 1.4 + ph).sin() * 0.035 * lively,
        Sp::Bush => sc.y = 1.0 + (t * 1.5 + ph).sin() * 0.03 * lively,
        Sp::Mushroom => sc.y = 1.0 + (t * 2.0 + ph).sin() * 0.03 * lively,
        Sp::Lily => off.y = (t * 1.5 + ph).sin() * 0.8,
        Sp::Bee => {
            let a = t * 1.4 * lively + ph;
            off = Vec2::new(a.cos() * 8.0, -24.0 + (t * 3.0 + ph).sin() * 3.0 * lively);
            flip = a.sin() > 0.0;
        }
        Sp::Rabbit => {
            if st >= 1 {
                off.y = -(t * 3.2 + ph).sin().max(0.0) * 4.0 * if st == 2 { 1.0 } else { 0.4 };
            }
        }
        Sp::Bird => {
            if st == 2 {
                off.y = -(t * 4.0 + ph).sin().max(0.0) * 3.0;
            }
        }
        Sp::Frog => off.y = (t * 2.0 + ph).sin() * 0.8,
        Sp::Crab => off.x = (t * 1.8 * lively + ph).sin() * 3.0,
        Sp::Fox => rot = (t * 3.0 * lively + ph).sin() * 0.03,
    }
    let mut s = 1.0;
    let mut alpha = 1.0;
    if e.born < 0.45 {
        s = sim::ease_back((e.born / 0.45).clamp(0.0, 1.0)).max(0.01);
    }
    if e.dying > 0.0 {
        let k = (e.dying / 0.8).clamp(0.0, 1.0);
        s = if e.how == sim::Death::Wither { 1.0 - k * 0.3 } else { 1.0 - k * 0.7 };
        alpha = 1.0 - k;
    }
    if e.dying == 0.0 && e.sad_t > 7.0 {
        off.x += (t * 30.0).sin() * ((e.sad_t - 7.0) / 9.0).clamp(0.0, 1.0) * 1.8;
    }
    let tint = if st == 0 && e.dying == 0.0 { 0.82 } else { 1.0 };
    (off, rot, sc * s, flip, alpha, tint)
}

#[allow(clippy::type_complexity)]
pub fn sync_life(
    mut commands: Commands,
    session: Res<Session>,
    art: Res<Art>,
    mut index: ResMut<VisIndex>,
    mut q: Query<(&mut Transform, &mut Sprite), With<LifeVis>>,
) {
    let t = session.t;
    let islands: Vec<(usize, &Island)> = match (&session.scene, &session.game) {
        (Scene::Title, _) | (_, None) => vec![(TITLE, &session.title)],
        (_, Some(g)) => g.isl.iter().enumerate().collect(),
    };
    let mut seen: HashSet<(usize, u32)> = HashSet::new();
    // (key, texture, island-space ground point, offset, rotation, scale, flip, colour, z)
    let mut want: Vec<((usize, u32), Tex, Vec2, f32, Vec2, bool, Color, f32)> = vec![];
    for (slot, isl) in &islands {
        for e in &isl.ents {
            let (off, rot, sc, flip, alpha, tint) = creature_pose(e, t);
            let tex = art.sp(e.sp, e.v).clone();
            let p = wpos(*slot, e.x + off.x, e.y + off.y);
            let color = Color::srgba(tint, tint, tint * 1.04, alpha * if tint < 1.0 { 0.85 } else { 1.0 });
            want.push(((*slot, e.id), tex, p, rot, sc, flip, color, zfor(e.y)));
        }
        for b in &isl.beetles {
            let mut alpha = 1.0;
            let mut sc = Vec2::ONE;
            let mut lift = 0.0;
            if b.dead > 0.0 {
                alpha = 1.0 - b.dead / 0.7;
                sc = Vec2::new(1.2, 0.35);
            } else if b.leave {
                alpha = b.lift;
                lift = (1.0 - b.lift) * 30.0;
            }
            let wob = if b.eat > 0.0 { (t * 20.0).sin() * 0.08 } else { (t * 12.0).sin() * 0.05 };
            let p = wpos(*slot, b.x, b.y - lift);
            want.push(((*slot, BEETLE_KEY + b.id), art.beetle.clone(), p, wob, sc, b.dir.cos() < 0.0, Color::WHITE.with_alpha(alpha.max(0.0)), zfor(b.y)));
        }
        for s in &isl.storms {
            let alpha = if s.blown { (1.0 - s.bt).clamp(0.0, 1.0) } else if s.struck { (1.0 - (s.st - 1.0) / 0.8).clamp(0.0, 1.0) } else { 1.0 };
            let k = 1.5 * s.sc * (1.0 + s.pulse * 0.15);
            let p = wpos(*slot, s.cx, s.cy);
            want.push(((*slot, STORM_KEY + s.id), art.cloud.clone(), p, 0.0, Vec2::splat(k), false, Color::WHITE.with_alpha(alpha), 4.0));
        }
    }
    for (key, tex, p, rot, sc, flip, color, z) in want {
        seen.insert(key);
        let tf = Transform::from_xyz(p.x, p.y, z).with_rotation(Quat::from_rotation_z(-rot)).with_scale(Vec3::new(sc.x, sc.y, 1.0));
        match index.map.get(&key).copied() {
            Some(ent) => {
                if let Ok((mut t0, mut spr)) = q.get_mut(ent) {
                    *t0 = tf;
                    spr.flip_x = flip;
                    spr.color = color;
                }
            }
            None => {
                let ent = commands
                    .spawn((
                        Sprite { image: tex.h.clone(), custom_size: Some(tex.size), anchor: Anchor::Custom(tex.anchor), flip_x: flip, color, ..default() },
                        tf,
                        RenderLayers::layer(LAYER_WORLD),
                        LifeVis,
                    ))
                    .id();
                index.map.insert(key, ent);
            }
        }
    }
    let gone: Vec<(usize, u32)> = index.map.keys().filter(|k| !seen.contains(k)).copied().collect();
    for k in gone {
        if let Some(e) = index.map.remove(&k) {
            commands.entity(e).despawn();
        }
    }
}

/* ---------------- particles ---------------- */

#[derive(Component)]
pub struct Particle {
    slot: usize,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max: f32,
    kind: PKind,
    rot: f32,
}

pub fn spawn_particles(mut commands: Commands, mut pending: ResMut<Pending>, art: Res<Art>, mut rng: Local<Option<Rng>>, count: Query<(), With<Particle>>) {
    let rng = rng.get_or_insert_with(Rng::from_time);
    let mut budget = 500usize.saturating_sub(count.iter().count());
    let events: Vec<Ev> = pending.ev.iter().filter(|e| matches!(e, Ev::Burst { .. } | Ev::Float { .. })).cloned().collect();
    pending.ev.retain(|e| !matches!(e, Ev::Burst { .. } | Ev::Float { .. }));
    for e in events {
        let (slot, x, y, kind, n, float) = match e {
            Ev::Burst { isl, x, y, kind, n } => (isl, x, y, kind, n, false),
            Ev::Float { isl, x, y, kind } => (isl, x, y, kind, 1, true),
            _ => continue,
        };
        for _ in 0..n {
            if budget == 0 {
                return;
            }
            budget -= 1;
            let (vx, vy, max) = if float {
                ((rng.f() - 0.5) * 10.0, -26.0, 1.4)
            } else if kind == PKind::Drop {
                (-20.0, 200.0, 0.45)
            } else {
                let a = rng.f() * TAU;
                let sp = 20.0 + rng.f() * 50.0;
                (a.cos() * sp, a.sin() * sp * 0.6 - 30.0, 0.6 + rng.f() * 0.6)
            };
            let (tex, color, size) = match kind {
                PKind::Heart => (&art.heart, Color::WHITE, 1.0),
                PKind::Spark => (&art.spark, Color::WHITE, 1.0),
                PKind::Leaf => (&art.leaf, bc(*rng.pick(&[0x8fd070, 0x6fbe5c, 0xb6e37b])), 1.0),
                PKind::LeafDead => (&art.leaf, bc(*rng.pick(&[0xc9a46a, 0xa88b5c])), 1.0),
                PKind::Puff => (&art.circle, Color::WHITE, 0.2),
                PKind::Soot => (&art.circle, bc(0x6d6478), 0.2),
                PKind::Splat => (&art.circle, bc(0x9b7fc0), 0.16),
                PKind::Drop => (&art.drop, Color::WHITE, 1.0),
            };
            let p = wpos(slot, x, y);
            commands.spawn((
                Sprite { image: tex.h.clone(), custom_size: Some(tex.size * size), color, ..default() },
                Transform::from_xyz(p.x, p.y, 3.0),
                RenderLayers::layer(LAYER_WORLD),
                Particle { slot, x, y, vx, vy, life: 0.0, max, kind, rot: rng.f() * TAU },
            ));
        }
    }
}

pub fn move_particles(mut commands: Commands, time: Res<Time>, session: Res<Session>, mut q: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>) {
    let paused = session.game.as_ref().map(|g| g.paused).unwrap_or(false);
    let dt = if paused { 0.0 } else { time.delta_secs().min(0.05) };
    for (ent, mut p, mut tf, mut spr) in &mut q {
        p.life += dt;
        if p.life >= p.max {
            commands.entity(ent).despawn();
            continue;
        }
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        match p.kind {
            PKind::Heart | PKind::Spark => {
                p.vx *= 0.96;
                p.vy *= 0.98;
            }
            PKind::Drop => {}
            _ => {
                p.vy += 120.0 * dt;
                p.vx *= 0.97;
            }
        }
        let k = p.life / p.max;
        let pos = wpos(p.slot, p.x, p.y);
        tf.translation = Vec3::new(pos.x, pos.y, 3.0);
        let a = 1.0 - k;
        let base = spr.color;
        spr.color = base.with_alpha(a);
        match p.kind {
            PKind::Puff | PKind::Soot => tf.scale = Vec3::splat(1.0 + k * 2.2),
            PKind::Leaf | PKind::LeafDead => tf.rotation = Quat::from_rotation_z(-(p.rot + p.life * 5.0)),
            PKind::Spark => tf.rotation = Quat::from_rotation_z(-p.life * 3.0),
            _ => {}
        }
    }
}

/* ---------------- overlays ---------------- */

/// World-space immediate-mode sprite (lives one frame).
pub struct WPen<'a, 'w, 's> {
    pub cmd: &'a mut Commands<'w, 's>,
}

impl WPen<'_, '_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub fn tex(&mut self, tex: &Tex, slot: usize, x: f32, y: f32, scale: Vec2, color: Color, z: f32, layer: usize) {
        let p = wpos(slot, x, y);
        self.cmd.spawn((
            Sprite { image: tex.h.clone(), custom_size: Some(tex.size), anchor: Anchor::Custom(tex.anchor), color, ..default() },
            Transform::from_xyz(p.x, p.y, z).with_scale(Vec3::new(scale.x, scale.y, 1.0)),
            RenderLayers::layer(layer),
            Ephemeral,
        ));
    }
}

fn state_color(st: u8) -> Color {
    match st {
        2 => bc(0x6cc26a),
        1 => bc(0xf4c64e),
        _ => bc(0xe96b5f),
    }
}

fn dashed_curve(g: &mut Gizmos<WorldGizmos>, a: Vec2, ctrl: Vec2, b: Vec2, color: Color, offset: f32) {
    let n = 24;
    let pt = |k: f32| a * (1.0 - k) * (1.0 - k) + ctrl * 2.0 * k * (1.0 - k) + b * k * k;
    for i in 0..n {
        if (i + (offset as i32).rem_euclid(2) as usize) % 2 == 1 {
            continue;
        }
        let (k0, k1) = (i as f32 / n as f32, (i as f32 + 0.8) / n as f32);
        g.line_2d(pt(k0), pt(k1), color);
    }
}

fn ring(g: &mut Gizmos<WorldGizmos>, c: Vec2, r: f32, frac: f32, color: Color) {
    let n = (48.0 * frac).ceil().max(2.0) as usize;
    let pts: Vec<Vec2> = (0..=n)
        .map(|i| {
            let a = PI / 2.0 - frac * TAU * i as f32 / n as f32;
            c + Vec2::new(a.cos(), a.sin()) * r
        })
        .collect();
    g.linestrip_2d(pts, color);
}

/// Hearts and lines showing who helps (or bothers) whom around a tile.
fn relation_lines(pen: &mut WPen, g: &mut Gizmos<WorldGizmos>, art: &Art, isl: &Island, slot: usize, tile: usize, sp: Sp, from: Vec2, t: f32) {
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
        let to = Vec2::new(e.x, e.y - def(usp).head * 0.5);
        let mid = Vec2::new((from.x + to.x) / 2.0, from.y.min(to.y) - 16.0);
        let good = w > 0.0;
        let col = if good { bca(0x6cc26a, 0.95) } else { bca(0xe96b5f, 0.95) };
        let o = origin(slot);
        let flipy = |p: Vec2| Vec2::new(o.x + p.x, o.y - p.y);
        dashed_curve(g, flipy(from), flipy(mid), flipy(to), col, t * 4.0);
        let hp = (from + mid * 2.0 + to) / 4.0;
        let s = (4.5 + (w.abs() * 12.0).min(3.0)) / 5.0;
        pen.tex(if good { &art.heart } else { &art.heart_broken }, slot, hp.x, hp.y, Vec2::splat(s), Color::WHITE, 8.0, LAYER_OVER);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_overlays(mut commands: Commands, session: Res<Session>, art: Res<Art>, pointer: Res<Pointer>, mut g: Gizmos<WorldGizmos>) {
    let t = session.t;
    let mut pen = WPen { cmd: &mut commands };
    let islands: Vec<(usize, &Island)> = match (&session.scene, &session.game) {
        (Scene::Title, _) | (_, None) => vec![(TITLE, &session.title)],
        (_, Some(gm)) => gm.isl.iter().enumerate().collect(),
    };
    // things every camera sees: sad clouds, bee shadows, storm shadows, bolts, shields
    for (slot, isl) in &islands {
        for e in &isl.ents {
            if e.dying > 0.0 {
                continue;
            }
            if e.sp == Sp::Bee {
                let a = t * 1.4 + e.ph;
                pen.tex(&art.shadow, *slot, e.x + a.cos() * 8.0, e.y, Vec2::new(0.35, 0.4), bca(0x3c3228, 0.13), zfor(e.y) - 0.0005, LAYER_WORLD);
            }
            if state_of(e.hd) == 0 && session.scene != Scene::Title {
                let hy = e.y - def(e.sp).head - 10.0;
                pen.tex(&art.cloud, *slot, e.x, hy, Vec2::splat(0.35), Color::WHITE.with_alpha(0.9), 2.9, LAYER_WORLD);
                let dk = (t * 1.5 + e.ph) % 1.0;
                pen.tex(&art.circle, *slot, e.x - 2.0, hy + 7.0 + dk * 8.0, Vec2::splat(0.1), bca(0x7fb8f0, 1.0 - dk), 2.9, LAYER_WORLD);
            }
        }
        for s in &isl.storms {
            if s.active() {
                let warn = (s.t / STORM_T).clamp(0.0, 1.0);
                let r = HS * (1.4 + warn * 0.9) * s.sc;
                pen.tex(&art.circle, *slot, s.x, s.y, Vec2::new(r / 16.0, r * SQ / 16.0), Color::srgba(0.16, 0.12, 0.24, 0.1 + warn * 0.22), 0.5, LAYER_WORLD);
            }
            if s.struck && s.st < 0.35 {
                pen.tex(&art.bolt, *slot, s.x, s.y - 36.0, Vec2::splat(2.4), Color::WHITE.with_alpha(1.0 - s.st / 0.35), 4.5, LAYER_WORLD);
            }
        }
        if isl.shield > 0.0 {
            let b = isl.map.b;
            let blink = if isl.shield < 4.0 && (t * 16.0).sin() < 0.0 { 0.5 } else { 1.0 };
            let (rx, ry) = ((b.x1 - b.x0) / 2.0 + 26.0, (b.y1 - b.y0) / 2.0 + 60.0);
            pen.tex(&art.dome, *slot, b.cx(), b.cy() + 10.0, Vec2::new(rx / 96.0, ry / 96.0), Color::WHITE.with_alpha(blink), 5.0, LAYER_WORLD);
        }
    }

    let Some(gm) = session.game.as_ref() else { return };
    if session.scene != Scene::Play || gm.paused {
        return;
    }
    let slot = gm.view;
    let isl = &gm.isl[slot];
    // main view only: storm timers and tap pips, beetle munch progress
    for s in &isl.storms {
        if !s.active() {
            continue;
        }
        let warn = (s.t / STORM_T).clamp(0.0, 1.0);
        let c = wpos(slot, s.cx + 38.0 * s.sc, s.cy - 22.0 * s.sc);
        ring(&mut g, c, 7.0, 1.0 - warn, bc(0xe96b5f));
        for i in 0..s.need {
            let x = s.cx - (s.need as f32 - 1.0) * 5.0 + i as f32 * 10.0;
            let col = if i < s.taps { Color::WHITE } else { Color::WHITE.with_alpha(0.35) };
            pen.tex(&art.circle, slot, x, s.cy + 30.0 * s.sc, Vec2::splat(0.19), col, 6.0, LAYER_OVER);
        }
        // danger zone
        let n = 40;
        let o = wpos(slot, s.x, s.y);
        for i in (0..n).step_by(2) {
            let (a0, a1) = (i as f32 / n as f32 * TAU, (i as f32 + 1.0) / n as f32 * TAU);
            let p = |a: f32| o + Vec2::new(a.cos() * HS * 2.2, -a.sin() * HS * 2.2 * SQ);
            g.line_2d(p(a0), p(a1), bca(0xe96b5f, 0.4 + (t * 10.0).sin() * 0.3));
        }
    }
    for b in &isl.beetles {
        if b.active() && b.eat > 0.0 {
            ring(&mut g, wpos(slot, b.x, b.y - 20.0), 6.0, b.eat / 3.0, bc(0xe96b5f));
        }
    }

    let sel = gm.selected();
    let hover = pointer.hover_tile;
    match sel {
        Some(Card::Nature(sp)) if slot == 0 => {
            let pul = 1.0 + (t * 4.0).sin() * 0.12;
            for (tile, _v, h) in gm.hints(sp) {
                let tl = &isl.map.tiles[tile];
                pen.tex(&art.hex, slot, tl.x, tl.y, Vec2::splat(0.92), Color::WHITE.with_alpha(0.22), 0.3, LAYER_OVER);
                let st = state_of(h);
                let r = if st == 2 { 5.0 * pul } else { 3.6 };
                pen.tex(&art.circle, slot, tl.x, tl.y, Vec2::splat((r + 1.6) / 16.0), Color::WHITE, 0.31, LAYER_OVER);
                pen.tex(&art.circle, slot, tl.x, tl.y, Vec2::splat(r / 16.0), state_color(st), 0.32, LAYER_OVER);
            }
            if let Some(tile) = hover {
                let tl = &isl.map.tiles[tile];
                if isl.can_place(tile, sp) {
                    let (_, h) = isl.place_value(tile, sp);
                    relation_lines(&mut pen, &mut g, &art, isl, slot, tile, sp, Vec2::new(tl.x, tl.y - def(sp).head * 0.5), t);
                    pen.tex(art.sp(sp, 1), slot, tl.x, tl.y - if sp == Sp::Bee { 24.0 } else { 0.0 }, Vec2::ONE, Color::WHITE.with_alpha(0.75), 7.0, LAYER_OVER);
                    pen.tex(&art.faces[state_of(h) as usize], slot, tl.x, tl.y - def(sp).head - 16.0, Vec2::splat(0.9), Color::WHITE, 9.0, LAYER_OVER);
                } else {
                    let o = wpos(slot, tl.x, tl.y);
                    g.line_2d(o + Vec2::new(-7.0, 5.0), o + Vec2::new(7.0, -5.0), bc(0xe96b5f));
                    g.line_2d(o + Vec2::new(7.0, 5.0), o + Vec2::new(-7.0, -5.0), bc(0xe96b5f));
                }
            }
        }
        Some(Card::Storm) if slot != 0 => {
            if let Some(tile) = hover {
                let tl = &isl.map.tiles[tile];
                let o = wpos(slot, tl.x, tl.y);
                let n = 40;
                for i in (0..n).step_by(2) {
                    let (a0, a1) = (i as f32 / n as f32 * TAU, (i as f32 + 1.0) / n as f32 * TAU);
                    let p = |a: f32| o + Vec2::new(a.cos() * HS * 2.2, -a.sin() * HS * 2.2 * SQ);
                    g.line_2d(p(a0), p(a1), bc(0xe96b5f));
                }
                pen.tex(&art.cloud, slot, tl.x, tl.y - 70.0, Vec2::splat(1.2), Color::WHITE.with_alpha(0.7), 8.0, LAYER_OVER);
            }
        }
        None if gm.shovel && slot == 0 => {
            for tl in isl.map.tiles.iter().filter(|t| t.occ.is_some()) {
                pen.tex(&art.hex_line, slot, tl.x, tl.y, Vec2::splat(0.9), Color::WHITE.with_alpha(0.55), 0.3, LAYER_OVER);
            }
            if let Some(tile) = hover {
                let tl = &isl.map.tiles[tile];
                if tl.occ.is_some() {
                    pen.tex(&art.hex_line, slot, tl.x, tl.y, Vec2::splat(0.95), bc(0xe96b5f), 0.35, LAYER_OVER);
                }
            }
        }
        None => {
            if let Some(tile) = gm.inspect {
                if let Some((id, sp)) = isl.map.tiles.get(tile).and_then(|t| t.occ) {
                    if let Some(e) = isl.ent(id) {
                        relation_lines(&mut pen, &mut g, &art, isl, slot, tile, sp, Vec2::new(e.x, e.y - def(sp).head * 0.5), t);
                        let fy = e.y - def(sp).head - 16.0;
                        let st = state_of(e.h);
                        pen.tex(&art.faces[st as usize], slot, e.x, fy, Vec2::splat(0.9), Color::WHITE, 9.0, LAYER_OVER);
                        if st == 0 && e.sad_t > 0.0 {
                            ring(&mut g, wpos(slot, e.x, fy), 12.0, 1.0 - e.sad_t / WITHER_T, bc(0xe96b5f));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
