use std::sync::Arc;

use tickora_core::fx::Fx;
use tickora_core::rng::Rng;

use crate::format::MAX_SAMPLE;
use crate::graph::SubmixId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SoundId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    NoRepeats(u32),
    WithoutReplacement,
}

impl Default for Pick {
    fn default() -> Self {
        Pick::NoRepeats(1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Sine {
        hz: Fx,
    },
    Square {
        hz: Fx,
    },
    Noise,
    Pcm {
        samples: Arc<[i32]>,
        rate: u32,
    },
    Takes {
        takes: Arc<[Arc<[i32]>]>,
        rate: u32,
        weights: Arc<[u32]>,
        pick: Pick,
    },
}

impl Source {
    pub fn pcm(&self, take: u32) -> Option<(&[i32], u32)> {
        match self {
            Source::Pcm { samples, rate } => Some((samples, *rate)),
            Source::Takes { takes, rate, .. } => takes
                .get(take as usize)
                .map(|samples| (&samples[..], *rate)),
            _ => None,
        }
    }

    pub fn take_count(&self) -> u32 {
        match self {
            Source::Takes { takes, .. } => takes.len() as u32,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Envelope {
    pub attack: u32,
    pub release: u32,
    pub duration: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variation {
    pub gain: (Fx, Fx),
    pub pitch: (Fx, Fx),
}

impl Default for Variation {
    fn default() -> Self {
        Variation {
            gain: (Fx::ONE, Fx::ONE),
            pitch: (Fx::ONE, Fx::ONE),
        }
    }
}

impl Variation {
    pub fn draw(&self, rng: &mut Rng) -> (Fx, Fx, u64) {
        let gain = draw_between(rng, self.gain);
        let pitch = draw_between(rng, self.pitch);
        (gain, pitch, rng.next_u64())
    }
}

fn draw_between(rng: &mut Rng, (low, high): (Fx, Fx)) -> Fx {
    let fraction = i128::from(rng.next_u32());
    let span = i128::from(high.raw()) - i128::from(low.raw());
    let offset = ((span.max(0) * fraction) >> 32) as i64;
    Fx::from_raw(low.raw().saturating_add(offset))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attenuation {
    None,
    Linear {
        inner_radius: Fx,
        falloff_distance: Fx,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundDef {
    pub source: Source,
    pub envelope: Envelope,
    pub variation: Variation,
    pub attenuation: Attenuation,
    pub submix: SubmixId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SoundBank {
    names: Vec<String>,
    defs: Vec<SoundDef>,
}

impl SoundBank {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: &str, def: SoundDef) -> Result<SoundId, String> {
        if self.id(name).is_some() {
            return Err(format!("sound {name:?} is already in the bank"));
        }
        validate(name, &def)?;
        let id = SoundId(u32::try_from(self.defs.len()).map_err(|_| "sound bank is full")?);
        self.names.push(name.to_string());
        self.defs.push(def);
        Ok(id)
    }

    pub fn id(&self, name: &str) -> Option<SoundId> {
        self.names
            .iter()
            .position(|known| known == name)
            .map(|index| SoundId(index as u32))
    }

    pub fn get(&self, id: SoundId) -> Option<&SoundDef> {
        self.defs.get(id.0 as usize)
    }

    pub fn name(&self, id: SoundId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn defs(&self) -> impl Iterator<Item = &SoundDef> {
        self.defs.iter()
    }
}

fn validate(name: &str, def: &SoundDef) -> Result<(), String> {
    let (gain, pitch) = (def.variation.gain, def.variation.pitch);
    if gain.0 > gain.1 || pitch.0 > pitch.1 {
        return Err(format!("sound {name:?}: a variation range runs backwards"));
    }
    if gain.0 < Fx::ZERO || pitch.0 < Fx::ZERO {
        return Err(format!(
            "sound {name:?}: gain and pitch must not be negative"
        ));
    }
    if let Attenuation::Linear {
        inner_radius,
        falloff_distance,
    } = def.attenuation
        && (inner_radius < Fx::ZERO || falloff_distance < Fx::ZERO)
    {
        return Err(format!(
            "sound {name:?}: attenuation distances must not be negative"
        ));
    }
    match &def.source {
        Source::Sine { hz } | Source::Square { hz } if *hz < Fx::ZERO => {
            Err(format!("sound {name:?}: frequency must not be negative"))
        }
        Source::Pcm { samples, rate } => validate_pcm(name, samples, *rate),
        Source::Takes { takes, rate, .. } => {
            if takes.is_empty() {
                return Err(format!("sound {name:?}: takes need at least one take"));
            }
            takes
                .iter()
                .try_for_each(|samples| validate_pcm(name, samples, *rate))
        }
        _ => Ok(()),
    }
}

fn validate_pcm(name: &str, samples: &[i32], rate: u32) -> Result<(), String> {
    if rate == 0 || samples.is_empty() {
        return Err(format!(
            "sound {name:?}: PCM needs samples and a rate above 0"
        ));
    }
    if samples.iter().any(|sample| sample.abs() > MAX_SAMPLE) {
        return Err(format!("sound {name:?}: PCM samples exceed 24 bits"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone() -> SoundDef {
        SoundDef {
            source: Source::Sine {
                hz: Fx::from_int(440),
            },
            envelope: Envelope {
                attack: 0,
                release: 0,
                duration: 100,
            },
            variation: Variation::default(),
            attenuation: Attenuation::None,
            submix: SubmixId::MASTER,
        }
    }

    #[test]
    fn names_map_to_ids_in_insertion_order() {
        let mut bank = SoundBank::new();
        assert_eq!(bank.add("a", tone()), Ok(SoundId(0)));
        assert_eq!(bank.add("b", tone()), Ok(SoundId(1)));
        assert!(bank.add("a", tone()).is_err());
        assert_eq!(bank.id("b"), Some(SoundId(1)));
        assert_eq!(bank.name(SoundId(0)), Some("a"));
        assert_eq!(bank.get(SoundId(2)), None);
    }

    #[test]
    fn bad_definitions_are_rejected() {
        let mut bank = SoundBank::new();
        let mut def = tone();
        def.variation.gain = (Fx::ONE, Fx::ZERO);
        assert!(bank.add("backwards", def).is_err());
        let mut def = tone();
        def.source = Source::Pcm {
            samples: Arc::from(vec![1 << 23]),
            rate: 48_000,
        };
        assert!(bank.add("loud", def).is_err());
        let mut def = tone();
        def.attenuation = Attenuation::Linear {
            inner_radius: -Fx::ONE,
            falloff_distance: Fx::ONE,
        };
        assert!(bank.add("inside out", def).is_err());
    }

    #[test]
    fn variation_draws_stay_in_range_and_a_fixed_range_still_draws() {
        let variation = Variation {
            gain: (Fx::from_ratio(1, 2), Fx::ONE),
            pitch: (Fx::from_ratio(4, 5), Fx::from_ratio(6, 5)),
        };
        let mut rng = Rng::from_seed(3);
        for _ in 0..1000 {
            let (gain, pitch, _) = variation.draw(&mut rng);
            assert!(gain >= Fx::from_ratio(1, 2) && gain < Fx::ONE);
            assert!(pitch >= Fx::from_ratio(4, 5) && pitch < Fx::from_ratio(6, 5));
        }
        let mut a = Rng::from_seed(3);
        let mut b = Rng::from_seed(3);
        Variation::default().draw(&mut a);
        variation.draw(&mut b);
        assert_eq!(a, b);
    }
}
