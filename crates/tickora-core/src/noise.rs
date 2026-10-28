use crate::ecs::{Component, Entity, FieldValue, Fields, World};
use crate::fx::Fx;
use crate::hash::StateHasher;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Location {
    pub x: Fx,
    pub y: Fx,
    pub z: Fx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Noise {
    pub location: Location,
    pub loudness: Fx,
    pub max_range: Fx,
    pub instigator: Option<Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hearing {
    pub range: Fx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeardNoise {
    pub location: Location,
    pub loudness: Fx,
    pub instigator: Option<Entity>,
    pub tick: u64,
}

impl Location {
    pub const fn new(x: Fx, y: Fx, z: Fx) -> Location {
        Location { x, y, z }
    }

    fn hash_into(&self, hasher: &mut StateHasher) {
        self.x.hash_into(hasher);
        self.y.hash_into(hasher);
        self.z.hash_into(hasher);
    }

    fn fields(&self) -> Fields {
        vec![
            ("x", FieldValue::Fx(self.x)),
            ("y", FieldValue::Fx(self.y)),
            ("z", FieldValue::Fx(self.z)),
        ]
    }
}

impl Noise {
    pub const fn new(location: Location, loudness: Fx) -> Noise {
        Noise {
            location,
            loudness,
            max_range: Fx::ZERO,
            instigator: None,
        }
    }
}

impl Component for Location {
    const NAME: &'static str = "location";

    fn hash_into(&self, hasher: &mut StateHasher) {
        Location::hash_into(self, hasher);
    }

    fn fields(&self) -> Fields {
        Location::fields(self)
    }
}

impl Component for Noise {
    const NAME: &'static str = "noise";

    fn hash_into(&self, hasher: &mut StateHasher) {
        self.location.hash_into(hasher);
        self.loudness.hash_into(hasher);
        self.max_range.hash_into(hasher);
        hash_instigator(self.instigator, hasher);
    }

    fn fields(&self) -> Fields {
        let mut fields = self.location.fields();
        fields.push(("loudness", FieldValue::Fx(self.loudness)));
        fields.push(("max_range", FieldValue::Fx(self.max_range)));
        fields.extend(instigator_fields(self.instigator));
        fields
    }
}

impl Component for Hearing {
    const NAME: &'static str = "hearing";

    fn hash_into(&self, hasher: &mut StateHasher) {
        self.range.hash_into(hasher);
    }

    fn fields(&self) -> Fields {
        vec![("range", FieldValue::Fx(self.range))]
    }
}

impl Component for HeardNoise {
    const NAME: &'static str = "heard_noise";

    fn hash_into(&self, hasher: &mut StateHasher) {
        self.location.hash_into(hasher);
        self.loudness.hash_into(hasher);
        hash_instigator(self.instigator, hasher);
        hasher.write_u64(self.tick);
    }

    fn fields(&self) -> Fields {
        let mut fields = self.location.fields();
        fields.push(("loudness", FieldValue::Fx(self.loudness)));
        fields.extend(instigator_fields(self.instigator));
        fields.push(("tick", FieldValue::Int(self.tick as i64)));
        fields
    }
}

fn hash_instigator(instigator: Option<Entity>, hasher: &mut StateHasher) {
    match instigator {
        Some(entity) => {
            hasher.write_bool(true);
            hasher.write_u32(entity.index());
            hasher.write_u32(entity.generation());
        }
        None => hasher.write_bool(false),
    }
}

fn instigator_fields(instigator: Option<Entity>) -> Fields {
    instigator
        .map(|entity| {
            vec![
                (
                    "instigator_index",
                    FieldValue::Int(i64::from(entity.index())),
                ),
                (
                    "instigator_generation",
                    FieldValue::Int(i64::from(entity.generation())),
                ),
            ]
        })
        .unwrap_or_default()
}

pub fn register(world: &mut World) {
    world.register::<Location>();
    world.register::<Noise>();
    world.register::<Hearing>();
    world.register::<HeardNoise>();
}

pub fn emit(world: &mut World, noise: Noise) -> Entity {
    let entity = world.spawn();
    world
        .insert(entity, noise)
        .expect("noise::register was called on this world");
    entity
}

pub fn clear(world: &mut World) {
    let noises: Vec<Entity> = world.query::<Noise>().into_iter().map(|(e, _)| e).collect();
    for entity in noises {
        world.despawn(entity);
    }
}

pub fn hears(listener: &Location, hearing: &Hearing, noise: &Noise) -> bool {
    if noise.loudness <= Fx::ZERO || hearing.range <= Fx::ZERO {
        return false;
    }
    let distance_sq = distance_squared_shifted(listener, &noise.location);
    let within = |range: Fx| distance_sq <= limit_squared(range, noise.loudness);
    within(hearing.range) && (noise.max_range <= Fx::ZERO || within(noise.max_range))
}

pub fn hear(world: &mut World, tick: u64) {
    let noises: Vec<(Entity, Noise)> = world
        .query::<Noise>()
        .into_iter()
        .map(|(e, noise)| (e, *noise))
        .collect();
    let listeners: Vec<(Entity, Location, Hearing)> = world
        .query2::<Location, Hearing>()
        .into_iter()
        .map(|(e, location, hearing)| (e, *location, *hearing))
        .collect();
    for (listener, location, hearing) in listeners {
        let loudest = noises
            .iter()
            .filter(|(_, noise)| hears(&location, &hearing, noise))
            .min_by_key(|(entity, noise)| (core::cmp::Reverse(noise.loudness), entity.index()));
        if let Some((_, noise)) = loudest {
            let heard = HeardNoise {
                location: noise.location,
                loudness: noise.loudness,
                instigator: noise.instigator,
                tick,
            };
            world
                .insert(listener, heard)
                .expect("noise::register was called on this world");
        }
    }
}

type U256 = (u128, u128);

fn distance_squared_shifted(a: &Location, b: &Location) -> U256 {
    let axis = |p: Fx, q: Fx| {
        let delta = (i128::from(p.raw()) - i128::from(q.raw())).unsigned_abs();
        (0, delta * delta)
    };
    let sum = add(add(axis(a.x, b.x), axis(a.y, b.y)), axis(a.z, b.z));
    (sum.0 << 64 | sum.1 >> 64, sum.1 << 64)
}

fn limit_squared(range: Fx, loudness: Fx) -> U256 {
    let product = range.raw() as u128 * loudness.raw() as u128;
    multiply(product, product)
}

fn add(a: U256, b: U256) -> U256 {
    let (low, carry) = a.1.overflowing_add(b.1);
    (a.0 + b.0 + u128::from(carry), low)
}

fn multiply(a: u128, b: u128) -> U256 {
    let mask = u128::from(u64::MAX);
    let (a_hi, a_lo) = (a >> 64, a & mask);
    let (b_hi, b_lo) = (b >> 64, b & mask);
    let low = a_lo * b_lo;
    let cross_a = a_hi * b_lo;
    let cross_b = a_lo * b_hi;
    let high = a_hi * b_hi;
    let (middle, middle_carry) = cross_a.overflowing_add(cross_b);
    let (low_sum, low_carry) = low.overflowing_add(middle << 64);
    let high_sum = high + (middle >> 64) + (u128::from(middle_carry) << 64) + u128::from(low_carry);
    (high_sum, low_sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i32, y: i32, z: i32) -> Location {
        Location::new(Fx::from_int(x), Fx::from_int(y), Fx::from_int(z))
    }

    fn world() -> World {
        let mut world = World::new();
        register(&mut world);
        world
    }

    fn listener(world: &mut World, location: Location, range: i32) -> Entity {
        let entity = world.spawn();
        world.insert(entity, location).unwrap();
        world
            .insert(
                entity,
                Hearing {
                    range: Fx::from_int(range),
                },
            )
            .unwrap();
        entity
    }

    #[test]
    fn heard_exactly_at_the_limit_and_not_one_raw_unit_past() {
        let hearing = Hearing {
            range: Fx::from_int(1000),
        };
        let half = Fx::from_ratio(1, 2);
        let edge = Noise::new(at(500, 0, 0), half);
        assert!(hears(&at(0, 0, 0), &hearing, &edge));
        let past = Noise::new(
            Location::new(Fx::from_int(500) + Fx::EPSILON, Fx::ZERO, Fx::ZERO),
            half,
        );
        assert!(!hears(&at(0, 0, 0), &hearing, &past));
    }

    #[test]
    fn distance_counts_every_axis() {
        let hearing = Hearing {
            range: Fx::from_int(13),
        };
        assert!(hears(
            &at(0, 0, 0),
            &hearing,
            &Noise::new(at(3, 4, 12), Fx::ONE)
        ));
        assert!(!hears(
            &at(0, 0, 0),
            &hearing,
            &Noise::new(at(3, 4, 13), Fx::ONE)
        ));
        assert!(hears(
            &at(-3, -4, -12),
            &hearing,
            &Noise::new(at(0, 0, 0), Fx::ONE)
        ));
    }

    #[test]
    fn max_range_is_scaled_by_loudness_and_both_limits_apply() {
        let hearing = Hearing {
            range: Fx::from_int(5000),
        };
        let mut noise = Noise::new(at(250, 0, 0), Fx::from_ratio(1, 4));
        noise.max_range = Fx::from_int(1000);
        assert!(hears(&at(0, 0, 0), &hearing, &noise));
        noise.location = at(251, 0, 0);
        assert!(!hears(&at(0, 0, 0), &hearing, &noise));
        noise.max_range = Fx::ZERO;
        assert!(hears(&at(0, 0, 0), &hearing, &noise));
        noise.max_range = Fx::from_int(-1);
        assert!(hears(&at(0, 0, 0), &hearing, &noise));
        let deaf = Hearing {
            range: Fx::from_int(100),
        };
        noise.max_range = Fx::from_int(100_000);
        assert!(!hears(&at(0, 0, 0), &deaf, &noise));
    }

    #[test]
    fn no_loudness_or_no_range_is_never_heard() {
        let hearing = Hearing {
            range: Fx::from_int(100),
        };
        let here = at(0, 0, 0);
        assert!(!hears(&here, &hearing, &Noise::new(here, Fx::ZERO)));
        assert!(!hears(&here, &hearing, &Noise::new(here, -Fx::ONE)));
        let none = Hearing { range: Fx::ZERO };
        assert!(!hears(&here, &none, &Noise::new(here, Fx::ONE)));
    }

    #[test]
    fn far_noises_do_not_saturate_into_near_ones() {
        let hearing = Hearing {
            range: Fx::from_int(100_000),
        };
        let near = at(0, 0, 0);
        let far = Noise::new(at(i32::MAX, i32::MAX, i32::MAX), Fx::ONE);
        assert!(!hears(&near, &hearing, &far));
        let opposite = Location::new(Fx::MIN, Fx::MIN, Fx::MIN);
        let corner = Location::new(Fx::MAX, Fx::MAX, Fx::MAX);
        let huge = Hearing { range: Fx::MAX };
        assert!(!hears(
            &opposite,
            &huge,
            &Noise::new(corner, Fx::from_int(1))
        ));
        assert!(!hears(
            &opposite,
            &huge,
            &Noise::new(corner, Fx::from_int(2))
        ));
        assert!(hears(
            &opposite,
            &huge,
            &Noise::new(corner, Fx::from_int(4))
        ));
        let at_1km = Noise::new(at(100_000, 0, 0), Fx::ONE);
        assert!(hears(&near, &hearing, &at_1km));
        assert_eq!(
            Fx::from_int(100_000).saturating_mul(Fx::from_int(100_000)),
            Fx::MAX
        );
    }

    #[test]
    fn wide_multiply_matches_known_products() {
        assert_eq!(multiply(u128::MAX, u128::MAX), (u128::MAX - 1, 1));
        assert_eq!(multiply(1 << 64, 1 << 64), (1, 0));
        assert_eq!(multiply(12345, 678), (0, 12345 * 678));
        assert_eq!(add((0, u128::MAX), (0, 1)), (1, 0));
    }

    #[test]
    fn the_loudest_heard_noise_wins_and_ties_go_to_the_lower_index() {
        let mut world = world();
        let ear = listener(&mut world, at(0, 0, 0), 1000);
        emit(&mut world, Noise::new(at(10, 0, 0), Fx::from_ratio(1, 2)));
        emit(&mut world, Noise::new(at(20, 0, 0), Fx::ONE));
        emit(&mut world, Noise::new(at(30, 0, 0), Fx::ONE));
        emit(&mut world, Noise::new(at(5000, 0, 0), Fx::from_int(2)));
        hear(&mut world, 7);
        let heard = world.get::<HeardNoise>(ear).unwrap();
        assert_eq!(heard.location, at(20, 0, 0));
        assert_eq!(heard.loudness, Fx::ONE);
        assert_eq!(heard.tick, 7);
    }

    #[test]
    fn a_tie_is_settled_by_index_even_after_store_order_changes() {
        let mut world = world();
        let ear = listener(&mut world, at(0, 0, 0), 1000);
        let first = emit(&mut world, Noise::new(at(1, 0, 0), Fx::ONE));
        emit(&mut world, Noise::new(at(2, 0, 0), Fx::ONE));
        emit(&mut world, Noise::new(at(3, 0, 0), Fx::ONE));
        world.despawn(first);
        let reused = emit(&mut world, Noise::new(at(4, 0, 0), Fx::ONE));
        assert_eq!(reused.index(), first.index());
        let store_order: Vec<u32> = world
            .query::<Noise>()
            .into_iter()
            .map(|(e, _)| e.index())
            .collect();
        assert_ne!(store_order[0], reused.index());
        hear(&mut world, 1);
        assert_eq!(world.get::<HeardNoise>(ear).unwrap().location, at(4, 0, 0));
    }

    #[test]
    fn heard_noise_stays_until_a_newer_one_replaces_it() {
        let mut world = world();
        let ear = listener(&mut world, at(0, 0, 0), 1000);
        let mut noise = Noise::new(at(10, 0, 0), Fx::ONE);
        noise.instigator = Some(ear);
        emit(&mut world, noise);
        hear(&mut world, 1);
        clear(&mut world);
        hear(&mut world, 2);
        let heard = *world.get::<HeardNoise>(ear).unwrap();
        assert_eq!(heard.tick, 1);
        assert_eq!(heard.instigator, Some(ear));
        emit(&mut world, Noise::new(at(-10, 0, 0), Fx::from_ratio(1, 10)));
        hear(&mut world, 3);
        assert_eq!(world.get::<HeardNoise>(ear).unwrap().tick, 3);
    }

    #[test]
    fn clear_despawns_only_noise_entities() {
        let mut world = world();
        let ear = listener(&mut world, at(0, 0, 0), 1000);
        emit(&mut world, Noise::new(at(1, 0, 0), Fx::ONE));
        emit(&mut world, Noise::new(at(2, 0, 0), Fx::ONE));
        assert_eq!(world.entity_count(), 3);
        clear(&mut world);
        assert_eq!(world.entity_count(), 1);
        assert!(world.is_alive(ear));
        assert_eq!(world.component_count("noise"), Some(0));
    }

    #[test]
    fn fields_name_the_instigator_only_when_there_is_one() {
        let mut noise = Noise::new(at(1, 2, 3), Fx::ONE);
        let names = |fields: Fields| fields.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        assert_eq!(
            names(noise.fields()),
            ["x", "y", "z", "loudness", "max_range"]
        );
        noise.instigator = Some(Entity::new(4, 2));
        let fields = noise.fields();
        assert_eq!(fields[5], ("instigator_index", FieldValue::Int(4)));
        assert_eq!(fields[6], ("instigator_generation", FieldValue::Int(2)));
    }

    #[test]
    fn an_instigator_changes_the_hash() {
        let hash = |noise: &Noise| {
            let mut hasher = StateHasher::new();
            noise.hash_into(&mut hasher);
            hasher.finish()
        };
        let mut noise = Noise::new(at(1, 2, 3), Fx::ONE);
        let without = hash(&noise);
        noise.instigator = Some(Entity::new(0, 0));
        assert_ne!(hash(&noise), without);
    }
}
