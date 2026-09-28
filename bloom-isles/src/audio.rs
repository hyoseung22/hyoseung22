//! Every sound is synthesized once at startup into an in-memory WAV:
//! plucked-lute notes, wooden board-game clicks, and a small formant-synth
//! crowd that can go "wooow", cheer, clap and "aww".

use crate::rng::Rng;
use crate::sim::Sfx;
use bevy::audio::AudioSource;
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::TAU;

const RATE: u32 = 44_100;
const RF: f32 = RATE as f32;
/// D dorian, one octave from D4.
pub const DORIAN: [f32; 8] = [293.66, 329.63, 349.23, 392.0, 440.0, 493.88, 523.25, 587.33];

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Tri,
    Square,
}

/// A vowel as its first two formant frequencies.
#[derive(Clone, Copy)]
struct Vowel(f32, f32);
const U: Vowel = Vowel(320.0, 760.0);
const A: Vowel = Vowel(760.0, 1180.0);
const O: Vowel = Vowel(520.0, 860.0);
const E: Vowel = Vowel(480.0, 1900.0);
const I: Vowel = Vowel(300.0, 2250.0);

struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn new() -> Self {
        Biquad { b0: 0.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }
    /// Band-pass with 0 dB peak.
    fn bandpass(&mut self, f: f32, q: f32) {
        let w0 = TAU * f.min(RF * 0.45) / RF;
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        self.b0 = alpha / a0;
        self.b1 = 0.0;
        self.b2 = -alpha / a0;
        self.a1 = -2.0 * w0.cos() / a0;
        self.a2 = (1.0 - alpha) / a0;
    }
    fn lowpass(&mut self, f: f32, q: f32) {
        let w0 = TAU * f.min(RF * 0.45) / RF;
        let alpha = w0.sin() / (2.0 * q);
        let cs = w0.cos();
        let a0 = 1.0 + alpha;
        self.b0 = (1.0 - cs) / 2.0 / a0;
        self.b1 = (1.0 - cs) / a0;
        self.b2 = (1.0 - cs) / 2.0 / a0;
        self.a1 = -2.0 * cs / a0;
        self.a2 = (1.0 - alpha) / a0;
    }
    fn highpass(&mut self, f: f32, q: f32) {
        let w0 = TAU * f.min(RF * 0.45) / RF;
        let alpha = w0.sin() / (2.0 * q);
        let cs = w0.cos();
        let a0 = 1.0 + alpha;
        self.b0 = (1.0 + cs) / 2.0 / a0;
        self.b1 = -(1.0 + cs) / a0;
        self.b2 = (1.0 + cs) / 2.0 / a0;
        self.a1 = -2.0 * cs / a0;
        self.a2 = (1.0 - alpha) / a0;
    }
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

struct Buf {
    s: Vec<f32>,
    rng: Rng,
}

impl Buf {
    fn new(seed: u32) -> Self {
        Buf { s: vec![], rng: Rng::new(seed) }
    }
    fn at(&mut self, i: usize) -> &mut f32 {
        if self.s.len() <= i {
            self.s.resize(i + 1, 0.0);
        }
        &mut self.s[i]
    }
    fn white(&mut self) -> f32 {
        self.rng.f() * 2.0 - 1.0
    }

    fn tone(&mut self, f: f32, dur: f32, w: Wave, vol: f32, slide: f32, delay: f32) {
        let start = (delay * RF) as usize;
        let n = ((dur + 0.05) * RF) as usize;
        let f1 = (f + slide).max(30.0);
        let mut ph = 0.0f32;
        for i in 0..n {
            let t = i as f32 / RF;
            let k = (t / dur).min(1.0);
            let freq = if slide != 0.0 { f * (f1 / f).powf(k) } else { f };
            ph = (ph + freq / RF) % 1.0;
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
            let env = if t < 0.008 { vol * t / 0.008 } else if t < dur { vol * (0.0008f32 / vol).powf((t - 0.008) / (dur - 0.008).max(0.001)) } else { 0.0 };
            *self.at(start + i) += v * env;
        }
    }

    /// Filtered noise burst. kind: 0 low-pass, 1 band-pass, 2 high-pass.
    fn noise(&mut self, dur: f32, vol: f32, f: f32, delay: f32, q: f32, kind: u8) {
        let start = (delay * RF) as usize;
        let n = ((dur + 0.02) * RF) as usize;
        let mut bq = Biquad::new();
        match kind {
            0 => bq.lowpass(f, q),
            1 => bq.bandpass(f, q),
            _ => bq.highpass(f, q),
        }
        for i in 0..n {
            let t = i as f32 / RF;
            let x = self.white();
            let y = bq.run(x);
            let env = if t < 0.003 { t / 0.003 } else { (0.001f32).powf(t / dur) };
            *self.at(start + i) += y * vol * env;
        }
    }

    /// Filtered noise with a flat body and soft ends, for ambience that loops.
    fn noise_bed(&mut self, dur: f32, vol: f32, f: f32, q: f32, kind: u8) {
        let n = (dur * RF) as usize;
        let mut bq = Biquad::new();
        if kind == 0 { bq.lowpass(f, q) } else { bq.bandpass(f, q) }
        for i in 0..n {
            let t = i as f32 / RF;
            let env = (t / 0.25).min(1.0) * ((dur - t) / 0.25).clamp(0.0, 1.0);
            let x = self.white();
            let y = bq.run(x);
            *self.at(i) += y * vol * env;
        }
    }

    /// Karplus-Strong plucked string.
    fn pluck(&mut self, f: f32, dur: f32, vol: f32, delay: f32, bright: f32) {
        let start = (delay * RF) as usize;
        let len = (RF / f).max(2.0) as usize;
        let mut ring: Vec<f32> = (0..len).map(|_| self.rng.f() * 2.0 - 1.0).collect();
        // soften the attack
        for k in 1..len {
            ring[k] = ring[k] * bright + ring[k - 1] * (1.0 - bright);
        }
        let n = (dur * RF) as usize;
        let decay = 0.4985 + 0.0013 * (1.0 - (f / 1200.0).min(1.0));
        let mut idx = 0;
        for i in 0..n {
            let next = (idx + 1) % len;
            let out = ring[idx];
            ring[idx] = (ring[idx] + ring[next]) * decay;
            idx = next;
            let fade = if i + 800 > n { (n - i) as f32 / 800.0 } else { 1.0 };
            *self.at(start + i) += out * vol * fade;
        }
    }

    /// One formant-synthesized voice gliding through vowels.
    #[allow(clippy::too_many_arguments)]
    fn voice(&mut self, delay: f32, dur: f32, pitch: [f32; 3], vowels: &[Vowel], vol: f32, vib: f32, breath: f32) {
        let start = (delay * RF) as usize;
        let n = (dur * RF) as usize;
        let mut f1 = Biquad::new();
        let mut f2 = Biquad::new();
        let mut lp = Biquad::new();
        lp.lowpass(3500.0, 0.7);
        let mut ph = 0.0f32;
        let vib_rate = 5.0 + self.rng.f() * 2.0;
        for i in 0..n {
            let k = i as f32 / n as f32;
            if i % 32 == 0 {
                let seg = k * (vowels.len() - 1) as f32;
                let j = (seg as usize).min(vowels.len() - 2);
                let fr = seg - j as f32;
                let (a, b) = (vowels[j], vowels[j + 1]);
                f1.bandpass(a.0 + (b.0 - a.0) * fr, 6.0);
                f2.bandpass(a.1 + (b.1 - a.1) * fr, 9.0);
            }
            let p = if k < 0.5 { pitch[0] + (pitch[1] - pitch[0]) * (k * 2.0) } else { pitch[1] + (pitch[2] - pitch[1]) * ((k - 0.5) * 2.0) };
            let t = i as f32 / RF;
            let f = p * (1.0 + vib * (t * vib_rate * TAU).sin());
            ph = (ph + f / RF) % 1.0;
            let src = 2.0 * ph - 1.0 + self.white() * breath;
            let y = lp.run(f1.run(src) * 1.0 + f2.run(src) * 0.7);
            let env = (k / 0.12).min(1.0) * ((1.0 - k) / 0.25).min(1.0);
            *self.at(start + i) += y * vol * env;
        }
    }

    /// A crowd: many voices plus a murmur of breath.
    #[allow(clippy::too_many_arguments)]
    fn crowd(&mut self, n: usize, dur: f32, contour: [f32; 3], vowels: &[Vowel], vol: f32, spread: f32, delay: f32) {
        for _ in 0..n {
            let base = if self.rng.f() < 0.5 { 120.0 + self.rng.f() * 70.0 } else { 210.0 + self.rng.f() * 120.0 };
            let d = delay + self.rng.f() * spread;
            let len = dur * (0.75 + self.rng.f() * 0.45);
            let pitch = [base * contour[0], base * contour[1], base * contour[2]];
            let v = vol / (n as f32).sqrt() * (0.7 + self.rng.f() * 0.6);
            let vib = 0.012 + self.rng.f() * 0.02;
            self.voice(d, len, pitch, vowels, v, vib, 0.25);
        }
        self.noise(dur * 1.1, vol * 0.25, 1400.0, delay, 0.6, 1);
    }

    fn applause(&mut self, dur: f32, density: f32, vol: f32, delay: f32) {
        let claps = (dur * density) as usize;
        for _ in 0..claps {
            let t = self.rng.f().powf(0.8) * dur;
            let fade = 1.0 - (t / dur).powf(2.0);
            let f = 1200.0 + self.rng.f() * 2200.0;
            let len = 0.018 + self.rng.f() * 0.02;
            let v = vol * fade * (0.5 + self.rng.f() * 0.5);
            self.noise(len, v, f, delay + t, 1.2, 1);
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
            // gentle soft clip keeps loud crowds from crackling
            let v = (v * gain).tanh();
            out.extend_from_slice(&((v * 32000.0) as i16).to_le_bytes());
        }
        out
    }
}

fn synth(s: Sfx) -> Buf {
    use Wave::*;
    let mut b = Buf::new(match s {
        Sfx::Combo(n) => 100 + n as u32,
        _ => 7,
    });
    match s {
        Sfx::Place => {
            b.tone(820.0, 0.07, Sine, 0.25, -200.0, 0.0);
            b.noise(0.02, 0.2, 2500.0, 0.0, 2.0, 1);
            b.pluck(DORIAN[4], 0.6, 0.35, 0.02, 0.6);
        }
        Sfx::PlaceSoft => {
            b.tone(700.0, 0.06, Sine, 0.08, -150.0, 0.0);
        }
        Sfx::Sprout => b.pluck(DORIAN[7] * 2.0, 0.5, 0.2, 0.0, 0.5),
        Sfx::Pick => {
            b.tone(1100.0, 0.04, Sine, 0.12, -300.0, 0.0);
            b.noise(0.015, 0.1, 3000.0, 0.0, 2.0, 1);
        }
        Sfx::Nope => {
            b.tone(180.0, 0.12, Square, 0.05, -40.0, 0.0);
            b.tone(150.0, 0.14, Square, 0.05, -40.0, 0.09);
        }
        Sfx::Whoosh => {
            b.noise(0.5, 0.3, 900.0, 0.0, 1.5, 1);
            b.noise(0.35, 0.15, 2400.0, 0.1, 2.0, 1);
        }
        Sfx::Alarm => {
            b.tone(660.0, 0.14, Tri, 0.12, 0.0, 0.0);
            b.tone(495.0, 0.2, Tri, 0.12, 0.0, 0.16);
        }
        Sfx::Thunder => {
            b.noise(1.6, 0.8, 220.0, 0.0, 0.5, 0);
            b.noise(0.3, 0.45, 2000.0, 0.0, 0.8, 0);
        }
        Sfx::ThunderFar => b.noise(1.2, 0.25, 180.0, 0.0, 0.5, 0),
        Sfx::Puff => {
            b.noise(0.12, 0.25, 1200.0, 0.0, 1.0, 1);
            b.tone(420.0, 0.08, Sine, 0.08, -150.0, 0.0);
        }
        Sfx::Blown => {
            b.noise(0.7, 0.35, 1100.0, 0.0, 1.0, 1);
            for (i, k) in [2usize, 4, 7].iter().enumerate() {
                b.pluck(DORIAN[*k], 0.6, 0.25, i as f32 * 0.07, 0.7);
            }
        }
        Sfx::Squish => {
            b.noise(0.1, 0.35, 600.0, 0.0, 1.0, 0);
            b.tone(140.0, 0.08, Square, 0.06, -60.0, 0.0);
        }
        Sfx::Munch => {
            for i in 0..3 {
                b.noise(0.05, 0.18, 1500.0, i as f32 * 0.09, 1.5, 1);
            }
        }
        Sfx::Wither => b.tone(260.0, 0.5, Sine, 0.08, -140.0, 0.0),
        Sfx::Shield => {
            for i in 0..5 {
                b.tone(DORIAN[i + 2] * 2.0, 0.9, Sine, 0.05, 0.0, i as f32 * 0.04);
            }
        }
        Sfx::Bounce => b.tone(900.0, 0.25, Sine, 0.12, -500.0, 0.0),
        Sfx::Reroll => {
            for i in 0..4 {
                b.tone(900.0 + i as f32 * 80.0, 0.04, Sine, 0.1, -200.0, i as f32 * 0.045);
                b.noise(0.015, 0.1, 3000.0, i as f32 * 0.045, 2.0, 1);
            }
        }
        Sfx::Dig => {
            b.noise(0.2, 0.3, 450.0, 0.0, 0.8, 0);
            b.tone(170.0, 0.12, Tri, 0.08, -60.0, 0.0);
        }
        Sfx::Click => {
            b.tone(1000.0, 0.03, Sine, 0.1, -200.0, 0.0);
        }
        Sfx::Win => {
            for (i, k) in [0usize, 2, 4, 7, 4, 7].iter().enumerate() {
                b.pluck(DORIAN[*k] * if i < 3 { 1.0 } else { 2.0 }, 1.2, 0.35, i as f32 * 0.14, 0.7);
            }
            b.tone(DORIAN[0] / 2.0, 1.6, Tri, 0.1, 0.0, 0.0);
        }
        Sfx::Lose => {
            for (i, k) in [4usize, 3, 2, 0].iter().enumerate() {
                b.pluck(DORIAN[*k] / 2.0, 1.2, 0.3, i as f32 * 0.22, 0.5);
            }
        }
        Sfx::Tock => {
            b.tone(1250.0, 0.05, Sine, 0.18, -250.0, 0.0);
            b.tone(620.0, 0.08, Sine, 0.12, -100.0, 0.0);
            b.noise(0.012, 0.12, 3500.0, 0.0, 2.0, 1);
        }
        Sfx::Wow => b.voice(0.0, 0.9, [170.0, 245.0, 190.0], &[U, A, A, U], 0.45, 0.02, 0.15),
        Sfx::CrowdWow => {
            b.crowd(14, 1.4, [0.85, 1.25, 0.95], &[U, A, A, U], 0.9, 0.2, 0.0);
            b.tone(1800.0, 0.6, Sine, 0.03, 900.0, 0.3);
        }
        Sfx::Cheer => {
            b.crowd(14, 1.3, [1.1, 1.3, 1.15], &[E, A, A], 0.8, 0.25, 0.0);
            b.applause(1.4, 50.0, 0.25, 0.1);
        }
        Sfx::BigCheer => {
            b.crowd(26, 2.6, [1.1, 1.35, 1.2], &[I, E, A, A, A], 1.1, 0.4, 0.0);
            b.applause(3.0, 90.0, 0.35, 0.2);
            for i in 0..3 {
                let d = 0.4 + i as f32 * 0.5;
                b.tone(2000.0, 0.5, Sine, 0.05, 900.0, d);
            }
        }
        Sfx::Applause => b.applause(1.8, 70.0, 0.4, 0.0),
        Sfx::Aww => b.crowd(12, 1.3, [1.15, 1.0, 0.8], &[A, O, O], 0.8, 0.15, 0.0),
        Sfx::Drumroll => {
            let mut t = 0.0f32;
            while t < 1.6 {
                let k = t / 1.6;
                b.noise(0.06, 0.12 + 0.3 * k, 1800.0, t, 0.8, 1);
                b.tone(190.0, 0.04, Tri, 0.05 + 0.1 * k, -40.0, t);
                t += 1.0 / (16.0 + 16.0 * k);
            }
        }
        Sfx::Cymbal => {
            b.noise(2.2, 0.45, 5000.0, 0.0, 0.7, 2);
            for f in [3150.0, 4730.0, 5910.0, 7420.0] {
                b.tone(f, 1.8, Sine, 0.03, 0.0, 0.0);
            }
            b.tone(100.0, 0.4, Sine, 0.4, -55.0, 0.0);
        }
        Sfx::Thud => {
            b.tone(110.0, 0.35, Sine, 0.6, -65.0, 0.0);
            b.noise(0.03, 0.2, 1500.0, 0.0, 1.0, 0);
        }
        Sfx::Firework => {
            b.tone(600.0, 0.6, Sine, 0.04, 900.0, 0.0);
            b.noise(0.4, 0.6, 2600.0, 0.6, 0.7, 0);
            for _ in 0..40 {
                let t = 0.7 + b.rng.f() * 0.9;
                b.noise(0.01, 0.15, 5000.0, t, 1.5, 1);
            }
        }
        Sfx::Combo(n) => {
            let k = (n as usize).min(8);
            let base = DORIAN[(k + 1) % 8] * if k >= 7 { 2.0 } else { 1.0 };
            b.pluck(base, 0.7, 0.4, 0.0, 0.8);
            b.pluck(base * 1.5, 0.6, 0.25, 0.06, 0.8);
            b.tone(base * 4.0, 0.3, Sine, 0.04 + 0.01 * k as f32, 0.0, 0.1);
        }
        Sfx::Jackpot => {
            for i in 0..10 {
                b.pluck(DORIAN[i % 8] * if i >= 8 { 2.0 } else { 1.0 } * 2.0, 0.5, 0.25, i as f32 * 0.045, 0.9);
            }
            for i in 0..6 {
                b.tone(2637.0, 0.12, Sine, 0.05, 0.0, 0.45 + i as f32 * 0.07);
            }
            b.crowd(10, 1.0, [1.0, 1.3, 1.25], &[U, A, U], 0.5, 0.2, 0.35);
        }
        Sfx::Splash => {
            b.noise(1.0, 0.5, 800.0, 0.0, 0.6, 0);
            for i in 0..6 {
                let f = 500.0 + b.rng.f() * 700.0;
                b.tone(f, 0.08, Sine, 0.06, f * 0.8, 0.15 + i as f32 * 0.08);
            }
        }
        Sfx::Whale => {
            b.voice(0.0, 1.8, [140.0, 95.0, 120.0], &[U, O, U], 0.5, 0.03, 0.05);
            b.noise(0.8, 0.3, 1800.0, 1.3, 0.8, 1);
        }
        Sfx::Coin => {
            b.tone(988.0, 0.08, Square, 0.06, 0.0, 0.0);
            b.tone(1319.0, 0.35, Square, 0.06, 0.0, 0.08);
        }
        Sfx::Pollinate => {
            b.tone(1760.0, 0.18, Sine, 0.05, 300.0, 0.0);
            b.tone(2349.0, 0.22, Sine, 0.04, 0.0, 0.06);
            b.noise(0.12, 0.03, 4000.0, 0.0, 2.0, 1);
        }
        Sfx::Chomp => {
            b.noise(0.06, 0.4, 900.0, 0.0, 1.0, 0);
            b.tone(120.0, 0.12, Square, 0.08, -40.0, 0.0);
            b.noise(0.05, 0.3, 1400.0, 0.1, 1.0, 1);
        }
        Sfx::Tongue => {
            b.tone(300.0, 0.12, Sine, 0.18, 900.0, 0.0);
            b.noise(0.05, 0.15, 2000.0, 0.1, 1.5, 1);
        }
        Sfx::Peck => {
            for i in 0..2 {
                b.tone(1500.0, 0.03, Sine, 0.12, -600.0, i as f32 * 0.08);
                b.noise(0.01, 0.15, 3500.0, i as f32 * 0.08, 2.0, 1);
            }
        }
        Sfx::SeedDrop => {
            b.tone(700.0, 0.12, Sine, 0.12, -400.0, 0.0);
            b.pluck(DORIAN[5], 0.4, 0.2, 0.08, 0.6);
        }
        Sfx::Fear => b.tone(1200.0, 0.14, Sine, 0.08, 700.0, 0.0),
        Sfx::Bloom => {
            for i in 0..6 {
                b.pluck(DORIAN[(i * 2) % 8] * if i >= 4 { 2.0 } else { 1.0 }, 0.8, 0.18, i as f32 * 0.06, 0.8);
            }
        }
        Sfx::RainStart => {
            b.noise(2.5, 0.25, 3000.0, 0.0, 0.5, 0);
            b.tone(DORIAN[0] / 2.0, 1.5, Tri, 0.05, 0.0, 0.0);
        }
        Sfx::WindStart => b.noise(2.0, 0.35, 700.0, 0.0, 3.0, 1),
        Sfx::DroughtStart => {
            b.tone(DORIAN[4], 1.4, Sine, 0.05, 0.0, 0.0);
            b.tone(DORIAN[4] * 1.01, 1.4, Sine, 0.05, 0.0, 0.0);
            for i in 0..8 {
                b.tone(4200.0, 0.05, Square, 0.01, 0.0, 0.3 + i as f32 * 0.09);
            }
        }
        Sfx::ClearStart => {
            for (i, k) in [0usize, 4, 7].iter().enumerate() {
                b.pluck(DORIAN[*k], 1.0, 0.2, i as f32 * 0.1, 0.7);
            }
        }
        Sfx::RainLoop => {
            b.noise_bed(4.2, 0.08, 2600.0, 0.5, 0);
            for _ in 0..90 {
                let t = b.rng.f() * 4.0;
                b.noise(0.01, 0.05, 5000.0, t, 1.5, 1);
            }
        }
        Sfx::WindLoop => {
            b.noise_bed(4.2, 0.35, 500.0, 3.0, 1);
            b.noise_bed(4.2, 0.12, 1200.0, 4.0, 1);
        }
    }
    b
}

pub fn all_sfx() -> Vec<Sfx> {
    let mut v = vec![
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
        Sfx::Tock,
        Sfx::Wow,
        Sfx::CrowdWow,
        Sfx::Cheer,
        Sfx::BigCheer,
        Sfx::Applause,
        Sfx::Aww,
        Sfx::Drumroll,
        Sfx::Cymbal,
        Sfx::Thud,
        Sfx::Firework,
        Sfx::Jackpot,
        Sfx::Splash,
        Sfx::Whale,
        Sfx::Coin,
        Sfx::Pollinate,
        Sfx::Chomp,
        Sfx::Tongue,
        Sfx::Peck,
        Sfx::SeedDrop,
        Sfx::Fear,
        Sfx::Bloom,
        Sfx::RainStart,
        Sfx::WindStart,
        Sfx::DroughtStart,
        Sfx::ClearStart,
        Sfx::RainLoop,
        Sfx::WindLoop,
    ];
    v.extend((2..=8).map(Sfx::Combo));
    v
}

#[derive(Resource)]
pub struct Sounds {
    pub sfx: HashMap<Sfx, Handle<AudioSource>>,
    /// Plucked-lute notes of the D dorian scale.
    pub notes: Vec<Handle<AudioSource>>,
    /// Low drones (D and A).
    pub drones: Vec<Handle<AudioSource>>,
}

fn drone(f: f32, dur: f32, vol: f32) -> Buf {
    let mut b = Buf::new(3);
    let n = (dur * RF) as usize;
    let mut lp = Biquad::new();
    lp.lowpass(f * 3.0, 0.7);
    let mut ph = 0.0f32;
    for i in 0..n {
        let k = i as f32 / n as f32;
        ph = (ph + f * (1.0 + 0.002 * (i as f32 / RF * 4.0 * TAU).sin()) / RF) % 1.0;
        let env = (k / 0.3).min(1.0) * ((1.0 - k) / 0.4).min(1.0);
        let y = lp.run(2.0 * ph - 1.0);
        *b.at(i) += y * vol * env;
    }
    b
}

pub fn build_sounds(assets: &mut Assets<AudioSource>) -> Sounds {
    let mut sfx = HashMap::new();
    for s in all_sfx() {
        let b = synth(s);
        sfx.insert(s, assets.add(AudioSource { bytes: b.wav(1.5).into() }));
    }
    let notes = DORIAN
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            let mut b = Buf::new(50 + i as u32);
            b.pluck(f, 1.8, 0.16, 0.0, 0.55);
            assets.add(AudioSource { bytes: b.wav(1.0).into() })
        })
        .collect();
    let drones = [146.83f32, 110.0].iter().map(|&f| assets.add(AudioSource { bytes: drone(f, 4.5, 0.05).wav(1.0).into() })).collect();
    Sounds { sfx, notes, drones }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_is_finite_and_audible() {
        for s in all_sfx() {
            let b = synth(s);
            assert!(!b.s.is_empty(), "{s:?} is empty");
            assert!(b.s.iter().all(|v| v.is_finite()), "{s:?} has NaN");
            let peak = b.s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.01, "{s:?} is silent ({peak})");
        }
    }
}
