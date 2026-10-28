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

pub const READ_SAMPLE_RATE: u32 = 48_000;
const PCM_FORMAT: u16 = 1;
const READ_SHIFT: u32 = (BITS_PER_SAMPLE - 16) as u32;

pub fn read_mono16(bytes: &[u8]) -> Result<Vec<i32>, String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".to_string());
    }
    let mut at = 12;
    let mut format = None;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = bytes.get(at + 8..at + 8 + size).ok_or_else(|| {
            format!(
                "chunk {:?} runs past the end of the file",
                String::from_utf8_lossy(id)
            )
        })?;
        match id {
            b"fmt " => format = Some(read_format(body)?),
            b"data" => {
                if format.is_none() {
                    return Err("the data chunk comes before the fmt chunk".to_string());
                }
                if !size.is_multiple_of(2) {
                    return Err(format!("16-bit data of {size} bytes is not whole samples"));
                }
                return Ok(body
                    .chunks_exact(2)
                    .map(|pair| i32::from(i16::from_le_bytes([pair[0], pair[1]])) << READ_SHIFT)
                    .collect());
            }
            _ => {}
        }
        at += 8 + size + size % 2;
    }
    Err("no data chunk".to_string())
}

fn read_format(body: &[u8]) -> Result<(), String> {
    if body.len() < 16 {
        return Err(format!("fmt chunk of {} bytes is too short", body.len()));
    }
    let word = |at: usize| u16::from_le_bytes([body[at], body[at + 1]]);
    let (format, channels, bits) = (word(0), word(2), word(14));
    let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
    if (format, channels, rate, bits) != (PCM_FORMAT, 1, READ_SAMPLE_RATE, 16) {
        return Err(format!(
            "expected PCM, 1 channel, {READ_SAMPLE_RATE} Hz, 16 bits; found format {format}, {channels} channels, {rate} Hz, {bits} bits"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono16(rate: u32, channels: u16, bits: u16, data: &[u8], extra: &[u8]) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * u32::from(channels) * u32::from(bits / 8)).to_le_bytes());
        fmt.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        let mut body = b"WAVE".to_vec();
        for (id, chunk) in [(&b"fmt "[..], &fmt[..]), (b"LIST", extra), (b"data", data)] {
            body.extend_from_slice(id);
            body.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            body.extend_from_slice(chunk);
            if chunk.len() % 2 == 1 {
                body.push(0);
            }
        }
        [&b"RIFF"[..], &(body.len() as u32).to_le_bytes(), &body].concat()
    }

    #[test]
    fn a_16_bit_mono_file_reads_as_exact_24_bit_samples_past_other_chunks() {
        let data = [0x01, 0x00, 0xFF, 0xFF, 0xFF, 0x7F, 0x00, 0x80];
        let bytes = mono16(48_000, 1, 16, &data, b"odd");
        assert_eq!(
            read_mono16(&bytes),
            Ok(vec![1 << 8, -(1 << 8), 0x7FFF << 8, -0x8000 << 8])
        );
        assert_eq!(-0x8000 << 8, -crate::format::FULL_SCALE);
    }

    #[test]
    fn other_formats_and_broken_files_are_refused() {
        let data = [0u8; 4];
        assert!(read_mono16(&mono16(48_000, 2, 16, &data, b"")).is_err());
        assert!(read_mono16(&mono16(48_000, 1, 24, &[0u8; 6], b"")).is_err());
        assert!(read_mono16(&mono16(44_100, 1, 16, &data, b"")).is_err());
        assert!(read_mono16(&mono16(48_000, 1, 16, &[0u8; 3], b"")).is_err());
        let whole = mono16(48_000, 1, 16, &data, b"");
        assert!(read_mono16(&whole[..whole.len() - 2]).is_err());
        assert!(read_mono16(b"RIFF\0\0\0\0WAVX").is_err());
    }

    #[test]
    fn the_arena_sounds_on_disk_read() {
        let bytes = include_bytes!("../../../assets/gen/audio/spawn.wav");
        assert_eq!(read_mono16(bytes).unwrap().len(), 19_200);
    }

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
