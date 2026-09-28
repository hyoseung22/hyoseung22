//! Scripted screenshots for checking the look of every screen.
//! Only active when the `BLOOM_SHOTS` environment variable names an output folder.

use crate::input::Pointer;
use crate::sim::{Card, Fly};
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
    let steps: [f32; 14] = [2.5, 3.0, 5.0, 5.5, 8.5, 9.0, 13.0, 13.5, 16.0, 16.5, 17.5, 19.5, 20.0, 23.5];
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
    match shots.step {
        0 => shot(&mut commands, &dir, "1-title"),
        1 => session.start_game(1),
        2 => shot(&mut commands, &dir, "2-start"),
        3 => {
            // fast-forward: a bot plays for the player for a while
            let g = session.game.as_mut().unwrap();
            g.players[0].bot = true;
            for _ in 0..1500 {
                g.update(0.05);
            }
            g.ev.clear();
            g.players[0].bot = false;
            g.placed = 1;
            g.players[0].hand[0].card = Some(Card::Nature(crate::eco::Sp::Bee));
            g.players[0].hand[1].card = Some(Card::Storm);
            g.sel = Some(0);
            g.isl[0].storms.clear();
            g.isl[0].beetles.clear();
            g.isl[0].shield = 0.0;
            g.isl[0].whale = None;
            g.isl[0].whale_t = 0.0;
            let hints = g.hints(crate::eco::Sp::Bee);
            if let Some(best) = hints.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) {
                let t = &g.isl[0].map.tiles[best.0];
                ptr.pos = view.to_screen(t.x, t.y);
                let (x, y) = (t.x, t.y);
                g.ev.push(crate::sim::Ev::Combo { n: 4, x, y: y - 40.0 });
            }
            let b = g.isl[0].map.b;
            g.ev.push(crate::sim::Ev::Grow { isl: 0, tiles: vec![] });
            g.ev.push(crate::sim::Ev::Score { isl: 0, x: b.cx() - 60.0, y: b.cy(), amt: 3 });
            g.combo = 4;
            g.last_place = g.t;
        }
        4 => shot(&mut commands, &dir, "3-preview"),
        5 => {
            let g = session.game.as_mut().unwrap();
            g.fly.push(Fly { card: Card::Storm, from: 2, to: 0, t: 1.1, dur: 1.15, tile: None, done: false });
            g.fly.push(Fly { card: Card::Beetle, from: 1, to: 0, t: 1.1, dur: 1.15, tile: None, done: false });
            g.sel = Some(1);
        }
        6 => shot(&mut commands, &dir, "4-storm-aim"),
        7 => {
            let g = session.game.as_mut().unwrap();
            g.sel = None;
            g.view = 2;
        }
        8 => shot(&mut commands, &dir, "5-rival"),
        9 => {
            let g = session.game.as_mut().unwrap();
            g.view = 0;
            g.isl[0].score = crate::sim::GOAL + 1.0;
        }
        10 => {
            let g = session.game.as_mut().unwrap();
            g.end_t = 2.0;
        }
        11 => shot(&mut commands, &dir, "6-end-reveal"),
        12 => {
            let g = session.game.as_mut().unwrap();
            g.end_t = 5.0;
        }
        13 => shot(&mut commands, &dir, "7-end-final"),
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
        "TRACE scene={:?} t={:.1} placed={} attacks={} sel={:?} view={} ents={} hint={:?} paused={}",
        session.scene, g.t, g.placed, g.attacks, g.sel, g.view, g.isl[0].ents.len(), hint.map(|p| (p.x as i32, p.y as i32)), g.paused
    );
}
