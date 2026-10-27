use std::fmt;

const MAGIC: [u8; 4] = *b"TKIL";
const VERSION: u8 = 1;
const HEADER_LEN: usize = MAGIC.len() + 1 + 8;

pub trait InputCodec: Sized {
    fn encode(&self, out: &mut Vec<u8>);
    fn decode(bytes: &[u8]) -> Option<Self>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputEvent<I> {
    pub tick: u64,
    pub input: I,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputLog<I> {
    events: Vec<InputEvent<I>>,
}

impl<I> Default for InputLog<I> {
    fn default() -> Self {
        Self { events: Vec::new() }
    }
}

impl<I> InputLog<I> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn events(&self) -> &[InputEvent<I>] {
        &self.events
    }

    pub fn push(&mut self, tick: u64, input: I) {
        let at = self.events.partition_point(|event| event.tick <= tick);
        self.events.insert(at, InputEvent { tick, input });
    }

    pub fn at(&self, tick: u64) -> &[InputEvent<I>] {
        let start = self.events.partition_point(|event| event.tick < tick);
        let end = self.events.partition_point(|event| event.tick <= tick);
        &self.events[start..end]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputLogError {
    BadMagic,
    UnsupportedVersion(u8),
    Truncated,
    TrailingBytes,
    UnorderedTicks,
    BadPayload { tick: u64 },
}

impl fmt::Display for InputLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "input log has the wrong magic bytes"),
            Self::UnsupportedVersion(version) => {
                write!(f, "input log version {version} is not supported")
            }
            Self::Truncated => write!(f, "input log ends early"),
            Self::TrailingBytes => write!(f, "input log has bytes after the last event"),
            Self::UnorderedTicks => write!(f, "input log ticks are not in order"),
            Self::BadPayload { tick } => {
                write!(f, "input payload at tick {tick} cannot be decoded")
            }
        }
    }
}

impl std::error::Error for InputLogError {}

impl<I: InputCodec> InputLog<I> {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.events.len() * 16);
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&(self.events.len() as u64).to_le_bytes());
        let mut payload = Vec::new();
        for event in &self.events {
            payload.clear();
            event.input.encode(&mut payload);
            out.extend_from_slice(&event.tick.to_le_bytes());
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(&payload);
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InputLogError> {
        let mut reader = Reader { bytes };
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(InputLogError::BadMagic);
        }
        let version = reader.take(1)?[0];
        if version != VERSION {
            return Err(InputLogError::UnsupportedVersion(version));
        }
        let count = reader.read_u64()?;
        let mut events: Vec<InputEvent<I>> = Vec::new();
        for _ in 0..count {
            let tick = reader.read_u64()?;
            let len = reader.read_u32()? as usize;
            let payload = reader.take(len)?;
            if events.last().is_some_and(|last| last.tick > tick) {
                return Err(InputLogError::UnorderedTicks);
            }
            let input = I::decode(payload).ok_or(InputLogError::BadPayload { tick })?;
            events.push(InputEvent { tick, input });
        }
        if !reader.bytes.is_empty() {
            return Err(InputLogError::TrailingBytes);
        }
        Ok(Self { events })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], InputLogError> {
        if self.bytes.len() < len {
            return Err(InputLogError::Truncated);
        }
        let (head, rest) = self.bytes.split_at(len);
        self.bytes = rest;
        Ok(head)
    }

    fn read_u32(&mut self) -> Result<u32, InputLogError> {
        let raw = self.take(4)?;
        Ok(u32::from_le_bytes(raw.try_into().expect("length checked")))
    }

    fn read_u64(&mut self) -> Result<u64, InputLogError> {
        let raw = self.take(8)?;
        Ok(u64::from_le_bytes(raw.try_into().expect("length checked")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl InputCodec for u8 {
        fn encode(&self, out: &mut Vec<u8>) {
            out.push(*self);
        }

        fn decode(bytes: &[u8]) -> Option<Self> {
            match bytes {
                [value] => Some(*value),
                _ => None,
            }
        }
    }

    #[test]
    fn push_keeps_tick_order_and_insertion_order_within_a_tick() {
        let mut log = InputLog::new();
        log.push(5, 1u8);
        log.push(2, 2u8);
        log.push(5, 3u8);
        log.push(2, 4u8);
        let order: Vec<(u64, u8)> = log.events().iter().map(|e| (e.tick, e.input)).collect();
        assert_eq!(order, [(2, 2), (2, 4), (5, 1), (5, 3)]);
    }

    #[test]
    fn at_returns_only_that_tick() {
        let mut log = InputLog::new();
        log.push(1, 10u8);
        log.push(3, 30u8);
        log.push(3, 31u8);
        assert!(log.at(0).is_empty());
        assert_eq!(log.at(3).len(), 2);
        assert!(log.at(2).is_empty());
        assert!(log.at(4).is_empty());
    }

    #[test]
    fn bytes_round_trip() {
        let mut log = InputLog::new();
        log.push(0, 9u8);
        log.push(7, 8u8);
        let decoded = InputLog::<u8>::from_bytes(&log.to_bytes()).unwrap();
        assert_eq!(decoded, log);
    }

    #[test]
    fn layout_is_fixed_little_endian() {
        let mut log = InputLog::new();
        log.push(0x0102, 0xAAu8);
        assert_eq!(
            log.to_bytes(),
            [
                b'T', b'K', b'I', b'L', 1, 1, 0, 0, 0, 0, 0, 0, 0, 0x02, 0x01, 0, 0, 0, 0, 0, 0, 1,
                0, 0, 0, 0xAA
            ]
        );
    }

    #[test]
    fn rejects_malformed_input() {
        let mut log = InputLog::new();
        log.push(1, 1u8);
        let good = log.to_bytes();

        let mut bad_magic = good.clone();
        bad_magic[0] = b'X';
        assert_eq!(
            InputLog::<u8>::from_bytes(&bad_magic),
            Err(InputLogError::BadMagic)
        );

        let mut bad_version = good.clone();
        bad_version[4] = 9;
        assert_eq!(
            InputLog::<u8>::from_bytes(&bad_version),
            Err(InputLogError::UnsupportedVersion(9))
        );

        assert_eq!(
            InputLog::<u8>::from_bytes(&good[..good.len() - 1]),
            Err(InputLogError::Truncated)
        );

        let mut trailing = good.clone();
        trailing.push(0);
        assert_eq!(
            InputLog::<u8>::from_bytes(&trailing),
            Err(InputLogError::TrailingBytes)
        );

        let mut bad_payload = good.clone();
        bad_payload[HEADER_LEN + 8] = 2;
        bad_payload.push(0);
        bad_payload.push(0);
        assert_eq!(
            InputLog::<u8>::from_bytes(&bad_payload),
            Err(InputLogError::BadPayload { tick: 1 })
        );
    }

    #[test]
    fn rejects_unordered_ticks() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"TKIL");
        bytes.push(1);
        bytes.extend_from_slice(&2u64.to_le_bytes());
        for tick in [5u64, 3u64] {
            bytes.extend_from_slice(&tick.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.push(0);
        }
        assert_eq!(
            InputLog::<u8>::from_bytes(&bytes),
            Err(InputLogError::UnorderedTicks)
        );
    }
}
