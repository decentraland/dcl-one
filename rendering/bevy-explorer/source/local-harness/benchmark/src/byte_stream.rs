//! The bytes of one stream (csv or log), appended at the offsets the page names: an offset equal
//! to the length appends, bytes already stored are not appended again, a gap is refused, and a
//! file never passes its cap.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Body;
use tokio::sync::Mutex;

use crate::limits::BODY_IDLE;

pub struct ByteStream {
    path: PathBuf,
    max_bytes: u64,
    state: Mutex<State>,
}

struct State {
    file: Option<File>,
    length: u64,
    ended: bool,
    duplicates: u32,
    refused: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appended {
    Stored,
    /// The request starts past the stored length.
    Gap,
    /// The request's bytes that overlap the stored ones differ from them.
    Mismatch,
    /// `end` came already.
    Ended,
    TooLarge,
}

#[derive(Debug)]
pub enum AppendError {
    Disk(io::Error),
    /// The client dropped the body, or sent less than it announced.
    Body(String),
    /// The client stopped sending for too long.
    Idle,
}

/// What a stream holds when the run is over.
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamSummary {
    pub created: bool,
    pub length: u64,
    pub duplicates: u32,
    pub refused: u32,
}

impl ByteStream {
    pub fn new(path: PathBuf, max_bytes: u64) -> Self {
        Self {
            path,
            max_bytes,
            state: Mutex::new(State {
                file: None,
                length: 0,
                ended: false,
                duplicates: 0,
                refused: 0,
            }),
        }
    }

    /// The request's `length` bytes, read once and in order as they arrive: those that overlap the
    /// stored bytes are compared with them and the rest go to the file, so nothing but one piece
    /// of the body is in memory. A body that ends early leaves what arrived stored (the page then
    /// sends the same offset again and the overlap is compared, as with any retry). The file
    /// is written with no await between a write and the length that counts it, so a request that
    /// is dropped leaves the file and the length in step.
    pub async fn append<B>(
        &self,
        offset: u64,
        body: &mut B,
        length: u64,
    ) -> Result<Appended, AppendError>
    where
        B: Body<Data = Bytes> + Unpin,
        B::Error: std::fmt::Display,
    {
        let mut state = self.state.lock().await;
        if state.ended {
            return Ok(Appended::Ended);
        }

        if offset > state.length {
            state.refused += 1;
            return Ok(Appended::Gap);
        }

        // Bytes the file already holds. They must be the page's bytes again; anything else is a
        // page and a harness that disagree.
        let overlap = (state.length - offset).min(length);
        let fresh = length - overlap;

        if fresh > 0
            && state
                .length
                .checked_add(fresh)
                .is_none_or(|total| total > self.max_bytes)
        {
            state.refused += 1;
            return Ok(Appended::TooLarge);
        }

        // The first byte creates the file: a run that streams no CSV leaves no CSV, which the
        // report counts as a crashed run.
        if fresh > 0 && state.file.is_none() {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&self.path)
                .map_err(AppendError::Disk)?;
            state.file = Some(file);
        }

        let mut position = 0u64;
        while position < length {
            let piece = next_piece(body).await?;
            let mut piece = &piece[..piece.len().min((length - position) as usize)];

            while !piece.is_empty() {
                if position < overlap {
                    let count = piece.len().min((overlap - position) as usize);
                    let same = state
                        .holds(offset + position, &piece[..count])
                        .map_err(AppendError::Disk)?;
                    if !same {
                        state.refused += 1;
                        return Ok(Appended::Mismatch);
                    }
                    piece = &piece[count..];
                    position += count as u64;
                } else {
                    state.write(piece).map_err(AppendError::Disk)?;
                    position += piece.len() as u64;
                    piece = &[];
                }
            }
        }

        if fresh == 0 && length > 0 {
            state.duplicates += 1;
        }
        Ok(Appended::Stored)
    }

    pub async fn length(&self) -> u64 {
        self.state.lock().await.length
    }

    /// After `end` nothing more is accepted.
    pub async fn close(&self) {
        let mut state = self.state.lock().await;
        state.ended = true;
        state.file = None;
    }

    pub async fn summary(&self) -> StreamSummary {
        let state = self.state.lock().await;
        StreamSummary {
            created: state.file.is_some() || state.length > 0,
            length: state.length,
            duplicates: state.duplicates,
            refused: state.refused,
        }
    }
}

impl State {
    /// Whether the stored bytes from `at` are `bytes`.
    fn holds(&mut self, at: u64, bytes: &[u8]) -> io::Result<bool> {
        let file = self.file.as_mut().expect("overlap means stored bytes");
        let mut stored = vec![0u8; bytes.len()];
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut stored)?;
        Ok(stored == bytes)
    }

    /// Appends at the stored length. A write that fails part-way is cut off, so the file never
    /// holds bytes the length does not count.
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let length = self.length;
        let file = self.file.as_mut().expect("fresh bytes have a file");
        let written = file
            .seek(SeekFrom::Start(length))
            .and_then(|_| file.write_all(bytes));
        match written {
            Ok(()) => {
                self.length += bytes.len() as u64;
                Ok(())
            }
            Err(error) => {
                let _ = file.set_len(length);
                Err(error)
            }
        }
    }
}

/// The next bytes of the body; trailers are skipped.
async fn next_piece<B>(body: &mut B) -> Result<Bytes, AppendError>
where
    B: Body<Data = Bytes> + Unpin,
    B::Error: std::fmt::Display,
{
    loop {
        match tokio::time::timeout(BODY_IDLE, body.frame()).await {
            Err(_) => return Err(AppendError::Idle),
            Ok(None) => return Err(AppendError::Body("the body ended before its length".into())),
            Ok(Some(Err(error))) => return Err(AppendError::Body(error.to_string())),
            Ok(Some(Ok(frame))) => {
                if let Ok(data) = frame.into_data() {
                    return Ok(data);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Full;

    fn scratch(name: &str) -> PathBuf {
        crate::scratch("byte-stream", name).join("file.bin")
    }

    async fn append(stream: &ByteStream, offset: u64, bytes: &[u8]) -> Appended {
        let mut body = Full::new(Bytes::copy_from_slice(bytes));
        stream
            .append(offset, &mut body, bytes.len() as u64)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn appends_at_the_length_and_not_twice() {
        let path = scratch("appends.bin");
        let stream = ByteStream::new(path.clone(), 1024);
        assert_eq!(append(&stream, 0, b"abc").await, Appended::Stored);
        assert_eq!(append(&stream, 0, b"abc").await, Appended::Stored);
        assert_eq!(append(&stream, 3, b"def").await, Appended::Stored);
        assert_eq!(append(&stream, 4, b"efgh").await, Appended::Stored);
        let summary = stream.summary().await;
        assert_eq!(
            (summary.length, summary.duplicates, summary.refused),
            (8, 1, 0)
        );
        stream.close().await;
        assert_eq!(std::fs::read(path).unwrap(), b"abcdefgh");
    }

    #[tokio::test]
    async fn refuses_a_gap_and_an_overlap_that_differs() {
        let stream = ByteStream::new(scratch("refuses.bin"), 1024);
        assert_eq!(append(&stream, 0, b"abcdef").await, Appended::Stored);
        assert_eq!(append(&stream, 10, b"x").await, Appended::Gap);
        assert_eq!(append(&stream, 2, b"zz").await, Appended::Mismatch);
        assert_eq!(append(&stream, 5, b"fg").await, Appended::Stored);
        assert_eq!(stream.length().await, 7);
        assert_eq!(stream.summary().await.refused, 2);
    }

    #[tokio::test]
    async fn a_file_never_passes_its_cap_but_repeats_are_still_acknowledged() {
        let stream = ByteStream::new(scratch("cap.bin"), 8);
        assert_eq!(append(&stream, 0, b"12345678").await, Appended::Stored);
        assert_eq!(append(&stream, 8, b"9").await, Appended::TooLarge);
        assert_eq!(append(&stream, 4, b"5678").await, Appended::Stored);
        assert_eq!(stream.length().await, 8);
    }

    #[tokio::test]
    async fn nothing_is_accepted_after_the_end() {
        let stream = ByteStream::new(scratch("ended.bin"), 8);
        assert_eq!(append(&stream, 0, b"ab").await, Appended::Stored);
        stream.close().await;
        assert_eq!(append(&stream, 2, b"cd").await, Appended::Ended);
        assert_eq!(append(&stream, 0, b"ab").await, Appended::Ended);
    }

    #[tokio::test]
    async fn no_file_exists_before_the_first_byte() {
        let path = scratch("none.bin");
        let _ = std::fs::remove_file(&path);
        let stream = ByteStream::new(path.clone(), 8);
        assert_eq!(append(&stream, 0, b"").await, Appended::Stored);
        assert!(!path.exists());
        assert!(!stream.summary().await.created);
        assert_eq!(append(&stream, 0, b"a").await, Appended::Stored);
        assert!(path.exists());
        assert!(stream.summary().await.created);
    }

    #[tokio::test]
    async fn a_body_that_ends_early_keeps_what_arrived() {
        let stream = ByteStream::new(scratch("early.bin"), 64);
        let mut body = Full::new(Bytes::from_static(b"abc"));
        let result = stream.append(0, &mut body, 10).await;
        assert!(matches!(result, Err(AppendError::Body(_))));
        assert_eq!(stream.length().await, 3);
        assert_eq!(append(&stream, 0, b"abcdefghij").await, Appended::Stored);
        assert_eq!(stream.length().await, 10);
    }
}
