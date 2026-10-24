//! Hit testing and dragging of the crop frame. All positions are frame fractions.

use crate::geometry::{Size, fit_inside, frame_inside};
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

/// How close the pointer must be to an edge or corner to grab it, per axis, in frame fractions.
/// It is `percent` per cent of the base picture's shorter side, so the zone is square on screen.
/// It is capped at a third of the frame's width (or height), so the middle of a small frame still
/// moves it, and never below `floor`. A base side that is not positive returns `floor`.
pub fn handle_reach(base: Size, crop: CropRect, percent: f32, floor: (f32, f32)) -> (f32, f32) {
    if base.w <= 0.0 || base.h <= 0.0 {
        return floor;
    }
    let reach = percent / 100.0 * base.w.min(base.h);
    (
        (reach / base.w).min(crop.w / 3.0).max(floor.0),
        (reach / base.h).min(crop.h / 3.0).max(floor.1),
    )
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

/// Like `drag`, but the frame also stays inside the turned picture (see `geometry::frame_inside`),
/// so it never covers the white corners a fine angle leaves. A start frame that is not inside is
/// fitted first. When the whole movement would leave the picture, the frame goes as far as it can,
/// then slides along the edge on each axis alone.
pub fn drag_inside(
    start: CropRect,
    handle: Handle,
    delta: (f32, f32),
    base: Size,
    angle_deg: f32,
    min: (f32, f32),
) -> CropRect {
    let start = fit_inside(start, base, angle_deg);
    let at = |d: (f32, f32)| drag(start, handle, d, (0.5, 0.5), min);
    let ok = |d: (f32, f32)| frame_inside(at(d), base, angle_deg);
    if ok(delta) {
        return at(delta);
    }
    let t = furthest(|t| ok((delta.0 * t, delta.1 * t)));
    let mut reached = (delta.0 * t, delta.1 * t);
    let tx = furthest(|s| ok((reached.0 + (delta.0 - reached.0) * s, reached.1)));
    reached.0 += (delta.0 - reached.0) * tx;
    let ty = furthest(|s| ok((reached.0, reached.1 + (delta.1 - reached.1) * s)));
    reached.1 += (delta.1 - reached.1) * ty;
    at(reached)
}

/// The largest `t` in `0..=1` for which `ok(t)` holds. `ok(0)` must hold, and `ok` may switch from
/// true to false only once; both are true here because the allowed region is convex.
fn furthest(ok: impl Fn(f32) -> bool) -> f32 {
    if ok(1.0) {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if ok(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The fine angle after dragging a spin handle from `press` to `now` (screen points), turning about
/// `centre`. The change is the angle the pointer swept around the centre: on the right of the centre,
/// moving down turns clockwise, which is positive. The result is rounded to 0.1° and kept within
/// `-limit..=limit`.
pub fn spin_angle(
    start_deg: f32,
    centre: (f32, f32),
    press: (f32, f32),
    now: (f32, f32),
    limit: f32,
) -> f32 {
    let before = (press.1 - centre.1).atan2(press.0 - centre.0);
    let after = (now.1 - centre.1).atan2(now.0 - centre.0);
    let mut swept = (after - before).to_degrees();
    // atan2 jumps by 360° where the pointer crosses the left of the centre.
    if swept > 180.0 {
        swept -= 360.0;
    } else if swept < -180.0 {
        swept += 360.0;
    }
    let angle = ((start_deg + swept) * 10.0).round() / 10.0;
    angle.clamp(-limit, limit)
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

    fn base_1600() -> Size {
        Size {
            w: 1600.0,
            h: 1200.0,
        }
    }

    #[test]
    fn drag_inside_at_zero_angle_equals_drag() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        let inside = drag_inside(f, Handle::E, (0.3, 0.0), base_1600(), 0.0, MIN);
        let plain = drag(f, Handle::E, (0.3, 0.0), HALF, MIN);
        assert!(close(inside.cx, plain.cx) && close(inside.cy, plain.cy));
        assert!(close(inside.w, plain.w) && close(inside.h, plain.h));
    }

    #[test]
    fn drag_inside_stops_at_turned_edge() {
        let f = frame(0.0, 0.0, 0.5, 0.5);
        let d = drag_inside(f, Handle::E, (1.0, 0.0), base_1600(), 6.0, MIN);
        assert!(frame_inside(d, base_1600(), 6.0));
        let right = d.cx + d.w / 2.0;
        assert!((right - 0.4831).abs() < 1e-3, "right = {right}");
    }

    #[test]
    fn drag_inside_slides_along_edge() {
        let f = frame(0.3, 0.0, 0.2, 0.2);
        let d = drag_inside(f, Handle::Move, (0.5, 0.1), base_1600(), 6.0, MIN);
        assert!(frame_inside(d, base_1600(), 6.0));
        assert!((d.cx - 0.3935).abs() < 1e-3, "cx = {}", d.cx);
        assert!((d.cy - 0.0187).abs() < 1e-3, "cy = {}", d.cy);
    }

    #[test]
    fn reach_is_percent_of_shorter_side() {
        let r = handle_reach(base_1600(), frame(0.0, 0.0, 1.0, 1.0), 5.0, (0.0, 0.0));
        assert!(close(r.0, 0.0375), "x = {}", r.0);
        assert!(close(r.1, 0.05), "y = {}", r.1);
    }

    #[test]
    fn reach_is_capped_by_small_frame() {
        let r = handle_reach(base_1600(), frame(0.0, 0.0, 0.09, 0.09), 5.0, (0.0, 0.0));
        assert!(close(r.0, 0.03), "x = {}", r.0);
        assert!(close(r.1, 0.03), "y = {}", r.1);
    }

    #[test]
    fn reach_never_below_floor() {
        let floor = (0.1, 0.2);
        let r = handle_reach(base_1600(), frame(0.0, 0.0, 1.0, 1.0), 5.0, floor);
        assert!(close(r.0, 0.1) && close(r.1, 0.2), "{r:?}");
        let zero = Size { w: 0.0, h: 1200.0 };
        let r = handle_reach(zero, frame(0.0, 0.0, 1.0, 1.0), 5.0, floor);
        assert_eq!(r, floor);
    }

    #[test]
    fn centre_of_frame_still_moves_with_wide_reach() {
        let f = frame(0.1, -0.1, 0.4, 0.4);
        let reach = handle_reach(base_1600(), f, 15.0, (0.0, 0.0));
        assert_eq!(hit_test(f, (0.1, -0.1), reach), Some(Handle::Move));
    }

    const ORIGIN: (f32, f32) = (0.0, 0.0);

    #[test]
    fn right_handle_down_turns_clockwise() {
        let a = spin_angle(0.0, ORIGIN, (100.0, 0.0), (100.0, 10.0), 15.0);
        assert!((a - 5.7).abs() < 1e-4, "a = {a}");
    }

    #[test]
    fn left_handle_up_turns_clockwise() {
        let a = spin_angle(0.0, ORIGIN, (-100.0, 0.0), (-100.0, -10.0), 15.0);
        assert!((a - 5.7).abs() < 1e-4, "a = {a}");
    }

    #[test]
    fn crossing_the_left_does_not_jump() {
        let a = spin_angle(0.0, ORIGIN, (-100.0, 1.0), (-100.0, -1.0), 15.0);
        assert!((a - 1.1).abs() < 1e-4, "a = {a}");
    }

    #[test]
    fn result_is_rounded_to_a_tenth() {
        let a = spin_angle(0.37, ORIGIN, (100.0, 0.0), (100.0, 3.3), 15.0);
        assert!((a * 10.0 - (a * 10.0).round()).abs() < 1e-3, "a = {a}");
    }

    #[test]
    fn result_is_clamped() {
        let a = spin_angle(0.0, ORIGIN, (100.0, 0.0), (0.0, 100.0), 15.0);
        assert!((a - 15.0).abs() < 1e-4, "a = {a}");
    }

    #[test]
    fn no_movement_keeps_start() {
        let a = spin_angle(2.3, ORIGIN, (100.0, 40.0), (100.0, 40.0), 15.0);
        assert!((a - 2.3).abs() < 1e-4, "a = {a}");
    }
}
