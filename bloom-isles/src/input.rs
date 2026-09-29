//! Mouse, touch and keyboard.

use crate::hud::Layout;
use crate::sim::{beetle_dist, storm_hit, Sfx};
use crate::world::MainView;
use crate::{sfx, Pending, Scene, Session, Settings};
use bevy::app::AppExit;
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowFocused};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BtnId {
    Play,
    Dice,
    Quit,
    Mute,
    Pause,
    Speed,
    Resume,
    PauseHome,
    Reroll,
    Shovel,
    Again,
    EndHome,
}

#[derive(Clone, Debug)]
pub struct Drag {
    pub si: usize,
    pub start: Vec2,
    pub moved: bool,
    pub was_selected: bool,
}

#[derive(Resource, Default)]
pub struct Pointer {
    pub pos: Vec2,
    pub down: bool,
    pub hover_tile: Option<usize>,
    pub hover_card: Option<usize>,
    pub hover: Option<BtnId>,
    pub drag: Option<Drag>,
    touch_id: Option<u64>,
}

fn card_at(l: &Layout, p: Vec2) -> Option<usize> {
    (0..4).find(|&i| {
        let r = l.cards[i];
        crate::hud::R::new(r.x, r.y - 16.0, r.w, r.h + 16.0).contains(p)
    })
}

fn button_at(l: &Layout, scene: Scene, paused: bool, p: Vec2) -> Option<BtnId> {
    if l.mute.hit(p) {
        return Some(BtnId::Mute);
    }
    match scene {
        Scene::Title => {
            if l.title_play.hit(p) {
                Some(BtnId::Play)
            } else if l.title_dice.hit(p) {
                Some(BtnId::Dice)
            } else if l.title_quit.hit(p) {
                Some(BtnId::Quit)
            } else {
                None
            }
        }
        Scene::End => {
            if l.end_again.hit(p) {
                Some(BtnId::Again)
            } else if l.end_home.hit(p) {
                Some(BtnId::EndHome)
            } else {
                None
            }
        }
        Scene::Play if paused => {
            if l.pause_resume.hit(p) {
                Some(BtnId::Resume)
            } else if l.pause_home.hit(p) {
                Some(BtnId::PauseHome)
            } else {
                None
            }
        }
        Scene::Play => {
            if l.pause.hit(p) {
                Some(BtnId::Pause)
            } else if l.speed.hit(p) {
                Some(BtnId::Speed)
            } else if l.reroll.hit(p) {
                Some(BtnId::Reroll)
            } else if l.shovel.hit(p) {
                Some(BtnId::Shovel)
            } else {
                None
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn pointer_system(
    window: Single<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    layout: Res<Layout>,
    view: Res<MainView>,
    mut ptr: ResMut<Pointer>,
    mut session: ResMut<Session>,
    mut settings: ResMut<Settings>,
    mut pending: ResMut<Pending>,
    mut exit: EventWriter<AppExit>,
    mut hud_state: ResMut<crate::hud::HudState>,
) {
    let mut pressed = None;
    let mut released = None;
    if let Some(p) = window.cursor_position() {
        ptr.pos = p;
    }
    if mouse.just_pressed(MouseButton::Left) {
        pressed = Some(ptr.pos);
    }
    if mouse.just_released(MouseButton::Left) {
        released = Some(ptr.pos);
    }
    for t in touches.iter_just_pressed() {
        if ptr.touch_id.is_none() {
            ptr.touch_id = Some(t.id());
            ptr.pos = t.position();
            pressed = Some(t.position());
        }
    }
    if let Some(id) = ptr.touch_id {
        if let Some(t) = touches.get_pressed(id) {
            ptr.pos = t.position();
        }
        if let Some(t) = touches.iter_just_released().find(|t| t.id() == id) {
            ptr.pos = t.position();
            released = Some(t.position());
            ptr.touch_id = None;
        }
        if touches.iter_just_canceled().any(|t| t.id() == id) {
            ptr.touch_id = None;
            ptr.drag = None;
        }
    }

    let p = ptr.pos;
    // hover state
    let scene = session.scene;
    let paused = session.game.as_ref().map(|g| g.paused).unwrap_or(false);
    ptr.hover = button_at(&layout, scene, paused, p);
    ptr.hover_card = if scene == Scene::Play { card_at(&layout, p) } else { None };
    ptr.hover_tile = None;
    if scene == Scene::Play && layout.isle.contains(p) {
        if let Some(g) = session.game.as_ref() {
            let lp = view.to_local(p);
            ptr.hover_tile = g.isl[0].map.tile_at(lp.x, lp.y);
        }
    }
    if let Some(d) = ptr.drag.as_mut() {
        if !d.moved && p.distance(d.start) > 10.0 {
            d.moved = true;
        }
    }

    if let Some(at) = pressed {
        ptr.down = true;
        on_down(at, &layout, &view, &mut ptr, &mut session, &mut settings, &mut pending, &mut exit, &mut hud_state);
    }
    if let Some(at) = released {
        ptr.down = false;
        on_up(at, &layout, &view, &mut ptr, &mut session, &mut pending);
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn on_down(p: Vec2, l: &Layout, view: &MainView, ptr: &mut Pointer, session: &mut Session, settings: &mut Settings, pending: &mut Pending, exit: &mut EventWriter<AppExit>, hud_state: &mut crate::hud::HudState) {
    let scene = session.scene;
    let paused = session.game.as_ref().map(|g| g.paused).unwrap_or(false);
    if let Some(b) = button_at(l, scene, paused, p) {
        match b {
            BtnId::Mute => {
                settings.muted = !settings.muted;
                settings.save();
                sfx(pending, Sfx::Click);
            }
            BtnId::Play => {
                sfx(pending, Sfx::Place);
                session.start_game(settings.diff, settings.best);
                hud_state.reset_play();
            }
            BtnId::Dice => {
                session.reroll_title();
                sfx(pending, Sfx::Reroll);
            }
            BtnId::Quit => {
                exit.write(AppExit::Success);
            }
            BtnId::Again => {
                if session.game.as_ref().map(|g| g.end_t > crate::sim::END_BUTTONS_AT).unwrap_or(false) {
                    session.reroll_title();
                    session.start_game(settings.diff, settings.best);
                    hud_state.reset_play();
                    sfx(pending, Sfx::Place);
                }
            }
            BtnId::EndHome => {
                if session.game.as_ref().map(|g| g.end_t > crate::sim::END_BUTTONS_AT).unwrap_or(false) {
                    session.to_title();
                    sfx(pending, Sfx::Click);
                }
            }
            BtnId::Resume => {
                if let Some(g) = session.game.as_mut() {
                    g.paused = false;
                }
                sfx(pending, Sfx::Click);
            }
            BtnId::PauseHome => {
                session.to_title();
                sfx(pending, Sfx::Click);
            }
            BtnId::Speed => {
                if let Some(g) = session.game.as_mut() {
                    g.speed = if g.speed >= 2 { 1 } else { 2 };
                }
                sfx(pending, Sfx::Click);
            }
            BtnId::Pause => {
                if let Some(g) = session.game.as_mut() {
                    g.paused = true;
                }
                ptr.drag = None;
                sfx(pending, Sfx::Click);
            }
            BtnId::Reroll => {
                if let Some(g) = session.game.as_mut() {
                    if !g.reroll() {
                        sfx(pending, Sfx::Nope);
                    }
                    pending.ev.extend(g.ev.drain(..));
                }
            }
            BtnId::Shovel => {
                if let Some(g) = session.game.as_mut() {
                    g.shovel = !g.shovel;
                    g.sel = None;
                }
                sfx(pending, Sfx::Click);
            }
        }
        return;
    }
    if scene == Scene::Title {
        for (i, r) in l.title_diff.iter().enumerate() {
            if r.contains(p) {
                settings.diff = i;
                settings.save();
                sfx(pending, Sfx::Click);
            }
        }
        return;
    }
    if scene != Scene::Play || paused {
        return;
    }
    let Some(g) = session.game.as_mut() else { return };
    if let Some(ci) = card_at(l, p) {
        let slot = &mut g.hand[ci];
        if slot.card.is_none() {
            slot.wig = 1.0;
            sfx(pending, Sfx::Nope);
            return;
        }
        g.shovel = false;
        let was = g.sel == Some(ci);
        g.sel = Some(ci);
        ptr.drag = Some(Drag { si: ci, start: p, moved: false, was_selected: was });
        if !was {
            sfx(pending, Sfx::Pick);
        }
        return;
    }
    if l.isle.contains(p) {
        island_tap(g, view, p, pending);
    }
}

fn on_up(p: Vec2, l: &Layout, view: &MainView, ptr: &mut Pointer, session: &mut Session, pending: &mut Pending) {
    let Some(d) = ptr.drag.take() else { return };
    if session.scene != Scene::Play {
        return;
    }
    let Some(g) = session.game.as_mut() else { return };
    if g.paused {
        return;
    }
    if !d.moved {
        if d.was_selected && card_at(l, p) == Some(d.si) {
            g.sel = None;
        }
        return;
    }
    if g.sel != Some(d.si) || g.hand[d.si].card.is_none() {
        return;
    }
    if l.isle.contains(p) {
        island_tap(g, view, p, pending);
    }
}

fn island_tap(g: &mut crate::sim::Game, view: &MainView, p: Vec2, pending: &mut Pending) {
    let lp = view.to_local(p);
    // the whale, storms and beetles come first
    if let Some(w) = g.isl[0].whale.as_ref() {
        if w.active() && (lp.x - w.x).hypot((lp.y - w.y + 8.0) * 1.3) < 48.0 && g.tap_whale() {
            pending.ev.extend(g.ev.drain(..));
            return;
        }
    }
    if let Some(id) = g.isl[0].storms.iter().find(|s| s.active() && storm_hit(s, lp.x, lp.y)).map(|s| s.id) {
        g.tap_storm(id);
        pending.ev.extend(g.ev.drain(..));
        return;
    }
    let bug = g.isl[0]
        .beetles
        .iter()
        .filter(|b| b.active())
        .map(|b| (beetle_dist(b, lp.x, lp.y), b.id))
        .filter(|(d, _)| *d < 22.0)
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((_, id)) = bug {
        g.tap_beetle(id);
        pending.ev.extend(g.ev.drain(..));
        return;
    }
    let tile = g.isl[0].map.tile_at(lp.x, lp.y);
    match g.sel.filter(|&si| g.hand[si].card.is_some()) {
        Some(si) => {
            if let Some(t) = tile {
                if g.play(si, t) {
                    g.sel = None;
                    pending.ev.extend(g.ev.drain(..));
                    return;
                }
            }
            g.hand[si].wig = 1.0;
            sfx(pending, Sfx::Nope);
        }
        None => {
            if g.shovel {
                if let Some(t) = tile {
                    g.dig(t);
                    pending.ev.extend(g.ev.drain(..));
                }
                return;
            }
            g.inspect = tile.filter(|&t| g.isl[0].map.tiles[t].occ.is_some());
            if g.inspect.is_some() {
                sfx(pending, Sfx::Click);
            }
        }
    }
}

pub fn keyboard_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: EventReader<WindowFocused>,
    mut session: ResMut<Session>,
    mut pending: ResMut<Pending>,
    mut exit: EventWriter<AppExit>,
) {
    let lost_focus = focus.read().any(|f| !f.focused);
    match session.scene {
        Scene::Title => {
            if keys.just_pressed(KeyCode::Escape) {
                exit.write(AppExit::Success);
            }
        }
        Scene::End => {
            if keys.just_pressed(KeyCode::Escape) && session.game.as_ref().map(|g| g.end_t > crate::sim::END_BUTTONS_AT).unwrap_or(false) {
                session.to_title();
            }
        }
        Scene::Play => {
            let Some(g) = session.game.as_mut() else { return };
            if lost_focus {
                g.paused = true;
            }
            // space pauses and resumes, like a board game on the table
            if keys.just_pressed(KeyCode::Space) {
                g.paused = !g.paused;
                sfx(&mut pending, Sfx::Click);
            }
            if keys.just_pressed(KeyCode::Tab) && !g.paused {
                g.speed = if g.speed >= 2 { 1 } else { 2 };
                sfx(&mut pending, Sfx::Click);
            }
            if keys.just_pressed(KeyCode::Escape) {
                if g.sel.is_some() || g.shovel {
                    g.sel = None;
                    g.shovel = false;
                } else {
                    g.paused = !g.paused;
                }
            }
            if g.paused {
                return;
            }
            for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].iter().enumerate() {
                if keys.just_pressed(*k) && g.hand[i].card.is_some() {
                    g.sel = if g.sel == Some(i) { None } else { Some(i) };
                    g.shovel = false;
                    sfx(&mut pending, Sfx::Pick);
                }
            }
        }
    }
}
