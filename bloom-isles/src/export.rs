//! `bloom-isles --export <folder>` writes every generated resource to files:
//! 3D models as OBJ (with vertex colours), interface images as PNG and
//! sounds as WAV. Nothing in the game is loaded from disk; this is for
//! sharing and editing the assets elsewhere.

use crate::art::{self, Art, Tex};
use crate::audio;
use crate::eco::ALL;
use crate::models;
use bevy::audio::AudioSource;
use bevy::prelude::*;
use std::fs;
use std::path::Path;

impl Art {
    fn named(&self) -> Vec<(String, &Tex)> {
        let mut v: Vec<(String, &Tex)> = vec![
            ("bolt".into(), &self.bolt),
            ("shield".into(), &self.shield),
            ("heart".into(), &self.heart),
            ("heart_broken".into(), &self.heart_broken),
            ("spark".into(), &self.spark),
            ("hand".into(), &self.hand),
            ("goal_flag".into(), &self.trophy),
            ("crown".into(), &self.crown),
            ("crosshair".into(), &self.crosshair),
            ("glyph_leaf".into(), &self.glyph_leaf),
            ("glyph_paw".into(), &self.glyph_paw),
            ("glyph_target".into(), &self.glyph_target),
            ("sprout_on".into(), &self.sprout_on),
            ("sprout_off".into(), &self.sprout_off),
            ("dice".into(), &self.dice),
            ("icon_pause".into(), &self.icon_pause),
            ("icon_play".into(), &self.icon_play),
            ("icon_sound_on".into(), &self.icon_sound_on),
            ("icon_sound_off".into(), &self.icon_sound_off),
            ("icon_reroll".into(), &self.icon_reroll),
            ("icon_shovel".into(), &self.icon_shovel),
            ("icon_home".into(), &self.icon_home),
            ("icon_quit".into(), &self.icon_quit),
            ("hex".into(), &self.hex),
            ("rounded_rect_fill".into(), &self.rrect_fill),
            ("rounded_rect_line".into(), &self.rrect_line),
            ("wave".into(), &self.wave),
        ];
        for (i, name) in ["sad", "fine", "thriving"].iter().enumerate() {
            v.push((format!("mood_{name}"), &self.faces[i]));
        }
        v
    }
}

fn write_png(path: &Path, img: &Image) -> Result<(), String> {
    let (w, h) = (img.width(), img.height());
    let data = img.data.as_ref().ok_or("image has no CPU data")?;
    // tiny-skia stores premultiplied pixels
    let mut pre = Vec::with_capacity(data.len());
    for px in data.chunks(4) {
        let a = px[3] as u32;
        pre.extend_from_slice(&[(px[0] as u32 * a / 255) as u8, (px[1] as u32 * a / 255) as u8, (px[2] as u32 * a / 255) as u8, px[3]]);
    }
    let size = tiny_skia::IntSize::from_wh(w, h).ok_or("bad size")?;
    let pm = tiny_skia::Pixmap::from_vec(pre, size).ok_or("bad pixmap")?;
    pm.save_png(path).map_err(|e| e.to_string())
}

pub fn run(dir: &Path) -> Result<(), String> {
    let mkdir = |p: &Path| fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()));
    let (mdir, udir, sdir) = (dir.join("models"), dir.join("ui"), dir.join("sounds"));
    for d in [&mdir, &udir, &sdir] {
        mkdir(d)?;
    }
    let write = |p: std::path::PathBuf, bytes: &[u8]| fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()));

    // 3D models
    let mut count = 0;
    for sp in ALL {
        for v in 0..4u8 {
            let name = format!("{}_{v}", models::species_name(sp));
            write(mdir.join(format!("{name}.obj")), models::species(sp, v).to_obj(&name).as_bytes())?;
            count += 1;
        }
    }
    let props: Vec<(&str, models::Model)> = vec![
        ("rock_small", models::rock(false)),
        ("rock_big", models::rock(true)),
        ("storm_cloud", models::cloud()),
        ("lightning", models::bolt()),
        ("beetle", models::beetle()),
        ("golden_whale", models::whale()),
    ];
    for (name, m) in props {
        write(mdir.join(format!("{name}.obj")), m.to_obj(name).as_bytes())?;
        count += 1;
    }
    for (i, team) in ["red", "blue", "yellow", "violet"].iter().enumerate() {
        for (k, mood) in ["sad", "normal", "happy"].iter().enumerate() {
            let name = format!("pawn_{team}_{mood}");
            write(mdir.join(format!("{name}.obj")), models::pawn(art::TEAM[i].0, k as u8).to_obj(&name).as_bytes())?;
            count += 1;
        }
    }
    let map = crate::island::generate(20260928);
    write(mdir.join("island_sample.obj"), models::island(&map, 0.8).to_obj("island_sample").as_bytes())?;
    write(mdir.join("island_sample_shallows.obj"), models::shallows(&map).to_obj("island_sample_shallows").as_bytes())?;
    count += 2;

    // interface images
    let mut images = Assets::<Image>::default();
    let a = art::build_art(&mut images);
    let mut pngs = 0;
    for (name, tex) in a.named() {
        if let Some(img) = images.get(&tex.h) {
            write_png(&udir.join(format!("{name}.png")), img)?;
            pngs += 1;
        }
    }

    // sounds
    let mut sources = Assets::<AudioSource>::default();
    let s = audio::build_sounds(&mut sources);
    let mut wavs = 0;
    for sfx in audio::all_sfx() {
        if let Some(src) = s.sfx.get(&sfx).and_then(|h| sources.get(h)) {
            let name = format!("{sfx:?}").replace(['(', ')'], "_").trim_end_matches('_').to_lowercase();
            write(sdir.join(format!("{name}.wav")), &src.bytes)?;
            wavs += 1;
        }
    }
    for (i, h) in s.notes.iter().enumerate() {
        if let Some(src) = sources.get(h) {
            write(sdir.join(format!("lute_note_{i}.wav")), &src.bytes)?;
            wavs += 1;
        }
    }
    for (i, h) in s.drones.iter().enumerate() {
        if let Some(src) = sources.get(h) {
            write(sdir.join(format!("drone_{i}.wav")), &src.bytes)?;
            wavs += 1;
        }
    }
    let _ = &a;
    println!("exported {count} models, {pngs} images and {wavs} sounds to {}", dir.display());
    Ok(())
}
