pub fn colour(hz: f64) -> &'static str {
    if hz < 300.0 {
        "dark rumble"
    } else if hz < 1000.0 {
        "warm, low-mid"
    } else if hz < 3000.0 {
        "mid, present"
    } else if hz < 6000.0 {
        "bright"
    } else {
        "hissy, very bright"
    }
}

pub fn texture(flat: f64, tonal: bool) -> &'static str {
    if flat > 0.3 {
        "noise-like"
    } else if flat > 0.1 {
        if tonal {
            "noisy with some pitch"
        } else {
            "coloured noise"
        }
    } else if tonal {
        "pitched or tonal"
    } else {
        "narrow-band noise (whoosh or roar)"
    }
}

pub fn steadiness(spread_db: f64) -> &'static str {
    if spread_db < 1.5 {
        "steady"
    } else if spread_db < 4.0 {
        "gently moving"
    } else if spread_db < 8.0 {
        "gusting"
    } else {
        "very uneven"
    }
}

pub fn describe_event(band: &str, length_ms: i64) -> &'static str {
    match band {
        "sub" | "low" if length_ms < 400 => {
            "thump or buffet (mic handling or wind on the capsule if it is in a bed)"
        }
        "sub" | "low" => "low swell (wave, gust or vehicle)",
        "mid" if length_ms < 400 => "knock, step or voice-band burst",
        "mid" => "mid-band swell",
        _ if length_ms < 150 => "click, tick or crack",
        _ => "hiss burst (spray, gust in grass, rain shower)",
    }
}

pub fn tone(hz: f64, mains: bool) -> &'static str {
    if mains {
        "mains hum"
    } else if hz < 200.0 {
        "low drone"
    } else if hz > 1000.0 {
        "whistle or tone"
    } else {
        "tonal component"
    }
}

pub fn width(correlation: f64) -> &'static str {
    if correlation > 0.9 {
        "near mono"
    } else if correlation > 0.5 {
        "moderate width"
    } else if correlation > 0.0 {
        "wide"
    } else {
        "very wide or phase-cancelling in mono"
    }
}
