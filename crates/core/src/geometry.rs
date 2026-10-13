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
}
