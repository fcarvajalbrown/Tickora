use tickora_core::ecs::{Component, Entity, FieldValue, Fields, World};
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::noise::Location;
use tickora_core::rng::Rng;

use crate::bank::{SoundBank, SoundId, Source};
use crate::take::TakeHistory;

pub const AUDIO_STREAM: u64 = 0x0061_7564_696f;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Listener {
    pub location: Location,
    pub yaw: Fx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaySound {
    pub location: Location,
    pub sound: SoundId,
    pub gain: Fx,
    pub pitch: Fx,
    pub seed: u64,
    pub take: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundEmitter {
    pub sound: SoundId,
    pub gain: Fx,
    pub pitch: Fx,
    pub seed: u64,
}

impl Component for Listener {
    const NAME: &'static str = "listener";

    fn hash_into(&self, hasher: &mut StateHasher) {
        Component::hash_into(&self.location, hasher);
        self.yaw.hash_into(hasher);
    }

    fn fields(&self) -> Fields {
        let mut fields = Component::fields(&self.location);
        fields.push(("yaw", FieldValue::Fx(self.yaw)));
        fields
    }
}

impl Component for PlaySound {
    const NAME: &'static str = "play_sound";

    fn hash_into(&self, hasher: &mut StateHasher) {
        Component::hash_into(&self.location, hasher);
        hash_voice(self.sound, self.gain, self.pitch, self.seed, hasher);
        hasher.write_u32(self.take);
    }

    fn fields(&self) -> Fields {
        let mut fields = Component::fields(&self.location);
        fields.extend(voice_fields(self.sound, self.gain, self.pitch, self.seed));
        fields.push(("take", FieldValue::Int(i64::from(self.take))));
        fields
    }
}

impl Component for SoundEmitter {
    const NAME: &'static str = "sound_emitter";

    fn hash_into(&self, hasher: &mut StateHasher) {
        hash_voice(self.sound, self.gain, self.pitch, self.seed, hasher);
    }

    fn fields(&self) -> Fields {
        voice_fields(self.sound, self.gain, self.pitch, self.seed)
    }
}

fn hash_voice(sound: SoundId, gain: Fx, pitch: Fx, seed: u64, hasher: &mut StateHasher) {
    hasher.write_u32(sound.0);
    gain.hash_into(hasher);
    pitch.hash_into(hasher);
    hasher.write_u64(seed);
}

fn voice_fields(sound: SoundId, gain: Fx, pitch: Fx, seed: u64) -> Fields {
    vec![
        ("sound", FieldValue::Int(i64::from(sound.0))),
        ("gain", FieldValue::Fx(gain)),
        ("pitch", FieldValue::Fx(pitch)),
        ("seed", FieldValue::Text(seed.to_string())),
    ]
}

pub fn register(world: &mut World) {
    world.register::<Listener>();
    world.register::<PlaySound>();
    world.register::<SoundEmitter>();
}

pub fn audio_rng(seed: u64) -> Rng {
    Rng::new(seed, AUDIO_STREAM)
}

pub fn play(
    world: &mut World,
    bank: &SoundBank,
    rng: &mut Rng,
    takes: &mut TakeHistory,
    sound: SoundId,
    location: Location,
) -> Result<Entity, String> {
    let def = bank
        .get(sound)
        .ok_or_else(|| format!("sound {} is not in the bank", sound.0))?;
    let (gain, pitch, seed) = def.variation.draw(rng);
    let take = match &def.source {
        Source::Takes {
            takes: recordings,
            weights,
            pick,
            ..
        } => takes.pick(rng, sound, recordings.len() as u32, weights, *pick),
        _ => 0,
    };
    let entity = world.spawn();
    world
        .insert(
            entity,
            PlaySound {
                location,
                sound,
                gain,
                pitch,
                seed,
                take,
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(entity)
}

pub fn emitter(bank: &SoundBank, rng: &mut Rng, sound: SoundId) -> Result<SoundEmitter, String> {
    let def = bank
        .get(sound)
        .ok_or_else(|| format!("sound {} is not in the bank", sound.0))?;
    if let Source::Takes { .. } = def.source {
        return Err(format!(
            "sound {} has takes; looping emitters play one PCM or oscillator source",
            sound.0
        ));
    }
    let (gain, pitch, seed) = def.variation.draw(rng);
    Ok(SoundEmitter {
        sound,
        gain,
        pitch,
        seed,
    })
}

pub fn clear(world: &mut World) {
    let plays: Vec<Entity> = world
        .query::<PlaySound>()
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    for entity in plays {
        world.despawn(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank::{Attenuation, Envelope, SoundDef, Source, Variation};
    use crate::graph::SubmixId;

    fn bank() -> SoundBank {
        let mut bank = SoundBank::new();
        bank.add(
            "hit",
            SoundDef {
                source: Source::Noise,
                envelope: Envelope {
                    attack: 0,
                    release: 0,
                    duration: 10,
                },
                variation: Variation {
                    gain: (Fx::from_ratio(1, 2), Fx::ONE),
                    pitch: (Fx::ONE, Fx::from_int(2)),
                },
                attenuation: Attenuation::None,
                submix: SubmixId::MASTER,
            },
        )
        .unwrap();
        bank
    }

    fn world() -> World {
        let mut world = World::new();
        register(&mut world);
        world
    }

    #[test]
    fn play_draws_variation_into_hashed_state_and_clear_removes_only_plays() {
        let bank = bank();
        let mut world = world();
        let listener = world.spawn();
        world.insert(listener, Listener::default()).unwrap();
        let mut rng = audio_rng(1);
        let mut takes = TakeHistory::new();
        let here = Location::default();
        let a = play(&mut world, &bank, &mut rng, &mut takes, SoundId(0), here).unwrap();
        let b = play(&mut world, &bank, &mut rng, &mut takes, SoundId(0), here).unwrap();
        let (first, second) = (
            *world.get::<PlaySound>(a).unwrap(),
            *world.get::<PlaySound>(b).unwrap(),
        );
        assert_ne!(first.seed, second.seed);
        assert!(first.gain >= Fx::from_ratio(1, 2) && first.gain < Fx::ONE);
        assert!(play(&mut world, &bank, &mut rng, &mut takes, SoundId(9), here).is_err());
        assert_eq!(takes, TakeHistory::new());
        clear(&mut world);
        assert_eq!(world.entity_count(), 1);
        assert!(world.is_alive(listener));
    }

    fn takes_bank() -> SoundBank {
        let mut bank = SoundBank::new();
        let take = |value: i32| -> std::sync::Arc<[i32]> { vec![value; 4].into() };
        bank.add(
            "hit",
            SoundDef {
                source: Source::Takes {
                    takes: vec![take(1), take(2), take(3)].into(),
                    rate: 48_000,
                    weights: Vec::new().into(),
                    pick: crate::bank::Pick::NoRepeats(1),
                },
                envelope: Envelope::default(),
                variation: Variation::default(),
                attenuation: Attenuation::None,
                submix: SubmixId::MASTER,
            },
        )
        .unwrap();
        bank
    }

    #[test]
    fn a_play_with_takes_records_a_take_that_never_repeats_in_a_row() {
        let bank = takes_bank();
        let mut world = world();
        let mut rng = audio_rng(4);
        let mut takes = TakeHistory::new();
        let mut last = None;
        for _ in 0..200 {
            let entity = play(
                &mut world,
                &bank,
                &mut rng,
                &mut takes,
                SoundId(0),
                Location::default(),
            )
            .unwrap();
            let take = world.get::<PlaySound>(entity).unwrap().take;
            assert!(take < 3 && Some(take) != last);
            last = Some(take);
            clear(&mut world);
        }
        assert_eq!(takes.recent(SoundId(0)), [last.unwrap()]);
    }

    #[test]
    fn an_emitter_refuses_a_sound_with_takes() {
        assert!(emitter(&takes_bank(), &mut audio_rng(1), SoundId(0)).is_err());
    }

    #[test]
    fn the_audio_stream_is_not_the_gameplay_stream() {
        assert_ne!(audio_rng(7), Rng::from_seed(7));
    }

    #[test]
    fn fields_show_the_seed_as_a_decimal_string() {
        let emitter = SoundEmitter {
            sound: SoundId(2),
            gain: Fx::ONE,
            pitch: Fx::ONE,
            seed: u64::MAX,
        };
        let fields = emitter.fields();
        assert_eq!(fields[0], ("sound", FieldValue::Int(2)));
        assert_eq!(fields[3], ("seed", FieldValue::Text(u64::MAX.to_string())));
    }
}
