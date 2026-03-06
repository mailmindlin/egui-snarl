use core::f32;

use egui::{Context, Id, Pos2, Rect, Shape, Stroke, Ui, ahash::HashMap, cache::CacheTrait, pos2};

use crate::{InPinId, OutPinId};

const MAX_CURVE_SAMPLES: usize = 100;

/// Layer where wires are rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "egui-probe", derive(egui_probe::EguiProbe))]
#[cfg_attr(feature = "facet", derive(facet::Facet), repr(u8))]
#[derive(Default)]
pub enum WireLayer {
    /// Wires are rendered behind nodes.
    /// This is default.
    #[default]
    BehindNodes,

    /// Wires are rendered above nodes.
    AboveNodes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WireId {
    Connected {
        snarl_id: Id,
        out_pin: OutPinId,
        in_pin: InPinId,
    },
    NewInput {
        snarl_id: Id,
        in_pin: InPinId,
    },
    NewOutput {
        snarl_id: Id,
        out_pin: OutPinId,
    },
}

/// Controls style in which wire is rendered.
///
/// Variants are given in order of precedence when two pins require different styles.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "egui-probe", derive(egui_probe::EguiProbe))]
#[cfg_attr(feature = "facet", derive(facet::Facet), repr(u8))]
#[derive(Default)]
pub enum WireStyle {
    /// Straight line from one endpoint to another.
    Line,

    /// Draw wire as straight lines with 90 degree turns.
    /// Corners has radius of `corner_radius`.
    AxisAligned {
        /// Radius of corners in wire.
        corner_radius: f32,
    },

    /// Draw wire as 3rd degree Bezier curve.
    Bezier3,

    /// Draw wire as 5th degree Bezier curve.
    #[default]
    Bezier5,
}

pub const fn pick_wire_style(left: WireStyle, right: WireStyle) -> WireStyle {
    match (left, right) {
        (WireStyle::Line, _) | (_, WireStyle::Line) => WireStyle::Line,
        (
            WireStyle::AxisAligned { corner_radius: a },
            WireStyle::AxisAligned { corner_radius: b },
        ) => WireStyle::AxisAligned {
            corner_radius: f32::max(a, b),
        },
        (WireStyle::AxisAligned { corner_radius }, _)
        | (_, WireStyle::AxisAligned { corner_radius }) => WireStyle::AxisAligned { corner_radius },
        (WireStyle::Bezier3, _) | (_, WireStyle::Bezier3) => WireStyle::Bezier3,
        (WireStyle::Bezier5, WireStyle::Bezier5) => WireStyle::Bezier5,
    }
}

fn adjust_frame_size(
    mut frame_size: f32,
    upscale: bool,
    downscale: bool,
    from: Pos2,
    to: Pos2,
) -> f32 {
    let length = (from - to).length();
    if upscale {
        frame_size = frame_size.max(length / 6.0);
    }
    if downscale {
        frame_size = frame_size.min(length / 6.0);
    }
    frame_size
}

/// Returns 5th degree bezier curve control points for horizontal wires (pins on left/right)
fn wire_bezier_5_horizontal(frame_size: f32, from: Pos2, to: Pos2) -> [Pos2; 6] {
    let from_norm_x = frame_size;
    let from_2 = pos2(from.x + from_norm_x, from.y);
    let to_norm_x = -from_norm_x;
    let to_2 = pos2(to.x + to_norm_x, to.y);

    let between = (from_2 - to_2).length();

    if from_2.x <= to_2.x && between >= frame_size * 2.0 {
        let middle_1 = from_2 + (to_2 - from_2).normalized() * frame_size;
        let middle_2 = to_2 + (from_2 - to_2).normalized() * frame_size;

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if from_2.x <= to_2.x {
        let t = (between - (to_2.y - from_2.y).abs())
            / frame_size.mul_add(2.0, -(to_2.y - from_2.y).abs());

        let mut middle_1 = from_2 + (to_2 - from_2).normalized() * frame_size;
        let mut middle_2 = to_2 + (from_2 - to_2).normalized() * frame_size;

        if from_2.y >= to_2.y + frame_size {
            let u = (from_2.y - to_2.y - frame_size) / frame_size;

            let t0_middle_1 = pos2(
                (1.0 - u).mul_add(frame_size, from_2.x),
                frame_size.mul_add(-u, from_2.y),
            );
            let t0_middle_2 = pos2(to_2.x, to_2.y + frame_size);

            middle_1 = t0_middle_1.lerp(middle_1, t);
            middle_2 = t0_middle_2.lerp(middle_2, t);
        } else if from_2.y >= to_2.y {
            let u = (from_2.y - to_2.y) / frame_size;

            let t0_middle_1 = pos2(
                u.mul_add(frame_size, from_2.x),
                frame_size.mul_add(1.0 - u, from_2.y),
            );
            let t0_middle_2 = pos2(to_2.x, to_2.y + frame_size);

            middle_1 = t0_middle_1.lerp(middle_1, t);
            middle_2 = t0_middle_2.lerp(middle_2, t);
        } else if to_2.y >= from_2.y + frame_size {
            let u = (to_2.y - from_2.y - frame_size) / frame_size;

            let t0_middle_1 = pos2(from_2.x, from_2.y + frame_size);
            let t0_middle_2 = pos2(
                (1.0 - u).mul_add(-frame_size, to_2.x),
                frame_size.mul_add(-u, to_2.y),
            );

            middle_1 = t0_middle_1.lerp(middle_1, t);
            middle_2 = t0_middle_2.lerp(middle_2, t);
        } else if to_2.y >= from_2.y {
            let u = (to_2.y - from_2.y) / frame_size;

            let t0_middle_1 = pos2(from_2.x, from_2.y + frame_size);
            let t0_middle_2 = pos2(
                u.mul_add(-frame_size, to_2.x),
                frame_size.mul_add(1.0 - u, to_2.y),
            );

            middle_1 = t0_middle_1.lerp(middle_1, t);
            middle_2 = t0_middle_2.lerp(middle_2, t);
        } else {
            unreachable!();
        }

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if from_2.y >= frame_size.mul_add(2.0, to_2.y) {
        let middle_1 = pos2(from_2.x, from_2.y - frame_size);
        let middle_2 = pos2(to_2.x, to_2.y + frame_size);

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if from_2.y >= to_2.y + frame_size {
        let t = (from_2.y - to_2.y - frame_size) / frame_size;

        let middle_1 = pos2(
            (1.0 - t).mul_add(frame_size, from_2.x),
            frame_size.mul_add(-t, from_2.y),
        );
        let middle_2 = pos2(to_2.x, to_2.y + frame_size);

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if from_2.y >= to_2.y {
        let t = (from_2.y - to_2.y) / frame_size;

        let middle_1 = pos2(
            t.mul_add(frame_size, from_2.x),
            frame_size.mul_add(1.0 - t, from_2.y),
        );
        let middle_2 = pos2(to_2.x, to_2.y + frame_size);

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if to_2.y >= frame_size.mul_add(2.0, from_2.y) {
        let middle_1 = pos2(from_2.x, from_2.y + frame_size);
        let middle_2 = pos2(to_2.x, to_2.y - frame_size);

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if to_2.y >= from_2.y + frame_size {
        let t = (to_2.y - from_2.y - frame_size) / frame_size;

        let middle_1 = pos2(from_2.x, from_2.y + frame_size);
        let middle_2 = pos2(
            (1.0 - t).mul_add(-frame_size, to_2.x),
            frame_size.mul_add(-t, to_2.y),
        );

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if to_2.y >= from_2.y {
        let t = (to_2.y - from_2.y) / frame_size;

        let middle_1 = pos2(from_2.x, from_2.y + frame_size);
        let middle_2 = pos2(
            t.mul_add(-frame_size, to_2.x),
            frame_size.mul_add(1.0 - t, to_2.y),
        );

        [from, from_2, middle_1, middle_2, to_2, to]
    } else {
        unreachable!();
    }
}

/// Returns 5th degree bezier curve control points for vertical wires (pins on top/bottom).
/// This creates Houdini-style connections where wires extend vertically from pins.
fn wire_bezier_5_vertical(frame_size: f32, from: Pos2, to: Pos2) -> [Pos2; 6] {
    // When target is above source, reverse the computation so the wire
    // exits upward and enters downward, avoiding the overshoot loop.
    if to.y < from.y {
        let [a, b, c, d, e, f] = wire_bezier_5_vertical_down(frame_size, to, from);
        return [f, e, d, c, b, a];
    }
    wire_bezier_5_vertical_down(frame_size, from, to)
}

/// Vertical wire bezier for the "forward" case where `to` is below `from`.
fn wire_bezier_5_vertical_down(frame_size: f32, from: Pos2, to: Pos2) -> [Pos2; 6] {
    // Control points extend in Y direction: exit downward, enter upward.
    let from_norm_y = frame_size;
    let from_2 = pos2(from.x, from.y + from_norm_y);
    let to_norm_y = -from_norm_y;
    let to_2 = pos2(to.x, to.y + to_norm_y);

    let between = (from_2 - to_2).length();

    if from_2.y <= to_2.y && between >= frame_size * 2.0 {
        // Simple case: from is above to, enough space for smooth curve
        let middle_1 = from_2 + (to_2 - from_2).normalized() * frame_size;
        let middle_2 = to_2 + (from_2 - to_2).normalized() * frame_size;

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if from_2.y <= to_2.y {
        // from is above to, but close together
        let t = (between - (to_2.x - from_2.x).abs())
            / frame_size.mul_add(2.0, -(to_2.x - from_2.x).abs());

        let mut middle_1 = from_2 + (to_2 - from_2).normalized() * frame_size;
        let mut middle_2 = to_2 + (from_2 - to_2).normalized() * frame_size;

        if from_2.x >= to_2.x + frame_size {
            let u = (from_2.x - to_2.x - frame_size) / frame_size;

            let t0_middle_1 = pos2(
                frame_size.mul_add(-u, from_2.x),
                (1.0 - u).mul_add(frame_size, from_2.y),
            );
            let t0_middle_2 = pos2(to_2.x + frame_size, to_2.y);

            middle_1 = t0_middle_1.lerp(middle_1, t);
            middle_2 = t0_middle_2.lerp(middle_2, t);
        } else if from_2.x >= to_2.x {
            let u = (from_2.x - to_2.x) / frame_size;

            let t0_middle_1 = pos2(
                frame_size.mul_add(1.0 - u, from_2.x),
                u.mul_add(frame_size, from_2.y),
            );
            let t0_middle_2 = pos2(to_2.x + frame_size, to_2.y);

            middle_1 = t0_middle_1.lerp(middle_1, t);
            middle_2 = t0_middle_2.lerp(middle_2, t);
        } else if to_2.x >= from_2.x + frame_size {
            let u = (to_2.x - from_2.x - frame_size) / frame_size;

            let t0_middle_1 = pos2(from_2.x + frame_size, from_2.y);
            let t0_middle_2 = pos2(
                frame_size.mul_add(-u, to_2.x),
                (1.0 - u).mul_add(-frame_size, to_2.y),
            );

            middle_1 = t0_middle_1.lerp(middle_1, t);
            middle_2 = t0_middle_2.lerp(middle_2, t);
        } else if to_2.x >= from_2.x {
            let u = (to_2.x - from_2.x) / frame_size;

            let t0_middle_1 = pos2(from_2.x + frame_size, from_2.y);
            let t0_middle_2 = pos2(
                frame_size.mul_add(1.0 - u, to_2.x),
                u.mul_add(-frame_size, to_2.y),
            );

            middle_1 = t0_middle_1.lerp(middle_1, t);
            middle_2 = t0_middle_2.lerp(middle_2, t);
        } else {
            unreachable!();
        }

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if from_2.x >= frame_size.mul_add(2.0, to_2.x) {
        // from is to the right, wrap around
        let middle_1 = pos2(from_2.x - frame_size, from_2.y);
        let middle_2 = pos2(to_2.x + frame_size, to_2.y);

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if from_2.x >= to_2.x + frame_size {
        let t = (from_2.x - to_2.x - frame_size) / frame_size;

        let middle_1 = pos2(
            frame_size.mul_add(-t, from_2.x),
            (1.0 - t).mul_add(frame_size, from_2.y),
        );
        let middle_2 = pos2(to_2.x + frame_size, to_2.y);

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if from_2.x >= to_2.x {
        let t = (from_2.x - to_2.x) / frame_size;

        let middle_1 = pos2(
            frame_size.mul_add(1.0 - t, from_2.x),
            t.mul_add(frame_size, from_2.y),
        );
        let middle_2 = pos2(to_2.x + frame_size, to_2.y);

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if to_2.x >= frame_size.mul_add(2.0, from_2.x) {
        // to is to the right
        let middle_1 = pos2(from_2.x + frame_size, from_2.y);
        let middle_2 = pos2(to_2.x - frame_size, to_2.y);

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if to_2.x >= from_2.x + frame_size {
        let t = (to_2.x - from_2.x - frame_size) / frame_size;

        let middle_1 = pos2(from_2.x + frame_size, from_2.y);
        let middle_2 = pos2(
            frame_size.mul_add(-t, to_2.x),
            (1.0 - t).mul_add(-frame_size, to_2.y),
        );

        [from, from_2, middle_1, middle_2, to_2, to]
    } else if to_2.x >= from_2.x {
        let t = (to_2.x - from_2.x) / frame_size;

        let middle_1 = pos2(from_2.x + frame_size, from_2.y);
        let middle_2 = pos2(
            frame_size.mul_add(1.0 - t, to_2.x),
            t.mul_add(-frame_size, to_2.y),
        );

        [from, from_2, middle_1, middle_2, to_2, to]
    } else {
        unreachable!();
    }
}

/// Returns 3rd degree bezier curve control points for the wire
fn wire_bezier_3(frame_size: f32, from: Pos2, to: Pos2) -> [Pos2; 4] {
    let [a, b, _, _, c, d] = wire_bezier_5_horizontal(frame_size, from, to);
    [a, b, c, d]
}

/// Returns 3rd degree bezier curve control points for vertical wires
fn wire_bezier_3_vertical(frame_size: f32, from: Pos2, to: Pos2) -> [Pos2; 4] {
    let [a, b, _, _, c, d] = wire_bezier_5_vertical(frame_size, from, to);
    [a, b, c, d]
}

#[allow(clippy::too_many_arguments)]
pub fn draw_wire(
    ui: &Ui,
    wire: WireId,
    shapes: &mut Vec<Shape>,
    frame_size: f32,
    upscale: bool,
    downscale: bool,
    from: Pos2,
    to: Pos2,
    mut stroke: Stroke,
    threshold: f32,
    style: WireStyle,
    vertical: bool,
) {
    if !ui.is_visible() {
        return;
    }

    if stroke.width < 1.0 {
        stroke.color = stroke.color.gamma_multiply(stroke.width);
        stroke.width = 1.0;
    }

    let frame_size = adjust_frame_size(frame_size, upscale, downscale, from, to);

    let args = WireArgs {
        frame_size,
        from,
        to,
        radius: 0.0,
        vertical,
    };

    match style {
        WireStyle::Line => {
            let bb = Rect::from_two_pos(from, to);
            if ui.is_rect_visible(bb) {
                shapes.push(Shape::line_segment([from, to], stroke));
            }
        }
        WireStyle::Bezier3 => {
            draw_bezier_3(ui, wire, args, stroke, threshold, shapes);
        }

        WireStyle::Bezier5 => {
            draw_bezier_5(ui, wire, args, stroke, threshold, shapes);
        }

        WireStyle::AxisAligned { corner_radius } => {
            let args = WireArgs {
                radius: corner_radius,
                ..args
            };
            draw_axis_aligned(ui, wire, args, stroke, threshold, shapes);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn hit_wire(
    ctx: &Context,
    wire: WireId,
    frame_size: f32,
    upscale: bool,
    downscale: bool,
    from: Pos2,
    to: Pos2,
    pos: Pos2,
    hit_threshold: f32,
    style: WireStyle,
    vertical: bool,
) -> bool {
    let frame_size = adjust_frame_size(frame_size, upscale, downscale, from, to);

    let args = WireArgs {
        frame_size,
        from,
        to,
        radius: 0.0,
        vertical,
    };

    match style {
        WireStyle::Line => {
            let aabb = Rect::from_two_pos(from, to);
            let aabb_e = aabb.expand(hit_threshold);
            if !aabb_e.contains(pos) {
                return false;
            }

            let a = to - from;
            let b = pos - from;

            let dot = b.dot(a);
            let dist2 = b.length_sq() - dot * dot / a.length_sq();

            dist2 < hit_threshold * hit_threshold
        }
        WireStyle::Bezier3 => hit_wire_bezier_3(ctx, wire, args, pos, hit_threshold),
        WireStyle::Bezier5 => hit_wire_bezier_5(ctx, wire, args, pos, hit_threshold),
        WireStyle::AxisAligned { corner_radius } => {
            let args = WireArgs {
                radius: corner_radius,
                ..args
            };
            hit_wire_axis_aligned(ctx, wire, args, pos, hit_threshold)
        }
    }
}

#[inline]
fn bezier_arc_length_upper_bound(points: &[Pos2]) -> f32 {
    let mut size = 0.0;
    for i in 1..points.len() {
        size += (points[i] - points[i - 1]).length();
    }
    size
}

fn bezier_hit_samples_number(points: &[Pos2], threshold: f32) -> usize {
    let arc_length = bezier_arc_length_upper_bound(points);

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    ((arc_length / threshold).ceil().max(0.0) as usize)
}

fn bezier_derivative_3(points: &[Pos2; 4]) -> [Pos2; 3] {
    let [p0, p1, p2, p3] = *points;

    let factor = 3.0;

    [
        (factor * (p1 - p0)).to_pos2(),
        (factor * (p2 - p1)).to_pos2(),
        (factor * (p3 - p2)).to_pos2(),
    ]
}

fn bezier_derivative_5(points: &[Pos2; 6]) -> [Pos2; 5] {
    let [p0, p1, p2, p3, p4, p5] = *points;

    let factor = 5.0;

    [
        (factor * (p1 - p0)).to_pos2(),
        (factor * (p2 - p1)).to_pos2(),
        (factor * (p3 - p2)).to_pos2(),
        (factor * (p4 - p3)).to_pos2(),
        (factor * (p5 - p4)).to_pos2(),
    ]
}

fn bezier_draw_samples_number_3(points: &[Pos2; 4], threshold: f32) -> usize {
    #![allow(clippy::similar_names)]
    #![allow(clippy::cast_precision_loss)]

    let d = bezier_derivative_3(points);

    lower_bound(2, MAX_CURVE_SAMPLES, |n| {
        let mut prev = points[0];
        for i in 1..n {
            let t = i as f32 / (n - 1) as f32;
            let next = sample_bezier(points, t);

            let m = t - 0.5 / (n - 1) as f32;

            // Compare absolute error of mid point
            let mid_line = ((prev.to_vec2() + next.to_vec2()) * 0.5).to_pos2();
            let mid_curve = sample_bezier(points, m);

            let error_sq = (mid_curve - mid_line).length_sq();
            if error_sq > threshold * threshold {
                return false;
            }

            // Compare angular error of mid point
            let mid_line_dx = next.x - prev.x;
            let mid_line_dy = next.y - prev.y;

            let line_w = f32::hypot(mid_line_dx, mid_line_dy);

            let d_curve = sample_bezier(&d, m);
            let mid_curve_dx = d_curve.x;
            let mid_curve_dy = d_curve.y;

            let curve_w = f32::hypot(mid_curve_dx, mid_curve_dy);

            let error = f32::max(
                (mid_curve_dx / curve_w).mul_add(line_w, -mid_line_dx).abs(),
                (mid_curve_dy / curve_w).mul_add(line_w, -mid_line_dy).abs(),
            );
            if error > threshold * 2.0 {
                return false;
            }

            prev = next;
        }

        true
    })
}

fn bezier_draw_samples_number_5(points: &[Pos2; 6], threshold: f32) -> usize {
    #![allow(clippy::similar_names)]
    #![allow(clippy::cast_precision_loss)]

    let d = bezier_derivative_5(points);

    lower_bound(2, MAX_CURVE_SAMPLES, |n| {
        let mut prev = points[0];
        for i in 1..n {
            let t = i as f32 / (n - 1) as f32;
            let next = sample_bezier(points, t);

            let m = t - 0.5 / (n - 1) as f32;

            // Compare absolute error of mid point
            let mid_line = ((prev.to_vec2() + next.to_vec2()) * 0.5).to_pos2();
            let mid_curve = sample_bezier(points, m);

            let error_sq = (mid_curve - mid_line).length_sq();
            if error_sq > threshold * threshold {
                return false;
            }

            // Compare angular error of mid point
            let mid_line_dx = next.x - prev.x;
            let mid_line_dy = next.y - prev.y;

            let line_w = f32::hypot(mid_line_dx, mid_line_dy);

            let d_curve = sample_bezier(&d, m);
            let mid_curve_dx = d_curve.x;
            let mid_curve_dy = d_curve.y;

            let curve_w = f32::hypot(mid_curve_dx, mid_curve_dy);

            let error = f32::max(
                (mid_curve_dx / curve_w).mul_add(line_w, -mid_line_dx).abs(),
                (mid_curve_dy / curve_w).mul_add(line_w, -mid_line_dy).abs(),
            );
            if error > threshold * 2.0 {
                return false;
            }

            prev = next;
        }

        true
    })
}

#[derive(Clone, Copy, PartialEq)]
struct WireArgs {
    frame_size: f32,
    from: Pos2,
    to: Pos2,
    radius: f32,
    vertical: bool,
}

impl Default for WireArgs {
    fn default() -> Self {
        WireArgs {
            frame_size: 0.0,
            from: Pos2::ZERO,
            to: Pos2::ZERO,
            radius: 0.0,
            vertical: false,
        }
    }
}

struct WireCache3 {
    generation: u32,
    args: WireArgs,
    aabb: Rect,
    points: [Pos2; 4],
    threshold: f32,
    line: Vec<Pos2>,
}

impl Default for WireCache3 {
    fn default() -> Self {
        WireCache3 {
            generation: 0,
            args: WireArgs::default(),
            aabb: Rect::NOTHING,
            points: [Pos2::ZERO; 4],
            threshold: 0.0,
            line: Vec::new(),
        }
    }
}

impl WireCache3 {
    fn line(&mut self, threshold: f32) -> Vec<Pos2> {
        #[allow(clippy::float_cmp)]
        if !self.line.is_empty() && self.threshold == threshold {
            return self.line.clone();
        }

        let samples = bezier_draw_samples_number_3(&self.points, threshold);

        let line = (0..samples)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let t = i as f32 / (samples - 1) as f32;
                sample_bezier(&self.points, t)
            })
            .collect::<Vec<Pos2>>();

        self.threshold = threshold;
        self.line.clone_from(&line);

        line
    }
}

struct WireCache5 {
    generation: u32,
    args: WireArgs,
    aabb: Rect,
    points: [Pos2; 6],
    threshold: f32,
    line: Vec<Pos2>,
}

impl Default for WireCache5 {
    fn default() -> Self {
        Self {
            generation: 0,
            args: WireArgs::default(),
            aabb: Rect::NOTHING,
            points: [Pos2::ZERO; 6],
            threshold: 0.0,
            line: Vec::new(),
        }
    }
}

impl WireCache5 {
    fn line(&mut self, threshold: f32) -> Vec<Pos2> {
        #[allow(clippy::float_cmp)]
        if !self.line.is_empty() && self.threshold == threshold {
            return self.line.clone();
        }

        let samples = bezier_draw_samples_number_5(&self.points, threshold);

        let line = (0..samples)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let t = i as f32 / (samples - 1) as f32;
                sample_bezier(&self.points, t)
            })
            .collect::<Vec<Pos2>>();

        self.threshold = threshold;
        self.line.clone_from(&line);

        line
    }
}

#[derive(Default)]
struct WireCacheAA {
    generation: u32,
    args: WireArgs,
    aawire: AxisAlignedWire,
    threshold: f32,
    line: Vec<Pos2>,
}

impl WireCacheAA {
    fn line(&mut self, threshold: f32) -> Vec<Pos2> {
        #[allow(clippy::float_cmp)]
        if !self.line.is_empty() && self.threshold == threshold {
            return self.line.clone();
        }

        let mut line = Vec::new();

        for i in 0..self.aawire.turns {
            // shapes.push(Shape::line_segment(
            //     [wire.segments[i].0, wire.segments[i].1],
            //     stroke,
            // ));

            // Draw segment first
            line.push(self.aawire.segments[i].0);
            line.push(self.aawire.segments[i].1);

            if self.aawire.turn_radii[i] > 0.0 {
                let turn = self.aawire.turn_centers[i];
                let samples = turn_samples_number(self.aawire.turn_radii[i], self.threshold);

                let start = self.aawire.segments[i].1;
                let end = self.aawire.segments[i + 1].0;

                let sin_x = end.x - turn.x;
                let cos_x = start.x - turn.x;

                let sin_y = end.y - turn.y;
                let cos_y = start.y - turn.y;

                for j in 1..samples {
                    #[allow(clippy::cast_precision_loss)]
                    let a = std::f32::consts::FRAC_PI_2 * (j as f32 / samples as f32);

                    let (sin_a, cos_a) = a.sin_cos();

                    let point: Pos2 = pos2(
                        cos_x.mul_add(cos_a, sin_x.mul_add(sin_a, turn.x)),
                        cos_y.mul_add(cos_a, sin_y.mul_add(sin_a, turn.y)),
                    );
                    line.push(point);
                }
            }
        }

        line.push(self.aawire.segments[self.aawire.turns].0);
        line.push(self.aawire.segments[self.aawire.turns].1);

        self.threshold = threshold;
        self.line.clone_from(&line);

        line
    }
}

#[derive(Default)]
struct WiresCache {
    generation: u32,
    bezier_3: HashMap<WireId, WireCache3>,
    bezier_5: HashMap<WireId, WireCache5>,
    axis_aligned: HashMap<WireId, WireCacheAA>,
}

impl CacheTrait for WiresCache {
    fn update(&mut self) {
        self.bezier_3
            .retain(|_, cache| cache.generation == self.generation);
        self.bezier_5
            .retain(|_, cache| cache.generation == self.generation);
        self.axis_aligned
            .retain(|_, cache| cache.generation == self.generation);

        self.generation = self.generation.wrapping_add(1);
    }

    fn len(&self) -> usize {
        self.bezier_3.len() + self.bezier_5.len() + self.axis_aligned.len()
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl WiresCache {
    pub fn bezier_3(&mut self, wire: WireId, args: WireArgs) -> &mut WireCache3 {
        let cached = self.bezier_3.entry(wire).or_default();

        cached.generation = self.generation;

        if cached.args == args {
            return cached;
        }

        let points = if args.vertical {
            wire_bezier_3_vertical(args.frame_size, args.from, args.to)
        } else {
            wire_bezier_3(args.frame_size, args.from, args.to)
        };
        let aabb = Rect::from_points(&points);

        cached.args = args;
        cached.points = points;
        cached.aabb = aabb;
        cached.line.clear();

        cached
    }

    pub fn bezier_5(&mut self, wire: WireId, args: WireArgs) -> &mut WireCache5 {
        let cached = self.bezier_5.entry(wire).or_default();

        cached.generation = self.generation;

        if cached.args == args {
            return cached;
        }

        let points = if args.vertical {
            wire_bezier_5_vertical(args.frame_size, args.from, args.to)
        } else {
            wire_bezier_5_horizontal(args.frame_size, args.from, args.to)
        };
        let aabb = Rect::from_points(&points);

        cached.args = args;
        cached.points = points;
        cached.aabb = aabb;
        cached.line.clear();

        cached
    }

    #[allow(clippy::too_many_arguments)]
    pub fn axis_aligned(&mut self, wire: WireId, args: WireArgs) -> &mut WireCacheAA {
        let cached = self.axis_aligned.entry(wire).or_default();

        cached.generation = self.generation;

        if cached.args == args {
            return cached;
        }

        let aawire = wire_axis_aligned(args.radius, args.frame_size, args.from, args.to);

        cached.args = args;
        cached.aawire = aawire;
        cached.line.clear();

        cached
    }
}

#[inline(never)]
fn draw_bezier_3(
    ui: &Ui,
    wire: WireId,
    args: WireArgs,
    stroke: Stroke,
    threshold: f32,
    shapes: &mut Vec<Shape>,
) {
    debug_assert!(ui.is_visible(), "Must be checked earlier");

    let clip_rect = ui.clip_rect();

    ui.memory_mut(|m| {
        let cached = m.caches.cache::<WiresCache>().bezier_3(wire, args);

        if cached.aabb.intersects(clip_rect) {
            shapes.push(Shape::line(cached.line(threshold), stroke));
        }
    });

    // {
    //     let samples = bezier_draw_samples_number_3(points, threshold);
    //     shapes.push(Shape::line(
    //         points.to_vec(),
    //         Stroke::new(1.0, Color32::PLACEHOLDER),
    //     ));

    //     let samples = 100;
    //     shapes.push(Shape::line(
    //         (0..samples)
    //             .map(|i| {
    //                 #[allow(clippy::cast_precision_loss)]
    //                 let t = i as f32 / (samples - 1) as f32;
    //                 sample_bezier(points, t)
    //             })
    //             .collect(),
    //         Stroke::new(1.0, Color32::PLACEHOLDER),
    //     ));
    // }
}

fn draw_bezier_5(
    ui: &Ui,
    wire: WireId,
    args: WireArgs,
    stroke: Stroke,
    threshold: f32,
    shapes: &mut Vec<Shape>,
) {
    debug_assert!(ui.is_visible(), "Must be checked earlier");

    let clip_rect = ui.clip_rect();

    ui.memory_mut(|m| {
        let cached = m.caches.cache::<WiresCache>().bezier_5(wire, args);

        if cached.aabb.intersects(clip_rect) {
            shapes.push(Shape::line(cached.line(threshold), stroke));
        }
    });

    // {
    //     let samples = bezier_draw_samples_number_5(points, threshold);
    //     shapes.push(Shape::line(
    //         points.to_vec(),
    //         Stroke::new(1.0, Color32::PLACEHOLDER),
    //     ));

    //     let samples = 100;
    //     shapes.push(Shape::line(
    //         (0..samples)
    //             .map(|i| {
    //                 #[allow(clippy::cast_precision_loss)]
    //                 let t = i as f32 / (samples - 1) as f32;
    //                 sample_bezier(points, t)
    //             })
    //             .collect(),
    //         Stroke::new(1.0, Color32::PLACEHOLDER),
    //     ));
    // }
}

// #[allow(clippy::let_and_return)]
fn sample_bezier(points: &[Pos2], t: f32) -> Pos2 {
    match *points {
        [] => unimplemented!(),
        [p0] => p0,
        [p0, p1] => p0.lerp(p1, t),
        [p0, p1, p2] => {
            let p0_0 = p0;
            let p1_0 = p1;
            let p2_0 = p2;

            let p0_1 = p0_0.lerp(p1_0, t);
            let p1_1 = p1_0.lerp(p2_0, t);

            p0_1.lerp(p1_1, t)
        }
        [p0, p1, p2, p3] => {
            let p0_0 = p0;
            let p1_0 = p1;
            let p2_0 = p2;
            let p3_0 = p3;

            let p0_1 = p0_0.lerp(p1_0, t);
            let p1_1 = p1_0.lerp(p2_0, t);
            let p2_1 = p2_0.lerp(p3_0, t);

            sample_bezier(&[p0_1, p1_1, p2_1], t)
        }
        [p0, p1, p2, p3, p4] => {
            let p0_0 = p0;
            let p1_0 = p1;
            let p2_0 = p2;
            let p3_0 = p3;
            let p4_0 = p4;

            let p0_1 = p0_0.lerp(p1_0, t);
            let p1_1 = p1_0.lerp(p2_0, t);
            let p2_1 = p2_0.lerp(p3_0, t);
            let p3_1 = p3_0.lerp(p4_0, t);

            sample_bezier(&[p0_1, p1_1, p2_1, p3_1], t)
        }
        [p0, p1, p2, p3, p4, p5] => {
            let p0_0 = p0;
            let p1_0 = p1;
            let p2_0 = p2;
            let p3_0 = p3;
            let p4_0 = p4;
            let p5_0 = p5;

            let p0_1 = p0_0.lerp(p1_0, t);
            let p1_1 = p1_0.lerp(p2_0, t);
            let p2_1 = p2_0.lerp(p3_0, t);
            let p3_1 = p3_0.lerp(p4_0, t);
            let p4_1 = p4_0.lerp(p5_0, t);

            sample_bezier(&[p0_1, p1_1, p2_1, p3_1, p4_1], t)
        }
        _ => unimplemented!(),
    }
}

fn split_bezier_3(points: &[Pos2; 4], t: f32) -> [[Pos2; 4]; 2] {
    let [p0, p1, p2, p3] = *points;

    let p0_0 = p0;
    let p1_0 = p1;
    let p2_0 = p2;
    let p3_0 = p3;

    let p0_1 = p0_0.lerp(p1_0, t);
    let p1_1 = p1_0.lerp(p2_0, t);
    let p2_1 = p2_0.lerp(p3_0, t);

    let p0_2 = p0_1.lerp(p1_1, t);
    let p1_2 = p1_1.lerp(p2_1, t);

    let p0_3 = p0_2.lerp(p1_2, t);

    [[p0_0, p0_1, p0_2, p0_3], [p0_3, p1_2, p2_1, p3_0]]
}

fn hit_wire_bezier_3(
    ctx: &Context,
    wire: WireId,
    args: WireArgs,
    pos: Pos2,
    hit_threshold: f32,
) -> bool {
    let (aabb, points) = ctx.memory_mut(|m| {
        let cache = m.caches.cache::<WiresCache>().bezier_3(wire, args);

        (cache.aabb, cache.points)
    });

    let aabb_e = aabb.expand(hit_threshold);
    if !aabb_e.contains(pos) {
        return false;
    }

    hit_bezier_3(&points, pos, hit_threshold)
}

fn hit_bezier_3(points: &[Pos2; 4], pos: Pos2, hit_threshold: f32) -> bool {
    let samples = bezier_hit_samples_number(points, hit_threshold);
    if samples > 8 {
        let [points1, points2] = split_bezier_3(points, 0.5);

        let aabb_e = Rect::from_points(&points1).expand(hit_threshold);
        if aabb_e.contains(pos) && hit_bezier_3(&points1, pos, hit_threshold) {
            return true;
        }
        let aabb_e = Rect::from_points(&points2).expand(hit_threshold);
        if aabb_e.contains(pos) && hit_bezier_3(&points2, pos, hit_threshold) {
            return true;
        }
        return false;
    }

    let threshold_sq = hit_threshold * hit_threshold;

    for i in 0..samples {
        #[allow(clippy::cast_precision_loss)]
        let t = i as f32 / (samples - 1) as f32;
        let p = sample_bezier(points, t);
        if p.distance_sq(pos) <= threshold_sq {
            return true;
        }
    }

    false
}

fn split_bezier_5(points: &[Pos2; 6], t: f32) -> [[Pos2; 6]; 2] {
    let [p0, p1, p2, p3, p4, p5] = *points;

    let p0_0 = p0;
    let p1_0 = p1;
    let p2_0 = p2;
    let p3_0 = p3;
    let p4_0 = p4;
    let p5_0 = p5;

    let p0_1 = p0_0.lerp(p1_0, t);
    let p1_1 = p1_0.lerp(p2_0, t);
    let p2_1 = p2_0.lerp(p3_0, t);
    let p3_1 = p3_0.lerp(p4_0, t);
    let p4_1 = p4_0.lerp(p5_0, t);

    let p0_2 = p0_1.lerp(p1_1, t);
    let p1_2 = p1_1.lerp(p2_1, t);
    let p2_2 = p2_1.lerp(p3_1, t);
    let p3_2 = p3_1.lerp(p4_1, t);

    let p0_3 = p0_2.lerp(p1_2, t);
    let p1_3 = p1_2.lerp(p2_2, t);
    let p2_3 = p2_2.lerp(p3_2, t);

    let p0_4 = p0_3.lerp(p1_3, t);
    let p1_4 = p1_3.lerp(p2_3, t);

    let p0_5 = p0_4.lerp(p1_4, t);

    [
        [p0_0, p0_1, p0_2, p0_3, p0_4, p0_5],
        [p0_5, p1_4, p2_3, p3_2, p4_1, p5_0],
    ]
}

fn hit_wire_bezier_5(
    ctx: &Context,
    wire: WireId,
    args: WireArgs,
    pos: Pos2,
    hit_threshold: f32,
) -> bool {
    let (aabb, points) = ctx.memory_mut(|m| {
        let cache = m.caches.cache::<WiresCache>().bezier_5(wire, args);

        (cache.aabb, cache.points)
    });

    let aabb_e = aabb.expand(hit_threshold);
    if !aabb_e.contains(pos) {
        return false;
    }

    hit_bezier_5(&points, pos, hit_threshold)
}

fn hit_bezier_5(points: &[Pos2; 6], pos: Pos2, hit_threshold: f32) -> bool {
    let samples = bezier_hit_samples_number(points, hit_threshold);
    if samples > 16 {
        let [points1, points2] = split_bezier_5(points, 0.5);
        let aabb_e = Rect::from_points(&points1).expand(hit_threshold);
        if aabb_e.contains(pos) && hit_bezier_5(&points1, pos, hit_threshold) {
            return true;
        }
        let aabb_e = Rect::from_points(&points2).expand(hit_threshold);
        if aabb_e.contains(pos) && hit_bezier_5(&points2, pos, hit_threshold) {
            return true;
        }
        return false;
    }

    let threshold_sq = hit_threshold * hit_threshold;

    for i in 0..samples {
        #[allow(clippy::cast_precision_loss)]
        let t = i as f32 / (samples - 1) as f32;
        let p = sample_bezier(points, t);

        if p.distance_sq(pos) <= threshold_sq {
            return true;
        }
    }

    false
}

#[derive(Clone, Copy, PartialEq)]
struct AxisAlignedWire {
    aabb: Rect,
    turns: usize,
    segments: [(Pos2, Pos2); 5],
    turn_centers: [Pos2; 4],
    turn_radii: [f32; 4],
}

impl Default for AxisAlignedWire {
    #[inline]
    fn default() -> Self {
        Self {
            aabb: Rect::NOTHING,
            turns: 0,
            segments: [(Pos2::ZERO, Pos2::ZERO); 5],
            turn_centers: [Pos2::ZERO; 4],
            turn_radii: [0.0; 4],
        }
    }
}

#[allow(clippy::too_many_lines)]
fn wire_axis_aligned(corner_radius: f32, frame_size: f32, from: Pos2, to: Pos2) -> AxisAlignedWire {
    let corner_radius = corner_radius.max(0.0);

    let half_height = f32::abs(from.y - to.y) / 2.0;
    let max_radius = (half_height / 2.0).min(corner_radius);

    let frame_size = frame_size.max(max_radius * 2.0);

    let zero_segment = (Pos2::ZERO, Pos2::ZERO);

    if from.x + frame_size <= to.x - frame_size {
        if f32::abs(from.y - to.y) < 1.0 {
            // Single segment case.
            AxisAlignedWire {
                aabb: Rect::from_two_pos(from, to),
                segments: [
                    (from, to),
                    zero_segment,
                    zero_segment,
                    zero_segment,
                    zero_segment,
                ],
                turns: 0,
                turn_centers: [Pos2::ZERO; 4],
                turn_radii: [f32::NAN; 4],
            }
        } else {
            // Two turns case.
            let mid_x = f32::midpoint(from.x, to.x);
            let half_width = (to.x - from.x) / 2.0;

            let turn_radius = max_radius.min(half_width);

            let turn_vert_len = if from.y < to.y {
                turn_radius
            } else {
                -turn_radius
            };

            let segments = [
                (from, pos2(mid_x - turn_radius, from.y)),
                (
                    pos2(mid_x, from.y + turn_vert_len),
                    pos2(mid_x, to.y - turn_vert_len),
                ),
                (pos2(mid_x + turn_radius, to.y), to),
                zero_segment,
                zero_segment,
            ];

            let turn_centers = [
                pos2(mid_x - turn_radius, from.y + turn_vert_len),
                pos2(mid_x + turn_radius, to.y - turn_vert_len),
                Pos2::ZERO,
                Pos2::ZERO,
            ];

            let turn_radii = [turn_radius, turn_radius, f32::NAN, f32::NAN];

            AxisAlignedWire {
                aabb: Rect::from_two_pos(from, to),
                turns: 2,
                segments,
                turn_centers,
                turn_radii,
            }
        }
    } else {
        // Four turns case.
        let mid = f32::midpoint(from.y, to.y);

        let right = from.x + frame_size;
        let left = to.x - frame_size;

        let half_width = f32::abs(right - left) / 2.0;

        let ends_turn_radius = max_radius;
        let middle_turn_radius = max_radius.min(half_width);

        let ends_turn_vert_len = if from.y < to.y {
            ends_turn_radius
        } else {
            -ends_turn_radius
        };

        let middle_turn_vert_len = if from.y < to.y {
            middle_turn_radius
        } else {
            -middle_turn_radius
        };

        let segments = [
            (from, pos2(right - ends_turn_radius, from.y)),
            (
                pos2(right, from.y + ends_turn_vert_len),
                pos2(right, mid - middle_turn_vert_len),
            ),
            (
                pos2(right - middle_turn_radius, mid),
                pos2(left + middle_turn_radius, mid),
            ),
            (
                pos2(left, mid + middle_turn_vert_len),
                pos2(left, to.y - ends_turn_vert_len),
            ),
            (pos2(left + ends_turn_radius, to.y), to),
        ];

        let turn_centers = [
            pos2(right - ends_turn_radius, from.y + ends_turn_vert_len),
            pos2(right - middle_turn_radius, mid - middle_turn_vert_len),
            pos2(left + middle_turn_radius, mid + middle_turn_vert_len),
            pos2(left + ends_turn_radius, to.y - ends_turn_vert_len),
        ];

        let turn_radii = [
            ends_turn_radius,
            middle_turn_radius,
            middle_turn_radius,
            ends_turn_radius,
        ];

        AxisAlignedWire {
            aabb: Rect::from_min_max(
                pos2(f32::min(left, from.x), f32::min(from.y, to.y)),
                pos2(f32::max(right, to.x), f32::max(from.y, to.y)),
            ),
            turns: 4,
            segments,
            turn_centers,
            turn_radii,
        }
    }
}

fn hit_wire_axis_aligned(
    ctx: &Context,
    wire: WireId,
    args: WireArgs,
    pos: Pos2,
    hit_threshold: f32,
) -> bool {
    let aawire = ctx.memory_mut(|m| {
        let cache = m.caches.cache::<WiresCache>().axis_aligned(wire, args);

        cache.aawire
    });

    // Check AABB first
    if !aawire.aabb.expand(hit_threshold).contains(pos) {
        return false;
    }

    // Check all straight segments first
    // Number of segments is number of turns + 1
    for i in 0..aawire.turns + 1 {
        let (start, end) = aawire.segments[i];

        // Segments are always axis aligned
        // So we can use AABB for checking
        if Rect::from_two_pos(start, end)
            .expand(hit_threshold)
            .contains(pos)
        {
            return true;
        }
    }

    // Check all turns
    for i in 0..aawire.turns {
        if aawire.turn_radii[i] > 0.0 {
            let turn = aawire.turn_centers[i];
            let turn_aabb = Rect::from_two_pos(aawire.segments[i].1, aawire.segments[i + 1].0);
            if !turn_aabb.contains(pos) {
                continue;
            }

            // Avoid sqrt
            let dist2 = (turn - pos).length_sq();
            let min = aawire.turn_radii[i] - hit_threshold;
            let max = aawire.turn_radii[i] + hit_threshold;

            if dist2 <= max * max && dist2 >= min * min {
                return true;
            }
        }
    }

    false
}

fn turn_samples_number(radius: f32, threshold: f32) -> usize {
    #![allow(clippy::cast_sign_loss)]
    #![allow(clippy::cast_possible_truncation)]
    #![allow(clippy::cast_precision_loss)]

    if threshold / radius >= 1.0 {
        return 2;
    }

    let a: f32 = (1.0 - threshold / radius).acos();
    let samples = (std::f32::consts::PI / (4.0 * a) + 1.0)
        .min(MAX_CURVE_SAMPLES as f32)
        .ceil() as usize;

    samples.clamp(2, MAX_CURVE_SAMPLES)
}

#[allow(clippy::too_many_arguments)]
fn draw_axis_aligned(
    ui: &Ui,
    wire: WireId,
    args: WireArgs,
    stroke: Stroke,
    threshold: f32,
    shapes: &mut Vec<Shape>,
) {
    debug_assert!(ui.is_visible(), "Must be checked earlier");

    let clip_rect = ui.clip_rect();
    ui.memory_mut(|m| {
        let cached = m.caches.cache::<WiresCache>().axis_aligned(wire, args);

        if cached.aawire.aabb.intersects(clip_rect) {
            shapes.push(Shape::line(cached.line(threshold), stroke));
        }
    });
}

/// Very basic lower-bound algorithm
/// Finds the smallest number in range [min, max) that satisfies the predicate
/// If no such number exists, returns max
///
/// For the algorithm to work, the predicate must be monotonic
/// i.e. if f(i) is true, then f(j) is true for all j within (i, max)
/// and if f(i) is false, then f(j) is false for all j within [min, i)
fn lower_bound(min: usize, max: usize, f: impl Fn(usize) -> bool) -> usize {
    #![allow(clippy::similar_names)]

    let mut min = min;
    let mut max = max;

    while min < max {
        let mid = usize::midpoint(min, max);
        if f(mid) {
            max = mid;
        } else {
            min = mid + 1;
        }
    }

    max

    // for i in min..max {
    //     if f(i) {
    //         return i;
    //     }
    // }
    // max
}

/// Describes where and how to place a widget on a wire.
#[derive(Clone, Debug)]
pub struct WireWidgetDescriptor {
    /// Position along the wire curve. 0.0 = output pin, 1.0 = input pin.
    /// 
    /// Default: 0.5.
    pub t: f32,

    /// Per-widget alignment override.
    /// 
    /// Falls back to [`SnarlStyle::wire_widget_align`](super::SnarlStyle::wire_widget_align).
    pub align: Option<Align2>,

    /// Per-widget gap override.
    /// 
    /// Falls back to [`SnarlStyle::wire_widget_gap`](super::SnarlStyle::wire_widget_gap).
    pub gap: Option<f32>,
}

impl Default for WireWidgetDescriptor {
    fn default() -> Self {
        Self {
            t: 0.5,
            align: None,
            gap: None,
        }
    }
}

impl WireWidgetDescriptor {
    /// Creates a descriptor at the given parametric position `t` with default alignment and gap.
    #[must_use]
    pub const fn new(t: f32) -> Self {
        Self {
            t,
            align: None,
            gap: None,
        }
    }
}

/// Resolved context for a wire widget, passed to
/// [`SnarlViewer::show_wire_widget`](super::SnarlViewer::show_wire_widget).
///
/// Contains the final values after resolving descriptor overrides against style
/// defaults, plus the computed screen-space position.
#[derive(Clone, Debug)]
pub struct WireWidgetContext {
    /// Parametric position along the wire curve (same as [`WireWidgetDescriptor`](WireWidgetDescriptor::t)).
    pub t: f32,

    /// Screen-space position on the wire curve.
    pub pos: Pos2,

    /// Resolved alignment.
    pub align: Align2,

    /// Resolved gap.
    pub gap: f32,
}

/// Evaluate a point on the wire curve at parametric position `t` (0.0..=1.0).
///
/// `t = 0.0` is the output pin position (`from`), `t = 1.0` is the input pin
/// position (`to`).
///
/// For bezier curves, `t` is the standard parametric parameter, which is
/// **not** uniform with respect to arc length. For axis-aligned wires, `t` is
/// interpolated by arc length so that `t = 0.5` falls at the visual midpoint.
#[allow(clippy::too_many_arguments)]
pub fn point_on_wire(
    frame_size: f32,
    upscale: bool,
    downscale: bool,
    from: Pos2,
    to: Pos2,
    style: WireStyle,
    vertical: bool,
    t: f32,
) -> Pos2 {
    let frame_size = adjust_frame_size(frame_size, upscale, downscale, from, to);

    match style {
        WireStyle::Line => from.lerp(to, t),
        WireStyle::Bezier3 => {
            let points = if vertical {
                wire_bezier_3_vertical(frame_size, from, to)
            } else {
                wire_bezier_3(frame_size, from, to)
            };
            sample_bezier(&points, t)
        }
        WireStyle::Bezier5 => {
            let points = if vertical {
                wire_bezier_5_vertical(frame_size, from, to)
            } else {
                wire_bezier_5_horizontal(frame_size, from, to)
            };
            sample_bezier(&points, t)
        }
        WireStyle::AxisAligned { corner_radius } => {
            let aawire = wire_axis_aligned(corner_radius, frame_size, from, to);
            sample_axis_aligned_by_arc_length(&aawire, t)
        }
    }
}

/// Sample a point along an axis-aligned wire by arc-length fraction `t`.
///
/// The wire consists of straight segments interleaved with quarter-circle arc
/// turns. We compute the total arc length, then walk the segments/turns to
/// find the point at `t * total_length`.
fn sample_axis_aligned_by_arc_length(wire: &AxisAlignedWire, t: f32) -> Pos2 {
    let t = t.clamp(0.0, 1.0);

    // Collect lengths of each piece: segment, turn, segment, turn, ..., segment.
    // Total pieces = turns + 1 segments + turns arcs = 2*turns + 1.
    let num_segments = wire.turns + 1;
    let num_pieces = num_segments + wire.turns;

    // We'll walk the pieces inline rather than allocating.
    // First pass: compute total length.
    let mut total_length = 0.0f32;
    for i in 0..num_pieces {
        if i % 2 == 0 {
            // Straight segment
            let seg_idx = i / 2;
            let (start, end) = wire.segments[seg_idx];
            total_length += (end - start).length();
        } else {
            // Arc turn (quarter circle)
            let turn_idx = i / 2;
            let radius = wire.turn_radii[turn_idx];
            if radius > 0.0 {
                total_length += std::f32::consts::FRAC_PI_2 * radius;
            }
        }
    }

    if total_length <= 0.0 {
        return wire.segments[0].0;
    }

    // Second pass: walk to target distance.
    let target = t * total_length;
    let mut walked = 0.0f32;

    for i in 0..num_pieces {
        if i % 2 == 0 {
            // Straight segment
            let seg_idx = i / 2;
            let (start, end) = wire.segments[seg_idx];
            let seg_len = (end - start).length();
            if walked + seg_len >= target {
                let frac = if seg_len > 0.0 {
                    (target - walked) / seg_len
                } else {
                    0.0
                };
                return start.lerp(end, frac);
            }
            walked += seg_len;
        } else {
            // Arc turn (quarter circle)
            let turn_idx = i / 2;
            let radius = wire.turn_radii[turn_idx];
            if radius <= 0.0 {
                continue;
            }
            let arc_len = std::f32::consts::FRAC_PI_2 * radius;
            if walked + arc_len >= target {
                let frac = (target - walked) / arc_len;
                return sample_quarter_arc(wire, turn_idx, frac);
            }
            walked += arc_len;
        }
    }

    // Rounding: return the end point.
    wire.segments[wire.turns].1
}

/// Sample a quarter-circle arc turn at fractional position `frac` (0.0..=1.0).
///
/// The arc connects the end of `segments[turn_idx]` to the start of
/// `segments[turn_idx + 1]`, centered at `turn_centers[turn_idx]`.
fn sample_quarter_arc(wire: &AxisAlignedWire, turn_idx: usize, frac: f32) -> Pos2 {
    let center = wire.turn_centers[turn_idx];
    let arc_start = wire.segments[turn_idx].1;
    let arc_end = wire.segments[turn_idx + 1].0;

    // Determine start and sweep angles from the arc endpoints relative to center.
    let start_offset = arc_start - center;
    let end_offset = arc_end - center;
    let start_angle = f32::atan2(start_offset.y, start_offset.x);
    let mut end_angle = f32::atan2(end_offset.y, end_offset.x);

    // Ensure we sweep in the shorter direction (quarter circle).
    let mut sweep = end_angle - start_angle;
    if sweep > std::f32::consts::PI {
        sweep -= std::f32::consts::TAU;
    } else if sweep < -std::f32::consts::PI {
        sweep += std::f32::consts::TAU;
    }
    end_angle = start_angle + sweep;

    let angle = start_angle + frac * (end_angle - start_angle);
    let radius = wire.turn_radii[turn_idx];
    pos2(center.x + radius * angle.cos(), center.y + radius * angle.sin())
}
