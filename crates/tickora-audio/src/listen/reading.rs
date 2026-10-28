use super::analysis::{Analysis, band_names};
use crate::format::{BITS_PER_SAMPLE, CHANNELS};

pub fn py_float(value: f64) -> String {
    if value.is_nan() {
        "nan".to_string()
    } else if value.is_infinite() {
        if value > 0.0 { "inf" } else { "-inf" }.to_string()
    } else if value.fract() == 0.0 && value.abs() < 1e16 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

fn py_bands(bands: &[f64; 5]) -> String {
    let pairs: Vec<String> = band_names()
        .iter()
        .zip(bands)
        .map(|(name, share)| format!("'{name}': {}", *share as i64))
        .collect();
    format!("{{{}}}", pairs.join(", "))
}

pub fn reading(analysis: &Analysis, label: &str) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "{label}: {} s ({} ticks from tick {}), {} Hz, {CHANNELS} ch, PCM_{BITS_PER_SAMPLE}",
        py_float(analysis.duration_s),
        analysis.ticks,
        analysis.first_tick,
        analysis.rate
    ));
    lines.push(format!(
        "  loudness {} LUFS integrated, true peak {} dBTP, short-term range {} dB (10th to 95th percentile)",
        py_float(analysis.integrated_lufs.unwrap_or(f64::NEG_INFINITY)),
        py_float(analysis.true_peak_dbtp),
        py_float(analysis.short_term_range_db)
    ));
    if analysis.clipped_samples > 0 {
        lines.push(format!(
            "  CLIPPING: {} samples at full scale",
            analysis.clipped_samples
        ));
    }
    if analysis.dc_offset.abs() > 0.005 {
        lines.push(format!("  DC offset {}", py_float(analysis.dc_offset)));
    }
    let overall = &analysis.overall;
    lines.push(format!(
        "  overall: {}; centroid {} Hz; bands % {}",
        overall.reads_as,
        overall.centroid_hz as i64,
        py_bands(&overall.bands_pct)
    ));
    if overall.bands_pct[0] >= 40.0 {
        lines.push("  sub-80 Hz carries most of the energy: likely wind on the microphone or traffic rumble; a high-pass near 80 Hz would clean it".to_string());
    }
    let stereo = &analysis.stereo;
    let fold_loss = stereo.mono_fold_loss_db.unwrap_or(f64::NAN);
    lines.push(format!(
        "  stereo: {} (correlation {}); folded to mono it loses {} dB against identical channels{}",
        stereo.reads_as,
        py_float(stereo.correlation),
        py_float(fold_loss),
        if fold_loss > 3.5 {
            ", so it thins out on a mono speaker or a mono-folded 3D source"
        } else {
            ""
        }
    ));
    if let Some(score) = analysis.voice_likeness {
        let verdict = if score >= 0.35 {
            "rhythm at syllable rate in the voice band: possible voices, listen to check"
        } else {
            "no syllable-rate rhythm in the voice band"
        };
        lines.push(format!(
            "  voices: {verdict} (score {}, heuristic)",
            py_float(score)
        ));
    }
    for tone in &analysis.tones {
        lines.push(format!(
            "  tone {} Hz, {} dB above the noise: {}",
            py_float(tone.hz),
            py_float(tone.above_noise_db),
            tone.reads_as
        ));
    }
    lines.push(format!(
        "  transients: {} found over {} dB, loudest {}:",
        analysis.event_count,
        py_float(analysis.event_rise_db),
        analysis.events.len()
    ));
    for event in &analysis.events {
        lines.push(format!(
            "    {:7.2} s  tick {:>6}  +{} dB  {} ms  {}: {}",
            event.time_s,
            event.tick,
            py_float(event.rise_db),
            event.length_ms,
            event.band,
            event.reads_as
        ));
    }
    lines.push("  timeline:".to_string());
    for row in &analysis.segments {
        lines.push(format!(
            "    {:6.1}-{:6.1} s  ticks {:>6}-{:>6}  {:6.1} LUFS  swing {:4.1} dB  {:5} Hz  {}",
            row.from_s,
            row.to_s,
            row.from_tick,
            row.to_tick,
            row.loudness_lufs,
            row.swing_db,
            row.centroid_hz as i64,
            row.reads_as
        ));
    }
    if let Some(seam) = &analysis.whole_file_seam {
        lines.push(format!(
            "  looping the whole file: level jump {} dB, spectral jump {} dB at the seam",
            py_float(seam.level_jump_db),
            py_float(seam.spectral_jump_db)
        ));
    }
    for candidate in analysis.loops.iter().flatten() {
        lines.push(format!(
            "  loop candidate {} to {} s (ticks {} to {}): level jump {} dB, spectral jump {} dB",
            py_float(candidate.start_s),
            py_float(candidate.end_s),
            candidate.start_tick,
            candidate.end_tick,
            py_float(candidate.seam.level_jump_db),
            py_float(candidate.seam.spectral_jump_db)
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::super::analysis::{Settings, analyse};
    use super::super::{Audio, Clock};
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn floats_print_as_python_prints_them() {
        assert_eq!(py_float(-20.0), "-20.0");
        assert_eq!(py_float(-0.0), "-0.0");
        assert_eq!(py_float(0.35), "0.35");
        assert_eq!(py_float(-20.3), "-20.3");
        assert_eq!(py_float(1234.5), "1234.5");
        assert_eq!(py_float(f64::NEG_INFINITY), "-inf");
        assert_eq!(py_float(f64::NAN), "nan");
    }

    fn sine_with_clicks() -> Audio {
        let mut mono: Vec<f64> = (0..48_000 * 4)
            .map(|i| 0.1 * (2.0 * PI * 1000.0 * i as f64 / 48_000.0).sin())
            .collect();
        for start in [48_000, 96_000] {
            for (j, sample) in mono[start..start + 2400].iter_mut().enumerate() {
                *sample += 0.8 * (2.0 * PI * 100.0 * j as f64 / 48_000.0).sin();
            }
        }
        Audio::new(48_000, [mono.clone(), mono])
    }

    #[test]
    fn the_reading_follows_listen_py_line_by_line_with_ticks() {
        let settings = Settings {
            clock: Clock {
                first_tick: 120,
                tick_rate: 60,
            },
            loop_search: Some((1.0, 2.0)),
            ..Settings::default()
        };
        let analysis = analyse(&sine_with_clicks(), &settings).unwrap();
        let text = reading(&analysis, "arena ticks 120 to 360");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "arena ticks 120 to 360: 4.0 s (240 ticks from tick 120), 48000 Hz, 2 ch, PCM_24"
        );
        assert!(lines[1].starts_with("  loudness -"));
        assert!(lines[1].ends_with("dB (10th to 95th percentile)"));
        assert!(text.contains("  stereo: near mono (correlation 1.0); folded to mono it loses "));
        assert!(text.contains("  tone 1007.8 Hz, 63.5 dB above the noise: whistle or tone"));
        assert!(text.contains("  transients: 2 found over 10.0 dB, loudest 2:"));
        assert!(text.contains("       1.00 s  tick    180  +"));
        assert!(text.contains("       2.00 s  tick    240  +"));
        assert!(
            text.contains("  timeline:\n       0.0-   0.3 s  ticks    120-   140   -20.3 LUFS")
        );
        assert!(text.contains("  looping the whole file: level jump "));
        assert_eq!(text.matches("  loop candidate ").count(), 2);
        assert!(text.contains("  loop candidate 0.2 to 1.45 s (ticks 132 to 207): "));
    }

    #[test]
    fn silence_reads_as_listen_py_reads_it() {
        let audio = Audio::new(48_000, [vec![0.0; 96_000], vec![0.0; 96_000]]);
        let analysis = analyse(&audio, &Settings::default()).unwrap();
        let text = reading(&analysis, "silence");
        assert!(text.contains("  loudness -inf LUFS integrated, true peak -240.0 dBTP"));
        assert!(text.contains("folded to mono it loses nan dB against identical channels\n"));
        assert!(text.contains("  transients: 0 found over 10.0 dB, loudest 0:"));
    }
}
