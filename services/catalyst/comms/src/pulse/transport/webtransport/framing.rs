use std::collections::HashMap;

pub const STREAM_HEADER_SIZE: usize = 4;

pub const DATAGRAM_HEADER_SIZE: usize = 5;

#[allow(
    clippy::cast_possible_truncation,
    reason = "the u32 length prefix is the wire format; reliable sends are capped by max_message_bytes or the relay payload cap"
)]
pub fn stream_frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(STREAM_HEADER_SIZE + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn datagram_frame(channel_id: u8, seq: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(DATAGRAM_HEADER_SIZE + payload.len());
    out.push(channel_id);
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn parse_datagram(datagram: &[u8]) -> Option<(u8, u32, &[u8])> {
    if datagram.len() < DATAGRAM_HEADER_SIZE {
        return None;
    }
    let channel_id = datagram[0];
    let seq = u32::from_be_bytes([datagram[1], datagram[2], datagram[3], datagram[4]]);
    Some((channel_id, seq, &datagram[DATAGRAM_HEADER_SIZE..]))
}

/// A stream frame that declared a payload over the reader's cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFrameOverrun {
    pub length: u32,
    pub cap: usize,
}

impl StreamFrameOverrun {
    /// ENet's thresholds, with the cap in place of its 4096-byte receive buffer: up to twice the
    /// cap the reader skips the payload and the frame is charged to the corruption budget; over
    /// twice the cap the session ends at once.
    pub fn ends_session(&self) -> bool {
        self.length as usize > self.cap.saturating_mul(2)
    }
}

pub struct StreamFrameReader {
    max_message_len: usize,
    buffer: Vec<u8>,
    start: usize,
    end: usize,
    /// Payload bytes of a skipped frame that have not arrived yet; [`Self::append`] drops them.
    /// Non-zero only while the buffer is empty.
    skip: usize,
    /// A frame over twice the cap arrived: nothing after it is read.
    stopped: bool,
}

impl StreamFrameReader {
    pub fn new(max_message_len: usize) -> Self {
        assert!(max_message_len > 0, "max_message_len must be positive");
        Self {
            max_message_len,
            buffer: vec![0u8; STREAM_HEADER_SIZE + max_message_len],
            start: 0,
            end: 0,
            skip: 0,
            stopped: false,
        }
    }

    pub fn append(&mut self, chunk: &[u8]) {
        if self.stopped {
            return;
        }
        let skipped = self.skip.min(chunk.len());
        self.skip -= skipped;
        let chunk = &chunk[skipped..];
        self.ensure_writable(chunk.len());
        self.buffer[self.end..self.end + chunk.len()].copy_from_slice(chunk);
        self.end += chunk.len();
    }

    /// The next whole message, or the overrun of a frame over the cap. After an overrun that
    /// [`StreamFrameOverrun::ends_session`] nothing more is read; after any other the reader
    /// skips the payload and the next call reads the frame after it.
    pub fn try_read(&mut self) -> Result<Option<Vec<u8>>, StreamFrameOverrun> {
        if self.stopped || self.end - self.start < STREAM_HEADER_SIZE {
            return Ok(None);
        }
        let length = u32::from_be_bytes([
            self.buffer[self.start],
            self.buffer[self.start + 1],
            self.buffer[self.start + 2],
            self.buffer[self.start + 3],
        ]);
        if length as usize > self.max_message_len {
            let overrun = StreamFrameOverrun {
                length,
                cap: self.max_message_len,
            };
            if overrun.ends_session() {
                self.stopped = true;
                self.start = 0;
                self.end = 0;
            } else {
                // Drop the payload: what is buffered now, the rest as it arrives.
                self.start += STREAM_HEADER_SIZE;
                let buffered = (self.end - self.start).min(length as usize);
                self.start += buffered;
                self.skip = length as usize - buffered;
                if self.start == self.end {
                    self.start = 0;
                    self.end = 0;
                }
            }
            return Err(overrun);
        }
        let total = STREAM_HEADER_SIZE + length as usize;
        if self.end - self.start < total {
            return Ok(None);
        }
        let msg = self.buffer[self.start + STREAM_HEADER_SIZE..self.start + total].to_vec();
        self.start += total;
        if self.start == self.end {
            self.start = 0;
            self.end = 0;
        }
        Ok(Some(msg))
    }

    fn ensure_writable(&mut self, count: usize) {
        if self.end + count <= self.buffer.len() {
            return;
        }
        if self.start > 0 {
            let live = self.end - self.start;
            if live > 0 {
                self.buffer.copy_within(self.start..self.end, 0);
            }
            self.start = 0;
            self.end = live;
        }
        if self.end + count <= self.buffer.len() {
            return;
        }
        let mut size = self.buffer.len() * 2;
        while size < self.end + count {
            size *= 2;
        }
        self.buffer.resize(size, 0);
    }
}

#[derive(Default)]
pub struct DatagramSequencer {
    next: HashMap<u8, u32>,
}

impl DatagramSequencer {
    pub fn next(&mut self, channel_id: u8) -> u32 {
        let slot = self.next.entry(channel_id).or_insert(0);
        let seq = *slot;
        *slot = seq.wrapping_add(1);
        seq
    }
}

#[derive(Default)]
pub struct DatagramDeduper {
    last_seen: HashMap<u8, u32>,
}

impl DatagramDeduper {
    pub fn should_accept(&mut self, channel_id: u8, seq: u32) -> bool {
        match self.last_seen.get(&channel_id).copied() {
            None => {
                self.last_seen.insert(channel_id, seq);
                true
            }
            Some(last) => {
                if seq.wrapping_sub(last).cast_signed() > 0 {
                    self.last_seen.insert(channel_id, seq);
                    true
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn reader_survives_arbitrary_chunked_bytes(
            chunks in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..48), 0..48),
        ) {
            let cap = 64;
            let mut reader = StreamFrameReader::new(cap);
            for chunk in &chunks {
                reader.append(chunk);
                loop {
                    let read = reader.try_read();
                    prop_assert!(reader.start <= reader.end && reader.end <= reader.buffer.len());
                    match read {
                        Ok(Some(msg)) => prop_assert!(msg.len() <= cap, "message exceeds cap"),
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
            }
        }

        #[test]
        fn parse_datagram_survives_arbitrary_bytes(data in proptest::collection::vec(any::<u8>(), 0..64)) {
            match parse_datagram(&data) {
                Some((_, _, payload)) => prop_assert_eq!(payload.len(), data.len() - DATAGRAM_HEADER_SIZE),
                None => prop_assert!(data.len() < DATAGRAM_HEADER_SIZE),
            }
        }
    }

    #[test]
    fn stream_frame_roundtrips_through_reader_in_one_chunk() {
        let mut r = StreamFrameReader::new(4096);
        r.append(&stream_frame(b"hello"));
        assert_eq!(r.try_read().unwrap().as_deref(), Some(&b"hello"[..]));
        assert_eq!(r.try_read().unwrap(), None);
    }

    #[test]
    fn stream_reader_reassembles_across_split_chunks() {
        let framed = stream_frame(b"decentraland");
        let mut r = StreamFrameReader::new(4096);
        for (i, b) in framed.iter().enumerate() {
            r.append(std::slice::from_ref(b));
            if i + 1 < framed.len() {
                assert_eq!(r.try_read().unwrap(), None, "no full frame until last byte");
            }
        }
        assert_eq!(r.try_read().unwrap().as_deref(), Some(&b"decentraland"[..]));
    }

    #[test]
    fn stream_reader_splits_coalesced_frames() {
        let mut buf = stream_frame(b"aa");
        buf.extend_from_slice(&stream_frame(b"bbb"));
        buf.extend_from_slice(&stream_frame(b"c"));
        let mut r = StreamFrameReader::new(4096);
        r.append(&buf);
        assert_eq!(r.try_read().unwrap().as_deref(), Some(&b"aa"[..]));
        assert_eq!(r.try_read().unwrap().as_deref(), Some(&b"bbb"[..]));
        assert_eq!(r.try_read().unwrap().as_deref(), Some(&b"c"[..]));
        assert_eq!(r.try_read().unwrap(), None);
    }

    #[test]
    fn stream_reader_handles_empty_frame() {
        let mut r = StreamFrameReader::new(16);
        r.append(&stream_frame(b""));
        assert_eq!(r.try_read().unwrap().as_deref(), Some(&b""[..]));
    }

    #[test]
    fn stream_reader_skips_a_frame_up_to_twice_the_cap_across_chunks() {
        let mut r = StreamFrameReader::new(8);
        let mut bytes = stream_frame(&[0xff; 16]);
        bytes.extend_from_slice(&stream_frame(b"ok"));
        // The header and 3 payload bytes, then the rest of the payload with the next frame.
        let (head, tail) = bytes.split_at(7);
        r.append(head);
        assert_eq!(
            r.try_read().unwrap_err(),
            StreamFrameOverrun { length: 16, cap: 8 }
        );
        assert_eq!(r.try_read().unwrap(), None);
        r.append(tail);
        assert_eq!(r.try_read().unwrap().as_deref(), Some(&b"ok"[..]));
    }

    #[test]
    fn stream_reader_reads_nothing_after_a_frame_over_twice_the_cap() {
        let mut r = StreamFrameReader::new(8);
        r.append(&stream_frame(&[0u8; 17]));
        assert_eq!(
            r.try_read().unwrap_err(),
            StreamFrameOverrun { length: 17, cap: 8 }
        );
        r.append(&stream_frame(b"ok"));
        assert_eq!(r.try_read().unwrap(), None);
    }

    #[test]
    fn stream_reader_grows_beyond_initial_capacity() {
        let mut r = StreamFrameReader::new(64);
        let big = vec![7u8; 40];
        r.append(&stream_frame(&big));
        r.append(&stream_frame(&big));
        assert_eq!(r.try_read().unwrap().unwrap().len(), 40);
        assert_eq!(r.try_read().unwrap().unwrap().len(), 40);
    }

    #[test]
    fn datagram_frame_parses_back() {
        let d = datagram_frame(1, 0xDEAD_BEEF, b"pos");
        assert_eq!(d.len(), DATAGRAM_HEADER_SIZE + 3);
        let (ch, seq, payload) = parse_datagram(&d).unwrap();
        assert_eq!(ch, 1);
        assert_eq!(seq, 0xDEAD_BEEF);
        assert_eq!(payload, b"pos");
    }

    #[test]
    fn parse_datagram_rejects_short_header() {
        assert!(parse_datagram(&[1, 2, 3, 4]).is_none());
        assert!(parse_datagram(&[1, 0, 0, 0, 0]).is_some());
    }

    #[test]
    fn sequencer_is_monotonic_per_channel() {
        let mut s = DatagramSequencer::default();
        assert_eq!(s.next(1), 0);
        assert_eq!(s.next(1), 1);
        assert_eq!(s.next(2), 0);
        assert_eq!(s.next(1), 2);
    }

    #[test]
    fn deduper_accepts_newer_drops_stale_and_duplicate() {
        let mut d = DatagramDeduper::default();
        assert!(d.should_accept(1, 5));
        assert!(!d.should_accept(1, 5));
        assert!(!d.should_accept(1, 4));
        assert!(d.should_accept(1, 6));
        assert!(!d.should_accept(1, 6));
    }

    #[test]
    fn deduper_is_independent_per_channel() {
        let mut d = DatagramDeduper::default();
        assert!(d.should_accept(1, 100));
        assert!(d.should_accept(2, 1));
        assert!(!d.should_accept(2, 1));
    }

    #[test]
    fn deduper_survives_u32_wraparound() {
        let mut d = DatagramDeduper::default();
        assert!(d.should_accept(1, u32::MAX - 1));
        assert!(d.should_accept(1, u32::MAX));
        assert!(d.should_accept(1, 0));
        assert!(d.should_accept(1, 1));
        assert!(!d.should_accept(1, u32::MAX));
    }
}
