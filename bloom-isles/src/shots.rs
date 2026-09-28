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

pub fn plugin(app: &mut App) {
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
    let steps: [f32; 11] = [2.5, 3.0, 5.0, 5.5, 7.5, 8.0, 12.0, 12.5, 15.0, 15.5, 18.5];
    if shots.step >= steps.len() {
        // give the last screenshot a second to be written, then quit
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
            // fast-forward: let a bot play for the player for a minute, then hold a bee card
            let g = session.game.as_mut().unwrap();
            g.players[0].bot = true;
            for _ in 0..1200 {
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
            let hints = g.hints(crate::eco::Sp::Bee);
            if let Some(best) = hints.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) {
                let t = &g.isl[0].map.tiles[best.0];
                ptr.pos = view.to_screen(t.x, t.y);
            }
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
            shot(&mut commands, &dir, "6-end");
        }
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
