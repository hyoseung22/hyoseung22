//! The 3D world: cameras, island meshes, creatures, threats, particles, and
//! the little studio that renders model icons for the interface.
//!
//! Island-space points (x, y) follow the old 2D layout: y already carries the
//! ground foreshortening. In 3D a point sits at (x, height, y / SQ); the camera
//! pitch is asin(SQ), so ground points land on screen exactly where they did.

use crate::art::{bc, bca, pal, Art, Tex};
use crate::eco::{def, Sp, ALL};
use crate::hud::Layout;
use crate::island::{Terr, SQ};
use crate::models::{self, pitch, terrain_h, Model};
use crate::rng::Rng;
use crate::sim::{self, state_of, Ev, Island, PKind, PLAYERS, TITLE};
use crate::{Pending, Scene, Session, LAYER_BG, LAYER_WORLD};
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::camera::{ClearColorConfig, RenderTarget, ScalingMode, Viewport};
use bevy::render::render_resource::{Extent3d, Face, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;
use std::collections::{HashMap, HashSet};
use std::f32::consts::{PI, TAU};

/// Pieces are drawn a bit larger than the tile grid would suggest, like chunky board-game figures.
pub const PIECE: f32 = 1.45;

/// Render layer used only by the icon studio.
pub const LAYER_STUDIO: usize = 5;

pub fn origin(slot: usize) -> Vec3 {
    if slot == TITLE {
        Vec3::new(-8000.0, 0.0, 0.0)
    } else {
        Vec3::new(slot as f32 * 8000.0, 0.0, 0.0)
    }
}

/// Island-space point and height to world space.
pub fn wpos(slot: usize, x: f32, y: f32, h: f32) -> Vec3 {
    origin(slot) + Vec3::new(x, h, y / SQ)
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
    /// Screen point of something `h` units above the ground at (x, y).
    pub fn to_screen_h(&self, x: f32, y: f32, h: f32) -> Vec2 {
        Vec2::new(self.cx + x * self.s, self.cy + (y - h * pitch().cos()) * self.s)
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

/// Height of the ground under an island-space point.
pub fn ground_h(isl: &Island, x: f32, y: f32) -> f32 {
    isl.map.tile_at(x, y).map(|i| terrain_h(isl.map.tiles[i].t)).unwrap_or(0.0)
}

/* ---------------- meshes and materials ---------------- */

#[derive(Clone)]
pub struct Mh {
    pub body: Handle<Mesh>,
    pub outline: Option<Handle<Mesh>>,
    pub lo: Vec3,
    pub hi: Vec3,
}

#[derive(Resource, Clone)]
pub struct Models3d {
    pub flat: Handle<StandardMaterial>,
    pub flat_sad: Handle<StandardMaterial>,
    pub outline: Handle<StandardMaterial>,
    pub shadow: Handle<StandardMaterial>,
    pub dome_mat: Handle<StandardMaterial>,
    pub sp: Vec<Vec<Mh>>,
    pub rock: [Mh; 2],
    pub cloud: Mh,
    pub bolt: Mh,
    pub beetle: Mh,
    pub whale: Mh,
    pub pawns: Vec<[Mh; 3]>,
    pub pawn_white: Mh,
    pub disc: Handle<Mesh>,
    pub dome: Handle<Mesh>,
}

pub fn upload(meshes: &mut Assets<Mesh>, m: &Model) -> Mh {
    let (lo, hi) = m.bounds();
    Mh { body: meshes.add(m.body()), outline: m.outline().map(|o| meshes.add(o)), lo, hi }
}

pub fn build_models(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>) -> Models3d {
    let flat = mats.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() });
    let flat_sad = mats.add(StandardMaterial { base_color: Color::srgb(0.62, 0.62, 0.66), unlit: true, ..default() });
    let outline = mats.add(StandardMaterial { base_color: bc(models::INK), unlit: true, cull_mode: Some(Face::Front), ..default() });
    let shadow = mats.add(StandardMaterial { base_color: Color::srgba(0.03, 0.08, 0.1, 0.28), unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
    let dome_mat = mats.add(StandardMaterial { base_color: Color::srgba(0.7, 0.92, 1.0, 0.14), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, ..default() });
    let sp = ALL.iter().map(|&s| (0..4u8).map(|v| upload(meshes, &models::species(s, v))).collect()).collect();
    let team = crate::art::TEAM;
    Models3d {
        flat,
        flat_sad,
        outline,
        shadow,
        dome_mat,
        sp,
        rock: [upload(meshes, &models::rock(false)), upload(meshes, &models::rock(true))],
        cloud: upload(meshes, &models::cloud()),
        bolt: upload(meshes, &models::bolt()),
        beetle: upload(meshes, &models::beetle()),
        whale: upload(meshes, &models::whale()),
        pawns: (0..4).map(|i| [0u8, 1, 2].map(|m| upload(meshes, &models::pawn(team[i].0, m)))).collect(),
        pawn_white: upload(meshes, &models::pawn(0xeeeeee, 1)),
        disc: meshes.add(Cylinder::new(1.0, 0.2).mesh().resolution(18).build()),
        dome: meshes.add(Sphere::new(1.0).mesh().ico(2).expect("dome")),
    }
}

/// A model: an invisible parent with the flat body and its outline shell as children.
pub fn spawn_model(commands: &mut Commands, mh: &Mh, m3: &Models3d, tf: Transform, layer: usize) -> (Entity, Entity) {
    let parent = commands.spawn((tf, Visibility::default())).id();
    let body = commands.spawn((Mesh3d(mh.body.clone()), MeshMaterial3d(m3.flat.clone()), Transform::IDENTITY, RenderLayers::layer(layer))).id();
    commands.entity(parent).add_child(body);
    if let Some(o) = &mh.outline {
        let ol = commands.spawn((Mesh3d(o.clone()), MeshMaterial3d(m3.outline.clone()), Transform::IDENTITY, RenderLayers::layer(layer))).id();
        commands.entity(parent).add_child(ol);
    }
    (parent, body)
}

/* ---------------- cameras ---------------- */

#[derive(Component)]
pub struct MainCam;
#[derive(Component)]
pub struct PortraitCam(pub usize);
#[derive(Component)]
pub struct BgCam;
#[derive(Component)]
pub struct UiCam;

fn ortho() -> Projection {
    Projection::from(OrthographicProjection { scaling_mode: ScalingMode::WindowSize, near: -6000.0, far: 6000.0, ..OrthographicProjection::default_3d() })
}

/// Camera looking at world point `f` from the board-game angle.
fn look(f: Vec3) -> Transform {
    let p = pitch();
    Transform::from_translation(f + Vec3::new(0.0, p.sin(), p.cos()) * 2000.0).looking_at(f, Vec3::Y)
}

pub fn spawn_cameras(commands: &mut Commands) {
    commands.spawn((Camera2d, Camera { order: 0, clear_color: ClearColorConfig::Custom(bc(pal::SEA)), ..default() }, RenderLayers::layer(LAYER_BG), BgCam));
    commands.spawn((Camera3d::default(), Camera { order: 1, clear_color: ClearColorConfig::None, ..default() }, ortho(), Tonemapping::None, RenderLayers::layer(LAYER_WORLD), MainCam));
    for i in 0..PLAYERS {
        commands.spawn((
            Camera3d::default(),
            Camera { order: 11 + i as isize, clear_color: ClearColorConfig::Custom(bc(pal::PORTRAIT)), is_active: false, ..default() },
            ortho(),
            Tonemapping::None,
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
    commands.spawn((Sprite { image: art.pixel.h.clone(), color: bc(pal::SEA), ..default() }, Transform::from_xyz(0.0, 0.0, 0.0), RenderLayers::layer(LAYER_BG), SeaGrad));
    let mut rng = Rng::new(3);
    for _ in 0..70 {
        commands.spawn((
            Sprite { image: art.wave.h.clone(), color: bca(pal::WAVE, 0.45), custom_size: Some(art.wave.size), ..default() },
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
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
    mut smooth: Local<Option<(usize, bool, f32, f32, f32)>>,
) {
    let (w, h) = (layout.w, layout.h);
    let t = session.t;
    let weather = session.game.as_ref().filter(|_| session.scene == Scene::Play).map(|g| g.weather).unwrap_or(sim::Weather::Clear);
    let sea = match weather {
        sim::Weather::Rain => 0x1f6c85,
        sim::Weather::Drought => 0x3294a6,
        sim::Weather::Wind => 0x2680a0,
        sim::Weather::Clear => pal::SEA,
    };
    for (mut tf, mut spr) in &mut bg {
        spr.custom_size = Some(Vec2::new(w + 4.0, h + 4.0));
        let cur = spr.color.to_srgba();
        let tgt = bc(sea).to_srgba();
        let k = 1.0 - (-time.delta_secs() * 1.5).exp();
        spr.color = Color::srgb(cur.red + (tgt.red - cur.red) * k, cur.green + (tgt.green - cur.green) * k, cur.blue + (tgt.blue - cur.blue) * k);
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
    let title = session.scene == Scene::Title;
    let a = if title { layout.title_isle } else { layout.isle };
    let (ts, tcx, tcy) = fit(isl, a.x, a.y, a.w, a.h);
    // ease toward the target so a growing island zooms out gently
    let (s, cx, cy) = match *smooth {
        Some((sl, ti, s0, cx0, cy0)) if sl == slot && ti == title => {
            let k = 1.0 - (-time.delta_secs() * 4.0).exp();
            (s0 + (ts - s0) * k, cx0 + (tcx - cx0) * k, cy0 + (tcy - cy0) * k)
        }
        _ => (ts, tcx, tcy),
    };
    *smooth = Some((slot, title, s, cx, cy));
    let bob = if title { (t * 0.9).sin() * 4.0 } else { 0.0 };
    *view = MainView { s, cx, cy: cy + bob };
    let shake = session.game.as_ref().map(|g| g.shake).unwrap_or(0.0);
    let jitter = if shake > 0.0 { Vec2::new((t * 97.0).sin(), (t * 131.0).cos()) * shake * 8.0 } else { Vec2::ZERO };
    if let Ok((mut tf, mut proj)) = main.single_mut() {
        let lx = (w / 2.0 - cx - jitter.x) / s;
        let ly = (h / 2.0 - (cy + bob) - jitter.y) / s;
        *tf = look(wpos(slot, lx, ly, 0.0));
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
        cam.clear_color = ClearColorConfig::Custom(if viewing { bc(pal::PORTRAIT_VIEW) } else { bc(pal::PORTRAIT) });
        let g = session.game.as_ref().unwrap();
        let (vw, vh) = ((x1 - x0) as f32 / sf, (y1 - y0) as f32 / sf);
        let (s, cx, cy) = fit(&g.isl[pc.0], 0.0, 6.0, vw, vh - 6.0);
        *tf = look(wpos(pc.0, (vw / 2.0 - cx) / s, (vh / 2.0 - cy) / s, 0.0));
        set_ortho(&mut proj, 1.0 / s);
    }
}

/* ---------------- islands ---------------- */

#[derive(Component)]
pub struct IslandPart(pub usize);

/// Tracks which island meshes and life entities exist.
#[derive(Resource, Default)]
pub struct VisIndex {
    gen: u32,
    title: bool,
    /// slot -> (map version, lushness step, soil version, time built)
    built: HashMap<usize, (u32, u8, u32, f32)>,
    map: HashMap<(usize, u32), (Entity, Entity)>,
}

#[derive(Component)]
pub struct LifeVis;

fn build_island(commands: &mut Commands, meshes: &mut Assets<Mesh>, m3: &Models3d, slot: usize, isl: &Island, lush: f32) {
    let o = origin(slot);
    let part = |commands: &mut Commands, mh: &Mh| {
        let (e, _) = spawn_model(commands, mh, m3, Transform::from_translation(o), LAYER_WORLD);
        commands.entity(e).insert(IslandPart(slot));
    };
    part(commands, &upload(meshes, &models::island(&isl.map, lush)));
    part(commands, &upload(meshes, &models::shallows(&isl.map)));
    for t in isl.map.tiles.iter().filter(|t| t.t == Terr::Rock) {
        let tf = Transform::from_translation(wpos(slot, t.x, t.y, terrain_h(t.t))).with_rotation(Quat::from_rotation_y(t.shade * TAU));
        let (e, _) = spawn_model(commands, &m3.rock[if t.shade > 0.5 { 1 } else { 0 }], m3, tf, LAYER_WORLD);
        commands.entity(e).insert(IslandPart(slot));
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_islands(
    mut commands: Commands,
    session: Res<Session>,
    mut meshes: ResMut<Assets<Mesh>>,
    m3: Res<Models3d>,
    mut index: ResMut<VisIndex>,
    parts: Query<(Entity, Option<&IslandPart>), Or<(With<IslandPart>, With<LifeVis>, With<Particle>)>>,
) {
    let title = session.scene == Scene::Title || session.game.is_none();
    if index.gen != session.gen || index.title != title {
        for (e, _) in &parts {
            commands.entity(e).despawn();
        }
        index.map.clear();
        index.built.clear();
        index.gen = session.gen;
        index.title = title;
    }
    let islands: Vec<(usize, &Island)> = if title { vec![(TITLE, &session.title)] } else { session.game.as_ref().unwrap().isl.iter().enumerate().collect() };
    let mut rebuilt = false;
    for (slot, isl) in &islands {
        let step = (isl.lush.clamp(0.0, 1.0) * 10.0).round() as u8;
        let urgent = match index.built.get(slot) {
            None => true,
            Some(b) => b.0 != isl.map_ver || b.1 != step,
        };
        // soil tints drift constantly: repaint at most one island per frame, each every 2 s
        let soil_stale = index.built.get(slot).map(|b| b.2 != isl.soil_ver && session.t - b.3 > 2.0).unwrap_or(false);
        if !urgent && !(soil_stale && !rebuilt) {
            continue;
        }
        rebuilt = true;
        // first build, the island grew, or it got noticeably lusher
        for (e, part) in &parts {
            if part.map(|p| p.0 == *slot).unwrap_or(false) {
                commands.entity(e).despawn();
            }
        }
        build_island(&mut commands, &mut meshes, &m3, *slot, isl, step as f32 / 10.0);
        index.built.insert(*slot, (isl.map_ver, step, isl.soil_ver, session.t));
    }
}

/* ---------------- creatures, beetles, clouds, whales ---------------- */

const BEETLE_KEY: u32 = 1 << 24;
const STORM_KEY: u32 = 2 << 24;
const WHALE_KEY: u32 = 3 << 24;
const SHADOW_KEY: u32 = 4 << 24;
const BOLT_KEY: u32 = 5 << 24;
const DOME_KEY: u32 = 6 << 24;

fn faces_left(sp: Sp, v: u8) -> bool {
    matches!(sp, Sp::Rabbit | Sp::Fox | Sp::Bird | Sp::Frog | Sp::Crab | Sp::Bee) && v % 2 == 1
}

/// Where a creature is this frame: world transform, whether it looks sad, ground height.
fn creature_tf(isl: &Island, slot: usize, e: &sim::Creature, t: f32) -> (Transform, bool, f32) {
    let st = state_of(e.hd);
    let lively = match st {
        2 => 1.6,
        1 => 1.0,
        _ => 0.35,
    };
    let plant = def(e.sp).plant;
    let mut tilt = 0.0;
    let mut lift = 0.0;
    let mut sc = Vec3::ONE;
    let mut yaw = if plant { e.ph } else if faces_left(e.sp, e.v) { PI + 0.55 } else { -0.55 };
    match e.sp {
        Sp::Flower => tilt = (t * 2.0 + e.ph).sin() * 0.07 * lively,
        Sp::Tree => tilt = (t * 1.2 + e.ph).sin() * 0.025 * lively,
        Sp::Palm => tilt = (t * 1.4 + e.ph).sin() * 0.035 * lively,
        Sp::Bush | Sp::Mushroom => sc.y = 1.0 + (t * 1.5 + e.ph).sin() * 0.025 * lively,
        Sp::Lily => lift = (t * 1.5 + e.ph).sin() * 0.5,
        _ => {}
    }
    if !plant {
        if st == 2 {
            // a pleased piece gets the odd little wiggle
            let k = (t * 0.9 + e.ph) % 4.0;
            if k < 0.4 {
                tilt = (k / 0.4 * TAU).sin() * 0.1;
            }
        }
        if e.hop > 0.0 {
            let k = e.hop.clamp(0.0, 1.0);
            lift += (k * PI).sin() * 26.0;
            let d = Vec2::new(e.tx - e.fx, (e.ty - e.fy) / SQ);
            if d.length_squared() > 0.01 {
                yaw = (-d.y).atan2(d.x);
            }
            tilt = (k * PI).sin() * 0.2;
            let stretch = 1.0 + (k * PI).sin() * 0.1;
            sc = Vec3::new(1.0 / stretch, stretch, 1.0 / stretch);
        }
        if e.jump > 0.0 {
            let k = 1.0 - e.jump;
            lift += (k * PI).sin() * 20.0;
            yaw += (k * TAU).sin() * 0.6;
        }
    }
    let mut s = 1.0;
    if e.born < 0.45 {
        s = sim::ease_back((e.born / 0.45).clamp(0.0, 1.0)).max(0.01);
    }
    if e.dying > 0.0 {
        let k = (e.dying / 0.8).clamp(0.0, 1.0);
        s = if e.how == sim::Death::Wither { (1.0 - k * 0.5) * (1.0 - k) + 0.01 } else { 1.0 - k + 0.01 };
    }
    let mut x = e.x;
    if e.dying == 0.0 && e.sad_t > 7.0 {
        x += (t * 30.0).sin() * ((e.sad_t - 7.0) / 9.0).clamp(0.0, 1.0) * 1.8;
    }
    let gh = ground_h(isl, e.x, e.y);
    let tf = Transform::from_translation(wpos(slot, x, e.y, gh + lift)).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_z(tilt)).with_scale(sc * s * PIECE);
    (tf, st == 0 && e.dying == 0.0, gh)
}

struct Want {
    key: (usize, u32),
    mh: Mh,
    tf: Transform,
    sad: bool,
}

#[allow(clippy::type_complexity)]
pub fn sync_life(
    mut commands: Commands,
    session: Res<Session>,
    m3: Res<Models3d>,
    mut index: ResMut<VisIndex>,
    mut tfs: Query<&mut Transform, With<LifeVis>>,
    mut bodies: Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    let t = session.t;
    let islands: Vec<(usize, &Island)> = match (&session.scene, &session.game) {
        (Scene::Title, _) | (_, None) => vec![(TITLE, &session.title)],
        (_, Some(g)) => g.isl.iter().enumerate().collect(),
    };
    let cosp = pitch().cos();
    let mut want: Vec<Want> = vec![];
    let mut shadows: Vec<((usize, u32), Transform)> = vec![];
    for (slot, isl) in &islands {
        let slot = *slot;
        for e in &isl.ents {
            let (tf, sad, gh) = creature_tf(isl, slot, e, t);
            let mh = m3.sp[e.sp.idx()][(e.v % 4) as usize].clone();
            let r = ((mh.hi.x - mh.lo.x).max(mh.hi.z - mh.lo.z) * 0.5 * PIECE).min(22.0);
            let lift = tf.translation.y - gh;
            let k = (1.0 - lift / 60.0).clamp(0.3, 1.0) * (tf.scale.x / PIECE).min(1.2);
            shadows.push(((slot, SHADOW_KEY + e.id), Transform::from_translation(wpos(slot, e.x, e.y, gh + 0.3)).with_scale(Vec3::new(r * 1.1 * k, 1.0, r * 0.9 * k))));
            want.push(Want { key: (slot, e.id), mh, tf, sad });
        }
        for b in &isl.beetles {
            let gh = ground_h(isl, b.x, b.y);
            let mut sc = Vec3::ONE;
            let mut lift = 0.0;
            if b.dead > 0.0 {
                sc = Vec3::new(1.3, (0.3 - b.dead * 0.3).max(0.05), 1.3);
            } else if b.leave {
                lift = (1.0 - b.lift) * 40.0;
                sc = Vec3::splat(b.lift.max(0.05));
            }
            sc *= 1.3;
            let wob = if b.eat > 0.0 { (t * 20.0).sin() * 0.12 } else { (t * 12.0).sin() * 0.06 };
            let yaw = (-(b.dir.sin() / SQ)).atan2(b.dir.cos());
            let tf = Transform::from_translation(wpos(slot, b.x, b.y, gh + lift)).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(wob)).with_scale(sc);
            want.push(Want { key: (slot, BEETLE_KEY + b.id), mh: m3.beetle.clone(), tf, sad: false });
        }
        for s in &isl.storms {
            let fade = if s.blown { (1.0 - s.bt).clamp(0.0, 1.0) } else if s.struck { (1.0 - (s.st - 1.0) / 0.8).clamp(0.0, 1.0) } else { 1.0 };
            let k = 1.3 * s.sc * (1.0 + s.pulse * 0.15) * fade.max(0.02);
            let h = (s.y - s.cy) / cosp;
            let tf = Transform::from_translation(wpos(slot, s.cx, s.y, h)).with_scale(Vec3::splat(k));
            want.push(Want { key: (slot, STORM_KEY + s.id), mh: m3.cloud.clone(), tf, sad: false });
            if s.active() {
                let warn = (s.t / sim::STORM_T).clamp(0.0, 1.0);
                let r = crate::island::HS * (1.4 + warn * 0.9) * s.sc;
                shadows.push(((slot, SHADOW_KEY + STORM_KEY + s.id), Transform::from_translation(wpos(slot, s.x, s.y, ground_h(isl, s.x, s.y) + 0.4)).with_scale(Vec3::new(r, 1.0, r))));
            }
            if s.struck && s.st < 0.35 {
                let tf = Transform::from_translation(wpos(slot, s.x, s.y, h)).with_scale(Vec3::new(1.4, h.max(1.0), 1.4));
                want.push(Want { key: (slot, BOLT_KEY + s.id), mh: m3.bolt.clone(), tf, sad: false });
            }
        }
        if let Some(w) = &isl.whale {
            let rise = (w.t / 0.6).clamp(0.0, 1.0);
            let (dy, sc) = if !w.active() {
                let k = (w.gone / 1.2).clamp(0.0, 1.0);
                if w.caught {
                    (k * 70.0, (1.0 - k).max(0.02) * (1.0 + k * 0.5))
                } else {
                    (-k * 20.0, (1.0 - k).max(0.02))
                }
            } else {
                (-(1.0 - rise) * 18.0 + (t * 2.0).sin() * 1.5, 1.0)
            };
            let yaw = if w.x < isl.map.b.cx() { PI - 0.4 } else { 0.4 };
            let tf = Transform::from_translation(wpos(slot, w.x, w.y, -9.0 + dy)).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_z((t * 1.3).sin() * 0.05)).with_scale(Vec3::splat(sc));
            want.push(Want { key: (slot, WHALE_KEY), mh: m3.whale.clone(), tf, sad: false });
        }
        if isl.shield > 0.0 {
            let b = isl.map.b;
            let blink = if isl.shield < 4.0 && (t * 16.0).sin() < 0.0 { 0.97 } else { 1.0 };
            let (rx, rz) = ((b.x1 - b.x0) / 2.0 + 26.0, (b.y1 - b.y0) / SQ / 2.0 + 26.0);
            let tf = Transform::from_translation(wpos(slot, b.cx(), b.cy(), 0.0)).with_scale(Vec3::new(rx, 90.0, rz) * blink);
            want.push(Want { key: (slot, DOME_KEY), mh: Mh { body: m3.dome.clone(), outline: None, lo: Vec3::ZERO, hi: Vec3::ZERO }, tf, sad: false });
        }
    }
    let mut seen: HashSet<(usize, u32)> = HashSet::new();
    for w in want {
        seen.insert(w.key);
        let dome = w.key.1 & 0xff00_0000 == DOME_KEY;
        match index.map.get(&w.key).copied() {
            Some((parent, body)) => {
                if let Ok(mut t0) = tfs.get_mut(parent) {
                    *t0 = w.tf;
                }
                if !dome {
                    if let Ok(mut mat) = bodies.get_mut(body) {
                        let target = if w.sad { &m3.flat_sad } else { &m3.flat };
                        if mat.0 != *target {
                            mat.0 = target.clone();
                        }
                    }
                }
            }
            None => {
                let (parent, body) = spawn_model(&mut commands, &w.mh, &m3, w.tf, LAYER_WORLD);
                commands.entity(parent).insert(LifeVis);
                if dome {
                    commands.entity(body).insert(MeshMaterial3d(m3.dome_mat.clone()));
                }
                index.map.insert(w.key, (parent, body));
            }
        }
    }
    for (key, tf) in shadows {
        seen.insert(key);
        match index.map.get(&key).copied() {
            Some((parent, _)) => {
                if let Ok(mut t0) = tfs.get_mut(parent) {
                    *t0 = tf;
                }
            }
            None => {
                let e = commands.spawn((Mesh3d(m3.disc.clone()), MeshMaterial3d(m3.shadow.clone()), tf, RenderLayers::layer(LAYER_WORLD), LifeVis)).id();
                index.map.insert(key, (e, e));
            }
        }
    }
    let gone: Vec<(usize, u32)> = index.map.keys().filter(|k| !seen.contains(k)).copied().collect();
    for k in gone {
        if let Some((e, _)) = index.map.remove(&k) {
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
    y0: f32,
    h0: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max: f32,
    kind: PKind,
    size: f32,
}

fn dot_mesh(meshes: &mut Assets<Mesh>, cache: &mut HashMap<u32, Handle<Mesh>>, color: u32) -> Handle<Mesh> {
    cache
        .entry(color)
        .or_insert_with(|| {
            let mut m = Model::default();
            m.add_thin(models::ico(1.0, 0), Transform::IDENTITY, color, 0.0);
            meshes.add(m.body())
        })
        .clone()
}

/// A floating label requested by the simulation; the HUD draws it.
#[derive(Clone)]
pub struct PopupReq {
    pub slot: usize,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size: f32,
    pub color: u32,
    pub max: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_particles(
    mut commands: Commands,
    mut pending: ResMut<Pending>,
    m3: Res<Models3d>,
    mut meshes: ResMut<Assets<Mesh>>,
    session: Res<Session>,
    mut cache: Local<HashMap<u32, Handle<Mesh>>>,
    mut rng: Local<Option<Rng>>,
    count: Query<(), With<Particle>>,
) {
    let rng = rng.get_or_insert_with(Rng::from_time);
    let mut budget = 500usize.saturating_sub(count.iter().count());
    let view = session.game.as_ref().map(|g| g.view).unwrap_or(TITLE);
    let visual = |e: &Ev| matches!(e, Ev::Burst { .. } | Ev::Float { .. } | Ev::Score { .. } | Ev::Combo { .. } | Ev::Jackpot { .. } | Ev::Grow { .. } | Ev::Party { .. });
    let events: Vec<Ev> = pending.ev.iter().filter(|e| visual(e)).cloned().collect();
    pending.ev.retain(|e| !visual(e));
    let mut bursts: Vec<(usize, f32, f32, PKind, u8, bool)> = vec![];
    let mut pops: Vec<PopupReq> = vec![];
    for e in events {
        match e {
            Ev::Burst { isl, x, y, kind, n } => bursts.push((isl, x, y, kind, n, false)),
            Ev::Float { isl, x, y, kind } => bursts.push((isl, x, y, kind, 1, true)),
            Ev::Score { isl, x, y, amt } => {
                if isl == view {
                    pops.push(PopupReq { slot: isl, x, y, text: format!("+{amt}"), size: 16.0, color: pal::GOLD, max: 1.1 });
                }
            }
            Ev::Combo { n, x, y } => {
                if view == 0 {
                    let c = [pal::CREAM, pal::GOLD, 0xff9a3c, 0xff5a5a, 0xe86bff][((n as usize).saturating_sub(2)).min(4)];
                    pops.push(PopupReq { slot: 0, x, y, text: format!("x{n}"), size: 24.0 + n.min(8) as f32 * 3.0, color: c, max: 1.2 });
                }
            }
            Ev::Jackpot { isl, x, y } => {
                if isl == view {
                    pops.push(PopupReq { slot: isl, x, y: y - 30.0, text: "MEGA!".into(), size: 32.0, color: pal::GOLD, max: 1.5 });
                }
            }
            Ev::Grow { isl, tiles } => {
                if isl == view {
                    if let Some(g) = session.game.as_ref() {
                        let b = g.isl[isl].map.b;
                        pops.push(PopupReq { slot: isl, x: b.cx(), y: b.y0 - 10.0, text: "WOW!".into(), size: 58.0, color: pal::CREAM, max: 1.8 });
                        for &t in &tiles {
                            if let Some(tl) = g.isl[isl].map.tiles.get(t) {
                                bursts.push((isl, tl.x, tl.y, PKind::Gold, 4, false));
                            }
                        }
                    }
                }
            }
            Ev::Party { isl } => {
                if isl == view {
                    if let Some(g) = session.game.as_ref() {
                        let b = g.isl[isl].map.b;
                        for _ in 0..8 {
                            let x = b.x0 + rng.f() * (b.x1 - b.x0);
                            let y = b.y0 + rng.f() * (b.y1 - b.y0);
                            bursts.push((isl, x, y - 20.0, PKind::Gold, 5, false));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    pending.popups.extend(pops);
    let island = |slot: usize| -> Option<&Island> {
        if slot == TITLE {
            Some(&session.title)
        } else {
            session.game.as_ref().and_then(|g| g.isl.get(slot))
        }
    };
    for (slot, x, y, kind, n, float) in bursts {
        let h0 = island(slot).map(|i| ground_h(i, x, y)).unwrap_or(0.0) + 2.0;
        for _ in 0..n {
            if budget == 0 {
                return;
            }
            budget -= 1;
            let (vx, vy, max) = if float {
                ((rng.f() - 0.5) * 10.0, -26.0, 1.4)
            } else {
                match kind {
                    PKind::Drop => (-20.0, 200.0, 0.45),
                    PKind::Dust => ((rng.f() - 0.5) * 40.0, -6.0 - rng.f() * 8.0, 0.45),
                    PKind::Splash => ((rng.f() - 0.5) * 50.0, -60.0 - rng.f() * 70.0, 0.9),
                    PKind::Gold => {
                        let a = rng.f() * TAU;
                        let sp = 40.0 + rng.f() * 90.0;
                        (a.cos() * sp, a.sin() * sp * 0.6 - 70.0, 0.9 + rng.f() * 0.6)
                    }
                    _ => {
                        let a = rng.f() * TAU;
                        let sp = 20.0 + rng.f() * 50.0;
                        (a.cos() * sp, a.sin() * sp * 0.6 - 30.0, 0.6 + rng.f() * 0.6)
                    }
                }
            };
            let (color, size) = match kind {
                PKind::Heart => (0xe0474c, 2.2),
                PKind::Spark => (0xfff1b8, 1.6),
                PKind::Leaf => (*rng.pick(&[0x5a9a3c, 0x4c8a35, 0x7fb34a]), 1.6),
                PKind::LeafDead => (*rng.pick(&[0xa98a58, 0x8a6f45]), 1.6),
                PKind::Puff => (0xe9eef0, 2.6),
                PKind::Soot => (0x4a4f5a, 2.6),
                PKind::Splat => (0x5c4a78, 1.6),
                PKind::Drop => (0xcfe8ff, 0.9),
                PKind::Dust => (0xcaa874, 1.8),
                PKind::Splash => (0xd8f2f6, 1.6),
                PKind::Gold => (*rng.pick(&[pal::GOLD, 0xffe08a, 0xffffff, 0xff9a3c]), 1.7),
            };
            let mesh = dot_mesh(&mut meshes, &mut cache, color);
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(m3.flat.clone()),
                Transform::from_translation(wpos(slot, x, y, h0)).with_scale(Vec3::splat(size)),
                RenderLayers::layer(LAYER_WORLD),
                Particle { slot, x, y, y0: y, h0, vx, vy, life: 0.0, max, kind, size },
            ));
        }
    }
}

pub fn move_particles(mut commands: Commands, time: Res<Time>, session: Res<Session>, mut q: Query<(Entity, &mut Particle, &mut Transform)>) {
    let paused = session.game.as_ref().map(|g| g.paused).unwrap_or(false);
    let dt = if paused { 0.0 } else { time.delta_secs().min(0.05) };
    let cosp = pitch().cos();
    for (ent, mut p, mut tf) in &mut q {
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
            PKind::Dust => p.vx *= 0.9,
            PKind::Gold | PKind::Splash => {
                p.vy += 160.0 * dt;
                p.vx *= 0.98;
            }
            _ => {
                p.vy += 120.0 * dt;
                p.vx *= 0.97;
            }
        }
        let k = p.life / p.max;
        // screen-space motion: rising on screen means rising above the spawn point
        let h = p.h0 + (p.y0 - p.y) / cosp;
        tf.translation = wpos(p.slot, p.x, p.y0, h);
        let s = match p.kind {
            PKind::Puff | PKind::Soot | PKind::Dust => p.size * (1.0 + k * 1.5) * (1.0 - k * 0.6),
            _ => p.size * (1.0 - k),
        };
        tf.scale = Vec3::splat(s.max(0.01));
        tf.rotation = Quat::from_rotation_y(p.life * 5.0);
    }
}

/* ---------------- icon studio ---------------- */

/// Model renders used by the interface (cards, pawns, crowd).
#[derive(Resource, Clone)]
pub struct Icons {
    pub sp: Vec<Tex>,
    pub storm: Tex,
    pub beetle: Tex,
    pub whale: Tex,
    pub pawns: Vec<[Tex; 3]>,
    pub pawn_white: Tex,
}

#[derive(Component)]
pub struct StudioCam(u32);
#[derive(Component)]
pub struct StudioProp;

/// Render each model once into a small texture with a transparent background.
pub fn spawn_studio(commands: &mut Commands, images: &mut Assets<Image>, m3: &Models3d) -> Icons {
    let mut n = 0usize;
    let mut shoot = |commands: &mut Commands, mh: &Mh, px: u32, tilt: f32| -> Tex {
        let at = Vec3::new(-60_000.0 + n as f32 * 500.0, 0.0, -60_000.0);
        n += 1;
        let size = Extent3d { width: px, height: px, depth_or_array_layers: 1 };
        let mut img = Image::new_fill(size, TextureDimension::D2, &[0, 0, 0, 0], TextureFormat::Bgra8UnormSrgb, RenderAssetUsages::default());
        img.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
        let h = images.add(img);
        let (lo, hi) = (mh.lo, mh.hi);
        let centre = (lo + hi) / 2.0;
        let extent = (hi.x - lo.x).max(hi.y - lo.y).max(hi.z - lo.z) * 1.12 + 6.0;
        let tf = Transform::from_translation(at).with_rotation(Quat::from_rotation_y(-0.55) * Quat::from_rotation_z(tilt));
        let (e, _) = spawn_model(commands, mh, m3, tf, LAYER_STUDIO);
        commands.entity(e).insert(StudioProp);
        commands.spawn((
            Camera3d::default(),
            Camera { order: -20 - n as isize, target: RenderTarget::Image(h.clone().into()), clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
            Projection::from(OrthographicProjection { scaling_mode: ScalingMode::FixedVertical { viewport_height: extent }, near: -3000.0, far: 3000.0, ..OrthographicProjection::default_3d() }),
            Tonemapping::None,
            RenderLayers::layer(LAYER_STUDIO),
            look(at + centre),
            StudioCam(4),
        ));
        Tex { h, size: Vec2::splat(extent), anchor: Vec2::ZERO }
    };
    let sp = ALL.iter().map(|&s| shoot(commands, &m3.sp[s.idx()][1], 192, 0.0)).collect();
    let storm = shoot(commands, &m3.cloud, 160, 0.0);
    let beetle = shoot(commands, &m3.beetle, 128, 0.0);
    let whale = shoot(commands, &m3.whale, 192, 0.0);
    let pawns = (0..4).map(|i| [0usize, 1, 2].map(|m| shoot(commands, &m3.pawns[i][m], 128, if m == 0 { 0.2 } else { 0.0 }))).collect();
    let pawn_white = shoot(commands, &m3.pawn_white, 96, 0.0);
    Icons { sp, storm, beetle, whale, pawns, pawn_white }
}

/// Studio cameras only need a few frames; then switch them off and clear the props.
pub fn retire_studio(mut commands: Commands, mut cams: Query<(Entity, &mut Camera, &mut StudioCam)>, props: Query<Entity, With<StudioProp>>) {
    let mut all_done = true;
    let mut any = false;
    for (e, mut cam, mut sc) in &mut cams {
        any = true;
        if sc.0 > 0 {
            sc.0 -= 1;
            all_done = false;
        } else {
            cam.is_active = false;
            commands.entity(e).despawn();
        }
    }
    if any && all_done {
        for e in &props {
            commands.entity(e).despawn();
        }
    }
}
