//! Bloom Isles: a cozy, text-free island-gardening race against three bots.

// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod art;
mod audio;
mod eco;
mod hud;
mod input;
mod island;
mod rng;
mod shots;
mod sim;
mod world;

use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::window::WindowResolution;
use rng::Rng;
use sim::{Ev, Game, Island, Sfx};

pub const LAYER_WORLD: usize = 0;
pub const LAYER_BG: usize = 1;
pub const LAYER_HUD: usize = 2;
pub const LAYER_OVER: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    Title,
    Play,
    End,
}

#[derive(Resource)]
pub struct Session {
    pub scene: Scene,
    pub game: Option<Game>,
    pub title: Island,
    pub rng: Rng,
    /// Bumped whenever the set of islands on screen changes.
    pub gen: u32,
    pub t: f32,
    pub dice_t: f32,
}

impl Session {
    pub fn reroll_title(&mut self) {
        let seed = self.rng.next_u32();
        let mut isl = Island::new(island::generate(seed));
        isl.seed_showcase(&mut self.rng);
        self.title = isl;
        self.gen += 1;
        self.dice_t = 0.0;
    }

    pub fn start_game(&mut self, diff: usize) {
        let mut map = self.title.map.clone();
        for t in &mut map.tiles {
            t.occ = None;
        }
        let rng = Rng::new(self.rng.next_u32());
        self.game = Some(Game::new(map, diff, rng));
        self.scene = Scene::Play;
        self.gen += 1;
    }

    pub fn to_title(&mut self) {
        self.game = None;
        self.scene = Scene::Title;
        self.reroll_title();
    }
}

/// Persisted between runs: difficulty, sound and which tutorial hints were learned.
#[derive(Resource, Clone)]
pub struct Settings {
    pub diff: usize,
    pub muted: bool,
    pub tut_place: bool,
    pub tut_attack: bool,
    pub tut_defend: bool,
}

impl Settings {
    fn path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local").join("share")))?;
        Some(base.join("BloomIsles").join("settings.txt"))
    }

    pub fn load() -> Self {
        let mut s = Settings { diff: 1, muted: false, tut_place: false, tut_attack: false, tut_defend: false };
        if let Some(text) = Self::path().and_then(|p| std::fs::read_to_string(p).ok()) {
            for line in text.lines() {
                let mut kv = line.splitn(2, '=');
                let (k, v) = (kv.next().unwrap_or(""), kv.next().unwrap_or("").trim());
                let b = v == "1";
                match k.trim() {
                    "diff" => s.diff = v.parse::<usize>().unwrap_or(1).min(2),
                    "muted" => s.muted = b,
                    "tut_place" => s.tut_place = b,
                    "tut_attack" => s.tut_attack = b,
                    "tut_defend" => s.tut_defend = b,
                    _ => {}
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = Self::path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let b = |v: bool| if v { 1 } else { 0 };
        let _ = std::fs::write(
            p,
            format!(
                "diff={}\nmuted={}\ntut_place={}\ntut_attack={}\ntut_defend={}\n",
                self.diff,
                b(self.muted),
                b(self.tut_place),
                b(self.tut_attack),
                b(self.tut_defend)
            ),
        );
    }
}

/// Sprites that live for a single frame (HUD and overlays are drawn immediate-mode).
#[derive(Component)]
pub struct Ephemeral;

/// Events from the simulation waiting to become particles and sounds.
#[derive(Resource, Default)]
pub struct Pending {
    pub ev: Vec<Ev>,
}

#[derive(Resource)]
pub struct Fonts {
    pub title: Handle<Font>,
}

fn main() {
    App::new()
        .insert_resource(ClearColor(art::bc(0x8ed5d2)))
        .insert_resource(Settings::load())
        .init_resource::<Pending>()
        .init_resource::<input::Pointer>()
        .init_resource::<hud::Layout>()
        .init_resource::<hud::HudState>()
        .init_resource::<world::VisIndex>()
        .init_resource::<world::MainView>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bloom Isles".into(),
                resolution: WindowResolution::new(1280.0, 800.0),
                ..default()
            }),
            ..default()
        }))
        .init_gizmo_group::<world::WorldGizmos>()
        .init_gizmo_group::<hud::HudGizmos>()
        .add_plugins(shots::plugin)
        .add_systems(Startup, (setup, hud::setup_title_text).chain())
        .add_systems(
            Update,
            (
                clear_ephemeral,
                hud::update_layout,
                world::update_cameras,
                input::pointer_system,
                input::keyboard_system,
                tick,
                world::sync_islands,
                world::sync_life,
                world::spawn_particles,
                world::move_particles,
                world::draw_overlays,
                hud::draw_hud,
                play_sounds,
            )
                .chain(),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut fonts: ResMut<Assets<Font>>,
    mut gizmo_store: ResMut<GizmoConfigStore>,
) {
    let art = art::build_art(&mut images);
    commands.insert_resource(audio::build_sounds(&mut audio));
    let font = Font::try_from_bytes(include_bytes!("../assets/fonts/BricolageGrotesque-ExtraBold.ttf").to_vec()).expect("embedded font");
    commands.insert_resource(Fonts { title: fonts.add(font) });

    let (cfg, _) = gizmo_store.config_mut::<world::WorldGizmos>();
    cfg.render_layers = RenderLayers::layer(LAYER_OVER);
    cfg.line.width = 3.0;
    let (cfg, _) = gizmo_store.config_mut::<hud::HudGizmos>();
    cfg.render_layers = RenderLayers::layer(LAYER_HUD);
    cfg.line.width = 4.0;

    world::spawn_cameras(&mut commands);
    world::spawn_background(&mut commands, &art);
    commands.insert_resource(art);

    let mut rng = Rng::from_time();
    let seed = rng.next_u32();
    let mut title = Island::new(island::generate(seed));
    title.seed_showcase(&mut rng);
    commands.insert_resource(Session { scene: Scene::Title, game: None, title, rng, gen: 1, t: 0.0, dice_t: 0.0 });
}

fn clear_ephemeral(mut commands: Commands, q: Query<Entity, With<Ephemeral>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Advance the simulation and hand its events to the renderer.
pub fn tick(time: Res<Time>, mut session: ResMut<Session>, mut pending: ResMut<Pending>, mut settings: ResMut<Settings>, mut hud_state: ResMut<hud::HudState>) {
    let dt = time.delta_secs().min(0.05);
    session.t += dt;
    session.dice_t += dt;
    match session.scene {
        Scene::Title => {
            for e in &mut session.title.ents {
                e.born += dt;
            }
        }
        Scene::Play | Scene::End => {
            let Some(g) = session.game.as_mut() else { return };
            g.update(dt);
            let events: Vec<Ev> = g.ev.drain(..).collect();
            for e in events {
                match e {
                    Ev::Shake => g.shake = 0.5,
                    Ev::Flash => g.flash = 0.35,
                    other => pending.ev.push(other),
                }
            }
            // tutorial progress
            let mut changed = false;
            if g.placed > 0 && !settings.tut_place {
                settings.tut_place = true;
                changed = true;
            }
            if g.attacks > 0 && !settings.tut_attack {
                settings.tut_attack = true;
                changed = true;
            }
            if g.defends > 0 && !settings.tut_defend {
                settings.tut_defend = true;
                changed = true;
            }
            if changed {
                settings.save();
            }
            if g.over && session.scene == Scene::Play {
                session.scene = Scene::End;
                hud_state.start_confetti(session.game.as_ref().map(|g| g.ranks.first() == Some(&0)).unwrap_or(false));
            }
        }
    }
}

fn play_sounds(
    mut commands: Commands,
    mut pending: ResMut<Pending>,
    sounds: Res<audio::Sounds>,
    settings: Res<Settings>,
    session: Res<Session>,
    time: Res<Time>,
    mut music_t: Local<f32>,
    mut note: Local<usize>,
    mut drone_t: Local<i32>,
    mut rng: Local<Option<Rng>>,
) {
    let rng = rng.get_or_insert_with(Rng::from_time);
    let mut played: Vec<Sfx> = vec![];
    for e in pending.ev.drain(..) {
        if let Ev::Sfx(s) = e {
            // one copy of each sound per frame is enough
            if !settings.muted && !played.contains(&s) {
                played.push(s);
                if let Some(h) = sounds.sfx.get(&s) {
                    commands.spawn((AudioPlayer::new(h.clone()), PlaybackSettings::DESPAWN));
                }
            }
        }
    }
    let paused = session.game.as_ref().map(|g| g.paused).unwrap_or(false);
    if settings.muted || paused || session.scene == Scene::End {
        return;
    }
    // a wandering lute melody in D dorian over a slow drone
    *music_t -= time.delta_secs();
    if *music_t <= 0.0 {
        *music_t = *rng.pick(&[0.36, 0.36, 0.54, 0.72]);
        if rng.chance(0.8) {
            let step = *rng.pick(&[-2i32, -1, -1, 1, 1, 2, 0]);
            *note = (*note as i32 + step).clamp(0, 7) as usize;
            commands.spawn((AudioPlayer::new(sounds.notes[*note].clone()), PlaybackSettings::DESPAWN));
        }
        *drone_t -= 1;
        if *drone_t <= 0 {
            *drone_t = 9;
            let d = rng.below(sounds.drones.len());
            commands.spawn((AudioPlayer::new(sounds.drones[d].clone()), PlaybackSettings::DESPAWN));
        }
    }
}

pub fn sfx(pending: &mut Pending, s: Sfx) {
    pending.ev.push(Ev::Sfx(s));
}
