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

/// The four corners of a frame as signs: left/right, top/bottom.
const CORNERS: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];

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

/// Whether all four corners of the frame are inside (see `point_inside`).
pub fn frame_inside(crop: CropRect, base: Size, angle_deg: f32) -> bool {
    CORNERS.iter().all(|&(sx, sy)| {
        let corner = (crop.cx + sx * crop.w / 2.0, crop.cy + sy * crop.h / 2.0);
        point_inside(corner, base, angle_deg)
    })
}

/// The frame shrunk about its centre, keeping its shape, until it lies inside the upright base
/// rectangle and inside the picture turned by `angle_deg`. A frame already inside comes back
/// unchanged; the frame never grows. A frame whose centre is outside is first moved to the middle.
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
    let mut s: f32 = 1.0;
    // Upright rectangle: |c ± s·half| <= 0.5 on each axis.
    for (c, half) in [(crop.cx, crop.w / 2.0), (crop.cy, crop.h / 2.0)] {
        if half > 0.0 {
            s = s.min((0.5 - c.abs()) / half);
        }
    }
    // Turned picture, in pixels: u + s·v must stay within -l..=l for every corner and axis,
    // where u is the turned centre and v the turned corner offset.
    let turn = turn_back(angle_deg);
    let (ux, uy) = turn(crop.cx * base.w, crop.cy * base.h);
    let (lx, ly) = (base.w / 2.0, base.h / 2.0);
    for (sx, sy) in CORNERS {
        let (vx, vy) = turn(sx * crop.w * base.w / 2.0, sy * crop.h * base.h / 2.0);
        for (u, v, l) in [(ux, vx, lx), (uy, vy, ly)] {
            if v > 1e-6 {
                s = s.min((l - u) / v);
            } else if v < -1e-6 {
                s = s.min((-l - u) / v);
            }
        }
    }
    let s = s.clamp(0.0, 1.0);
    CropRect {
        w: crop.w * s,
        h: crop.h * s,
        ..crop
    }
}

/// The output size of the cropped picture. The picture is never enlarged.
pub fn output_size(base: Size, crop: CropRect, resize: Resize) -> (u32, u32) {
    let mid_w = (crop.w * base.w).round().max(1.0);
    let mid_h = (crop.h * base.h).round().max(1.0);
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
/// with the picture, so the same part of the page stays framed.
pub fn quarter_turn(a: Adjust, clockwise: bool) -> Adjust {
    let c = a.crop;
    let current = a.quarter_turns % 4;
    let (quarter_turns, crop) = if clockwise {
        (
            (current + 1) % 4,
            CropRect {
                cx: -c.cy,
                cy: c.cx,
                w: c.h,
                h: c.w,
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
            },
        )
    };
    Adjust {
        quarter_turns,
        crop,
        ..a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Adjust, CropRect};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn close_rect(a: CropRect, b: CropRect) -> bool {
        close(a.cx, b.cx) && close(a.cy, b.cy) && close(a.w, b.w) && close(a.h, b.h)
    }

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
        CropRect { cx, cy, w, h }
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
}
