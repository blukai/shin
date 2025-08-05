use std::time::Instant;

use crate::Key;

struct Read {
    key: Key,
    frame_start: Instant,
    payload: Option<anyhow::Result<String>>,
}

struct Write {
    frame_start: Instant,
    payload: String,
}

/// reads are lagged 1 frame behind:
/// - ui requests a read at frame 1;
/// - event loop fulfills it at the end of frame 1;
/// - ui may consume the read at frame 2.
///
/// writes are immediate:
/// - ui requests a write at frame 1;
/// - event loop fulfills it at the end of frame 1.
#[derive(Default)]
pub struct ClipboardService {
    frame_start: Option<Instant>,
    read: Option<Read>,
    write: Option<Write>,
}

impl ClipboardService {
    pub fn begin_frame(&mut self, frame_start: Instant) {
        self.frame_start = Some(frame_start);
    }

    pub fn end_frame(&mut self) {
        let frame_start = self.frame_start.take().expect("didn't begin frame");

        // NOTE: clean up request older than current frame (orphaned or unconsumed).
        self.read
            .take_if(|r| r.frame_start < frame_start)
            .inspect(|cr| log::debug!("[clipboard] evict read (key {:?})", cr.key));

        // NOTE: clean up request older than current frame (unconsumed).
        self.write
            .take_if(|w| w.frame_start < frame_start)
            .inspect(|_| log::debug!("[clipboard] evict write"));
    }

    /// widget requests clipboard read.
    ///
    /// it will only be possible to consume clipboard read next frame.
    pub fn request_read(&mut self, key: Key) {
        log::debug!("[clipboard] request read (key {key:?})");
        let frame_start = self.frame_start.expect("didn't begin frame");
        self.read = Some(Read {
            key,
            frame_start,
            payload: None,
        });
    }

    /// event loop needs to fulfill clipboard read request at the end of the frame at which the
    /// read was requested so that widget(the requester) can take(/consume) the result next frame.
    pub fn is_awaiting_read(&mut self) -> bool {
        self.read.as_ref().is_some_and(|r| r.payload.is_none())
    }

    /// [`Self::fulfill_read`] must be called only if [`Self::is_awaiting_read`] returned true.
    pub fn fulfill_read(&mut self, payload: anyhow::Result<String>) {
        assert!(self.is_awaiting_read());
        let Some(ref mut r) = self.read else {
            unreachable!();
        };
        r.payload = Some(payload);
    }

    /// widget(/the requester) takes(/consumes) clipboard read.
    pub fn try_take_read(&mut self, key: Key) -> Option<String> {
        self.read
            .take_if(|r| r.key == key && r.payload.is_some())
            .and_then(|r| match r.payload {
                Some(Ok(payload)) => {
                    log::debug!("[clipboard] took successful read ({key:?})");
                    Some(payload)
                }
                Some(Err(err)) => {
                    // TODO: do i need to be somehow more elaborate with handling this error? this
                    // semi-silent approach is probably ok.
                    log::error!("[clipboard] took (but sort of ignored) failed read: {err:?}");
                    None
                }
                None => unreachable!(),
            })
    }

    /// widget requests clipboard write.
    pub fn request_write(&mut self, payload: String) {
        log::debug!("[clipboard] request write (text {payload})");
        let frame_start = self.frame_start.expect("didn't begin frame");
        self.write = Some(Write {
            frame_start,
            payload,
        });
    }

    /// event loop needs to put this into clipboard at the end of the frame.
    pub fn take_write(&mut self) -> Option<String> {
        self.write.take().map(|w| w.payload)
    }
}
