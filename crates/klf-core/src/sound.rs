//! 1.8.5: the short sounds of the program, made here (no sound file, nothing borrowed): a soft
//! rising two-note "tick" when a text is fixed, and one lower, even quieter note when a fix is
//! undone or toggled back. Never the Windows error or warning sounds.

const RATE: u32 = 22_050;

/// A mono 16-bit WAV in memory: each (frequency Hz, length ms) note, with soft attack and decay.
pub fn wav(notes: &[(f32, u32)], volume: f32) -> Vec<u8> {
    let mut samples: Vec<i16> = Vec::new();
    for &(f, ms) in notes {
        let n = (RATE * ms / 1000) as usize;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            // 8 ms attack, then a smooth decay to silence: no click at either end
            let attack = (i as f32 / (RATE as f32 * 0.008)).min(1.0);
            let decay = 1.0 - i as f32 / n as f32;
            let env = attack * decay * decay;
            // a pure tone with a little second harmonic: round, not a beep
            let v = (2.0 * std::f32::consts::PI * f * t).sin() * 0.85 + (4.0 * std::f32::consts::PI * f * t).sin() * 0.15;
            samples.push((v * env * volume * i16::MAX as f32) as i16);
        }
    }
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
        w.extend_from_slice(&s.to_le_bytes());
    }
    w
}

/// Fixed: two soft rising notes (E6 then A6), about 110 ms, low volume.
pub fn fixed() -> Vec<u8> {
    wav(&[(1318.5, 50), (1760.0, 60)], 0.18)
}

/// Undone / toggled back: one lower, shorter, quieter note.
pub fn undone() -> Vec<u8> {
    wav(&[(880.0, 45)], 0.12)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_quiet_valid_wav() {
        for w in [fixed(), undone()] {
            assert_eq!(&w[0..4], b"RIFF");
            assert_eq!(&w[8..16], b"WAVEfmt ");
            let len = u32::from_le_bytes([w[40], w[41], w[42], w[43]]) as usize;
            assert_eq!(len + 44, w.len());
            let ms = len / 2 * 1000 / RATE as usize;
            assert!((30..=150).contains(&ms), "{ms} ms");
            // quiet: never above 20 % of full scale, and it ends in silence
            let s: Vec<i16> = w[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
            assert!(s.iter().all(|x| x.unsigned_abs() < (i16::MAX as u16) / 5));
            assert!(s.last().unwrap().unsigned_abs() < 200);
        }
    }
}
