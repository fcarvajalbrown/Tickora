use std::collections::BTreeMap;
use std::io::Write;

use serde_json::{Map, Value, json};
use tickora_audio::listen::analysis::{Analysis, Settings, analyse, band_names};
use tickora_audio::listen::reading::reading;
use tickora_audio::listen::seam::Seam;
use tickora_audio::listen::sheet::{self, Mark, hex, mark_colours};
use tickora_audio::listen::spectrum::SEGMENT;
use tickora_audio::listen::{Audio, Clock, MAX_ANALYSIS_TICKS};
use tickora_audio::mixer::{MAX_VOICES, UNITY};
use tickora_audio::{Origin, VoiceEvent, VoiceStart, wav};
use tickora_render::Image;

use crate::host::Host;

pub struct Hearing {
    pub sounds: Vec<Value>,
    pub sound_lines: Vec<String>,
    pub analysis: Option<Analysis>,
    pub reading: String,
    pub sheet: Option<Image>,
    pub colours: BTreeMap<String, String>,
}

impl Hearing {
    pub fn text(&self) -> String {
        let mut text = self.reading.clone();
        text.push_str(&format!("\n  sounds: {} events", self.sound_lines.len()));
        for line in &self.sound_lines {
            text.push('\n');
            text.push_str(line);
        }
        text
    }

    pub fn to_json(&self) -> Value {
        json!({
            "sounds": self.sounds,
            "analysis": self.analysis.as_ref().map(analysis_json),
            "reading": self.reading,
            "sheet_colours": self.colours,
        })
    }
}

fn gain_db(gain: i64) -> Option<f64> {
    (gain > 0).then(|| 20.0 * (gain as f64 / UNITY as f64).log10())
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn origin_json(origin: Origin) -> Value {
    match origin {
        Origin::At(at) => json!({ "location": [at.x.to_f64(), at.y.to_f64(), at.z.to_f64()] }),
        Origin::Emitter(entity) => json!({
            "emitter": { "index": entity.index(), "generation": entity.generation() }
        }),
    }
}

fn db_text(gain: Option<f64>) -> String {
    gain.map_or("silent".to_string(), |db| format!("{db:.1} dB"))
}

struct SoundList {
    names: BTreeMap<u64, String>,
    events: Vec<Value>,
    lines: Vec<String>,
    marks: Vec<Mark>,
}

impl SoundList {
    fn start(&mut self, tick: u64, start: &VoiceStart, name: &str, submix: &str, seconds: f64) {
        let gains = start.gains.map(|gain| gain_db(gain).map(round2));
        let (gain, pitch) = (start.gain.to_f64(), start.pitch.to_f64());
        self.events.push(json!({
            "tick": tick,
            "event": "start",
            "id": start.id,
            "sound": name,
            "take": start.take,
            "origin": origin_json(start.origin),
            "gain": gain,
            "pitch": pitch,
            "distance_cm": start.distance.map(|cm| cm as f64),
            "gain_db": gains,
            "submix": submix,
        }));
        let distance = start.distance.map_or("no listener".to_string(), |cm| {
            format!("{cm} cm from the listener")
        });
        self.lines.push(format!(
            "    tick {tick:>6}  {name} take {} starts: gain {gain:.3}, pitch {pitch:.3}, {distance}, left {}, right {}, submix {submix}",
            start.take,
            db_text(gains[0]),
            db_text(gains[1]),
        ));
        self.marks.push(Mark {
            time_s: seconds,
            name: name.to_string(),
        });
    }

    fn end(&mut self, tick: u64, id: u64, name: &str, reason: &str) {
        self.events.push(json!({
            "tick": tick,
            "event": "end",
            "id": id,
            "sound": name,
            "reason": reason,
        }));
        let words = match reason {
            "finished" => "finishes",
            "stopped" => "is stopped",
            _ => "is still playing at the end of the range",
        };
        self.lines
            .push(format!("    tick {tick:>6}  {name} {words}"));
    }
}

pub fn hear<H: Host + ?Sized>(
    host: &H,
    from: u64,
    to: u64,
    label: &str,
    mut wav_out: Option<&mut dyn Write>,
) -> Result<Hearing, String> {
    let format = host.audio().format();
    host.audio().range(from, to)?;
    if to - from > MAX_ANALYSIS_TICKS {
        return Err(format!(
            "the range holds {} ticks; hearing analyses at most {MAX_ANALYSIS_TICKS} ticks at a time",
            to - from
        ));
    }
    let samples = u64::from(format.samples_per_tick());
    if let Some(out) = wav_out.as_mut() {
        out.write_all(&wav::header(format.sample_rate(), (to - from) * samples)?)
            .map_err(|e| e.to_string())?;
    }
    let tick_rate = f64::from(format.tick_rate());
    let mut frames = Vec::with_capacity(((to - from) * samples) as usize);
    let mut list = SoundList {
        names: BTreeMap::new(),
        events: Vec::new(),
        lines: Vec::new(),
        marks: Vec::new(),
    };
    host.replay_audio(to, &mut |tick, block, mixer| {
        let heard = tick >= from;
        let seconds = (tick.saturating_sub(from)) as f64 / tick_rate;
        for event in &block.events {
            match event {
                VoiceEvent::Start(start) => {
                    let name = mixer.bank().name(start.sound).unwrap_or("?").to_string();
                    if heard {
                        let submix = mixer.graph().name(start.submix).unwrap_or("?");
                        list.start(tick, start, &name, submix, seconds);
                    }
                    list.names.insert(start.id, name);
                }
                VoiceEvent::End { id, stopped } => {
                    let name = list.names.remove(id).unwrap_or_default();
                    if heard {
                        let reason = if *stopped { "stopped" } else { "finished" };
                        list.end(tick, *id, &name, reason);
                    }
                }
                VoiceEvent::Dropped { sound, origin } if heard => {
                    let name = mixer.bank().name(*sound).unwrap_or("?");
                    list.events.push(json!({
                        "tick": tick,
                        "event": "dropped",
                        "sound": name,
                        "origin": origin_json(*origin),
                    }));
                    list.lines.push(format!(
                        "    tick {tick:>6}  {name} dropped at the {MAX_VOICES}-voice cap"
                    ));
                }
                VoiceEvent::Dropped { .. } => {}
            }
        }
        if heard {
            frames.extend_from_slice(&block.frames);
            if let Some(out) = wav_out.as_mut() {
                wav::write_frames(out, &block.frames).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })?;
    for (id, name) in std::mem::take(&mut list.names) {
        list.end(to, id, &name, "playing");
    }
    let colours = mark_colours(&list.marks)
        .into_iter()
        .map(|(name, colour)| (name, hex(colour)))
        .collect();
    let mut hearing = Hearing {
        sounds: list.events,
        sound_lines: list.lines,
        analysis: None,
        reading: String::new(),
        sheet: None,
        colours,
    };
    if frames.len() < SEGMENT {
        let least = (SEGMENT as u64).div_ceil(samples);
        hearing.reading = format!(
            "{label}: the range holds {} ticks; the analysis needs at least {least}",
            to - from
        );
        return Ok(hearing);
    }
    let audio = Audio::from_frames(format.sample_rate(), &frames);
    drop(frames);
    let settings = Settings {
        clock: Clock {
            first_tick: from,
            tick_rate: format.tick_rate(),
        },
        ..Settings::default()
    };
    let analysis = analyse(&audio, &settings)?;
    let onset_times: Vec<f64> = analysis.events.iter().map(|onset| onset.time_s).collect();
    let raster = sheet::sheet(
        &audio,
        &onset_times,
        &list.marks,
        sheet::DEFAULT_WIDTH,
        sheet::DEFAULT_HEIGHT,
    )?;
    hearing.reading = reading(&analysis, label);
    hearing.analysis = Some(analysis);
    hearing.sheet = Some(Image::new(raster.width, raster.height, raster.pixels)?);
    Ok(hearing)
}

fn bands_json(bands: &[f64; 5]) -> Value {
    let map: Map<String, Value> = band_names()
        .iter()
        .zip(bands)
        .map(|(name, share)| (name.to_string(), json!(share)))
        .collect();
    Value::Object(map)
}

fn seam_json(seam: &Seam) -> Value {
    json!({ "level_jump_db": seam.level_jump_db, "spectral_jump_db": seam.spectral_jump_db })
}

pub fn analysis_json(analysis: &Analysis) -> Value {
    json!({
        "duration_s": analysis.duration_s,
        "first_tick": analysis.first_tick,
        "ticks": analysis.ticks,
        "rate": analysis.rate,
        "integrated_lufs": analysis.integrated_lufs,
        "true_peak_dbtp": analysis.true_peak_dbtp,
        "short_term_range_db": analysis.short_term_range_db,
        "clipped_samples": analysis.clipped_samples,
        "dc_offset": analysis.dc_offset,
        "overall": {
            "centroid_hz": analysis.overall.centroid_hz,
            "bands_pct": bands_json(&analysis.overall.bands_pct),
            "reads_as": analysis.overall.reads_as,
        },
        "stereo": {
            "correlation": analysis.stereo.correlation,
            "side_to_mid": analysis.stereo.side_to_mid,
            "mono_fold_loss_db": analysis.stereo.mono_fold_loss_db,
            "reads_as": analysis.stereo.reads_as,
        },
        "voice_likeness": analysis.voice_likeness,
        "tones": analysis.tones.iter().map(|tone| json!({
            "hz": tone.hz,
            "above_noise_db": tone.above_noise_db,
            "reads_as": tone.reads_as,
        })).collect::<Vec<_>>(),
        "events": analysis.events.iter().map(|onset| json!({
            "time_s": onset.time_s,
            "tick": onset.tick,
            "rise_db": onset.rise_db,
            "length_ms": onset.length_ms,
            "band": onset.band,
            "reads_as": onset.reads_as,
        })).collect::<Vec<_>>(),
        "event_count": analysis.event_count,
        "event_rise_db": analysis.event_rise_db,
        "segments": analysis.segments.iter().map(|row| json!({
            "from_s": row.from_s,
            "to_s": row.to_s,
            "from_tick": row.from_tick,
            "to_tick": row.to_tick,
            "loudness_lufs": row.loudness_lufs,
            "swing_db": row.swing_db,
            "centroid_hz": row.centroid_hz,
            "bands_pct": bands_json(&row.bands_pct),
            "reads_as": row.reads_as,
        })).collect::<Vec<_>>(),
        "whole_file_seam": analysis.whole_file_seam.as_ref().map(seam_json),
        "loops": analysis.loops.as_ref().map(|loops| loops.iter().map(|candidate| json!({
            "start_s": candidate.start_s,
            "end_s": candidate.end_s,
            "start_tick": candidate.start_tick,
            "end_tick": candidate.end_tick,
            "level_jump_db": candidate.seam.level_jump_db,
            "spectral_jump_db": candidate.seam.spectral_jump_db,
        })).collect::<Vec<_>>()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::{ArenaHost, HIT_SOUND, SPAWN_SOUND};
    use crate::host::write_wav;
    use serde_json::json;

    fn arena() -> ArenaHost {
        let mut host = ArenaHost::new(7);
        host.step(&[json!({"type": "spawn"})]).unwrap();
        for _ in 0..9 {
            host.step(&[]).unwrap();
        }
        host.step(&[json!({"type": "damage", "amount": 1})])
            .unwrap();
        for _ in 0..169 {
            host.step(&[]).unwrap();
        }
        host
    }

    #[test]
    fn the_sound_list_names_each_start_and_end_at_its_tick() {
        let host = arena();
        let hearing = hear(&host, 0, 180, "arena", None).unwrap();
        let starts: Vec<(u64, &str)> = hearing
            .sounds
            .iter()
            .filter(|event| event["event"] == "start")
            .map(|event| {
                (
                    event["tick"].as_u64().unwrap(),
                    event["sound"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(starts, vec![(0, SPAWN_SOUND), (10, HIT_SOUND)]);
        let ends = hearing
            .sounds
            .iter()
            .filter(|event| event["event"] == "end")
            .count();
        assert_eq!(ends, 2);
        assert_eq!(hearing.sounds[0]["submix"], "sfx");
        assert!(
            hearing
                .reading
                .starts_with("arena: 3.0 s (180 ticks from tick 0), 48000 Hz, 2 ch, PCM_24")
        );
        assert!(
            hearing
                .text()
                .contains(&format!("    tick      0  {SPAWN_SOUND} take 0 starts: "))
        );
        assert_eq!(hearing.colours.len(), 2);
        let sheet = hearing.sheet.as_ref().unwrap();
        assert_eq!((sheet.width, sheet.height), (1000, 500));
        assert_eq!(hearing.analysis.as_ref().unwrap().ticks, 180);
    }

    #[test]
    fn a_voice_from_before_the_range_keeps_its_name_and_the_wav_matches() {
        let host = arena();
        let mut shared = Vec::new();
        let hearing = hear(&host, 5, 180, "arena", Some(&mut shared)).unwrap();
        let mut alone = Vec::new();
        write_wav(&host, 5, 180, &mut alone).unwrap();
        assert_eq!(shared, alone);
        let ends: Vec<(u64, &str)> = hearing
            .sounds
            .iter()
            .filter(|event| event["event"] == "end")
            .map(|event| {
                (
                    event["tick"].as_u64().unwrap(),
                    event["sound"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(ends, vec![(23, SPAWN_SOUND), (72, HIT_SOUND)]);
    }

    #[test]
    fn a_range_shorter_than_one_frame_lists_sounds_without_analysis() {
        let host = arena();
        let short = hear(&host, 0, 2, "arena", None).unwrap();
        assert!(short.analysis.is_none() && short.sheet.is_none());
        assert_eq!(short.sounds.len(), 2);
        assert_eq!(
            short.reading,
            "arena: the range holds 2 ticks; the analysis needs at least 3"
        );
        assert!(hear(&host, 0, 3, "arena", None).unwrap().analysis.is_some());
        assert!(hear(&host, 0, 181, "arena", None).is_err());
    }
}
