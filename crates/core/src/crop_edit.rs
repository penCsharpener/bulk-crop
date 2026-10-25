//! Hit testing and dragging of the crop frame. All positions are frame fractions.

use crate::geometry::{Size, fit_inside, frame_corners, frame_inside, quad_ok};
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

/// The four edges of the frame rectangle: left, right, top, bottom.
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

/// Whether `pos` is within `tol` of `corner` on both axes.
fn near_corner(pos: (f32, f32), corner: (f32, f32), tol: (f32, f32)) -> bool {
    (pos.0 - corner.0).abs() <= tol.0 && (pos.1 - corner.1).abs() <= tol.1
}

/// Whether `pos` is on the edge from `a` to `b`, which is treated as running along the x axis: the
/// pointer's x is within the corners' x range widened by `tol.0`, and its y within `tol.1` of the
/// edge's y at that x. The edge's y is linear between the corners; when their x are equal it is the
/// mean of their y.
fn near_edge_x(pos: (f32, f32), a: (f32, f32), b: (f32, f32), tol: (f32, f32)) -> bool {
    let within = pos.0 >= a.0.min(b.0) - tol.0 && pos.0 <= a.0.max(b.0) + tol.0;
    let edge_y = if a.0 == b.0 {
        (a.1 + b.1) / 2.0
    } else {
        a.1 + ((pos.0 - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) * (b.1 - a.1)
    };
    within && (pos.1 - edge_y).abs() <= tol.1
}

/// The same for an edge running along the y axis: the axes swapped.
fn near_edge_y(pos: (f32, f32), a: (f32, f32), b: (f32, f32), tol: (f32, f32)) -> bool {
    let swap = |p: (f32, f32)| (p.1, p.0);
    near_edge_x(swap(pos), swap(a), swap(b), (tol.1, tol.0))
}

/// Whether `pos` is inside the frame or on its outline: for each side, in the order NW, NE, SE, SW,
/// the pointer is on the inner (right-hand, on screen) side of the line through it or on it.
fn inside_frame(pos: (f32, f32), p: &[(f32, f32); 4]) -> bool {
    (0..4).all(|i| {
        let (from, to) = (p[i], p[(i + 1) % 4]);
        (to.0 - from.0) * (pos.1 - from.1) - (to.1 - from.1) * (pos.0 - from.0) >= 0.0
    })
}

/// Which handle is under `pos`. Corners are checked first, then edges, then the inside of the frame
/// (`Move`). The frame is the real four-cornered shape (`geometry::frame_corners`). `tol` is the
/// reach of a handle on each axis. Anything further out is `None`.
pub fn hit_test(crop: CropRect, pos: (f32, f32), tol: (f32, f32)) -> Option<Handle> {
    let p = frame_corners(crop);
    let [nw, ne, se, sw] = p;
    if near_corner(pos, nw, tol) {
        Some(Handle::NW)
    } else if near_corner(pos, ne, tol) {
        Some(Handle::NE)
    } else if near_corner(pos, sw, tol) {
        Some(Handle::SW)
    } else if near_corner(pos, se, tol) {
        Some(Handle::SE)
    } else if near_edge_y(pos, nw, sw, tol) {
        Some(Handle::W)
    } else if near_edge_y(pos, ne, se, tol) {
        Some(Handle::E)
    } else if near_edge_x(pos, nw, ne, tol) {
        Some(Handle::N)
    } else if near_edge_x(pos, sw, se, tol) {
        Some(Handle::S)
    } else if inside_frame(pos, &p) {
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
        return CropRect {
            cx,
            cy,
            w,
            h,
            ..start
        };
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
        ..start
    }
}

/// Like `drag`, but the frame also stays inside the turned picture (see `geometry::frame_inside`),
/// so it never covers the white corners a fine angle leaves, and stays a valid four-sided shape
/// (`geometry::quad_ok`). A start frame that is not inside is fitted first. When the whole movement
/// would break either, the frame goes as far as it can, then slides along the limit on each axis alone.
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
    let ok = |d: (f32, f32)| {
        let frame = at(d);
        frame_inside(frame, base, angle_deg) && quad_ok(frame)
    };
    at(reach_towards(ok, delta))
}

/// Moves one corner of the frame alone: its shift becomes the start shift plus `delta`, the total
/// movement since the drag began. `start` is fitted first, like in `drag_inside`. The corner stays
/// inside the turned picture and the frame stays a valid shape (`geometry::quad_ok`); where the
/// movement would break either, the corner goes as far as it can, then slides along the limit on each
/// axis alone. Any handle that is not a corner returns the fitted start unchanged.
pub fn drag_corner(
    start: CropRect,
    handle: Handle,
    delta: (f32, f32),
    base: Size,
    angle_deg: f32,
) -> CropRect {
    let start = fit_inside(start, base, angle_deg);
    let index = match handle {
        Handle::NW => 0,
        Handle::NE => 1,
        Handle::SE => 2,
        Handle::SW => 3,
        _ => return start,
    };
    let at = |d: (f32, f32)| {
        let mut frame = start;
        let (sx, sy) = start.corner_shift[index];
        frame.corner_shift[index] = (sx + d.0, sy + d.1);
        frame
    };
    let ok = |d: (f32, f32)| {
        let frame = at(d);
        frame_inside(frame, base, angle_deg) && quad_ok(frame)
    };
    at(reach_towards(ok, delta))
}

/// How much of `delta` can be applied while `ok` holds: all of it, or else the furthest point on the
/// straight way there, then the furthest along x alone, then along y alone. `ok` must hold for no
/// movement. A movement that is not finite counts as none.
fn reach_towards(ok: impl Fn((f32, f32)) -> bool, delta: (f32, f32)) -> (f32, f32) {
    if !(delta.0.is_finite() && delta.1.is_finite()) {
        return (0.0, 0.0);
    }
    if ok(delta) {
        return delta;
    }
    let t = furthest(|t| ok((delta.0 * t, delta.1 * t)));
    let mut reached = (delta.0 * t, delta.1 * t);
    let tx = furthest(|s| ok((reached.0 + (delta.0 - reached.0) * s, reached.1)));
    reached.0 += (delta.0 - reached.0) * tx;
    let ty = furthest(|s| ok((reached.0, reached.1 + (delta.1 - reached.1) * s)));
    reached.1 += (delta.1 - reached.1) * ty;
    reached
}

/// The largest `t` in `0..=1` for which `ok(t)` holds. `ok(0)` must hold, and `ok` may switch from
/// true to false only once; that holds for the frame limits here, which a straight movement leaves
/// at most once.
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

/// How close the pointer must be to a spin handle's dot, in screen points: `percent` per cent of the
/// base picture's shorter side as it is drawn (`base_points`, width and height), and never less
/// than `floor`. It is the distance the frame handles reach, without their cap at a third of the
/// frame, so one setting controls both.
pub fn spin_reach(base_points: (f32, f32), percent: f32, floor: f32) -> f32 {
    (percent / 100.0 * base_points.0.min(base_points.1)).max(floor)
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

    #[test]
    fn spin_reach_is_a_share_of_the_shorter_side() {
        assert!(close(spin_reach((800.0, 600.0), 5.0, 10.0), 30.0));
        assert!(close(spin_reach((600.0, 800.0), 5.0, 10.0), 30.0));
    }

    #[test]
    fn spin_reach_never_below_the_floor() {
        assert!(close(spin_reach((800.0, 600.0), 1.0, 10.0), 10.0));
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn frame(cx: f32, cy: f32, w: f32, h: f32) -> CropRect {
        CropRect {
            cx,
            cy,
            w,
            h,
            ..CropRect::default()
        }
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

    fn shifted(crop: CropRect, shift: [(f32, f32); 4]) -> CropRect {
        CropRect {
            corner_shift: shift,
            ..crop
        }
    }

    fn nw_shift(dx: f32, dy: f32) -> [(f32, f32); 4] {
        [(dx, dy), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]
    }

    #[test]
    fn hit_test_finds_a_shifted_corner() {
        // NW is shifted from (-0.25, -0.25) to (-0.15, -0.2).
        let f = shifted(frame(0.0, 0.0, 0.5, 0.5), nw_shift(0.1, 0.05));
        assert_eq!(hit_test(f, (-0.15, -0.2), TOL), Some(Handle::NW));
        assert_eq!(hit_test(f, (-0.14, -0.19), TOL), Some(Handle::NW));
        assert_eq!(hit_test(f, (-0.25, -0.25), TOL), None);
    }

    #[test]
    fn hit_test_follows_a_slanted_edge() {
        // The N edge now runs from (-0.15, -0.15) to (0.25, -0.25); at x = 0.05 it is at y = -0.2.
        let f = shifted(frame(0.0, 0.0, 0.5, 0.5), nw_shift(0.1, 0.1));
        assert_eq!(hit_test(f, (0.05, -0.2), TOL), Some(Handle::N));
        assert_eq!(hit_test(f, (0.05, -0.19), TOL), Some(Handle::N));
        assert_ne!(hit_test(f, (0.05, -0.25), TOL), Some(Handle::N));
    }

    #[test]
    fn hit_test_follows_a_slanted_side_edge() {
        // The W edge runs from (-0.15, -0.25) to (-0.25, 0.25); at y = 0 it is at x = -0.2.
        let f = shifted(frame(0.0, 0.0, 0.5, 0.5), nw_shift(0.1, 0.0));
        assert_eq!(hit_test(f, (-0.2, 0.0), TOL), Some(Handle::W));
        assert_ne!(hit_test(f, (-0.25, 0.0), TOL), Some(Handle::W));
    }

    #[test]
    fn edge_with_equal_corner_x_is_hit_at_the_mean_y() {
        // NW and NE both at x = 0 (not a valid frame, but hit_test must not divide by zero), at
        // y -0.3 and -0.1: a point near the mean y is N.
        let f = shifted(
            frame(0.0, 0.0, 0.6, 0.6),
            [(0.3, 0.0), (-0.3, 0.2), (0.0, 0.0), (0.0, 0.0)],
        );
        assert_eq!(hit_test(f, (0.0, -0.2), TOL), Some(Handle::N));
        assert_ne!(hit_test(f, (0.0, -0.3), TOL), Some(Handle::N));
    }

    #[test]
    fn inside_of_a_trapezoid_is_move() {
        let f = shifted(
            frame(0.0, 0.0, 0.6, 0.6),
            [(0.1, 0.0), (-0.1, 0.0), (0.0, 0.0), (0.0, 0.0)],
        );
        assert_eq!(hit_test(f, (0.0, 0.0), TOL), Some(Handle::Move));
        assert_eq!(hit_test(f, (-0.15, -0.1), TOL), Some(Handle::Move));
        // Inside the rectangle, but outside the slanted side.
        assert_eq!(hit_test(f, (-0.28, -0.28), TOL), None);
        assert_eq!(hit_test(f, (0.28, -0.28), TOL), None);
    }

    #[test]
    fn corner_drag_moves_one_corner() {
        let start = shifted(
            frame(0.0, 0.0, 0.5, 0.5),
            [(0.01, 0.0), (0.0, 0.02), (0.0, 0.0), (0.03, 0.0)],
        );
        let d = drag_corner(start, Handle::NE, (0.05, 0.03), base_1600(), 0.0);
        assert!(close(d.corner_shift[1].0, 0.05), "{:?}", d.corner_shift);
        assert!(close(d.corner_shift[1].1, 0.05), "{:?}", d.corner_shift);
        for i in [0, 2, 3] {
            assert_eq!(d.corner_shift[i], start.corner_shift[i], "corner {i}");
        }
        assert!(close(d.cx, 0.0) && close(d.cy, 0.0));
        assert!(close(d.w, 0.5) && close(d.h, 0.5));
    }

    #[test]
    fn corner_drag_stops_at_the_picture_edge() {
        // NW starts shifted in by 0.1 on both axes; dragging it out by 0.3 can only reach the edge.
        let start = shifted(CropRect::default(), nw_shift(0.1, 0.1));
        let d = drag_corner(start, Handle::NW, (-0.3, -0.3), base_1600(), 0.0);
        assert!(frame_inside(d, base_1600(), 0.0), "{d:?}");
        assert!(d.corner_shift[0].0.abs() < 1e-3, "{:?}", d.corner_shift);
        assert!(d.corner_shift[0].1.abs() < 1e-3, "{:?}", d.corner_shift);
    }

    #[test]
    fn corner_drag_never_dents_the_frame() {
        // NE and SW sit close to the middle, so the NE-SW diagonal is x + y = 0.28.
        let start = shifted(
            frame(0.0, 0.0, 0.6, 0.6),
            [(0.0, 0.0), (0.0, 0.28), (0.0, 0.0), (0.28, 0.0)],
        );
        assert!(quad_ok(start));
        let d = drag_corner(start, Handle::SE, (-0.4, -0.4), base_1600(), 0.0);
        assert!(quad_ok(d), "{d:?}");
        let se = frame_corners(d)[2];
        assert!(se.0 + se.1 > 0.28 - 1e-3, "SE at {se:?}");
        assert!(d.corner_shift[2].0 < -0.1, "SE should have moved in");
    }

    #[test]
    fn corner_drag_slides_along_the_limit() {
        // NW already sits at the left edge; moving it left is refused, moving it down is not.
        let d = drag_corner(
            CropRect::default(),
            Handle::NW,
            (-0.2, 0.1),
            base_1600(),
            0.0,
        );
        assert!(frame_inside(d, base_1600(), 0.0), "{d:?}");
        assert!(d.corner_shift[0].0 > -1e-3, "{:?}", d.corner_shift);
        assert!(close(d.corner_shift[0].1, 0.1), "{:?}", d.corner_shift);
    }

    #[test]
    fn plain_drag_keeps_the_shift() {
        let start = shifted(
            frame(0.0, 0.0, 0.6, 0.6),
            [(0.05, 0.0), (0.0, 0.02), (0.0, 0.0), (0.0, 0.01)],
        );
        let d = drag_inside(start, Handle::W, (0.1, 0.0), base_1600(), 0.0, MIN);
        assert!(close(d.w, 0.5), "w = {}", d.w);
        assert_eq!(d.corner_shift, start.corner_shift);
    }

    #[test]
    fn plain_drag_stops_before_crossing_corners() {
        // NW is shifted 0.25 to the right. Shrinking from the W edge by 0.2 would push it past the
        // frame's centre line; the drag stops at 0.1.
        let start = shifted(frame(0.0, 0.0, 0.6, 0.6), nw_shift(0.25, 0.0));
        assert!(quad_ok(start));
        let d = drag_inside(start, Handle::W, (0.2, 0.0), base_1600(), 0.0, MIN);
        assert!(quad_ok(d), "{d:?}");
        assert!(close_to(d.w, 0.5, 1e-3), "w = {}", d.w);
    }

    #[test]
    fn edge_handle_in_drag_corner_changes_nothing() {
        let start = shifted(
            frame(0.0, 0.0, 0.5, 0.5),
            [(0.01, 0.0), (0.0, 0.02), (0.0, 0.0), (0.03, 0.0)],
        );
        for handle in [Handle::N, Handle::S, Handle::E, Handle::W, Handle::Move] {
            let d = drag_corner(start, handle, (0.1, 0.1), base_1600(), 0.0);
            assert_eq!(d, start, "{handle:?}");
        }
    }

    fn close_to(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }
}
