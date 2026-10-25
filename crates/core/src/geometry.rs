//! Turning and cropping: the mapping from the crop frame to the source picture.

use serde::{Deserialize, Serialize};

use crate::model::{Adjust, CropRect};

/// A size in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

/// How the cropped picture is shrunk for output.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Resize {
    None,
    LongEdge(u32),
    Percent(f32),
}

/// The size of the picture after its quarter turns: width and height swap for odd turns.
pub fn base_size(src: Size, quarter_turns: u8) -> Size {
    if quarter_turns % 2 == 1 {
        Size { w: src.h, h: src.w }
    } else {
        src
    }
}

/// Maps a point of the upright crop frame, measured from the picture centre, back to a position in
/// the source picture (origin at its top-left corner), undoing the quarter turns and the fine angle.
pub fn frame_to_src(p: (f32, f32), src: Size, quarter_turns: u8, angle_deg: f32) -> (f32, f32) {
    let total = (f32::from(quarter_turns % 4) * 90.0 + angle_deg).to_radians();
    let (sin, cos) = total.sin_cos();
    let (px, py) = p;
    let qx = px * cos + py * sin;
    let qy = -px * sin + py * cos;
    (qx + src.w / 2.0, qy + src.h / 2.0)
}

/// Half the width and height of the frame's allowed range, as fractions of the base picture.
/// The picture turned by `angle_deg` is the box the frame may reach.
pub fn half_extents(base: Size, angle_deg: f32) -> (f32, f32) {
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    let (sin, cos) = (sin.abs(), cos.abs());
    let half_x = (base.w * cos + base.h * sin) / (2.0 * base.w);
    let half_y = (base.w * sin + base.h * cos) / (2.0 * base.h);
    (half_x, half_y)
}

/// Slack for the inside tests, in fractions of the base picture, so a frame fitted exactly to an
/// edge still counts as inside after rounding.
const INSIDE_EPS: f32 = 1e-4;

/// The four corners of the frame, NW, NE, SE, SW, in fractions of the base picture from its centre:
/// the rectangle's corners plus `corner_shift`.
pub fn frame_corners(crop: CropRect) -> [(f32, f32); 4] {
    let (hw, hh) = (crop.w / 2.0, crop.h / 2.0);
    let signs = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let mut out = [(0.0, 0.0); 4];
    for ((o, (sx, sy)), (dx, dy)) in out.iter_mut().zip(signs).zip(crop.corner_shift) {
        *o = (crop.cx + sx * hw + dx, crop.cy + sy * hh + dy);
    }
    out
}

/// Whether the frame's four corners make a shape the export can straighten: every corner stays in
/// its own quarter around the rectangle's centre (NW up and left of it, and so on), and the shape is
/// strictly convex. Non-finite values are not valid.
pub fn quad_ok(crop: CropRect) -> bool {
    let p = frame_corners(crop);
    let finite = p.iter().all(|c| c.0.is_finite() && c.1.is_finite())
        && crop.cx.is_finite()
        && crop.cy.is_finite();
    if !finite {
        return false;
    }
    let (cx, cy) = (crop.cx, crop.cy);
    let quarters = p[0].0 < cx
        && p[0].1 < cy
        && p[1].0 > cx
        && p[1].1 < cy
        && p[2].0 > cx
        && p[2].1 > cy
        && p[3].0 < cx
        && p[3].1 > cy;
    quarters
        && (0..4).all(|i| {
            let (prev, here, next) = (p[(i + 3) % 4], p[i], p[(i + 1) % 4]);
            let (ax, ay) = (here.0 - prev.0, here.1 - prev.1);
            let (bx, by) = (next.0 - here.0, next.1 - here.1);
            ax * by - ay * bx > 0.0
        })
}

/// The projective map from the unit square onto a four-sided shape: (0, 0) goes to the first corner,
/// (1, 0) to the second, (1, 1) to the third and (0, 1) to the fourth, so NW, NE, SE, SW. Straight
/// lines stay straight. For a rectangle or a parallelogram it is the plain linear map.
/// After Heckbert, "Fundamentals of Texture Mapping", 1989.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquareToQuad {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
    g: f32,
    h: f32,
}

impl SquareToQuad {
    /// The map onto `corners`, given NW, NE, SE, SW in any unit. A shape with three corners on one
    /// line has no such map; it gets the linear map instead, and `quad_ok` keeps such frames out of
    /// the settings.
    pub fn new(corners: [(f32, f32); 4]) -> SquareToQuad {
        let [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] = corners;
        let (sx, sy) = (x0 - x1 + x2 - x3, y0 - y1 + y2 - y3);
        let (dx1, dy1) = (x1 - x2, y1 - y2);
        let (dx2, dy2) = (x3 - x2, y3 - y2);
        let den = dx1 * dy2 - dx2 * dy1;
        let (g, h) = if den == 0.0 {
            (0.0, 0.0)
        } else {
            ((sx * dy2 - dx2 * sy) / den, (dx1 * sy - sx * dy1) / den)
        };
        SquareToQuad {
            a: x1 - x0 + g * x1,
            b: x3 - x0 + h * x3,
            c: x0,
            d: y1 - y0 + g * y1,
            e: y3 - y0 + h * y3,
            f: y0,
            g,
            h,
        }
    }

    /// Where the point (`s`, `t`) of the unit square lands.
    pub fn apply(&self, s: f32, t: f32) -> (f32, f32) {
        let w = self.g * s + self.h * t + 1.0;
        (
            (self.a * s + self.b * t + self.c) / w,
            (self.d * s + self.e * t + self.f) / w,
        )
    }
}

/// Turns an offset from the picture centre back by the fine angle, the same way `frame_to_src` does.
fn turn_back(angle_deg: f32) -> impl Fn(f32, f32) -> (f32, f32) {
    let (sin, cos) = angle_deg.to_radians().sin_cos();
    move |x, y| (x * cos + y * sin, -x * sin + y * cos)
}

/// Whether a point, in fractions of the base picture from its centre, lies inside the upright base
/// rectangle and inside the picture turned by `angle_deg`.
pub fn point_inside(p: (f32, f32), base: Size, angle_deg: f32) -> bool {
    let (x, y) = p;
    let upright = x.abs() <= 0.5 + INSIDE_EPS && y.abs() <= 0.5 + INSIDE_EPS;
    let (qx, qy) = turn_back(angle_deg)(x * base.w, y * base.h);
    let turned = qx.abs() <= base.w * (0.5 + INSIDE_EPS) && qy.abs() <= base.h * (0.5 + INSIDE_EPS);
    upright && turned
}

/// Whether all four corners of the frame, shifts included, are inside (see `point_inside`).
pub fn frame_inside(crop: CropRect, base: Size, angle_deg: f32) -> bool {
    frame_corners(crop)
        .iter()
        .all(|&corner| point_inside(corner, base, angle_deg))
}

/// The frame shrunk about its centre, keeping its shape, until it lies inside the upright base
/// rectangle and inside the picture turned by `angle_deg`. The size and the corner shifts shrink by
/// the same factor. A frame already inside comes back unchanged; the frame never grows. A frame
/// whose centre is outside is first moved to the middle.
pub fn fit_inside(crop: CropRect, base: Size, angle_deg: f32) -> CropRect {
    let crop = if point_inside((crop.cx, crop.cy), base, angle_deg) {
        crop
    } else {
        CropRect {
            cx: 0.0,
            cy: 0.0,
            ..crop
        }
    };
    let offsets = frame_corners(crop).map(|(x, y)| (x - crop.cx, y - crop.cy));
    let mut s: f32 = 1.0;
    // Upright rectangle: c + s·v must stay within -0.5..=0.5 for every corner and axis.
    for &(vx, vy) in &offsets {
        for (u, v) in [(crop.cx, vx), (crop.cy, vy)] {
            s = s.min(reach(u, v, 0.5));
        }
    }
    // Turned picture, in pixels: u + s·v must stay within -l..=l for every corner and axis,
    // where u is the turned centre and v the turned corner offset.
    let turn = turn_back(angle_deg);
    let (ux, uy) = turn(crop.cx * base.w, crop.cy * base.h);
    let (lx, ly) = (base.w / 2.0, base.h / 2.0);
    for &(vx, vy) in &offsets {
        let (vx, vy) = turn(vx * base.w, vy * base.h);
        for (u, v, l) in [(ux, vx, lx), (uy, vy, ly)] {
            s = s.min(reach(u, v, l));
        }
    }
    let s = s.clamp(0.0, 1.0);
    CropRect {
        w: crop.w * s,
        h: crop.h * s,
        corner_shift: crop.corner_shift.map(|(dx, dy)| (dx * s, dy * s)),
        ..crop
    }
}

/// The largest factor `s` for which `u + s·v` stays within `-l..=l`, given `u` inside it. A `v` of
/// (nearly) zero puts no limit on `s`.
fn reach(u: f32, v: f32, l: f32) -> f32 {
    if v > 1e-6 {
        (l - u) / v
    } else if v < -1e-6 {
        (-l - u) / v
    } else {
        f32::INFINITY
    }
}

/// The frame measured in pixels as a rectangle: the width is the mean of the top edge (NW to NE) and
/// the bottom edge (SW to SE), the height the mean of the left edge (NW to SW) and the right edge
/// (NE to SE). With no shift this is `w · base.w` and `h · base.h`.
fn frame_pixels(base: Size, crop: CropRect) -> (f32, f32) {
    let [nw, ne, se, sw] = crop.corner_shift;
    // The edge vectors in pixels. Written from `w` and `h` and the shifts, not from the corner
    // points, so a frame without shift measures exactly `w · base.w` and `h · base.h`.
    let length = |dx: f32, dy: f32| (dx * base.w).hypot(dy * base.h);
    let top = length(crop.w + ne.0 - nw.0, ne.1 - nw.1);
    let bottom = length(crop.w + se.0 - sw.0, se.1 - sw.1);
    let left = length(sw.0 - nw.0, crop.h + sw.1 - nw.1);
    let right = length(se.0 - ne.0, crop.h + se.1 - ne.1);
    ((top + bottom) / 2.0, (left + right) / 2.0)
}

/// The output size of the cropped picture. The picture is never enlarged.
pub fn output_size(base: Size, crop: CropRect, resize: Resize) -> (u32, u32) {
    let (frame_w, frame_h) = frame_pixels(base, crop);
    let mid_w = frame_w.round().max(1.0);
    let mid_h = frame_h.round().max(1.0);
    let scale = match resize {
        Resize::None => 1.0,
        Resize::LongEdge(n) => (n as f32 / mid_w.max(mid_h)).min(1.0),
        Resize::Percent(p) => (p / 100.0).clamp(0.01, 1.0),
    };
    (
        (mid_w * scale).round().max(1.0) as u32,
        (mid_h * scale).round().max(1.0) as u32,
    )
}

/// Turns the adjustment by one quarter turn, clockwise or counter-clockwise. The crop frame turns
/// with the picture, so the same part of the page stays framed. Each corner moves to the next place
/// (clockwise NW → NE → SE → SW → NW) and its shift turns with it.
pub fn quarter_turn(a: Adjust, clockwise: bool) -> Adjust {
    let c = a.crop;
    let current = a.quarter_turns % 4;
    let mut corner_shift = [(0.0, 0.0); 4];
    for (i, &(dx, dy)) in c.corner_shift.iter().enumerate() {
        if clockwise {
            corner_shift[(i + 1) % 4] = (-dy, dx);
        } else {
            corner_shift[(i + 3) % 4] = (dy, -dx);
        }
    }
    let (quarter_turns, crop) = if clockwise {
        (
            (current + 1) % 4,
            CropRect {
                cx: -c.cy,
                cy: c.cx,
                w: c.h,
                h: c.w,
                corner_shift,
            },
        )
    } else {
        (
            (current + 3) % 4,
            CropRect {
                cx: c.cy,
                cy: -c.cx,
                w: c.h,
                h: c.w,
                corner_shift,
            },
        )
    };
    Adjust {
        quarter_turns,
        crop,
        ..a
    }
}

/// A frame made for a picture with `from` quarter turns, turned to fit the same picture with `to`
/// quarter turns, so it frames the same part of the page.
pub fn turn_crop(crop: CropRect, from: u8, to: u8) -> CropRect {
    let steps = (to % 4 + 4 - from % 4) % 4;
    let mut a = Adjust {
        quarter_turns: from % 4,
        crop,
        ..Adjust::default()
    };
    for _ in 0..steps {
        a = quarter_turn(a, true);
    }
    a.crop
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Adjust, CropRect};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn close_rect(a: CropRect, b: CropRect) -> bool {
        close(a.cx, b.cx)
            && close(a.cy, b.cy)
            && close(a.w, b.w)
            && close(a.h, b.h)
            && a.corner_shift
                .iter()
                .zip(b.corner_shift)
                .all(|(p, q)| close(p.0, q.0) && close(p.1, q.1))
    }

    /// A frame with a different shift on every corner.
    const SHIFTS: [(f32, f32); 4] = [(0.01, 0.02), (-0.03, 0.01), (0.02, -0.02), (-0.01, -0.04)];

    #[test]
    fn worked_example_1_quarter_turn() {
        let src = Size { w: 400.0, h: 300.0 };
        let (x, y) = frame_to_src((-150.0, -200.0), src, 1, 0.0);
        assert!(close(x, 0.0), "x = {x}");
        assert!(close(y, 300.0), "y = {y}");
    }

    #[test]
    fn worked_example_2_identity() {
        let src = Size { w: 400.0, h: 300.0 };
        let (x, y) = frame_to_src((0.0, 0.0), src, 0, 0.0);
        assert!(close(x, 200.0), "x = {x}");
        assert!(close(y, 150.0), "y = {y}");
    }

    #[test]
    fn four_clockwise_turns_return_to_start() {
        let start = Adjust {
            crop: CropRect {
                cx: 0.1,
                cy: -0.2,
                w: 0.5,
                h: 0.8,
                corner_shift: SHIFTS,
            },
            ..Adjust::default()
        };
        let mut turned = start;
        for _ in 0..4 {
            turned = quarter_turn(turned, true);
        }
        assert_eq!(turned.quarter_turns, start.quarter_turns);
        assert!(close_rect(turned.crop, start.crop));
    }

    #[test]
    fn clockwise_then_counter_is_identity() {
        let start = Adjust {
            quarter_turns: 2,
            crop: CropRect {
                cx: 0.1,
                cy: -0.2,
                w: 0.5,
                h: 0.8,
                corner_shift: SHIFTS,
            },
            ..Adjust::default()
        };
        let back = quarter_turn(quarter_turn(start, true), false);
        assert_eq!(back.quarter_turns, start.quarter_turns);
        assert!(close_rect(back.crop, start.crop));
    }

    #[test]
    fn half_extents_at_zero_are_half() {
        let (hx, hy) = half_extents(
            Size {
                w: 1600.0,
                h: 1200.0,
            },
            0.0,
        );
        assert!(close(hx, 0.5));
        assert!(close(hy, 0.5));
    }

    #[test]
    fn output_size_none_long_edge_percent() {
        let base = Size {
            w: 1600.0,
            h: 1200.0,
        };
        let crop = CropRect {
            cx: 0.0,
            cy: 0.0,
            w: 0.5,
            h: 1.0,
            ..CropRect::default()
        };
        assert_eq!(output_size(base, crop, Resize::None), (800, 1200));
        assert_eq!(output_size(base, crop, Resize::LongEdge(600)), (400, 600));
        assert_eq!(output_size(base, crop, Resize::Percent(25.0)), (200, 300));
    }

    #[test]
    fn long_edge_never_enlarges() {
        let base = Size { w: 100.0, h: 80.0 };
        assert_eq!(
            output_size(base, CropRect::default(), Resize::LongEdge(500)),
            (100, 80)
        );
    }

    fn base_1600() -> Size {
        Size {
            w: 1600.0,
            h: 1200.0,
        }
    }

    fn rect(cx: f32, cy: f32, w: f32, h: f32) -> CropRect {
        CropRect {
            cx,
            cy,
            w,
            h,
            ..CropRect::default()
        }
    }

    fn shifted(crop: CropRect, shift: [(f32, f32); 4]) -> CropRect {
        CropRect {
            corner_shift: shift,
            ..crop
        }
    }

    #[test]
    fn full_frame_at_zero_angle_is_unchanged() {
        let full = CropRect::default();
        assert!(frame_inside(full, base_1600(), 0.0));
        assert!(close_rect(fit_inside(full, base_1600(), 0.0), full));
    }

    #[test]
    fn fit_keeps_shape_and_centre() {
        let start = rect(0.1, -0.05, 0.6, 0.8);
        let fitted = fit_inside(start, base_1600(), 6.0);
        assert!(close(fitted.cx, start.cx));
        assert!(close(fitted.cy, start.cy));
        assert!(close(fitted.w / fitted.h, start.w / start.h));
        assert!(fitted.w < start.w);
    }

    #[test]
    fn fitted_frame_is_inside() {
        let frames = [
            CropRect::default(),
            rect(0.1, -0.05, 0.6, 0.8),
            rect(-0.05, 0.02, 0.3, 0.95),
        ];
        for angle in [-10.0, -3.0, 0.5, 7.0, 10.0] {
            for frame in frames {
                let fitted = fit_inside(frame, base_1600(), angle);
                assert!(
                    frame_inside(fitted, base_1600(), angle),
                    "angle {angle}, frame {frame:?}, fitted {fitted:?}"
                );
            }
        }
    }

    #[test]
    fn full_frame_at_5_degrees_is_not_inside() {
        assert!(!frame_inside(CropRect::default(), base_1600(), 5.0));
    }

    #[test]
    fn fit_never_grows() {
        let small = rect(0.05, 0.0, 0.2, 0.2);
        assert!(close_rect(fit_inside(small, base_1600(), 3.0), small));
    }

    #[test]
    fn centre_outside_is_moved_to_middle() {
        let outside = rect(0.9, 0.9, 0.2, 0.2);
        let fitted = fit_inside(outside, base_1600(), 2.0);
        assert!(close(fitted.cx, 0.0) && close(fitted.cy, 0.0));
        assert!(frame_inside(fitted, base_1600(), 2.0));
    }

    #[test]
    fn fitted_render_has_no_white_corners() {
        use crate::render::render_rgb;
        use image::{Rgb, RgbImage};

        let src = RgbImage::from_pixel(800, 600, Rgb([100, 100, 100]));
        let base = Size { w: 800.0, h: 600.0 };
        let crop = fit_inside(rect(0.1, -0.1, 0.9, 0.9), base, 4.0);
        let adj = Adjust {
            angle_deg: 4.0,
            crop,
            ..Adjust::default()
        };
        let out = render_rgb(&src, &adj, Resize::None);
        let (w, h) = out.dimensions();
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                assert!(out.get_pixel(x, y).0[0] < 200, "white at ({x},{y})");
            }
        }
    }

    #[test]
    fn turn_crop_by_zero_is_identity() {
        let crop = rect(0.2, -0.1, 0.5, 0.9);
        for k in 0..4 {
            assert!(close_rect(turn_crop(crop, k, k), crop));
        }
    }

    #[test]
    fn turn_crop_round_trip() {
        let crop = rect(0.2, -0.1, 0.5, 0.9);
        let there = turn_crop(crop, 1, 3);
        assert!(close_rect(turn_crop(there, 3, 1), crop));
    }

    #[test]
    fn frame_corners_of_plain_frame_are_its_corners() {
        let p = frame_corners(rect(0.1, -0.2, 0.4, 0.6));
        let want = [(-0.1, -0.5), (0.3, -0.5), (0.3, 0.1), (-0.1, 0.1)];
        for (got, want) in p.iter().zip(want) {
            assert!(close(got.0, want.0) && close(got.1, want.1), "{p:?}");
        }
    }

    #[test]
    fn frame_corners_add_the_shift() {
        let crop = shifted(
            rect(0.0, 0.0, 0.6, 0.6),
            [(0.07, 0.0), (-0.07, 0.01), (0.0, 0.0), (0.02, -0.03)],
        );
        let p = frame_corners(crop);
        let want = [(-0.23, -0.3), (0.23, -0.29), (0.3, 0.3), (-0.28, 0.27)];
        for (got, want) in p.iter().zip(want) {
            assert!(close(got.0, want.0) && close(got.1, want.1), "{p:?}");
        }
    }

    #[test]
    fn rectangle_is_valid_quad() {
        assert!(quad_ok(CropRect::default()));
        assert!(quad_ok(rect(0.1, -0.2, 0.4, 0.6)));
    }

    #[test]
    fn crossed_corners_are_not_valid() {
        let crop = shifted(
            rect(0.0, 0.0, 0.6, 0.6),
            [(0.7, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        );
        assert!(!quad_ok(crop));
    }

    #[test]
    fn dented_frame_is_not_valid() {
        let crop = shifted(
            rect(0.0, 0.0, 0.6, 0.6),
            [(0.0, 0.0), (0.0, 0.28), (-0.28, -0.28), (0.28, 0.0)],
        );
        assert!(!quad_ok(crop));
    }

    #[test]
    fn nan_shift_is_not_valid() {
        let crop = shifted(
            rect(0.0, 0.0, 0.6, 0.6),
            [(f32::NAN, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        );
        assert!(!quad_ok(crop));
    }

    fn same_point(a: (f32, f32), b: (f32, f32)) -> bool {
        close(a.0, b.0) && close(a.1, b.1)
    }

    const TRAPEZOID: [(f32, f32); 4] = [(-2.0, -3.0), (2.0, -3.0), (4.0, 3.0), (-4.0, 3.0)];

    #[test]
    fn square_to_quad_hits_the_corners() {
        let map = SquareToQuad::new(TRAPEZOID);
        for (unit, want) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
            .into_iter()
            .zip(TRAPEZOID)
        {
            let got = map.apply(unit.0, unit.1);
            assert!(same_point(got, want), "{unit:?} -> {got:?}");
        }
    }

    #[test]
    fn square_to_quad_centre_is_where_diagonals_cross() {
        let got = SquareToQuad::new(TRAPEZOID).apply(0.5, 0.5);
        assert!(same_point(got, (0.0, -1.0)), "{got:?}");
    }

    #[test]
    fn square_to_quad_keeps_lines_straight() {
        let map = SquareToQuad::new(TRAPEZOID);
        for (t, want) in [(0.0, -3.0), (0.5, -1.0), (1.0, 3.0)] {
            let got = map.apply(0.5, t);
            assert!(same_point(got, (0.0, want)), "t {t} -> {got:?}");
        }
    }

    #[test]
    fn square_to_quad_of_rectangle_is_linear() {
        let map = SquareToQuad::new([(-4.0, -3.0), (4.0, -3.0), (4.0, 3.0), (-4.0, 3.0)]);
        let got = map.apply(0.25, 0.75);
        assert!(same_point(got, (-2.0, 1.5)), "{got:?}");
    }

    #[test]
    fn fit_scales_the_shift_too() {
        let start = shifted(
            rect(0.1, 0.0, 0.9, 0.9),
            [(0.05, 0.02), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        );
        let fitted = fit_inside(start, base_1600(), 6.0);
        let k = fitted.w / start.w;
        assert!(k < 1.0, "the frame should have shrunk");
        assert!(close(fitted.corner_shift[0].0, 0.05 * k));
        assert!(close(fitted.corner_shift[0].1, 0.02 * k));
        assert!(frame_inside(fitted, base_1600(), 6.0));
    }

    #[test]
    fn shifted_frame_outside_is_not_inside() {
        let crop = shifted(
            CropRect::default(),
            [(-0.05, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        );
        assert!(!frame_inside(crop, base_1600(), 0.0));
    }

    #[test]
    fn output_size_of_trapezoid() {
        let base = Size {
            w: 1000.0,
            h: 1000.0,
        };
        let crop = shifted(
            rect(0.0, 0.0, 0.8, 0.6),
            [(0.1, 0.0), (-0.1, 0.0), (0.0, 0.0), (0.0, 0.0)],
        );
        assert_eq!(output_size(base, crop, Resize::None), (700, 608));
    }

    #[test]
    fn quarter_turn_moves_shifts_round() {
        let start = Adjust {
            crop: shifted(
                CropRect::default(),
                [(0.01, 0.02), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
            ),
            ..Adjust::default()
        };
        let cw = quarter_turn(start, true).crop.corner_shift;
        assert!(same_point(cw[1], (-0.02, 0.01)), "{cw:?}");
        assert!(same_point(cw[0], (0.0, 0.0)), "{cw:?}");
        let ccw = quarter_turn(start, false).crop.corner_shift;
        assert!(same_point(ccw[3], (0.02, -0.01)), "{ccw:?}");
    }
}
