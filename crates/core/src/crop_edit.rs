//! Hit testing and dragging of the crop frame. All positions are frame fractions.

use crate::model::CropRect;

/// The part of the frame under the pointer: the whole frame, one of its four edges or corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    Move,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

/// The four edges of the frame: left, right, top, bottom.
fn edges(c: CropRect) -> (f32, f32, f32, f32) {
    (
        c.cx - c.w / 2.0,
        c.cx + c.w / 2.0,
        c.cy - c.h / 2.0,
        c.cy + c.h / 2.0,
    )
}

/// Clamps `v` into `lo..=hi`. Unlike `f32::clamp` this never panics when `lo > hi`.
fn clamp_to(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

/// Which handle is under `pos`. Corners are checked first, then edges, then the inside of the frame
/// (`Move`). `tol` is the reach of a handle on each axis. Anything further out is `None`.
pub fn hit_test(crop: CropRect, pos: (f32, f32), tol: (f32, f32)) -> Option<Handle> {
    let (x, y) = pos;
    let (left, right, top, bottom) = edges(crop);
    let (tx, ty) = tol;
    let near_left = (x - left).abs() <= tx;
    let near_right = (x - right).abs() <= tx;
    let near_top = (y - top).abs() <= ty;
    let near_bottom = (y - bottom).abs() <= ty;
    let within_x = x >= left - tx && x <= right + tx;
    let within_y = y >= top - ty && y <= bottom + ty;

    if near_left && near_top {
        Some(Handle::NW)
    } else if near_right && near_top {
        Some(Handle::NE)
    } else if near_left && near_bottom {
        Some(Handle::SW)
    } else if near_right && near_bottom {
        Some(Handle::SE)
    } else if near_left && within_y {
        Some(Handle::W)
    } else if near_right && within_y {
        Some(Handle::E)
    } else if near_top && within_x {
        Some(Handle::N)
    } else if near_bottom && within_x {
        Some(Handle::S)
    } else if x >= left && x <= right && y >= top && y <= bottom {
        Some(Handle::Move)
    } else {
        None
    }
}

/// The frame after dragging `handle` by `delta`, the total movement since the drag began.
/// `start` is the frame when the drag began. The frame never leaves `-half..=half` on either axis,
/// and an edge never comes closer than `min` to the opposite edge.
pub fn drag(
    start: CropRect,
    handle: Handle,
    delta: (f32, f32),
    half: (f32, f32),
    min: (f32, f32),
) -> CropRect {
    let (left, right, top, bottom) = edges(start);
    let (dx, dy) = delta;
    let (hx, hy) = half;
    let (min_x, min_y) = min;
    let (mut l, mut r, mut t, mut b) = (left, right, top, bottom);
    if handle == Handle::Move {
        let (w, h) = (start.w, start.h);
        let cx = clamp_to(start.cx + dx, -hx + w / 2.0, hx - w / 2.0);
        let cy = clamp_to(start.cy + dy, -hy + h / 2.0, hy - h / 2.0);
        return CropRect { cx, cy, w, h };
    }
    match handle {
        Handle::W | Handle::NW | Handle::SW => l = clamp_to(left + dx, -hx, right - min_x),
        Handle::E | Handle::NE | Handle::SE => r = clamp_to(right + dx, left + min_x, hx),
        _ => {}
    }
    match handle {
        Handle::N | Handle::NE | Handle::NW => t = clamp_to(top + dy, -hy, bottom - min_y),
        Handle::S | Handle::SE | Handle::SW => b = clamp_to(bottom + dy, top + min_y, hy),
        _ => {}
    }
    CropRect {
        cx: (l + r) / 2.0,
        cy: (t + b) / 2.0,
        w: r - l,
        h: b - t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn frame(cx: f32, cy: f32, w: f32, h: f32) -> CropRect {
        CropRect { cx, cy, w, h }
    }

    const TOL: (f32, f32) = (0.02, 0.02);
    const HALF: (f32, f32) = (0.5, 0.5);
    const MIN: (f32, f32) = (0.02, 0.02);

    #[test]
    fn hit_corner_before_edge() {
        // The frame's top-left corner is (-0.25, -0.25); the point is near both edges.
        let f = frame(0.0, 0.0, 0.5, 0.5);
        assert_eq!(hit_test(f, (-0.24, -0.24), TOL), Some(Handle::NW));
    }

    #[test]
    fn hit_inside_is_move() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        assert_eq!(hit_test(f, (0.0, 0.0), TOL), Some(Handle::Move));
    }

    #[test]
    fn hit_far_outside_is_none() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        assert_eq!(hit_test(f, (0.9, 0.9), TOL), None);
    }

    #[test]
    fn move_is_clamped_to_bounds() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        let moved = drag(f, Handle::Move, (2.0, 2.0), HALF, MIN);
        assert!(close(moved.cx, 0.25), "cx = {}", moved.cx);
        assert!(close(moved.cy, 0.25), "cy = {}", moved.cy);
        assert!(close(moved.w, 0.5) && close(moved.h, 0.5));
    }

    #[test]
    fn east_handle_changes_only_right_edge() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        let d = drag(f, Handle::E, (0.1, 0.3), HALF, MIN);
        let (left, right) = (d.cx - d.w / 2.0, d.cx + d.w / 2.0);
        let (top, bottom) = (d.cy - d.h / 2.0, d.cy + d.h / 2.0);
        assert!(close(left, -0.25), "left = {left}");
        assert!(close(top, -0.25), "top = {top}");
        assert!(close(bottom, 0.25), "bottom = {bottom}");
        assert!(close(right, 0.35), "right = {right}");
    }

    #[test]
    fn north_west_handle_changes_top_and_left() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        let d = drag(f, Handle::NW, (-0.1, -0.1), HALF, MIN);
        let (left, right) = (d.cx - d.w / 2.0, d.cx + d.w / 2.0);
        let (top, bottom) = (d.cy - d.h / 2.0, d.cy + d.h / 2.0);
        assert!(close(left, -0.35), "left = {left}");
        assert!(close(top, -0.35), "top = {top}");
        assert!(close(right, 0.25) && close(bottom, 0.25));
    }

    #[test]
    fn cannot_shrink_below_min() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        let d = drag(f, Handle::W, (0.5, 0.0), HALF, MIN);
        assert!(close(d.w, 0.02), "w = {}", d.w);
    }

    #[test]
    fn cannot_drag_edge_past_bounds() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        let d = drag(f, Handle::E, (1.0, 0.0), HALF, MIN);
        let right = d.cx + d.w / 2.0;
        assert!(close(right, 0.5), "right = {right}");
    }
}
