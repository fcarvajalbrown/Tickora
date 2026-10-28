use tickora_core::ecs::{Component, Entity, FieldValue, Fields, World};
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::noise::Location;
use tickora_core::rng::Rng;

use crate::bank::{SoundBank, SoundId};

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
    }

    fn fields(&self) -> Fields {
        let mut fields = Component::fields(&self.location);
        fields.extend(voice_fields(self.sound, self.gain, self.pitch, self.seed));
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
    sound: SoundId,
    location: Location,
) -> Result<Entity, String> {
    let def = bank
        .get(sound)
        .ok_or_else(|| format!("sound {} is not in the bank", sound.0))?;
    let (gain, pitch, seed) = def.variation.draw(rng);
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
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(entity)
}

pub fn emitter(bank: &SoundBank, rng: &mut Rng, sound: SoundId) -> Result<SoundEmitter, String> {
    let def = bank
        .get(sound)
        .ok_or_else(|| format!("sound {} is not in the bank", sound.0))?;
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
        let here = Location::default();
        let a = play(&mut world, &bank, &mut rng, SoundId(0), here).unwrap();
        let b = play(&mut world, &bank, &mut rng, SoundId(0), here).unwrap();
        let (first, second) = (
            *world.get::<PlaySound>(a).unwrap(),
            *world.get::<PlaySound>(b).unwrap(),
        );
        assert_ne!(first.seed, second.seed);
        assert!(first.gain >= Fx::from_ratio(1, 2) && first.gain < Fx::ONE);
        assert!(play(&mut world, &bank, &mut rng, SoundId(9), here).is_err());
        clear(&mut world);
        assert_eq!(world.entity_count(), 1);
        assert!(world.is_alive(listener));
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
