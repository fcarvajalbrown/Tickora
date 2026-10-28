use tickora_core::hash::StateHasher;

use crate::format::{AudioFormat, FULL_SCALE};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockSummary {
    pub hash: u64,
    pub samples: u64,
    pub peak: [u32; 2],
    pub sum_squares: [u128; 2],
    pub clipped: u32,
    pub voices: u32,
    pub dropped: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Meters {
    pub from: u64,
    pub to: u64,
    pub hash: u64,
    pub peak_dbfs: [Option<f64>; 2],
    pub rms_dbfs: [Option<f64>; 2],
    pub clipped: u64,
    pub voices: u32,
    pub dropped: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    format: AudioFormat,
    blocks: Vec<BlockSummary>,
}

impl Capture {
    pub fn new(format: AudioFormat) -> Self {
        Capture {
            format,
            blocks: Vec::new(),
        }
    }

    pub fn format(&self) -> AudioFormat {
        self.format
    }

    pub fn push(&mut self, summary: BlockSummary) {
        self.blocks.push(summary);
    }

    pub fn ticks(&self) -> u64 {
        self.blocks.len() as u64
    }

    pub fn blocks(&self) -> &[BlockSummary] {
        &self.blocks
    }

    pub fn last(&self) -> Option<&BlockSummary> {
        self.blocks.last()
    }

    pub fn range(&self, from: u64, to: u64) -> Result<&[BlockSummary], String> {
        if from > to {
            return Err(format!("audio range [{from}, {to}) runs backwards"));
        }
        if to > self.ticks() {
            return Err(format!(
                "audio range [{from}, {to}) ends past the {} rendered ticks",
                self.ticks()
            ));
        }
        Ok(&self.blocks[from as usize..to as usize])
    }

    pub fn hash(&self, from: u64, to: u64) -> Result<u64, String> {
        Ok(chain_hash(self.range(from, to)?))
    }

    pub fn meters(&self, from: u64, to: u64) -> Result<Meters, String> {
        let blocks = self.range(from, to)?;
        let samples: u64 = blocks.iter().map(|block| block.samples).sum();
        let channel = |c: usize| {
            let peak = blocks.iter().map(|block| block.peak[c]).max().unwrap_or(0);
            let squares: u128 = blocks.iter().map(|block| block.sum_squares[c]).sum();
            (peak_dbfs(peak), rms_dbfs(squares, samples))
        };
        let (left, right) = (channel(0), channel(1));
        Ok(Meters {
            from,
            to,
            hash: chain_hash(blocks),
            peak_dbfs: [left.0, right.0],
            rms_dbfs: [left.1, right.1],
            clipped: blocks.iter().map(|block| u64::from(block.clipped)).sum(),
            voices: blocks.iter().map(|block| block.voices).max().unwrap_or(0),
            dropped: blocks.iter().map(|block| u64::from(block.dropped)).sum(),
        })
    }
}

fn chain_hash(blocks: &[BlockSummary]) -> u64 {
    let mut hasher = StateHasher::new();
    for block in blocks {
        hasher.write_u64(block.hash);
    }
    hasher.finish()
}

pub fn peak_dbfs(peak: u32) -> Option<f64> {
    (peak > 0).then(|| 20.0 * (f64::from(peak) / f64::from(FULL_SCALE)).log10())
}

pub fn rms_dbfs(sum_squares: u128, samples: u64) -> Option<f64> {
    if sum_squares == 0 || samples == 0 {
        return None;
    }
    let mean = sum_squares as f64 / samples as f64;
    Some(10.0 * (mean / (f64::from(FULL_SCALE) * f64::from(FULL_SCALE))).log10())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(hash: u64, peak: u32, voices: u32) -> BlockSummary {
        BlockSummary {
            hash,
            samples: 800,
            peak: [peak, 0],
            sum_squares: [u128::from(peak) * u128::from(peak) * 800, 0],
            clipped: 1,
            voices,
            dropped: 0,
        }
    }

    #[test]
    fn full_scale_is_zero_dbfs_and_silence_is_none() {
        assert_eq!(peak_dbfs(1 << 23), Some(0.0));
        assert!((peak_dbfs(1 << 22).unwrap() + 6.020_599_913).abs() < 1e-6);
        assert_eq!(peak_dbfs(0), None);
        assert_eq!(rms_dbfs(0, 10), None);
        let full = u128::from(1u32 << 23).pow(2);
        assert_eq!(rms_dbfs(full * 4, 4), Some(0.0));
    }

    #[test]
    fn meters_cover_the_half_open_range() {
        let mut capture = Capture::new(AudioFormat::DEFAULT);
        capture.push(block(1, 1 << 22, 2));
        capture.push(block(2, 1 << 21, 5));
        capture.push(block(3, 0, 0));
        let meters = capture.meters(1, 3).unwrap();
        assert!((meters.peak_dbfs[0].unwrap() + 12.041_199_826).abs() < 1e-6);
        assert_eq!(meters.peak_dbfs[1], None);
        assert_eq!(meters.voices, 5);
        assert_eq!(meters.clipped, 2);
        assert_eq!(meters.hash, capture.hash(1, 3).unwrap());
        assert_ne!(capture.hash(0, 3).unwrap(), capture.hash(1, 3).unwrap());
        assert_eq!(capture.hash(2, 2), Ok(StateHasher::new().finish()));
        assert!(capture.meters(2, 4).is_err());
        assert!(capture.meters(2, 1).is_err());
    }
}
