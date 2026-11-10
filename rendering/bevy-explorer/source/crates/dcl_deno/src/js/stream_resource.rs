use crate::js::errors::{type_error, OpError};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::{poll_fn, Future},
    rc::Rc,
    task::{Context, Poll, Waker},
};

use deno_core::{
    error::AnyError, op2, unsync::TaskQueue, AsyncResult, BufView, CancelFuture, CancelHandle,
    JsBuffer, OpDecl, OpState, RcRef, Resource, ResourceId,
};
use deno_error::JsErrorBox;

// Replaces deno_web's ReadableStream resource ops (the producer side of a fetch upload).
// deno_web hands JS a raw pointer to the channel and dereferences whatever pointer comes
// back; here the producer holds a resource id instead, so a wrong, stale or repeated handle
// is a resource-table miss.
pub fn override_ops() -> Vec<OpDecl> {
    vec![
        op_readable_stream_resource_allocate(),
        op_readable_stream_resource_allocate_sized(),
        op_readable_stream_resource_get_sink(),
        op_readable_stream_resource_write_buf(),
        op_readable_stream_resource_write_sync(),
        op_readable_stream_resource_write_error(),
        op_readable_stream_resource_close(),
        op_readable_stream_resource_await_close(),
        op_read_all(),
    ]
}

const MAX_BUFFERED_CHUNKS: usize = 1024;
const MAX_BUFFERED_BYTES: usize = 64 * 1024;
// Streams and their sinks a runtime may hold. A reader preallocates at most 64 KiB per
// stream; `op_read_all` keeps what it reads until the stream ends, at most
// MAX_READ_ALL_BYTES for a stream a scene feeds.
const MAX_OPEN_STREAMS: usize = 256;
// a stream's producer is scene code, so reading one to its end stops here
const MAX_READ_ALL_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
struct Channel {
    chunks: VecDeque<BufView>,
    size: usize,
    error: Option<JsErrorBox>,
    closed: bool,
    read_waker: Option<Waker>,
    write_waker: Option<Waker>,
}

impl Channel {
    fn can_write(&self) -> bool {
        self.closed
            || self.error.is_some()
            || (self.chunks.len() < MAX_BUFFERED_CHUNKS && self.size < MAX_BUFFERED_BYTES)
    }

    fn can_read(&self) -> bool {
        self.closed || self.error.is_some() || !self.chunks.is_empty()
    }

    fn poll_write_ready(&mut self, cx: &mut Context) -> Poll<()> {
        if self.can_write() {
            Poll::Ready(())
        } else {
            self.write_waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    fn poll_read_ready(&mut self, cx: &mut Context) -> Poll<()> {
        if self.can_read() {
            Poll::Ready(())
        } else {
            self.read_waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    /// Returns 0 if closed, 1 if written, 2 if full.
    fn write(&mut self, buffer: JsBuffer) -> u32 {
        if self.closed {
            return 0;
        }
        if !self.can_write() {
            return 2;
        }
        // writes after an error are dropped, as the reader only sees the error
        if self.error.is_none() {
            self.size += buffer.len();
            self.chunks.push_back(BufView::from(buffer));
            if let Some(waker) = self.read_waker.take() {
                waker.wake();
            }
        }
        1
    }

    fn write_error(&mut self, error: JsErrorBox) {
        self.error = Some(error);
        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
    }

    fn close(&mut self) {
        self.closed = true;
        if let Some(waker) = self.write_waker.take() {
            waker.wake();
        }
        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
    }

    fn read(&mut self, limit: usize) -> Result<BufView, JsErrorBox> {
        let Some(mut chunk) = self.chunks.pop_front() else {
            return match self.error.take() {
                Some(error) => Err(error),
                None => Ok(BufView::empty()),
            };
        };
        if chunk.len() > limit {
            let head = chunk.split_to(limit.max(1));
            self.chunks.push_front(chunk);
            chunk = head;
        }
        self.size -= chunk.len();
        if self.can_write() {
            if let Some(waker) = self.write_waker.take() {
                waker.wake();
            }
        }
        Ok(chunk)
    }
}

#[derive(Default)]
struct Completion {
    complete: bool,
    waker: Option<Waker>,
}

/// Counts a runtime's live stream resources and sinks.
#[derive(Clone, Default)]
struct OpenStreams(Rc<Cell<usize>>);

struct OpenStream(OpenStreams);

impl OpenStream {
    fn take(state: &mut OpState) -> Result<Self, AnyError> {
        if !state.has::<OpenStreams>() {
            state.put(OpenStreams::default());
        }
        let open = state.borrow::<OpenStreams>().clone();
        if open.0.get() >= MAX_OPEN_STREAMS {
            return Err(type_error("too many open streams"));
        }
        open.0.set(open.0.get() + 1);
        Ok(Self(open))
    }
}

impl Drop for OpenStream {
    fn drop(&mut self) {
        let count = &self.0 .0;
        count.set(count.get() - 1);
    }
}

struct ReadableStreamResource {
    channel: Rc<RefCell<Channel>>,
    read_queue: Rc<TaskQueue>,
    cancel_handle: CancelHandle,
    completion: Rc<RefCell<Completion>>,
    length: Option<u64>,
    _open: OpenStream,
}

impl ReadableStreamResource {
    fn add(state: &mut OpState, length: Option<u64>) -> Result<ResourceId, AnyError> {
        let open = OpenStream::take(state)?;
        Ok(state.resource_table.add(Self {
            channel: Default::default(),
            read_queue: Default::default(),
            cancel_handle: Default::default(),
            completion: Default::default(),
            length,
            _open: open,
        }))
    }

    async fn read(self: Rc<Self>, limit: usize) -> Result<BufView, JsErrorBox> {
        let cancel_handle = RcRef::map(&self, |r| &r.cancel_handle);
        let _permit = self.read_queue.acquire().await;
        poll_fn(|cx| self.channel.borrow_mut().poll_read_ready(cx))
            .or_cancel(cancel_handle)
            .await
            .map_err(JsErrorBox::from_err)?;
        self.channel.borrow_mut().read(limit)
    }

    fn close_channel(&self) {
        // resolves op_readable_stream_resource_await_close, which cancels the JS reader
        let waker = {
            let mut completion = self.completion.borrow_mut();
            completion.complete = true;
            completion.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
        self.cancel_handle.cancel();
        self.channel.borrow_mut().close();
    }
}

impl Resource for ReadableStreamResource {
    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed("readableStream")
    }

    fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
        Box::pin(ReadableStreamResource::read(self, limit))
    }

    fn close(self: Rc<Self>) {
        self.close_channel();
    }

    // only an upper bound: an exact hint makes `op_read_all` preallocate the whole length
    fn size_hint(&self) -> (u64, Option<u64>) {
        (0, self.length)
    }
}

impl Drop for ReadableStreamResource {
    fn drop(&mut self) {
        self.close_channel();
    }
}

/// The producer's handle on a stream's channel.
struct ReadableStreamSink {
    channel: Rc<RefCell<Channel>>,
    _open: OpenStream,
}

impl Resource for ReadableStreamSink {
    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed("readableStreamSink")
    }
}

impl Drop for ReadableStreamSink {
    fn drop(&mut self) {
        self.channel.borrow_mut().close();
    }
}

fn sink_channel(state: &OpState, sink: ResourceId) -> Result<Rc<RefCell<Channel>>, AnyError> {
    Ok(state
        .resource_table
        .get::<ReadableStreamSink>(sink)?
        .channel
        .clone())
}

#[op2(fast)]
#[smi]
fn op_readable_stream_resource_allocate(state: &mut OpState) -> Result<ResourceId, OpError> {
    Ok(ReadableStreamResource::add(state, None)?)
}

#[op2(fast)]
#[smi]
fn op_readable_stream_resource_allocate_sized(
    state: &mut OpState,
    #[number] length: u64,
) -> Result<ResourceId, OpError> {
    Ok(ReadableStreamResource::add(state, Some(length))?)
}

#[op2(fast)]
#[smi]
fn op_readable_stream_resource_get_sink(
    state: &mut OpState,
    #[smi] rid: ResourceId,
) -> Result<ResourceId, OpError> {
    let channel = state
        .resource_table
        .get::<ReadableStreamResource>(rid)?
        .channel
        .clone();
    let open = OpenStream::take(state)?;
    Ok(state.resource_table.add(ReadableStreamSink {
        channel,
        _open: open,
    }))
}

#[op2]
async fn op_readable_stream_resource_write_buf(
    state: Rc<RefCell<OpState>>,
    #[smi] sink: ResourceId,
    #[buffer] buffer: JsBuffer,
) -> Result<bool, OpError> {
    let channel = sink_channel(&state.borrow(), sink)?;
    poll_fn(|cx| channel.borrow_mut().poll_write_ready(cx)).await;
    let mut channel = channel.borrow_mut();
    // can_write held when the poll returned and nothing ran in between
    channel.write(buffer);
    Ok(!channel.closed)
}

/// Returns 0 if the channel was closed, 1 if the write succeeded, 2 if the channel is full
/// and the caller must wait with `op_readable_stream_resource_write_buf`.
#[op2]
fn op_readable_stream_resource_write_sync(
    state: &mut OpState,
    #[smi] sink: ResourceId,
    #[buffer] buffer: JsBuffer,
) -> Result<u32, OpError> {
    Ok(sink_channel(state, sink)?.borrow_mut().write(buffer))
}

#[op2(fast)]
fn op_readable_stream_resource_write_error(
    state: &mut OpState,
    #[smi] sink: ResourceId,
    #[string] error: String,
) -> Result<bool, OpError> {
    let channel = sink_channel(state, sink)?;
    let mut channel = channel.borrow_mut();
    channel.write_error(JsErrorBox::type_error(error));
    Ok(!channel.closed)
}

#[op2(fast)]
fn op_readable_stream_resource_close(
    state: &mut OpState,
    #[smi] sink: ResourceId,
) -> Result<(), OpError> {
    // dropping the sink closes the channel
    state.resource_table.take::<ReadableStreamSink>(sink)?;
    Ok(())
}

#[op2]
fn op_readable_stream_resource_await_close(
    state: &mut OpState,
    #[smi] rid: ResourceId,
) -> impl Future<Output = ()> {
    let completion = state
        .resource_table
        .get::<ReadableStreamResource>(rid)
        .ok()
        .map(|r| r.completion.clone());
    poll_fn(move |cx| {
        let Some(completion) = &completion else {
            return Poll::Ready(());
        };
        let mut completion = completion.borrow_mut();
        if completion.complete {
            Poll::Ready(())
        } else {
            completion.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    })
}

/// Takes the stream the fetch JS made for an upload body; any other resource is refused.
pub(crate) fn take_upload(
    state: &mut OpState,
    rid: ResourceId,
) -> Result<Rc<dyn Resource>, AnyError> {
    Ok(state.resource_table.take::<ReadableStreamResource>(rid)?)
}

/// deno_core's `op_read_all`, stopping a scene-fed stream at [`MAX_READ_ALL_BYTES`]. Other
/// resources it reads, such as fetch response bodies, are already in memory.
#[op2]
#[buffer]
async fn op_read_all(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
) -> Result<Vec<u8>, OpError> {
    let (resource, cap) = {
        let state = state.borrow();
        let cap = match state.resource_table.get::<ReadableStreamResource>(rid) {
            Ok(_) => MAX_READ_ALL_BYTES,
            Err(_) => usize::MAX,
        };
        (state.resource_table.get_any(rid)?, cap)
    };
    let mut out = Vec::new();
    loop {
        let chunk = resource.clone().read(64 * 1024).await?;
        if chunk.is_empty() {
            return Ok(out);
        }
        if out.len().saturating_add(chunk.len()) > cap {
            return Err(type_error(format!("stream is larger than {cap} bytes")).into());
        }
        out.extend_from_slice(&chunk);
    }
}
