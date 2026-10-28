use std::io::{self, Write};

use crate::format::{BITS_PER_SAMPLE, CHANNELS};

pub const HEADER_BYTES: usize = 44;
const BYTES_PER_FRAME: u64 = (CHANNELS as u64) * (BITS_PER_SAMPLE as u64 / 8);

pub fn header(sample_rate: u32, frames: u64) -> Result<[u8; HEADER_BYTES], String> {
    let data = frames
        .checked_mul(BYTES_PER_FRAME)
        .and_then(|bytes| u32::try_from(bytes).ok())
        .filter(|&bytes| bytes <= u32::MAX - 36)
        .ok_or_else(|| format!("{frames} frames do not fit in one WAV file"))?;
    let block_align = CHANNELS * (BITS_PER_SAMPLE / 8);
    let byte_rate = sample_rate * u32::from(block_align);
    let mut bytes = [0u8; HEADER_BYTES];
    let mut at = 0;
    let mut put = |chunk: &[u8]| {
        bytes[at..at + chunk.len()].copy_from_slice(chunk);
        at += chunk.len();
    };
    put(b"RIFF");
    put(&(36 + data).to_le_bytes());
    put(b"WAVE");
    put(b"fmt ");
    put(&16u32.to_le_bytes());
    put(&1u16.to_le_bytes());
    put(&CHANNELS.to_le_bytes());
    put(&sample_rate.to_le_bytes());
    put(&byte_rate.to_le_bytes());
    put(&block_align.to_le_bytes());
    put(&BITS_PER_SAMPLE.to_le_bytes());
    put(b"data");
    put(&data.to_le_bytes());
    Ok(bytes)
}

pub fn write_frames(out: &mut dyn Write, frames: &[[i32; 2]]) -> io::Result<()> {
    let mut bytes = Vec::with_capacity(frames.len() * BYTES_PER_FRAME as usize);
    for frame in frames {
        for value in frame {
            bytes.extend_from_slice(&value.to_le_bytes()[..3]);
        }
    }
    out.write_all(&bytes)
}

pub fn encode(sample_rate: u32, frames: &[[i32; 2]]) -> Result<Vec<u8>, String> {
    let mut bytes = header(sample_rate, frames.len() as u64)?.to_vec();
    write_frames(&mut bytes, frames).map_err(|e| e.to_string())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_is_24_bit_pcm_stereo() {
        let bytes = header(48_000, 800).unwrap();
        let expected: Vec<u8> = [
            &b"RIFF"[..],
            &(36u32 + 4800).to_le_bytes(),
            b"WAVE",
            b"fmt ",
            &16u32.to_le_bytes(),
            &1u16.to_le_bytes(),
            &2u16.to_le_bytes(),
            &48_000u32.to_le_bytes(),
            &288_000u32.to_le_bytes(),
            &6u16.to_le_bytes(),
            &24u16.to_le_bytes(),
            b"data",
            &4800u32.to_le_bytes(),
        ]
        .concat();
        assert_eq!(bytes.to_vec(), expected);
    }

    #[test]
    fn samples_are_three_little_endian_bytes_left_then_right() {
        let bytes = encode(48_000, &[[1, -1], [0x12_3456, -0x80_0000]]).unwrap();
        assert_eq!(
            &bytes[HEADER_BYTES..],
            [1, 0, 0, 0xFF, 0xFF, 0xFF, 0x56, 0x34, 0x12, 0, 0, 0x80]
        );
    }

    #[test]
    fn a_file_past_four_gigabytes_is_refused() {
        assert!(header(48_000, 715_827_877).is_err());
        assert!(header(48_000, 715_827_876).is_ok());
    }
}
