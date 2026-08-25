//! RGA 2D blitter model — a recording `improcess` fake.
//!
//! `warden_rga.c` offloads copies/scales/format-conversions to the RGA via
//! librga's `improcess(src, dst, ..., IM_SYNC)`, and falls back to the CPU draw
//! path when it returns anything but `IM_STATUS_SUCCESS`. The blit *pixels* are
//! not modelled — what matters for testing is the **dispatch** logic: which ops
//! get sent, with what geometry/format, and that a non-success status drives the
//! CPU fallback. So the sim records each requested op and returns a programmable
//! status. It rides its own call seam (behind the driver's `#if WARDEN_USE_RGA`),
//! not the register bus.

/// A rectangle in a surface (im2d `im_rect`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// A surface descriptor — the subset of im2d `rga_buffer_t` the dispatch cares
/// about (dimensions + pixel format).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Surface {
    pub width: i32,
    pub height: i32,
    pub format: u32,
}

/// One recorded RGA operation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Blit {
    pub src: Surface,
    pub dst: Surface,
    pub srect: Rect,
    pub drect: Rect,
}

/// im2d status subset: success, or a failure that must drive the CPU fallback.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImStatus {
    Success,
    Failed,
}

/// A recording RGA. `status` is what `improcess` returns; `blits` is the log.
pub struct RgaSim {
    status: ImStatus,
    blits: Vec<Blit>,
}

impl RgaSim {
    /// A working RGA whose `improcess` succeeds.
    pub fn new() -> Self {
        Self {
            status: ImStatus::Success,
            blits: Vec::new(),
        }
    }

    /// Program the status `improcess` returns (set `Failed` to drive the driver's
    /// CPU fallback path).
    pub fn set_status(&mut self, s: ImStatus) {
        self.status = s;
    }

    /// Model one `improcess()` call: record it and return the programmed status.
    pub fn improcess(&mut self, src: Surface, dst: Surface, srect: Rect, drect: Rect) -> ImStatus {
        self.blits.push(Blit {
            src,
            dst,
            srect,
            drect,
        });
        self.status
    }

    /// The recorded blit log, in dispatch order.
    pub fn blits(&self) -> &[Blit] {
        &self.blits
    }

    /// Number of blits dispatched.
    pub fn count(&self) -> usize {
        self.blits.len()
    }

    /// The most recent blit, if any.
    pub fn last(&self) -> Option<&Blit> {
        self.blits.last()
    }

    /// Forget the recorded blits (e.g. between frames).
    pub fn clear(&mut self) {
        self.blits.clear();
    }
}

impl Default for RgaSim {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surf(w: i32, h: i32) -> Surface {
        Surface {
            width: w,
            height: h,
            format: 0,
        }
    }
    fn rect(w: i32, h: i32) -> Rect {
        Rect { x: 0, y: 0, w, h }
    }

    #[test]
    fn records_a_successful_blit() {
        let mut r = RgaSim::new();
        let st = r.improcess(
            surf(720, 720),
            surf(360, 360),
            rect(720, 720),
            rect(360, 360),
        );
        assert_eq!(st, ImStatus::Success);
        assert_eq!(r.count(), 1);
        let b = r.last().unwrap();
        assert_eq!(b.src, surf(720, 720));
        assert_eq!(b.drect, rect(360, 360));
    }

    #[test]
    fn failed_status_drives_fallback() {
        let mut r = RgaSim::new();
        r.set_status(ImStatus::Failed);
        assert_eq!(
            r.improcess(surf(10, 10), surf(10, 10), rect(10, 10), rect(10, 10)),
            ImStatus::Failed
        );
        // the op is still recorded — the driver dispatched it, then fell back.
        assert_eq!(r.count(), 1);
    }

    #[test]
    fn logs_multiple_in_order() {
        let mut r = RgaSim::new();
        r.improcess(surf(1, 1), surf(1, 1), rect(1, 1), rect(1, 1));
        r.improcess(surf(2, 2), surf(2, 2), rect(2, 2), rect(2, 2));
        assert_eq!(r.count(), 2);
        assert_eq!(r.blits()[0].src, surf(1, 1));
        assert_eq!(r.blits()[1].src, surf(2, 2));
    }

    #[test]
    fn clear_empties_the_log() {
        let mut r = RgaSim::new();
        r.improcess(surf(1, 1), surf(1, 1), rect(1, 1), rect(1, 1));
        r.clear();
        assert_eq!(r.count(), 0);
        assert!(r.last().is_none());
    }

    #[test]
    fn default_is_empty_and_succeeds() {
        let r = RgaSim::default();
        assert_eq!(r.count(), 0);
        assert!(r.last().is_none());
    }
}
