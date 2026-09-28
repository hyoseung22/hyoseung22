//! Scripted screenshots for checking the look of every screen.
//! Only active when the `BLOOM_SHOTS` environment variable names an output folder.

use crate::input::Pointer;
use crate::eco::Biome;
use crate::sim::Card;
use crate::world::MainView;
use crate::Session;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

#[derive(Resource)]
pub struct Shots {
    dir: String,
    t: f32,
    step: usize,
}

/// Present when the game runs as a model contact sheet (BLOOM_GALLERY=<file.png>).
#[derive(Resource)]
pub struct Gallery {
    path: String,
    t: f32,
}

fn gallery(mut commands: Commands, time: Res<Time>, mut g: ResMut<Gallery>, mut exit: EventWriter<AppExit>, mut shot_taken: Local<bool>) {
    g.t += time.delta_secs();
    if g.t > 3.0 && !*shot_taken {
        *shot_taken = true;
        let path = g.path.clone();
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if g.t > 5.0 {
        exit.write(AppExit::Success);
    }
}

pub fn plugin(app: &mut App) {
    if let Ok(path) = std::env::var("BLOOM_GALLERY") {
        app.insert_resource(Gallery { path, t: 0.0 });
        app.add_systems(Update, gallery);
    }
    if std::env::var("BLOOM_TRACE").is_ok() {
        app.add_systems(Update, trace.after(crate::tick));
    }
    if let Ok(dir) = std::env::var("BLOOM_SHOTS") {
        let _ = std::fs::create_dir_all(&dir);
        app.insert_resource(Shots { dir, t: 0.0, step: 0 });
        app.add_systems(Update, run.after(crate::input::pointer_system).before(crate::tick));
    }
}

fn shot(commands: &mut Commands, dir: &str, name: &str) {
    let path = format!("{dir}/{name}.png");
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
}

fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shots: ResMut<Shots>,
    mut session: ResMut<Session>,
    mut ptr: ResMut<Pointer>,
    view: Res<MainView>,
    mut exit: EventWriter<AppExit>,
) {
    shots.t += time.delta_secs();
    let steps: [f32; 16] = [2.5, 3.0, 5.0, 5.5, 8.5, 9.0, 12.0, 12.5, 15.5, 16.0, 19.0, 19.5, 20.5, 22.5, 23.0, 26.5];
    if shots.step >= steps.len() {
        if shots.t > steps[steps.len() - 1] + 1.5 {
            exit.write(AppExit::Success);
        }
        return;
    }
    if shots.t < steps[shots.step] {
        return;
    }
    let dir = shots.dir.clone();
    // the tile where the first biome goes: an open, busy-ish spot
    let pick = |g: &crate::sim::Game, avoid: &[usize]| -> Option<usize> {
        let isl = &g.isl[0];
        (0..isl.map.tiles.len())
            .filter(|&t| isl.can_biome(t) && !avoid.iter().any(|&a| isl.map.dist(a, t) < 3))
            .max_by_key(|&t| {
                let tl = &isl.map.tiles[t];
                tl.n1.len() as i32 * 4 + tl.n1.iter().filter(|&&u| isl.map.tiles[u].occ.is_some()).count() as i32 - (tl.x.abs() / 30.0) as i32
            })
    };
    match shots.step {
        0 => shot(&mut commands, &dir, "1-title"),
        1 => session.start_game(1, 240.0),
        2 => shot(&mut commands, &dir, "2-start"),
        3 => {
            // fast-forward: the autopilot plays for a while
            let g = session.game.as_mut().unwrap();
            g.auto = true;
            for _ in 0..1300 {
                g.update(0.05);
            }
            g.ev.clear();
            g.auto = false;
            g.placed = 1;
            g.biomes_placed = 0;
            g.weather = crate::sim::Weather::Clear;
            g.weather_t = 30.0;
            g.hand[0].card = Some(Card::Biome(Biome::Volcano));
            g.hand[1].card = Some(Card::Biome(Biome::Lake));
            g.hand[2].card = Some(Card::Nature(crate::eco::Sp::Deer));
            g.sel = Some(0);
            g.isl[0].storms.clear();
            g.isl[0].beetles.clear();
            g.isl[0].whale = None;
            g.isl[0].whale_t = 99.0;
            if let Some(t) = pick(g, &[]) {
                let tl = &g.isl[0].map.tiles[t];
                ptr.pos = view.to_screen(tl.x, tl.y);
            }
        }
        4 => shot(&mut commands, &dir, "3-biome-preview"),
        5 => {
            let g = session.game.as_mut().unwrap();
            let a = pick(g, &[]).unwrap_or(0);
            g.play(0, a);
            let b = pick(g, &[a]).unwrap_or(0);
            g.play(1, b);
            g.sel = None;
            ptr.pos = Vec2::new(-100.0, -100.0);
        }
        6 => {
            let g = session.game.as_mut().unwrap();
            g.isl[0].erupt_t = 0.0;
        }
        7 => shot(&mut commands, &dir, "4-volcano-lake"),
        8 => {
            let g = session.game.as_mut().unwrap();
            let avoid: Vec<usize> = g.isl[0].biomes.iter().map(|b| b.1).collect();
            g.hand[0].card = Some(Card::Biome(Biome::Peak));
            g.hand[1].card = Some(Card::Biome(Biome::Desert));
            if let Some(c) = pick(g, &avoid) {
                g.play(0, c);
            }
            let avoid: Vec<usize> = g.isl[0].biomes.iter().map(|b| b.1).collect();
            if let Some(c) = pick(g, &avoid) {
                g.play(1, c);
            }
            for _ in 0..500 {
                g.update(0.05);
            }
            g.ev.clear();
            g.weather = crate::sim::Weather::Rain;
            g.weather_t = 12.0;
            g.hand[2].card = Some(Card::Nature(crate::eco::Sp::Goat));
            g.sel = Some(2);
            let hints = g.hints(crate::eco::Sp::Goat);
            if let Some(best) = hints.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) {
                let t = &g.isl[0].map.tiles[best.0];
                ptr.pos = view.to_screen(t.x, t.y);
            }
        }
        9 => shot(&mut commands, &dir, "5-four-biomes-rain"),
        10 => {
            let g = session.game.as_mut().unwrap();
            g.sel = None;
            g.weather = crate::sim::Weather::Wind;
            g.weather_t = 12.0;
            g.inspect = g.isl[0].ents.iter().find(|e| !crate::eco::def(e.sp).plant && e.dying == 0.0).map(|e| e.tile);
            ptr.pos = Vec2::new(-100.0, -100.0);
        }
        11 => shot(&mut commands, &dir, "6-inspect-wind"),
        12 => {
            let g = session.game.as_mut().unwrap();
            g.t = crate::sim::DUR - 0.01;
        }
        13 => {
            let g = session.game.as_mut().unwrap();
            g.end_t = 2.6;
        }
        14 => shot(&mut commands, &dir, "7-end-stars"),
        15 => shot(&mut commands, &dir, "8-end-final"),
        _ => {}
    }
    shots.step += 1;
}

/// Prints the state once a second so input can be tested from outside.
fn trace(time: Res<Time>, session: Res<Session>, view: Res<MainView>, mut acc: Local<f32>) {
    *acc += time.delta_secs();
    if *acc < 1.0 {
        return;
    }
    *acc = 0.0;
    let Some(g) = session.game.as_ref() else {
        println!("TRACE scene={:?}", session.scene);
        return;
    };
    let hint = g.selected().and_then(|c| match c {
        Card::Nature(sp) => g.hints(sp).into_iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|h| {
            let t = &g.isl[0].map.tiles[h.0];
            view.to_screen(t.x, t.y)
        }),
        _ => None,
    });
    println!(
        "TRACE scene={:?} t={:.1} placed={} biomes={} sel={:?} ents={} hint={:?} paused={}",
        session.scene, g.t, g.placed, g.biomes_placed, g.sel, g.isl[0].ents.len(), hint.map(|p| (p.x as i32, p.y as i32)), g.paused
    );
}
