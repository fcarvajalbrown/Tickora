use tickora_core::hash::StateHasher;
use tickora_core::rng::Rng;

use crate::bank::{Pick, SoundId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TakeHistory {
    sounds: Vec<(SoundId, Vec<u32>)>,
}

impl TakeHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn recent(&self, sound: SoundId) -> &[u32] {
        self.sounds
            .binary_search_by_key(&sound, |(id, _)| *id)
            .map_or(&[], |at| &self.sounds[at].1)
    }

    pub fn pick(
        &mut self,
        rng: &mut Rng,
        sound: SoundId,
        count: u32,
        weights: &[u32],
        pick: Pick,
    ) -> u32 {
        if count <= 1 {
            return 0;
        }
        let at = match self.sounds.binary_search_by_key(&sound, |(id, _)| *id) {
            Ok(at) => at,
            Err(at) => {
                self.sounds.insert(at, (sound, Vec::new()));
                at
            }
        };
        let recent = &mut self.sounds[at].1;
        let excluded = match pick {
            Pick::NoRepeats(n) => {
                let n = n.min(count - 1) as usize;
                recent[recent.len().saturating_sub(n)..].to_vec()
            }
            Pick::WithoutReplacement => {
                if recent.len() as u32 >= count {
                    let last = recent.last().copied();
                    recent.clear();
                    last.into_iter().collect()
                } else {
                    recent.clone()
                }
            }
        };
        let candidates: Vec<u32> = (0..count).filter(|take| !excluded.contains(take)).collect();
        let chosen = weighted(rng, &candidates, weights);
        recent.push(chosen);
        if let Pick::NoRepeats(n) = pick {
            let keep = n.min(count - 1) as usize;
            let drop = recent.len().saturating_sub(keep);
            recent.drain(..drop);
        }
        chosen
    }

    pub fn hash_into(&self, hasher: &mut StateHasher) {
        hasher.write_u64(self.sounds.len() as u64);
        for (sound, recent) in &self.sounds {
            hasher.write_u32(sound.0);
            hasher.write_u64(recent.len() as u64);
            for take in recent {
                hasher.write_u32(*take);
            }
        }
    }
}

fn weight(weights: &[u32], take: u32) -> u64 {
    if weights.is_empty() {
        1
    } else {
        u64::from(weights[take as usize % weights.len()])
    }
}

fn weighted(rng: &mut Rng, candidates: &[u32], weights: &[u32]) -> u32 {
    let mut total: u64 = candidates.iter().map(|&take| weight(weights, take)).sum();
    let even = total == 0;
    if even {
        total = candidates.len() as u64;
    }
    let mut target = (u64::from(rng.next_u32()) * total) >> 32;
    for &take in candidates {
        let share = if even { 1 } else { weight(weights, take) };
        if target < share {
            return take;
        }
        target -= share;
    }
    candidates[candidates.len() - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIT: SoundId = SoundId(3);

    fn picks(pick: Pick, count: u32, weights: &[u32], n: usize) -> Vec<u32> {
        let mut history = TakeHistory::new();
        let mut rng = Rng::from_seed(11);
        (0..n)
            .map(|_| history.pick(&mut rng, HIT, count, weights, pick))
            .collect()
    }

    #[test]
    fn no_repeats_one_never_plays_a_take_twice_in_a_row() {
        let run = picks(Pick::NoRepeats(1), 3, &[], 10_000);
        assert!(run.windows(2).all(|pair| pair[0] != pair[1]));
        for take in 0..3 {
            assert!(run.iter().filter(|&&t| t == take).count() > 3_000);
        }
    }

    #[test]
    fn no_repeats_two_keeps_the_last_two_out() {
        let run = picks(Pick::NoRepeats(2), 4, &[], 5_000);
        assert!(
            run.windows(3)
                .all(|w| w[0] != w[1] && w[1] != w[2] && w[0] != w[2])
        );
    }

    #[test]
    fn no_repeats_past_the_take_count_is_clamped_not_stuck() {
        let run = picks(Pick::NoRepeats(9), 3, &[], 30);
        assert!(
            run.windows(3)
                .all(|w| w[0] != w[1] && w[1] != w[2] && w[0] != w[2])
        );
    }

    #[test]
    fn without_replacement_plays_every_take_once_per_round_and_never_twice_in_a_row() {
        let run = picks(Pick::WithoutReplacement, 4, &[], 4_000);
        for round in run.chunks(4) {
            let mut sorted = round.to_vec();
            sorted.sort();
            assert_eq!(sorted, [0, 1, 2, 3]);
        }
        assert!(run.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn weights_are_respected_and_repeat_when_shorter_than_the_takes() {
        let run = picks(Pick::NoRepeats(0), 4, &[3, 1], 40_000);
        let count = |take| run.iter().filter(|&&t| t == take).count();
        assert!(count(0) > 3 * count(1) * 9 / 10 && count(0) < 3 * count(1) * 11 / 10);
        assert!(count(2) > 3 * count(3) * 9 / 10 && count(2) < 3 * count(3) * 11 / 10);
        let zero = picks(Pick::NoRepeats(0), 3, &[0, 5, 0], 100);
        assert!(zero.iter().all(|&take| take == 1));
    }

    #[test]
    fn all_zero_weights_fall_back_to_an_even_pick() {
        let run = picks(Pick::NoRepeats(1), 3, &[0], 3_000);
        assert!((0..3).all(|take| run.contains(&take)));
    }

    #[test]
    fn a_seed_gives_the_same_picks_and_history_hash() {
        let mut a = TakeHistory::new();
        let mut b = TakeHistory::new();
        let (mut ra, mut rb) = (Rng::from_seed(5), Rng::from_seed(5));
        for _ in 0..50 {
            assert_eq!(
                a.pick(&mut ra, HIT, 5, &[], Pick::NoRepeats(2)),
                b.pick(&mut rb, HIT, 5, &[], Pick::NoRepeats(2))
            );
        }
        let (mut ha, mut hb) = (StateHasher::new(), StateHasher::new());
        a.hash_into(&mut ha);
        b.hash_into(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
        assert_eq!(a.recent(HIT).len(), 2);
    }

    #[test]
    fn one_take_draws_nothing() {
        let mut history = TakeHistory::new();
        let mut rng = Rng::from_seed(1);
        let before = rng.clone();
        assert_eq!(history.pick(&mut rng, HIT, 1, &[], Pick::default()), 0);
        assert_eq!(rng, before);
        assert_eq!(history, TakeHistory::new());
    }
}
