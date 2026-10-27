use tickora_core::hash::StateHasher;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, String> {
        let expected = width as usize * height as usize * 4;
        if pixels.len() != expected {
            return Err(format!(
                "{width}x{height} RGBA needs {expected} bytes, got {}",
                pixels.len()
            ));
        }
        Ok(Image {
            width,
            height,
            pixels,
        })
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.pixels[at],
            self.pixels[at + 1],
            self.pixels[at + 2],
            self.pixels[at + 3],
        ]
    }

    pub fn pixel_hash(&self) -> u64 {
        let mut hasher = StateHasher::new();
        hasher.write_u32(self.width);
        hasher.write_u32(self.height);
        hasher.write_bytes(&self.pixels);
        hasher.finish()
    }

    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&self.pixels)
            .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        Ok(out)
    }

    pub fn from_png(bytes: &[u8]) -> Result<Self, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let size = reader
            .output_buffer_size()
            .ok_or("PNG is too large to decode")?;
        let mut buffer = vec![0; size];
        let frame = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
        if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
            return Err(format!(
                "PNG must be 8-bit RGBA, found {:?} at {:?}",
                frame.color_type, frame.bit_depth
            ));
        }
        buffer.truncate(frame.buffer_size());
        Image::new(frame.width, frame.height, buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> Image {
        let mut pixels = Vec::new();
        for y in 0..3u8 {
            for x in 0..5u8 {
                pixels.extend_from_slice(&[x * 40, y * 80, (x + y) * 10, 255 - x]);
            }
        }
        Image::new(5, 3, pixels).unwrap()
    }

    #[test]
    fn png_round_trips() {
        let image = checker();
        let back = Image::from_png(&image.to_png().unwrap()).unwrap();
        assert_eq!(back, image);
    }

    #[test]
    fn pixel_hash_depends_on_size_and_pixels() {
        let image = checker();
        let mut other = image.clone();
        other.pixels[7] ^= 1;
        assert_ne!(image.pixel_hash(), other.pixel_hash());
        let reshaped = Image::new(3, 5, image.pixels.clone()).unwrap();
        assert_ne!(image.pixel_hash(), reshaped.pixel_hash());
    }

    #[test]
    fn rejects_wrong_buffer_length() {
        assert!(Image::new(2, 2, vec![0; 15]).is_err());
    }

    #[test]
    fn pixel_reads_rgba() {
        assert_eq!(checker().pixel(4, 2), [160, 160, 60, 251]);
    }
}
