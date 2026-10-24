---
name: image-geometry
description: Coordinate systems, rotation direction, crop-frame units and the output-pixel-to-source-pixel formula used by pagecrop. Use for any work in crates/core geometry.rs, crop_edit.rs or render.rs, and for drawing or dragging the crop frame in the editor.
---

# Image geometry in pagecrop

Every rule here is fixed. The preview in the app and the export in core must use the same rules, or the
exported page will not match what the user framed.

## Names

| Name | Meaning |
|---|---|
| source `S` | The photo after its EXIF orientation is applied. Size `(w0, h0)` in pixels. |
| `k` | `quarter_turns`, 0 to 3. Each is 90° clockwise. |
| `a` | `angle_deg`, the fine angle. Positive turns the picture clockwise on screen. |
| base `B` | The source after the quarter turns. Size `(wb, hb)`: equal to `(w0, h0)` when `k` is even, `(h0, w0)` when `k` is odd. |
| total | `k * 90 + a` degrees. `t` is the same in radians. |
| frame space | The upright space the user looks at. Origin at the image centre, x to the right, y **down**. |

## The crop frame

The frame is always an upright rectangle in frame space. The picture turns underneath it.

`CropRect { cx, cy, w, h }` is stored without pixels:

- `cx`, `w` are fractions of `wb`. `cy`, `h` are fractions of `hb`.
- `cx = 0, cy = 0` is the image centre. The full image is `{ cx: 0, cy: 0, w: 1, h: 1 }`.

In pixels: centre `(cx * wb, cy * hb)`, size `(w * wb, h * hb)`.
Edges: `left = cx - w/2`, `right = cx + w/2`, `top = cy - h/2`, `bottom = cy + h/2`.

Because the units are fractions, the same `CropRect` works on the full photo and on a small preview.

## Rotation

Screen coordinates have y pointing down. With that, this matrix turns a point **clockwise** for a
positive `t`:

```
display:  px = qx * cos(t) - qy * sin(t)
          py = qx * sin(t) + qy * cos(t)
```

`q` is a point of the source measured from the source centre. `p` is where it appears in frame space.

Export needs the opposite direction, from a frame point back to the source:

```
inverse:  qx =  px * cos(t) + py * sin(t)
          qy = -px * sin(t) + py * cos(t)
source pixel position:  sx = qx + w0 / 2,   sy = qy + h0 / 2
```

### Worked example 1 (quarter turn)

`w0 = 400, h0 = 300, k = 1, a = 0`. Then `wb = 300, hb = 400`, `t = 90°`, `cos = 0`, `sin = 1`.
Frame top-left corner `p = (-150, -200)`:
`qx = py = -200`, `qy = -px = 150`, so `sx = 0`, `sy = 300`.
That is the bottom-left corner of the source. Correct: turning a photo clockwise brings its bottom-left
corner to the top-left.

### Worked example 2 (no rotation)

`k = 0, a = 0`: `q = p`, so frame point `(0, 0)` maps to the source centre `(w0/2, h0/2)`.

## From an output pixel to the source

Output image size `(ow, oh)`. Crop size in pixels `cw = w * wb`, `ch = h * hb`.
For output pixel `(u, v)` (integers, pixel centre at `+0.5`):

```
px = cx * wb + ((u + 0.5) / ow - 0.5) * cw
py = cy * hb + ((v + 0.5) / oh - 0.5) * ch
(sx, sy) = inverse rotation of (px, py), plus (w0/2, h0/2)
```

`(sx, sy)` is a continuous position where pixel `i` covers `i .. i+1` and its centre is `i + 0.5`.
For bilinear sampling use `fx = sx - 0.5`, `fy = sy - 0.5`, then the four neighbours
`floor(fx)`, `floor(fx)+1`, `floor(fy)`, `floor(fy)+1`.

A neighbour outside the source uses the fill colour, white `(255, 255, 255)`.

Check: with `k = 0`, `a = 0`, full crop and `ow = w0`, pixel `u` gives `sx = u + 0.5`, `fx = u`.
The output equals the source exactly.

## Output size

```
mid = (round(cw).max(1), round(ch).max(1))          size after rotate and crop
Resize::None        -> mid
Resize::LongEdge(n) -> s = min(1, n / max(mid.w, mid.h))
Resize::Percent(p)  -> s = clamp(p / 100, 0.01, 1)
output = (round(mid.w * s).max(1), round(mid.h * s).max(1))
```

The scale never goes above 1. The app does not enlarge.

## Pressing a 90° button

The frame must turn with the picture, so the same part of the page stays framed.

- Clockwise: `k = (k + 1) % 4` and `{cx, cy, w, h}` becomes `{-cy, cx, h, w}`.
- Counter-clockwise: `k = (k + 3) % 4` and `{cx, cy, w, h}` becomes `{cy, -cx, h, w}`.

## How far the frame may go

The frame may reach the upright box around the base turned by the fine angle `a` (not `k`):

```
half_x = (wb * |cos a| + hb * |sin a|) / (2 * wb)
half_y = (wb * |sin a| + hb * |cos a|) / (2 * hb)
```

Frame edges are clamped to `-half_x ..= half_x` and `-half_y ..= half_y` while dragging. Parts of the
frame outside the photo come out white.

## Frame stays inside

The picture after its quarter turns is the base. The fine angle turns it about its centre, and on
screen it is clipped to the upright base rectangle. The frame, in fractions of the base, must lie inside
both:

1. the upright base rectangle: every corner has `|x| <= 0.5` and `|y| <= 0.5`, and
2. the turned picture: every corner, turned back by the fine angle, lies within the base rectangle.

Use `geometry::{point_inside, frame_inside, fit_inside}` and `crop_edit::drag_inside`. `fit_inside`
shrinks about the frame's centre and never grows it. The turn back is the formula of `frame_to_src` with
only the fine angle; do not write a second version. `half_extents` is no longer used for dragging.

## Screen and frame space in the editor

`zoom` is screen points per base pixel. `c` is the screen position of the image centre.

```
frame fraction -> screen:  sx = c.x + nx * wb * zoom,   sy = c.y + ny * hb * zoom
screen -> frame fraction:  nx = (sx - c.x) / (wb * zoom), ny = (sy - c.y) / (hb * zoom)
```

Use the preview texture size as `(w0, h0)` in the editor. Only the shape matters there.

## Mistakes that have a known symptom

| Symptom | Cause |
|---|---|
| Export is turned the opposite way to the preview | The inverse formula was used for display, or the sign of `sin` is flipped in one place. |
| Export is shifted by half a pixel, or blurry at angle 0 | The `+ 0.5` / `- 0.5` pixel-centre steps are missing. |
| Frame jumps when a 90° button is pressed | The crop was not transformed together with `k`. |
| Frame has the wrong shape on portrait pages | `w0, h0` were used where `wb, hb` belong. |
| A tall photo arrives lying on its side | EXIF orientation was not applied before anything else. |
