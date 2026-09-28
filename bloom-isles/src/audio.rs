//! Sound effects and music notes are synthesized once at startup into WAV
//! buffers, so no audio files are needed.

use crate::sim::Sfx;
use bevy::audio::AudioSource;
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::TAU;

const RATE: u32 = 44_100;
pub const PENTA: [f32; 6] = [523.25, 587.33, 659.25, 783.99, 880.0, 1046.5];

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Tri,
    Square,
}

struct Buf {
    s: Vec<f32>,
    noise_seed: u32,
}

impl Buf {
    fn new() -> Self {
        Buf { s: vec![], noise_seed: 7 }
    }
    fn ensure(&mut self, n: usize) {
        if self.s.len() < n {
            self.s.resize(n, 0.0);
        }
    }
    fn tone(&mut self, f: f32, dur: f32, w: Wave, vol: f32, slide: f32, delay: f32) {
        let start = (delay * RATE as f32) as usize;
        let n = ((dur + 0.05) * RATE as f32) as usize;
        self.ensure(start + n);
        let f1 = (f + slide).max(30.0);
        let mut ph = 0.0f32;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let k = (t / dur).min(1.0);
            let freq = if slide != 0.0 { f * (f1 / f).powf(k) } else { f };
            ph = (ph + freq / RATE as f32) % 1.0;
            let v = match w {
                Wave::Sine => (ph * TAU).sin(),
                Wave::Tri => 1.0 - 4.0 * (ph - 0.5).abs(),
                Wave::Square => {
                    if ph < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
            };
            let env = if t < 0.012 { vol * t / 0.012 } else if t < dur { vol * (0.0008f32 / vol).powf((t - 0.012) / (dur - 0.012).max(0.001)) } else { 0.0 };
            self.s[start + i] += v * env;
        }
    }
    fn noise(&mut self, dur: f32, vol: f32, cutoff: f32, delay: f32, q: f32) {
        let start = (delay * RATE as f32) as usize;
        let n = ((dur + 0.05) * RATE as f32) as usize;
        self.ensure(start + n);
        // RBJ low-pass biquad
        let w0 = TAU * cutoff / RATE as f32;
        let alpha = w0.sin() / (2.0 * q);
        let cs = w0.cos();
        let (b0, b1, b2) = ((1.0 - cs) / 2.0, 1.0 - cs, (1.0 - cs) / 2.0);
        let (a0, a1, a2) = (1.0 + alpha, -2.0 * cs, 1.0 - alpha);
        let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for i in 0..n {
            self.noise_seed ^= self.noise_seed << 13;
            self.noise_seed ^= self.noise_seed >> 17;
            self.noise_seed ^= self.noise_seed << 5;
            let x = (self.noise_seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let y = (b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
            x2 = x1;
            x1 = x;
            y2 = y1;
            y1 = y;
            let t = i as f32 / RATE as f32;
            let env = if t < dur { vol * (0.001f32 / vol).powf(t / dur) } else { 0.0 };
            self.s[start + i] += y * env;
        }
    }
    fn wav(&self, gain: f32) -> Vec<u8> {
        let data_len = (self.s.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for &v in &self.s {
            let v = (v * gain).clamp(-1.0, 1.0);
            out.extend_from_slice(&((v * 32767.0) as i16).to_le_bytes());
        }
        out
    }
}

fn synth(s: Sfx) -> Buf {
    use Wave::*;
    let mut b = Buf::new();
    match s {
        Sfx::Place => {
            b.tone(440.0, 0.14, Tri, 0.2, 220.0, 0.0);
            b.tone(880.0, 0.16, Sine, 0.08, 0.0, 0.06);
            b.noise(0.08, 0.08, 900.0, 0.0, 0.8);
        }
        Sfx::PlaceSoft => b.tone(330.0, 0.1, Tri, 0.06, 120.0, 0.0),
        Sfx::Sprout => b.tone(PENTA[3], 0.25, Sine, 0.07, 0.0, 0.0),
        Sfx::Pick => b.tone(620.0, 0.06, Tri, 0.1, 120.0, 0.0),
        Sfx::Nope => b.tone(200.0, 0.14, Square, 0.05, -60.0, 0.0),
        Sfx::Whoosh => {
            b.noise(0.45, 0.22, 1400.0, 0.0, 2.0);
            b.tone(300.0, 0.4, Sine, 0.05, 400.0, 0.0);
        }
        Sfx::Alarm => {
            b.tone(740.0, 0.12, Square, 0.06, 0.0, 0.0);
            b.tone(560.0, 0.16, Square, 0.06, 0.0, 0.14);
        }
        Sfx::Thunder => {
            b.noise(1.4, 0.6, 260.0, 0.0, 0.5);
            b.noise(0.25, 0.35, 2200.0, 0.0, 0.8);
        }
        Sfx::ThunderFar => b.noise(1.0, 0.18, 200.0, 0.0, 0.5),
        Sfx::Puff => {
            b.tone(580.0, 0.08, Sine, 0.12, -200.0, 0.0);
            b.noise(0.1, 0.12, 1800.0, 0.0, 0.8);
        }
        Sfx::Blown => {
            for i in 0..3 {
                b.tone(660.0 + i as f32 * 220.0, 0.18, Tri, 0.1, 0.0, i as f32 * 0.07);
            }
            b.noise(0.5, 0.2, 1600.0, 0.0, 2.0);
        }
        Sfx::Squish => {
            b.noise(0.12, 0.3, 700.0, 0.0, 0.8);
            b.tone(160.0, 0.1, Square, 0.06, -80.0, 0.0);
        }
        Sfx::Munch => {
            b.noise(0.08, 0.1, 1200.0, 0.0, 0.8);
            b.noise(0.08, 0.1, 1200.0, 0.12, 0.8);
        }
        Sfx::Wither => b.tone(300.0, 0.4, Sine, 0.07, -180.0, 0.0),
        Sfx::Shield => {
            for i in 0..4 {
                b.tone(PENTA[i + 1], 0.5, Sine, 0.07, 0.0, i as f32 * 0.05);
            }
        }
        Sfx::Bounce => b.tone(900.0, 0.2, Sine, 0.1, -500.0, 0.0),
        Sfx::Reroll => {
            for i in 0..3 {
                b.tone(500.0 + i as f32 * 120.0, 0.06, Tri, 0.08, 0.0, i as f32 * 0.04);
            }
        }
        Sfx::Dig => {
            b.noise(0.18, 0.2, 500.0, 0.0, 0.8);
            b.tone(180.0, 0.12, Tri, 0.08, -60.0, 0.0);
        }
        Sfx::Click => b.tone(700.0, 0.05, Tri, 0.08, 0.0, 0.0),
        Sfx::Win => {
            for (i, k) in [0usize, 2, 4, 5, 4, 5].iter().enumerate() {
                let f = PENTA[*k] * if i < 3 { 1.0 } else { 2.0 };
                b.tone(f, 0.4, Tri, 0.12, 0.0, i as f32 * 0.13);
            }
        }
        Sfx::Lose => {
            for (i, k) in [4usize, 3, 1, 0].iter().enumerate() {
                b.tone(PENTA[*k] / 2.0, 0.45, Tri, 0.1, 0.0, i as f32 * 0.18);
            }
        }
    }
    b
}

pub const ALL_SFX: [Sfx; 21] = [
    Sfx::Place,
    Sfx::PlaceSoft,
    Sfx::Sprout,
    Sfx::Pick,
    Sfx::Nope,
    Sfx::Whoosh,
    Sfx::Alarm,
    Sfx::Thunder,
    Sfx::ThunderFar,
    Sfx::Puff,
    Sfx::Blown,
    Sfx::Squish,
    Sfx::Munch,
    Sfx::Wither,
    Sfx::Shield,
    Sfx::Bounce,
    Sfx::Reroll,
    Sfx::Dig,
    Sfx::Click,
    Sfx::Win,
    Sfx::Lose,
];

#[derive(Resource)]
pub struct Sounds {
    pub sfx: HashMap<Sfx, Handle<AudioSource>>,
    /// Soft music-box notes: two octaves of the pentatonic scale plus a low drone.
    pub notes: Vec<Handle<AudioSource>>,
}

pub fn build_sounds(assets: &mut Assets<AudioSource>) -> Sounds {
    let mut sfx = HashMap::new();
    for s in ALL_SFX {
        let b = synth(s);
        sfx.insert(s, assets.add(AudioSource { bytes: b.wav(1.6).into() }));
    }
    let mut notes = vec![];
    for oct in [0.5f32, 1.0] {
        for f in PENTA {
            let mut b = Buf::new();
            b.tone(f * oct, 1.1, Wave::Sine, 0.028, 0.0, 0.0);
            notes.push(assets.add(AudioSource { bytes: b.wav(1.6).into() }));
        }
    }
    let mut b = Buf::new();
    b.tone(PENTA[0] / 4.0, 1.8, Wave::Tri, 0.03, 0.0, 0.0);
    notes.push(assets.add(AudioSource { bytes: b.wav(1.6).into() }));
    Sounds { sfx, notes }
}
