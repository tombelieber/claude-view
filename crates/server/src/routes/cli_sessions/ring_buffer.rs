/// Fixed-capacity circular buffer for terminal scrollback.
///
/// Stores the most recent `capacity` bytes. Older data is silently
/// overwritten. Used for:
/// 1. Reconnection scrollback replay (new WS client sees recent output)
/// 2. Broadcast lag re-sync (slow client catches up via snapshot)
pub struct RingBuffer {
    buf: Vec<u8>,
    head: usize,
    len: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0u8; capacity],
            head: 0,
            len: 0,
        }
    }

    /// Append bytes. If data exceeds remaining capacity, oldest bytes
    /// are silently overwritten.
    pub fn write(&mut self, data: &[u8]) {
        let cap = self.buf.len();
        if cap == 0 {
            return;
        }

        // If data is larger than capacity, only keep the tail.
        let data = if data.len() > cap {
            &data[data.len() - cap..]
        } else {
            data
        };

        for &byte in data {
            self.buf[self.head] = byte;
            self.head = (self.head + 1) % cap;
        }
        self.len = (self.len + data.len()).min(cap);
    }

    /// Read the buffer contents in chronological order.
    pub fn as_bytes(&self) -> Vec<u8> {
        if self.len == 0 {
            return Vec::new();
        }
        let cap = self.buf.len();
        if self.len < cap {
            self.buf[..self.head].to_vec()
        } else {
            let mut out = Vec::with_capacity(cap);
            out.extend_from_slice(&self.buf[self.head..]);
            out.extend_from_slice(&self.buf[..self.head]);
            out
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_returns_empty() {
        let buf = RingBuffer::new(64);
        assert!(buf.as_bytes().is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn write_within_capacity() {
        let mut buf = RingBuffer::new(16);
        buf.write(b"hello");
        assert_eq!(buf.as_bytes(), b"hello");
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn write_wraps_around() {
        let mut buf = RingBuffer::new(8);
        buf.write(b"abcdefgh"); // fills exactly
        buf.write(b"XY"); // overwrites first 2
        assert_eq!(buf.as_bytes(), b"cdefghXY");
        assert_eq!(buf.len(), 8);
    }

    #[test]
    fn single_write_exceeding_capacity_keeps_tail() {
        let mut buf = RingBuffer::new(4);
        buf.write(b"abcdefgh");
        assert_eq!(buf.as_bytes(), b"efgh");
        assert_eq!(buf.len(), 4);
    }

    #[test]
    fn len_tracks_used_bytes_capped_at_capacity() {
        let mut buf = RingBuffer::new(16);
        assert_eq!(buf.len(), 0);
        buf.write(b"abc");
        assert_eq!(buf.len(), 3);
        buf.write(b"defghijklmnopqrs"); // 16 bytes, wraps
        assert_eq!(buf.len(), 16); // capped at capacity
    }

    #[test]
    fn zero_capacity_is_noop() {
        let mut buf = RingBuffer::new(0);
        buf.write(b"hello");
        assert!(buf.as_bytes().is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn multiple_small_writes_accumulate() {
        let mut buf = RingBuffer::new(16);
        buf.write(b"aaa");
        buf.write(b"bbb");
        buf.write(b"ccc");
        assert_eq!(buf.as_bytes(), b"aaabbbccc");
        assert_eq!(buf.len(), 9);
    }
}
