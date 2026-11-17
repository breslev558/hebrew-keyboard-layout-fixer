//! The short sounds of the program, made here (no sound file, nothing borrowed). 2.0.0: a soft
//! "bubble" when a text is fixed: one pure sine whose pitch rises from 420 to 980 Hz in 90 ms, with
//! a 4 ms attack and a quick decay (200 ms in all); a shorter, softer FALLING bubble (980 to
//! 420 Hz, 120 ms) when a fix is undone or toggled back. Never the Windows error or warning sounds.

const RATE: u32 = 22_050;

/// A mono 16-bit WAV in memory from samples in -1.0 ..= 1.0.
fn wav_of(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        w.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    w
}

/// A "bubble": one sine going from `f0` to `f1` Hz in 90 ms (continuous phase), then staying at
/// `f1`; envelope min(t / 4 ms, 1) · exp(−t / 60 ms); `ms` long with a linear fade over the last
/// 8 ms; no silence at the start; the loudest sample is `peak` of full scale.
fn bubble(f0: f64, f1: f64, ms: u32, peak: f64) -> Vec<u8> {
    const GLIDE: f64 = 0.09;
    const FADE: f64 = 0.008;
    let n = (RATE * ms / 1000) as usize;
    let len = n as f64 / RATE as f64;
    let raw: Vec<f64> = (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let phase = if t < GLIDE {
                f0 * t + (f1 - f0) * t * t / (2.0 * GLIDE)
            } else {
                f0 * GLIDE + (f1 - f0) * GLIDE / 2.0 + f1 * (t - GLIDE)
            };
            let env = (t / 0.004).min(1.0) * (-t / 0.060).exp();
            let fade = ((len - t) / FADE).min(1.0);
            (2.0 * std::f64::consts::PI * phase).sin() * env * fade
        })
        .collect();
    let max = raw.iter().fold(0.0f64, |m, x| m.max(x.abs())).max(1e-9);
    let samples: Vec<f32> = raw.iter().map(|x| (x / max * peak) as f32).collect();
    wav_of(&samples)
}

/// Fixed: a rising bubble, 420 → 980 Hz, 200 ms, peak 18 % of full scale.
pub fn fixed() -> Vec<u8> {
    bubble(420.0, 980.0, 200, 0.18)
}

/// Undone / toggled back: a falling bubble, 980 → 420 Hz, shorter (120 ms) and softer (12 %).
pub fn undone() -> Vec<u8> {
    bubble(980.0, 420.0, 120, 0.12)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(w: &[u8]) -> Vec<i16> {
        w[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
    }

    /// Zero crossings between two sample positions (a higher pitch crosses more often).
    fn crossings(s: &[i16], from: usize, to: usize) -> usize {
        s[from..to].windows(2).filter(|p| (p[0] < 0) != (p[1] < 0)).count()
    }

    #[test]
    fn bubbles_are_short_quiet_valid_wavs() {
        for (w, ms, peak, rising) in [(fixed(), 200, 0.18, true), (undone(), 120, 0.12, false)] {
            assert_eq!(&w[0..4], b"RIFF");
            assert_eq!(&w[8..16], b"WAVEfmt ");
            let len = u32::from_le_bytes([w[40], w[41], w[42], w[43]]) as usize;
            assert_eq!(len + 44, w.len());
            let got = len / 2 * 1000 / RATE as usize;
            assert!((ms - 5..=ms + 5).contains(&got), "{got} ms");
            let s = samples(&w);
            let max = s.iter().map(|x| x.unsigned_abs()).max().unwrap() as f64 / i16::MAX as f64;
            // quiet: never above 20 % of full scale, and the expected level (about 2 %)
            assert!(max <= 0.20 && (max - peak).abs() <= peak * 0.02, "peak {max}");
            // it ends in silence, starts at once (near 0, never a silent start)
            assert!(s.last().unwrap().unsigned_abs() < 200);
            assert!(s[0].unsigned_abs() < 200);
            assert!(s[9] != 0, "the 10th sample already sounds");
            // the pitch rises (fixed) or falls (undone): crossings in the first 40 ms vs the next 40 ms
            let q = (RATE / 25) as usize; // 40 ms
            let (a, b) = (crossings(&s, 0, q), crossings(&s, q, 2 * q));
            if rising {
                assert!(b > a, "rising: {a} then {b}");
            } else {
                assert!(b < a, "falling: {a} then {b}");
            }
        }
    }
}
