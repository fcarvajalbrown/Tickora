use std::sync::Arc;

use tickora_audio::component::{self, audio_rng};
use tickora_audio::{
    Attenuation, AudioFormat, Capture, Envelope, Listener, MixGraph, Mixer, SoundBank, SoundDef,
    Source, SubmixDef, TakeHistory, Variation,
};
use tickora_core::ecs::World;
use tickora_core::fx::Fx;
use tickora_core::noise::Location;

const TICKS: u64 = 240;
const PINNED: u64 = 708_255_716_613_875_523;

fn fx(value: i32) -> Fx {
    Fx::from_int(value)
}

fn graph() -> MixGraph {
    MixGraph::new(
        Fx::from_ratio(1, 2),
        &[
            SubmixDef {
                name: "sfx".to_string(),
                parent: "master".to_string(),
                gain: Fx::from_ratio(1, 2),
            },
            SubmixDef {
                name: "ambience".to_string(),
                parent: "master".to_string(),
                gain: Fx::from_ratio(1, 4),
            },
        ],
    )
    .unwrap()
}

fn bank(graph: &MixGraph) -> SoundBank {
    let sfx = graph.id("sfx").unwrap();
    let ambience = graph.id("ambience").unwrap();
    let spatial = Attenuation::Linear {
        inner_radius: fx(200),
        falloff_distance: fx(3000),
    };
    let ramp: Vec<i32> = (0..480).map(|i| (i * 17_000) - 4_000_000).collect();
    let defs = [
        (
            "tone",
            Source::Sine { hz: fx(523) },
            Envelope {
                attack: 120,
                release: 2400,
                duration: 9600,
            },
            spatial,
            sfx,
        ),
        (
            "buzz",
            Source::Square {
                hz: Fx::from_ratio(1101, 10),
            },
            Envelope {
                attack: 0,
                release: 800,
                duration: 4000,
            },
            Attenuation::None,
            sfx,
        ),
        (
            "hit",
            Source::Noise,
            Envelope {
                attack: 48,
                release: 6000,
                duration: 12000,
            },
            spatial,
            sfx,
        ),
        (
            "drip",
            Source::Pcm {
                samples: Arc::from(ramp),
                rate: 24_000,
            },
            Envelope::default(),
            spatial,
            sfx,
        ),
        (
            "hum",
            Source::Sine { hz: fx(60) },
            Envelope {
                attack: 4800,
                release: 4800,
                duration: 0,
            },
            spatial,
            ambience,
        ),
    ];
    let mut bank = SoundBank::new();
    for (name, source, envelope, attenuation, submix) in defs {
        bank.add(
            name,
            SoundDef {
                source,
                envelope,
                variation: Variation {
                    gain: (Fx::from_ratio(3, 5), Fx::ONE),
                    pitch: (Fx::from_ratio(9, 10), Fx::from_ratio(11, 10)),
                },
                attenuation,
                submix,
            },
        )
        .unwrap();
    }
    bank
}

fn render() -> Capture {
    let graph = graph();
    let bank = Arc::new(bank(&graph));
    let mut mixer = Mixer::new(AudioFormat::DEFAULT, bank.clone(), graph).unwrap();
    let mut world = World::new();
    world.register::<Location>();
    component::register(&mut world);
    let listener = world.spawn();
    world
        .insert(
            listener,
            Listener {
                location: Location::new(fx(0), fx(0), fx(170)),
                yaw: fx(30),
            },
        )
        .unwrap();
    let mut rng = audio_rng(0xC0FFEE);
    let mut takes = TakeHistory::new();
    let mut capture = Capture::new(AudioFormat::DEFAULT);
    let mut hum = None;
    for tick in 0..TICKS {
        component::clear(&mut world);
        let at = |x: i32, y: i32| Location::new(fx(x), fx(y), Fx::ZERO);
        let sound = |name: &str| bank.id(name).unwrap();
        match tick {
            0 => {
                let entity = world.spawn();
                world.insert(entity, at(800, -400)).unwrap();
                let emitter = component::emitter(&bank, &mut rng, sound("hum")).unwrap();
                world.insert(entity, emitter).unwrap();
                hum = Some(entity);
            }
            5 | 70 => {
                component::play(
                    &mut world,
                    &bank,
                    &mut rng,
                    &mut takes,
                    sound("tone"),
                    at(300, 900),
                )
                .unwrap();
            }
            12 => {
                component::play(
                    &mut world,
                    &bank,
                    &mut rng,
                    &mut takes,
                    sound("buzz"),
                    at(0, 0),
                )
                .unwrap();
                component::play(
                    &mut world,
                    &bank,
                    &mut rng,
                    &mut takes,
                    sound("hit"),
                    at(-600, -200),
                )
                .unwrap();
            }
            40 => {
                component::play(
                    &mut world,
                    &bank,
                    &mut rng,
                    &mut takes,
                    sound("drip"),
                    at(150, 50),
                )
                .unwrap();
            }
            200 => {
                world.despawn(hum.unwrap());
            }
            _ => {}
        }
        if let Some(entity) = hum
            && let Some(location) = world.get_mut::<Location>(entity)
        {
            location.y += fx(5);
        }
        capture.push(mixer.render(&world).summary);
    }
    capture
}

#[test]
fn the_same_world_renders_the_same_blocks() {
    let first = render();
    let second = render();
    assert_eq!(first.blocks(), second.blocks());
    let meters = first.meters(0, TICKS).unwrap();
    assert_eq!(meters.clipped, 0);
    assert!(meters.peak_dbfs[0].is_some() && meters.peak_dbfs[1].is_some());
    assert!(meters.voices >= 3, "{meters:?}");
    assert_eq!(first.blocks()[TICKS as usize - 1].voices, 0);
}

#[test]
fn the_audio_hash_of_the_test_world_is_pinned() {
    assert_eq!(render().hash(0, TICKS).unwrap(), PINNED);
}
