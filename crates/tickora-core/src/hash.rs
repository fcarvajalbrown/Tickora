const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateHasher {
    value: u64,
}

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl StateHasher {
    pub fn new() -> Self {
        Self {
            value: OFFSET_BASIS,
        }
    }

    pub fn finish(&self) -> u64 {
        self.value
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.value ^= u64::from(byte);
            self.value = self.value.wrapping_mul(PRIME);
        }
    }

    pub fn write_u8(&mut self, value: u8) {
        self.write_bytes(&[value]);
    }

    pub fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    pub fn write_u32(&mut self, value: u32) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub fn write_i64(&mut self, value: i64) {
        self.write_bytes(&value.to_le_bytes());
    }

    pub fn write_len_prefixed(&mut self, bytes: &[u8]) {
        self.write_u64(bytes.len() as u64);
        self.write_bytes(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_offset_basis() {
        assert_eq!(StateHasher::new().finish(), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn matches_fnv1a_64_reference_vectors() {
        let mut a = StateHasher::new();
        a.write_bytes(b"a");
        assert_eq!(a.finish(), 0xaf63_dc4c_8601_ec8c);
        let mut foobar = StateHasher::new();
        foobar.write_bytes(b"foobar");
        assert_eq!(foobar.finish(), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn integers_are_little_endian() {
        let mut via_int = StateHasher::new();
        via_int.write_u32(0x0403_0201);
        let mut via_bytes = StateHasher::new();
        via_bytes.write_bytes(&[1, 2, 3, 4]);
        assert_eq!(via_int, via_bytes);
    }

    #[test]
    fn length_prefix_separates_adjacent_fields() {
        let mut ab_c = StateHasher::new();
        ab_c.write_len_prefixed(b"ab");
        ab_c.write_len_prefixed(b"c");
        let mut a_bc = StateHasher::new();
        a_bc.write_len_prefixed(b"a");
        a_bc.write_len_prefixed(b"bc");
        assert_ne!(ab_c.finish(), a_bc.finish());
    }
}
