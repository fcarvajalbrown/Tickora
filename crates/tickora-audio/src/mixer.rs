use std::sync::Arc;

use tickora_core::ecs::{Entity, World};
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::noise::Location;
use tickora_core::rng::Rng;

use crate::bank::{Attenuation, Envelope, SoundBank, SoundDef, SoundId, Source};
use crate::capture::BlockSummary;
use crate::component::{Listener, PlaySound, SoundEmitter, audio_rng};
use crate::format::{AudioFormat, MAX_SAMPLE};
use crate::graph::{MixGraph, SubmixId};
use crate::math::{QUARTER, cosine, isqrt, mul_fx, ratio_fx, sine, sine_table};

pub const MAX_VOICES: usize = 64;
pub const MAX_PITCH: Fx = Fx::from_int(16);
pub const UNITY: i64 = 1 << 32;
const DISTANCE_SHIFT: u32 = 16;
const PAN_ONE: i64 = 1 << 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub frames: Vec<[i32; 2]>,
    pub summary: BlockSummary,
    pub events: Vec<VoiceEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceStart {
    pub id: u64,
    pub sound: SoundId,
    pub take: u32,
    pub origin: Origin,
    pub gain: Fx,
    pub pitch: Fx,
    pub distance: Option<u128>,
    pub gains: [i64; 2],
    pub submix: SubmixId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceEvent {
    Start(VoiceStart),
    End { id: u64, stopped: bool },
    Dropped { sound: SoundId, origin: Origin },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    At(Location),
    Emitter(Entity),
}

#[derive(Clone, Debug)]
struct Voice {
    id: u64,
    reported: bool,
    sound: SoundId,
    origin: Origin,
    gain: Fx,
    pitch: Fx,
    seed: u64,
    take: u32,
    played: u64,
    end: Option<u64>,
    stopping: bool,
    phase: u32,
    position: u64,
    noise: Rng,
    held: i32,
}

#[derive(Clone, Copy)]
struct Rates {
    increment: u32,
    step: u64,
}

impl Voice {
    fn new(
        id: u64,
        def: &SoundDef,
        sound: SoundId,
        origin: Origin,
        play: VoiceParams,
        rate: u32,
    ) -> Self {
        let mut noise = audio_rng(play.seed);
        let held = noise_sample(&mut noise);
        let mut voice = Voice {
            id,
            reported: false,
            sound,
            origin,
            gain: play.gain,
            pitch: play.pitch,
            seed: play.seed,
            take: play.take,
            played: 0,
            end: None,
            stopping: false,
            phase: 0,
            position: 0,
            noise,
            held,
        };
        if let Origin::At(_) = origin {
            voice.end = Some(voice.one_shot_length(def, rate));
        }
        voice
    }

    fn one_shot_length(&self, def: &SoundDef, rate: u32) -> u64 {
        match def.source.pcm(self.take) {
            Some((samples, _)) => {
                let step = self.rates(def, rate).step;
                if step == 0 {
                    return 0;
                }
                let span = u128::from(samples.len() as u64) << 32;
                span.div_ceil(u128::from(step)) as u64
            }
            None => u64::from(def.envelope.duration),
        }
    }

    fn pitch_raw(&self) -> u128 {
        self.pitch.clamp(Fx::ZERO, MAX_PITCH).raw() as u128
    }

    fn rates(&self, def: &SoundDef, rate: u32) -> Rates {
        let pitch = self.pitch_raw();
        let increment = match &def.source {
            Source::Sine { hz } | Source::Square { hz } => {
                let hz = hz.raw().max(0) as u128;
                (((hz * pitch) >> 32) / u128::from(rate)).min(u128::from(u32::MAX)) as u32
            }
            _ => 0,
        };
        let step = match (&def.source, def.source.pcm(self.take)) {
            (_, Some((_, source))) => {
                (u128::from(source) * pitch / u128::from(rate)).min(u128::from(u64::MAX)) as u64
            }
            (Source::Noise, None) => pitch.min(u128::from(u64::MAX)) as u64,
            _ => 0,
        };
        Rates { increment, step }
    }

    fn finished(&self) -> bool {
        self.end.is_some_and(|end| self.played >= end)
    }

    fn stop(&mut self, envelope: &Envelope) {
        self.stopping = true;
        let end = self.played + u64::from(envelope.release);
        self.end = Some(self.end.map_or(end, |known| known.min(end)));
    }

    fn level(&self, envelope: &Envelope) -> i64 {
        let mut level = UNITY;
        let attack = u64::from(envelope.attack);
        if self.played < attack {
            level = ratio_fx(self.played, attack);
        }
        let release = u64::from(envelope.release);
        if let Some(end) = self.end {
            let remaining = end.saturating_sub(self.played);
            if release > 0 && remaining <= release {
                level = level.min(ratio_fx(remaining, release));
            }
        }
        level
    }

    fn next(&mut self, def: &SoundDef, rates: Rates) -> i32 {
        match &def.source {
            Source::Sine { .. } => {
                let value = (i64::from(sine(self.phase)) * i64::from(MAX_SAMPLE)) >> 30;
                self.phase = self.phase.wrapping_add(rates.increment);
                value as i32
            }
            Source::Square { .. } => {
                let value = if self.phase < 1 << 31 {
                    MAX_SAMPLE
                } else {
                    -MAX_SAMPLE
                };
                self.phase = self.phase.wrapping_add(rates.increment);
                value
            }
            Source::Noise => {
                let value = self.held;
                self.position = self.position.saturating_add(rates.step);
                while self.position >= 1 << 32 {
                    self.held = noise_sample(&mut self.noise);
                    self.position -= 1 << 32;
                }
                value
            }
            Source::Pcm { .. } | Source::Takes { .. } => {
                let Some((samples, _)) = def.source.pcm(self.take) else {
                    return 0;
                };
                let looping = matches!(self.origin, Origin::Emitter(_));
                let span = (samples.len() as u64) << 32;
                if looping && self.position >= span {
                    self.position %= span;
                }
                let index = (self.position >> 32) as usize;
                let value = pcm_at(samples, index, self.position & 0xFFFF_FFFF, looping);
                self.position = self.position.saturating_add(rates.step);
                value
            }
        }
    }
}

#[derive(Clone, Copy)]
struct VoiceParams {
    gain: Fx,
    pitch: Fx,
    seed: u64,
    take: u32,
}

fn noise_sample(rng: &mut Rng) -> i32 {
    ((rng.next_u32() as i32) >> 8).max(-MAX_SAMPLE)
}

pub fn pcm_at(samples: &[i32], index: usize, fraction: u64, looping: bool) -> i32 {
    let Some(&here) = samples.get(index) else {
        return 0;
    };
    let next = match samples.get(index + 1) {
        Some(&next) => next,
        None if looping => samples[0],
        None => here,
    };
    let delta = i64::from(next) - i64::from(here);
    (i64::from(here) + ((delta * fraction as i64) >> 32)) as i32
}

pub fn linear_gain(inner_radius: Fx, falloff_distance: Fx, distance: u128) -> i64 {
    let inner = (inner_radius.raw().max(0) >> DISTANCE_SHIFT) as u128;
    let falloff = (falloff_distance.raw().max(0) >> DISTANCE_SHIFT) as u128;
    if distance <= inner {
        return UNITY;
    }
    let beyond = distance - inner;
    if beyond >= falloff {
        return 0;
    }
    (((falloff - beyond) << 32) / falloff) as i64
}

pub fn pan_gains(pan: i64) -> [i64; 2] {
    let pan = pan.clamp(-PAN_ONE, PAN_ONE);
    let index = (((pan + PAN_ONE) * QUARTER as i64) >> 31) as usize;
    let table = sine_table();
    [
        i64::from(table[index + QUARTER]) << 2,
        i64::from(table[index]) << 2,
    ]
}

pub fn yaw_phase(yaw: Fx) -> u32 {
    yaw.raw().div_euclid(360) as u32
}

fn delta(to: Fx, from: Fx) -> i128 {
    (i128::from(to.raw()) - i128::from(from.raw())) >> DISTANCE_SHIFT
}

pub fn listener_distance(listener: Option<&Listener>, location: Option<Location>) -> Option<u128> {
    let (listener, location) = (listener?, location?);
    let at = listener.location;
    let squared = [
        delta(location.x, at.x),
        delta(location.y, at.y),
        delta(location.z, at.z),
    ]
    .iter()
    .map(|d| d.unsigned_abs() * d.unsigned_abs())
    .sum::<u128>();
    Some(isqrt(squared) >> DISTANCE_SHIFT)
}

pub fn spatial_gains(
    attenuation: &Attenuation,
    listener: Option<&Listener>,
    location: Option<Location>,
) -> [i64; 2] {
    let Attenuation::Linear {
        inner_radius,
        falloff_distance,
    } = *attenuation
    else {
        return [UNITY, UNITY];
    };
    let (Some(listener), Some(location)) = (listener, location) else {
        return [0, 0];
    };
    let at = listener.location;
    let (dx, dy, dz) = (
        delta(location.x, at.x),
        delta(location.y, at.y),
        delta(location.z, at.z),
    );
    let squared = [dx, dy, dz]
        .iter()
        .map(|d| d.unsigned_abs() * d.unsigned_abs())
        .sum::<u128>();
    let distance = isqrt(squared);
    let gain = linear_gain(inner_radius, falloff_distance, distance);
    if gain == 0 {
        return [0, 0];
    }
    let phase = yaw_phase(listener.yaw);
    let (right_x, right_y) = (-i128::from(sine(phase)), i128::from(cosine(phase)));
    let pan = if distance == 0 {
        0
    } else {
        ((dx * right_x + dy * right_y) / distance as i128) as i64
    };
    pan_gains(pan).map(|channel| mul_fx(channel, gain))
}

pub struct Mixer {
    format: AudioFormat,
    bank: Arc<SoundBank>,
    graph: MixGraph,
    voices: Vec<Voice>,
    last_plays: Vec<Entity>,
    next_id: u64,
    events: Vec<VoiceEvent>,
}

impl Mixer {
    pub fn new(format: AudioFormat, bank: Arc<SoundBank>, graph: MixGraph) -> Result<Self, String> {
        if let Some(def) = bank.defs().find(|def| def.submix.0 as usize >= graph.len()) {
            return Err(format!(
                "a sound feeds submix {}, which the mix graph does not have",
                def.submix.0
            ));
        }
        Ok(Mixer {
            format,
            bank,
            graph,
            voices: Vec::new(),
            last_plays: Vec::new(),
            next_id: 0,
            events: Vec::new(),
        })
    }

    pub fn format(&self) -> AudioFormat {
        self.format
    }

    pub fn bank(&self) -> &Arc<SoundBank> {
        &self.bank
    }

    pub fn graph(&self) -> &MixGraph {
        &self.graph
    }

    pub fn voices(&self) -> usize {
        self.voices.len()
    }

    pub fn render(&mut self, world: &World) -> Block {
        let mut dropped = self.start_plays(world);
        dropped += self.follow_emitters(world);
        let listener = world
            .query::<Listener>()
            .into_iter()
            .min_by_key(|(entity, _)| entity.index())
            .map(|(_, listener)| *listener);
        let samples = self.format.samples_per_tick() as usize;
        let mut buses = vec![vec![[0i64; 2]; samples]; self.graph.len()];
        let voices = self.voices.len() as u32;
        let rate = self.format.sample_rate();
        for voice in &mut self.voices {
            let def = self
                .bank
                .get(voice.sound)
                .expect("voices start only for sounds in the bank");
            let location = match voice.origin {
                Origin::At(location) => Some(location),
                Origin::Emitter(entity) => world.get::<Location>(entity).copied(),
            };
            let [left, right] = spatial_gains(&def.attenuation, listener.as_ref(), location)
                .map(|channel| mul_fx(channel, voice.gain.raw()));
            if !voice.reported {
                voice.reported = true;
                self.events.push(VoiceEvent::Start(VoiceStart {
                    id: voice.id,
                    sound: voice.sound,
                    take: voice.take,
                    origin: voice.origin,
                    gain: voice.gain,
                    pitch: voice.pitch,
                    distance: listener_distance(listener.as_ref(), location),
                    gains: [left, right],
                    submix: def.submix,
                }));
            }
            let rates = voice.rates(def, rate);
            for frame in buses[def.submix.0 as usize].iter_mut() {
                if voice.finished() {
                    break;
                }
                let level = voice.level(&def.envelope);
                let value = mul_fx(i64::from(voice.next(def, rates)), level);
                frame[0] += mul_fx(value, left);
                frame[1] += mul_fx(value, right);
                voice.played += 1;
            }
        }
        for voice in self.voices.iter().filter(|voice| voice.finished()) {
            self.events.push(VoiceEvent::End {
                id: voice.id,
                stopped: voice.stopping,
            });
        }
        self.voices.retain(|voice| !voice.finished());
        for (child, parent, gain) in self.graph.children_first() {
            let child = std::mem::take(&mut buses[child]);
            for (into, from) in buses[parent].iter_mut().zip(child) {
                into[0] += mul_fx(from[0], gain.raw());
                into[1] += mul_fx(from[1], gain.raw());
            }
        }
        let master_gain = self
            .graph
            .gain(crate::graph::SubmixId::MASTER)
            .unwrap_or(Fx::ONE);
        let mut clipped = 0;
        let frames: Vec<[i32; 2]> = buses[0]
            .iter()
            .map(|frame| {
                frame.map(|value| {
                    let value = mul_fx(value, master_gain.raw());
                    if value.abs() > i64::from(MAX_SAMPLE) {
                        clipped += 1;
                    }
                    value.clamp(-i64::from(MAX_SAMPLE), i64::from(MAX_SAMPLE)) as i32
                })
            })
            .collect();
        let summary = summarize(&frames, clipped, voices, dropped);
        Block {
            frames,
            summary,
            events: std::mem::take(&mut self.events),
        }
    }

    fn start(&mut self, sound: SoundId, origin: Origin, params: VoiceParams) -> u32 {
        let def = match self.bank.get(sound) {
            Some(def) if self.voices.len() < MAX_VOICES => def,
            _ => {
                self.events.push(VoiceEvent::Dropped { sound, origin });
                return 1;
            }
        };
        let voice = Voice::new(
            self.next_id,
            def,
            sound,
            origin,
            params,
            self.format.sample_rate(),
        );
        self.next_id += 1;
        self.voices.push(voice);
        0
    }

    fn start_plays(&mut self, world: &World) -> u32 {
        let mut plays: Vec<(Entity, PlaySound)> = world
            .query::<PlaySound>()
            .into_iter()
            .map(|(entity, play)| (entity, *play))
            .collect();
        plays.sort_by_key(|(entity, _)| entity.index());
        let mut dropped = 0;
        for (entity, play) in &plays {
            if self.last_plays.contains(entity) {
                continue;
            }
            let params = VoiceParams {
                gain: play.gain,
                pitch: play.pitch,
                seed: play.seed,
                take: play.take,
            };
            dropped += self.start(play.sound, Origin::At(play.location), params);
        }
        self.last_plays = plays.into_iter().map(|(entity, _)| entity).collect();
        dropped
    }

    fn follow_emitters(&mut self, world: &World) -> u32 {
        let mut emitters: Vec<(Entity, SoundEmitter)> = world
            .query::<SoundEmitter>()
            .into_iter()
            .map(|(entity, emitter)| (entity, *emitter))
            .collect();
        emitters.sort_by_key(|(entity, _)| entity.index());
        for voice in &mut self.voices {
            let Origin::Emitter(entity) = voice.origin else {
                continue;
            };
            if voice.stopping {
                continue;
            }
            let current = emitters
                .iter()
                .find(|(e, _)| *e == entity)
                .map(|(_, em)| em);
            match current {
                Some(emitter) if emitter.sound == voice.sound && emitter.seed == voice.seed => {
                    voice.gain = emitter.gain;
                    voice.pitch = emitter.pitch;
                }
                _ => {
                    let envelope = self
                        .bank
                        .get(voice.sound)
                        .map(|def| def.envelope)
                        .unwrap_or_default();
                    voice.stop(&envelope);
                }
            }
        }
        let mut dropped = 0;
        for (entity, emitter) in emitters {
            let playing = self
                .voices
                .iter()
                .any(|voice| voice.origin == Origin::Emitter(entity) && !voice.stopping);
            if !playing {
                let params = VoiceParams {
                    gain: emitter.gain,
                    pitch: emitter.pitch,
                    seed: emitter.seed,
                    take: 0,
                };
                dropped += self.start(emitter.sound, Origin::Emitter(entity), params);
            }
        }
        dropped
    }
}

fn summarize(frames: &[[i32; 2]], clipped: u32, voices: u32, dropped: u32) -> BlockSummary {
    let mut hasher = StateHasher::new();
    let mut peak = [0u32; 2];
    let mut sum_squares = [0u128; 2];
    for frame in frames {
        for (channel, &value) in frame.iter().enumerate() {
            hasher.write_bytes(&value.to_le_bytes()[..3]);
            peak[channel] = peak[channel].max(value.unsigned_abs());
            let magnitude = u128::from(value.unsigned_abs());
            sum_squares[channel] += magnitude * magnitude;
        }
    }
    BlockSummary {
        hash: hasher.finish(),
        samples: frames.len() as u64,
        peak,
        sum_squares,
        clipped,
        voices,
        dropped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank::{Envelope, Variation};
    use crate::component::{self, register};
    use crate::graph::{SubmixDef, SubmixId};

    fn fx(value: i32) -> Fx {
        Fx::from_int(value)
    }

    fn at(x: i32, y: i32, z: i32) -> Location {
        Location::new(fx(x), fx(y), fx(z))
    }

    fn q16(cm: u128) -> u128 {
        cm << DISTANCE_SHIFT
    }

    fn def(source: Source, duration: u32) -> SoundDef {
        SoundDef {
            source,
            envelope: Envelope {
                attack: 0,
                release: 0,
                duration,
            },
            variation: Variation::default(),
            attenuation: Attenuation::None,
            submix: SubmixId::MASTER,
        }
    }

    fn mixer(defs: Vec<SoundDef>, graph: MixGraph) -> Mixer {
        let mut bank = SoundBank::new();
        for (i, def) in defs.into_iter().enumerate() {
            bank.add(&format!("s{i}"), def).unwrap();
        }
        Mixer::new(AudioFormat::DEFAULT, Arc::new(bank), graph).unwrap()
    }

    fn world() -> World {
        let mut world = World::new();
        world.register::<Location>();
        register(&mut world);
        world
    }

    fn play(world: &mut World, sound: u32, gain: Fx) {
        let entity = world.spawn();
        world
            .insert(
                entity,
                PlaySound {
                    location: Location::default(),
                    sound: SoundId(sound),
                    gain,
                    pitch: Fx::ONE,
                    seed: 1,
                    take: 0,
                },
            )
            .unwrap();
    }

    #[test]
    fn linear_attenuation_is_one_inside_falls_in_a_line_and_is_zero_beyond() {
        let (inner, falloff) = (fx(100), fx(400));
        assert_eq!(linear_gain(inner, falloff, 0), UNITY);
        assert_eq!(linear_gain(inner, falloff, q16(100)), UNITY);
        assert!(linear_gain(inner, falloff, q16(100) + 1) < UNITY);
        assert_eq!(linear_gain(inner, falloff, q16(300)), UNITY / 2);
        assert_eq!(linear_gain(inner, falloff, q16(400)), UNITY / 4);
        assert!(linear_gain(inner, falloff, q16(500) - 1) > 0);
        assert_eq!(linear_gain(inner, falloff, q16(500)), 0);
        assert_eq!(linear_gain(inner, falloff, q16(9000)), 0);
        assert_eq!(linear_gain(inner, Fx::ZERO, q16(100)), UNITY);
        assert_eq!(linear_gain(inner, Fx::ZERO, q16(100) + 1), 0);
    }

    #[test]
    fn equal_power_panning_from_hard_left_through_centre_to_hard_right() {
        assert_eq!(pan_gains(-PAN_ONE), [UNITY, 0]);
        assert_eq!(pan_gains(PAN_ONE), [0, UNITY]);
        let [left, right] = pan_gains(0);
        assert_eq!(left, right);
        let half_power = (left as f64 / UNITY as f64).powi(2);
        assert!((half_power - 0.5).abs() < 1e-9, "{half_power}");
        assert_eq!(pan_gains(-5 * PAN_ONE), pan_gains(-PAN_ONE));
    }

    #[test]
    fn sources_on_the_listeners_right_pan_right_whatever_the_yaw() {
        let attenuation = Attenuation::Linear {
            inner_radius: fx(1000),
            falloff_distance: fx(1000),
        };
        let facing_x = Listener {
            location: at(0, 0, 0),
            yaw: Fx::ZERO,
        };
        assert_eq!(
            spatial_gains(&attenuation, Some(&facing_x), Some(at(0, 500, 0))),
            [0, UNITY]
        );
        assert_eq!(
            spatial_gains(&attenuation, Some(&facing_x), Some(at(0, -500, 0))),
            [UNITY, 0]
        );
        let [left, right] = spatial_gains(&attenuation, Some(&facing_x), Some(at(500, 0, 0)));
        assert_eq!(left, right);
        let facing_y = Listener {
            location: at(0, 0, 0),
            yaw: fx(90),
        };
        assert_eq!(
            spatial_gains(&attenuation, Some(&facing_y), Some(at(-500, 0, 0))),
            [0, UNITY]
        );
        let [left, right] = spatial_gains(&attenuation, Some(&facing_x), Some(at(0, 0, 0)));
        assert_eq!(left, right);
        assert_eq!(spatial_gains(&attenuation, None, Some(at(0, 0, 0))), [0, 0]);
        assert_eq!(
            spatial_gains(&Attenuation::None, None, None),
            [UNITY, UNITY]
        );
    }

    #[test]
    fn pcm_interpolates_linearly_in_integers() {
        let samples = [0, 1000, -1000];
        assert_eq!(pcm_at(&samples, 0, 0, false), 0);
        assert_eq!(pcm_at(&samples, 0, 1 << 31, false), 500);
        assert_eq!(pcm_at(&samples, 0, 1 << 30, false), 250);
        assert_eq!(pcm_at(&samples, 1, 1 << 31, false), 0);
        assert_eq!(pcm_at(&samples, 2, 1 << 31, false), -1000);
        assert_eq!(pcm_at(&samples, 2, 1 << 31, true), -500);
        assert_eq!(pcm_at(&samples, 3, 0, false), 0);
    }

    #[test]
    fn pcm_at_half_pitch_plays_twice_as_long_through_midpoints() {
        let samples: Arc<[i32]> = Arc::from(vec![0, 1000, 2000, 3000]);
        let mut mixer = mixer(
            vec![def(
                Source::Pcm {
                    samples,
                    rate: 48_000,
                },
                0,
            )],
            MixGraph::default(),
        );
        let mut world = world();
        let entity = world.spawn();
        world
            .insert(
                entity,
                PlaySound {
                    location: Location::default(),
                    sound: SoundId(0),
                    gain: Fx::ONE,
                    pitch: Fx::from_ratio(1, 2),
                    seed: 0,
                    take: 0,
                },
            )
            .unwrap();
        let block = mixer.render(&world);
        let left: Vec<i32> = block.frames.iter().take(9).map(|f| f[0]).collect();
        assert_eq!(left, [0, 500, 1000, 1500, 2000, 2500, 3000, 3000, 0]);
        assert_eq!(mixer.voices(), 0);
    }

    #[test]
    fn a_play_reads_the_recording_of_its_own_take() {
        let takes: Vec<Arc<[i32]>> = vec![vec![1_000; 400].into(), vec![-2_000; 400].into()];
        let sound = def(
            Source::Takes {
                takes: takes.into(),
                rate: 48_000,
                weights: Vec::new().into(),
                pick: crate::bank::Pick::default(),
            },
            0,
        );
        let mut mixer = mixer(vec![sound], MixGraph::default());
        let mut world = world();
        let entity = world.spawn();
        world
            .insert(
                entity,
                PlaySound {
                    location: Location::default(),
                    sound: SoundId(0),
                    gain: Fx::ONE,
                    pitch: Fx::ONE,
                    seed: 0,
                    take: 1,
                },
            )
            .unwrap();
        let block = mixer.render(&world);
        assert!(
            block.frames[..400]
                .iter()
                .all(|frame| frame[0] < 0 && frame[1] < 0)
        );
        assert!(block.frames[400..].iter().all(|frame| *frame == [0, 0]));
    }

    #[test]
    fn the_ceiling_clamps_and_counts_clipped_samples() {
        let square = def(Source::Square { hz: fx(100) }, 800);
        let mut mixer = mixer(vec![square], MixGraph::default());
        let mut world = world();
        play(&mut world, 0, Fx::ONE);
        play(&mut world, 0, Fx::ONE);
        let block = mixer.render(&world);
        assert!(block.frames.iter().all(|f| f[0].abs() == MAX_SAMPLE));
        assert_eq!(block.summary.clipped, 1600);
        assert_eq!(block.summary.peak, [MAX_SAMPLE as u32; 2]);
        let mut quiet = mixer_with_one_square();
        let mut world = self::world();
        play(&mut world, 0, Fx::ONE);
        assert_eq!(quiet.render(&world).summary.clipped, 0);
    }

    fn mixer_with_one_square() -> Mixer {
        mixer(
            vec![def(Source::Square { hz: fx(100) }, 800)],
            MixGraph::default(),
        )
    }

    #[test]
    fn starts_past_the_voice_cap_are_dropped_and_counted() {
        let mut mixer = mixer(
            vec![def(Source::Sine { hz: fx(440) }, 100_000)],
            MixGraph::default(),
        );
        let mut world = world();
        for _ in 0..MAX_VOICES + 6 {
            play(&mut world, 0, Fx::from_ratio(1, 1000));
        }
        let block = mixer.render(&world);
        assert_eq!(block.summary.voices, MAX_VOICES as u32);
        assert_eq!(block.summary.dropped, 6);
        let again = mixer.render(&world);
        assert_eq!(again.summary.dropped, 0);
        assert_eq!(again.summary.voices, MAX_VOICES as u32);
    }

    #[test]
    fn a_play_starts_once_and_its_voice_outlives_the_entity() {
        let mut mixer = mixer(
            vec![def(Source::Sine { hz: fx(440) }, 2000)],
            MixGraph::default(),
        );
        let mut world = world();
        play(&mut world, 0, Fx::ONE);
        assert_eq!(mixer.render(&world).summary.voices, 1);
        assert_eq!(mixer.render(&world).summary.voices, 1);
        component::clear(&mut world);
        let third = mixer.render(&world);
        assert_eq!(third.summary.voices, 1);
        assert!(third.frames[..400].iter().any(|f| f[0] != 0));
        assert!(third.frames[400..].iter().all(|f| f[0] == 0));
        assert_eq!(mixer.render(&world).summary.voices, 0);
    }

    #[test]
    fn the_block_reports_starts_ends_and_drops() {
        let mut sound = def(Source::Sine { hz: fx(440) }, 1000);
        sound.attenuation = Attenuation::Linear {
            inner_radius: fx(100),
            falloff_distance: fx(400),
        };
        let mut mixer = mixer(vec![sound], MixGraph::default());
        let mut world = world();
        let listener = world.spawn();
        world
            .insert(
                listener,
                Listener {
                    location: at(0, 0, 0),
                    yaw: Fx::ZERO,
                },
            )
            .unwrap();
        let entity = world.spawn();
        world
            .insert(
                entity,
                PlaySound {
                    location: at(0, 300, 0),
                    sound: SoundId(0),
                    gain: Fx::from_ratio(1, 2),
                    pitch: Fx::ONE,
                    seed: 1,
                    take: 0,
                },
            )
            .unwrap();
        let first = mixer.render(&world);
        let [VoiceEvent::Start(start)] = first.events[..] else {
            panic!("expected one start, got {:?}", first.events);
        };
        assert_eq!(start.id, 0);
        assert_eq!(start.sound, SoundId(0));
        assert_eq!(start.origin, Origin::At(at(0, 300, 0)));
        assert_eq!(start.gain, Fx::from_ratio(1, 2));
        assert_eq!(start.distance, Some(300));
        assert_eq!(start.gains[0], 0);
        assert!((start.gains[1] - UNITY / 4).abs() <= 2);
        assert_eq!(start.submix, SubmixId::MASTER);
        let second = mixer.render(&world);
        assert_eq!(
            second.events,
            vec![VoiceEvent::End {
                id: 0,
                stopped: false
            }]
        );
        let mut full = mixer_with_one_square();
        let mut world = self::world();
        for _ in 0..MAX_VOICES + 1 {
            play(&mut world, 0, Fx::from_ratio(1, 1000));
        }
        let events = full.render(&world).events;
        let drops = events
            .iter()
            .filter(|event| matches!(event, VoiceEvent::Dropped { .. }))
            .count();
        let starts = events
            .iter()
            .filter(|event| matches!(event, VoiceEvent::Start(_)))
            .count();
        assert_eq!((starts, drops), (MAX_VOICES, 1));
    }

    #[test]
    fn submix_gains_multiply_down_to_master() {
        let graph = MixGraph::new(
            Fx::from_ratio(1, 2),
            &[SubmixDef {
                name: "sfx".to_string(),
                parent: "master".to_string(),
                gain: Fx::from_ratio(1, 2),
            }],
        )
        .unwrap();
        let mut sound = def(Source::Square { hz: fx(100) }, 800);
        sound.submix = graph.id("sfx").unwrap();
        let mut mixer = mixer(vec![sound], graph);
        let mut world = world();
        play(&mut world, 0, Fx::ONE);
        let block = mixer.render(&world);
        assert_eq!(block.frames[0], [MAX_SAMPLE / 4; 2]);
        assert_eq!(block.frames[479], [-MAX_SAMPLE / 4 - 1; 2]);
    }

    #[test]
    fn an_emitter_loops_follows_its_gain_and_releases_when_removed() {
        let mut sound = def(Source::Square { hz: fx(60) }, 0);
        sound.envelope.release = 400;
        let mut mixer = mixer(vec![sound], MixGraph::default());
        let mut world = world();
        let entity = world.spawn();
        let mut emitter = SoundEmitter {
            sound: SoundId(0),
            gain: Fx::ONE,
            pitch: Fx::ONE,
            seed: 0,
        };
        world.insert(entity, emitter).unwrap();
        for _ in 0..3 {
            assert_eq!(mixer.render(&world).summary.peak[0], MAX_SAMPLE as u32);
        }
        emitter.gain = Fx::from_ratio(1, 2);
        world.insert(entity, emitter).unwrap();
        assert_eq!(
            mixer.render(&world).summary.peak[0],
            (MAX_SAMPLE / 2 + 1) as u32
        );
        world.despawn(entity);
        let released = mixer.render(&world);
        assert_eq!(released.summary.voices, 1);
        assert!(released.frames[0][0] != 0 && released.frames[400][0] == 0);
        assert_eq!(mixer.voices(), 0);
    }

    #[test]
    fn noise_is_seeded_by_the_play() {
        let mut first = mixer(vec![def(Source::Noise, 800)], MixGraph::default());
        let mut second = mixer(vec![def(Source::Noise, 800)], MixGraph::default());
        let mut world = world();
        play(&mut world, 0, Fx::ONE);
        let a = first.render(&world);
        assert_eq!(a, second.render(&world));
        assert!(a.summary.peak[0] > 0);
        let mut reseeded = world.query_mut::<PlaySound>();
        reseeded[0].1.seed = 2;
        drop(reseeded);
        let mut third = mixer(vec![def(Source::Noise, 800)], MixGraph::default());
        assert_ne!(a.summary.hash, third.render(&world).summary.hash);
    }

    #[test]
    fn a_sound_feeding_a_missing_submix_is_rejected() {
        let mut sound = def(Source::Noise, 10);
        sound.submix = SubmixId(5);
        let mut bank = SoundBank::new();
        bank.add("lost", sound).unwrap();
        assert!(Mixer::new(AudioFormat::DEFAULT, Arc::new(bank), MixGraph::default()).is_err());
    }
}
