//! This module provides functionality for showing [`Snarl`] graph in [`Ui`].
#![allow(clippy::match_same_arms)]

use std::{collections::HashMap, hash::Hash};

use egui::{
    Align, Align2, Color32, CornerRadius, Frame, Id, Key, LayerId, Layout, Margin, Modifiers,
    PointerButton, Pos2, Rect, Scene, Sense, Stroke, StrokeKind, Style, Ui, UiBuilder,
    UiKind, UiStackInfo, Vec2,
    collapsing_header::paint_default_icon,
    emath::{GuiRounding, RectAlign, TSTransform},
    epaint::Shadow,
    pos2,
    response::Flags,
    vec2,
};
use egui_scale::EguiScale;
use smallvec::SmallVec;

use crate::{GroupId, InPin, InPinId, Node, NodeId, OutPin, OutPinId, Snarl, ui::wire::WireId};

mod background_pattern;
mod config;
mod pin;
mod scale;
mod state;
mod viewer;
mod wire;

use self::{
    pin::AnyPin,
    state::{GroupState, NewWires, NodeState, RowHeights, SnarlState},
    wire::{WireResponse, draw_wire},
};

pub use self::{
    background_pattern::{BackgroundPattern, Grid},
    config::{ModifierClick, SnapGrid, SnapGridType, SnarlConfig},
    pin::{AnyPins, PinContext, PinInfo, PinShape, PinWireInfo, SnarlPin},
    state::{selected_groups, selected_nodes},
    viewer::SnarlViewer,
    wire::{WireLayer, WireStyle, WireWidgetContext, WireWidgetDescriptor, point_on_wire},
};

/// Controls how header, pins, body and footer are placed in the node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "egui-probe", derive(egui_probe::EguiProbe))]
#[cfg_attr(feature = "facet", derive(facet::Facet), repr(u8))]
pub enum NodeLayoutKind {
    /// Input pins, body and output pins are placed horizontally.
    /// With header on top and footer on bottom.
    ///
    /// +---------------------+
    /// |       Header        |
    /// +----+-----------+----+
    /// | In |           | Out|
    /// | In |   Body    | Out|
    /// | In |           | Out|
    /// | In |           |    |
    /// +----+-----------+----+
    /// |       Footer        |
    /// +---------------------+
    ///
    #[default]
    Coil,

    /// All elements are placed in vertical stack.
    /// Header is on top, then input pins, body, output pins and footer.
    ///
    /// +---------------------+
    /// |       Header        |
    /// +---------------------+
    /// | In                  |
    /// | In                  |
    /// | In                  |
    /// | In                  |
    /// +---------------------+
    /// |       Body          |
    /// +---------------------+
    /// |                 Out |
    /// |                 Out |
    /// |                 Out |
    /// +---------------------+
    /// |       Footer        |
    /// +---------------------+
    Sandwich,

    /// All elements are placed in vertical stack.
    /// Header is on top, then output pins, body, input pins and footer.
    ///
    /// +---------------------+
    /// |       Header        |
    /// +---------------------+
    /// |                 Out |
    /// |                 Out |
    /// |                 Out |
    /// +---------------------+
    /// |       Body          |
    /// +---------------------+
    /// | In                  |
    /// | In                  |
    /// | In                  |
    /// | In                  |
    /// +---------------------+
    /// |       Footer        |
    /// +---------------------+
    FlippedSandwich,

    /// Input pins at top in horizontal row, body in middle, output pins at bottom in horizontal row.
    /// Wire attachment points are on top/bottom edges.
    ///
    /// +----+----+----+----+----+
    /// |        Header          |
    /// +----+----+----+----+----+
    /// | In | In | In | In |    |
    /// +------------------------+
    /// |         Body           |
    /// +------------------------+
    /// | Out| Out| Out|    |    |
    /// +----+----+----+----+----+
    /// |        Footer          |
    /// +------------------------+
    VerticalSandwichDown,

    /// Output pins at top in horizontal row, body in middle, input pins at bottom in horizontal row.
    /// Wire attachment points are on top/bottom edges.
    ///
    /// +----+----+----+----+----+
    /// |        Header          |
    /// +----+----+----+----+----+
    /// | Out| Out| Out|    |    |
    /// +------------------------+
    /// |         Body           |
    /// +------------------------+
    /// | In | In | In | In |    |
    /// +----+----+----+----+----+
    /// |        Footer          |
    /// +------------------------+
    VerticalSandwichUp,
}

impl NodeLayoutKind {
    /// Returns `true` if this layout arranges pins horizontally (top/bottom edges).
    #[must_use]
    #[inline]
    pub const fn is_vertical(self) -> bool {
        matches!(
            self,
            NodeLayoutKind::VerticalSandwichDown | NodeLayoutKind::VerticalSandwichUp
        )
    }
}

/// Cache params for wire widget
#[derive(Clone)]
pub struct WireWidgetCache {
    widget_size: Vec2,
}

/// Collected info for deferred wire widget rendering.
struct WireWidgetInfo<'a> {
    out_pin: &'a OutPin,
    in_pin: &'a InPin,
    from_pos: Pos2,
    to_pos: Pos2,
    wire_style: wire::WireStyle,
    vertical: bool,
    descriptors: Vec<wire::WireWidgetDescriptor>,
}

/// Controls how node elements are laid out.
///
///
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "egui-probe", derive(egui_probe::EguiProbe))]
pub struct NodeLayout {
    /// Controls method of laying out node elements.
    pub kind: NodeLayoutKind,

    /// Controls minimal height of pin rows.
    pub min_pin_row_height: f32,

    /// Controls how pin rows heights are set.
    /// If true, all pin rows will have the same height, matching the largest content.
    /// False by default.
    pub equal_pin_row_heights: bool,
}

impl NodeLayout {
    /// Creates new [`NodeLayout`] with `Coil` kind and flexible pin heights.
    #[must_use]
    #[inline]
    pub const fn coil() -> Self {
        NodeLayout {
            kind: NodeLayoutKind::Coil,
            min_pin_row_height: 0.0,
            equal_pin_row_heights: false,
        }
    }

    /// Creates new [`NodeLayout`] with `Sandwich` kind and flexible pin heights.
    #[must_use]
    #[inline]
    pub const fn sandwich() -> Self {
        NodeLayout {
            kind: NodeLayoutKind::Sandwich,
            min_pin_row_height: 0.0,
            equal_pin_row_heights: false,
        }
    }

    /// Creates new [`NodeLayout`] with `FlippedSandwich` kind and flexible pin heights.
    #[must_use]
    #[inline]
    pub const fn flipped_sandwich() -> Self {
        NodeLayout {
            kind: NodeLayoutKind::FlippedSandwich,
            min_pin_row_height: 0.0,
            equal_pin_row_heights: false,
        }
    }

    /// Creates new [`NodeLayout`] with `VerticalSandwichDown` kind and flexible pin widths.
    /// Inputs at top, outputs at bottom.
    #[must_use]
    #[inline]
    pub const fn vertical_sandwich_down() -> Self {
        NodeLayout {
            kind: NodeLayoutKind::VerticalSandwichDown,
            min_pin_row_height: 0.0,
            equal_pin_row_heights: false,
        }
    }

    /// Creates new [`NodeLayout`] with `VerticalSandwichUp` kind and flexible pin widths.
    /// Outputs at top, inputs at bottom.
    #[must_use]
    #[inline]
    pub const fn vertical_sandwich_up() -> Self {
        NodeLayout {
            kind: NodeLayoutKind::VerticalSandwichUp,
            min_pin_row_height: 0.0,
            equal_pin_row_heights: false,
        }
    }

    /// Returns new [`NodeLayout`] with same `kind` and specified pin heights.
    #[must_use]
    #[inline]
    pub const fn with_equal_pin_rows(self) -> Self {
        NodeLayout {
            kind: self.kind,
            min_pin_row_height: self.min_pin_row_height,
            equal_pin_row_heights: true,
        }
    }

    /// Returns new [`NodeLayout`] with same `kind` and specified minimum pin row height.
    #[must_use]
    #[inline]
    pub const fn with_min_pin_row_height(self, min_pin_row_height: f32) -> Self {
        NodeLayout {
            kind: self.kind,
            min_pin_row_height,
            equal_pin_row_heights: self.equal_pin_row_heights,
        }
    }
}

impl From<NodeLayoutKind> for NodeLayout {
    #[inline]
    fn from(kind: NodeLayoutKind) -> Self {
        NodeLayout {
            kind,
            min_pin_row_height: 0.0,
            equal_pin_row_heights: false,
        }
    }
}

impl Default for NodeLayout {
    #[inline]
    fn default() -> Self {
        NodeLayout::coil()
    }
}

#[derive(Clone, Copy, Debug)]
enum OuterHeights<'a> {
    Flexible { rows: &'a [f32] },
    Matching { max: f32 },
    Tight,
}

#[derive(Clone, Copy, Debug)]
struct Heights<'a> {
    rows: &'a [f32],
    outer: OuterHeights<'a>,
    min_outer: f32,
}

impl Heights<'_> {
    fn get(&self, idx: usize) -> (f32, f32) {
        let inner = match self.rows.get(idx) {
            Some(&value) => value,
            None => 0.0,
        };

        let outer = match &self.outer {
            OuterHeights::Flexible { rows } => match rows.get(idx) {
                Some(&outer) => outer.max(inner),
                None => inner,
            },
            OuterHeights::Matching { max } => max.max(inner),
            OuterHeights::Tight => inner,
        };

        (inner, outer.max(self.min_outer))
    }
}

impl NodeLayout {
    fn input_heights(self, state: &NodeState) -> Heights<'_> {
        let rows = state.input_heights().as_slice();

        let outer = match (self.kind, self.equal_pin_row_heights) {
            (NodeLayoutKind::Coil, false) => OuterHeights::Flexible {
                rows: state.output_heights().as_slice(),
            },
            (_, true) => {
                let mut max_height = 0.0f32;
                for &h in state.input_heights() {
                    max_height = max_height.max(h);
                }
                for &h in state.output_heights() {
                    max_height = max_height.max(h);
                }
                OuterHeights::Matching { max: max_height }
            }
            (_, false) => OuterHeights::Tight,
        };

        Heights {
            rows,
            outer,
            min_outer: self.min_pin_row_height,
        }
    }

    fn output_heights(self, state: &'_ NodeState) -> Heights<'_> {
        let rows = state.output_heights().as_slice();

        let outer = match (self.kind, self.equal_pin_row_heights) {
            (NodeLayoutKind::Coil, false) => OuterHeights::Flexible {
                rows: state.input_heights().as_slice(),
            },
            (_, true) => {
                let mut max_height = 0.0f32;
                for &h in state.input_heights() {
                    max_height = max_height.max(h);
                }
                for &h in state.output_heights() {
                    max_height = max_height.max(h);
                }
                OuterHeights::Matching { max: max_height }
            }
            (_, false) => OuterHeights::Tight,
        };

        Heights {
            rows,
            outer,
            min_outer: self.min_pin_row_height,
        }
    }

    fn input_widths(self, state: &NodeState) -> Widths<'_> {
        let cols = state.input_widths().as_slice();

        let outer = match (self.kind, self.equal_pin_row_heights) {
            (NodeLayoutKind::VerticalSandwichDown | NodeLayoutKind::VerticalSandwichUp, true) => {
                let mut max_width = 0.0f32;
                for &w in state.input_widths() {
                    max_width = max_width.max(w);
                }
                for &w in state.output_widths() {
                    max_width = max_width.max(w);
                }
                OuterWidths::Matching { max: max_width }
            }
            _ => OuterWidths::Tight,
        };

        Widths {
            cols,
            outer,
            min_outer: self.min_pin_row_height, // Reused as min column width
        }
    }

    fn output_widths(self, state: &NodeState) -> Widths<'_> {
        let cols = state.output_widths().as_slice();

        let outer = match (self.kind, self.equal_pin_row_heights) {
            (NodeLayoutKind::VerticalSandwichDown | NodeLayoutKind::VerticalSandwichUp, true) => {
                let mut max_width = 0.0f32;
                for &w in state.input_widths() {
                    max_width = max_width.max(w);
                }
                for &w in state.output_widths() {
                    max_width = max_width.max(w);
                }
                OuterWidths::Matching { max: max_width }
            }
            _ => OuterWidths::Tight,
        };

        Widths {
            cols,
            outer,
            min_outer: self.min_pin_row_height, // Reused as min column width
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum OuterWidths<'a> {
    #[allow(dead_code)]
    Flexible {
        cols: &'a [f32],
    },
    Matching {
        max: f32,
    },
    Tight,
}

#[derive(Clone, Copy, Debug)]
struct Widths<'a> {
    cols: &'a [f32],
    outer: OuterWidths<'a>,
    min_outer: f32,
}

impl Widths<'_> {
    fn get(&self, idx: usize) -> (f32, f32) {
        let inner = match self.cols.get(idx) {
            Some(&value) => value,
            None => 0.0,
        };

        let outer = match &self.outer {
            OuterWidths::Flexible { cols } => match cols.get(idx) {
                Some(&outer) => outer.max(inner),
                None => inner,
            },
            OuterWidths::Matching { max } => max.max(inner),
            OuterWidths::Tight => inner,
        };

        (inner, outer.max(self.min_outer))
    }
}

/// Controls style of node selection rect.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "egui-probe", derive(egui_probe::EguiProbe))]
pub struct SelectionStyle {
    /// Margin between selection rect and node frame.
    pub margin: Margin,

    /// Rounding of selection rect.
    pub rounding: CornerRadius,

    /// Fill color of selection rect.
    pub fill: Color32,

    /// Stroke of selection rect.
    pub stroke: Stroke,
}

/// Controls when pin labels are visible.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "egui-probe", derive(egui_probe::EguiProbe))]
#[cfg_attr(feature = "facet", derive(facet::Facet), repr(u8))]
pub enum PinLabelVisibility {
    /// Labels are always visible (default, current behavior).
    #[default]
    Always,

    /// Labels are shown only when the pin is hovered.
    OnHover,

    /// Labels are shown on all pins while dragging a wire.
    WhileConnecting,

    /// Labels are shown on nodes near the wire cursor while connecting.
    NearWire {
        /// Proximity threshold in logical pixels.
        threshold: f32,
    },
}

/// Controls how pins are placed in the node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "egui-probe", derive(egui_probe::EguiProbe))]
#[cfg_attr(feature = "facet", derive(facet::Facet), repr(u8))]
pub enum PinPlacement {
    /// Pins are placed inside the node frame.
    #[default]
    Inside,

    /// Pins are placed on the edge of the node frame.
    Edge,

    /// Pins are placed outside the node frame.
    Outside {
        /// Margin between node frame and pins.
        margin: f32,
    },
}

/// Style for rendering Snarl.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "egui-probe", derive(egui_probe::EguiProbe))]
pub struct SnarlStyle {
    /// Controls how nodes are laid out.
    /// Defaults to [`NodeLayoutKind::Coil`].
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub node_layout: Option<NodeLayout>,

    /// Frame used to draw nodes.
    /// Defaults to [`Frame::window`] constructed from current ui's style.
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "Option::is_none",
            default,
            with = "serde_frame_option"
        )
    )]
    pub node_frame: Option<Frame>,

    /// Frame used to draw node headers.
    /// Defaults to [`node_frame`] without shadow and transparent fill.
    ///
    /// If set, it should not have shadow and fill should be either opaque of fully transparent
    /// unless layering of header fill color with node fill color is desired.
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "Option::is_none",
            default,
            with = "serde_frame_option"
        )
    )]
    pub header_frame: Option<Frame>,

    /// Blank space for dragging node by its header.
    /// Elements in the header are placed after this space.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub header_drag_space: Option<Vec2>,

    /// Whether nodes can be collapsed.
    /// If true, headers will have collapsing button.
    /// When collapsed, node will not show its pins, body and footer.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub collapsible: Option<bool>,

    /// Size of pins.
    #[cfg_attr(feature = "egui-probe", egui_probe(range = 0.0..))]
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub pin_size: Option<f32>,

    /// Default fill color for pins.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub pin_fill: Option<Color32>,

    /// Default stroke for pins.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub pin_stroke: Option<Stroke>,

    /// Shape of pins.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub pin_shape: Option<PinShape>,

    /// Placement of pins.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub pin_placement: Option<PinPlacement>,

    /// Width of wires.
    #[cfg_attr(feature = "egui-probe", egui_probe(range = 0.0..))]
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub wire_width: Option<f32>,

    /// Size of wire frame which controls curvature of wires.
    #[cfg_attr(feature = "egui-probe", egui_probe(range = 0.0..))]
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub wire_frame_size: Option<f32>,

    /// Whether to downscale wire frame when nodes are close.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub downscale_wire_frame: Option<bool>,

    /// Weather to upscale wire frame when nodes are far.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub upscale_wire_frame: Option<bool>,

    /// Controls default style of wires.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub wire_style: Option<WireStyle>,

    /// Whether wires can be drawn vertically when the Y distance exceeds the X distance (Houdini-style).
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub vertical_wires: Option<bool>,

    /// Layer where wires are rendered.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub wire_layer: Option<WireLayer>,

    /// Frame used to draw background
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "Option::is_none",
            default,
            with = "serde_frame_option"
        )
    )]
    pub bg_frame: Option<Frame>,

    /// Background pattern.
    /// Defaults to [`BackgroundPattern::Grid`].
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub bg_pattern: Option<BackgroundPattern>,

    /// Stroke for background pattern.
    /// Defaults to `ui.visuals().widgets.noninteractive.bg_stroke`.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub bg_pattern_stroke: Option<Stroke>,

    /// Minimum viewport scale that can be set.
    #[cfg_attr(feature = "egui-probe", egui_probe(range = 0.0..=1.0))]
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub min_scale: Option<f32>,

    /// Maximum viewport scale that can be set.
    #[cfg_attr(feature = "egui-probe", egui_probe(range = 1.0..))]
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub max_scale: Option<f32>,

    /// Enable centering by double click on background
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub centering: Option<bool>,

    /// Stroke for selection.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub select_stoke: Option<Stroke>,

    /// Fill for selection.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub select_fill: Option<Color32>,

    /// Flag to control how rect selection works.
    /// If set to true, only nodes fully contained in selection rect will be selected.
    /// If set to false, nodes intersecting with selection rect will be selected.
    pub select_rect_contained: Option<bool>,

    /// Style for node selection.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub select_style: Option<SelectionStyle>,

    /// Controls whether to show magnified text in crisp mode.
    /// This zooms UI style to max scale and scales down the scene.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub crisp_magnified_text: Option<bool>,

    /// Controls smoothness of wire curves.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    #[cfg_attr(
        feature = "egui-probe",
        egui_probe(range = 0.0f32..=10.0f32 by 0.05f32)
    )]
    pub wire_smoothness: Option<f32>,

    /// Controls the gap between wire widget and wire.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    #[cfg_attr(
        feature = "egui-probe",
        egui_probe(range = 0.0..)
    )]
    pub wire_widget_gap: Option<f32>,

    /// Controls the alignment of wire widget with respect to the center point of wire
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    #[cfg_attr(feature = "egui-probe", egui_probe(skip))]
    pub wire_widget_align: Option<Align2>,

    /// Controls the scale factor for pin hover effect.
    /// Set to 1.0 to disable the hover effect.
    /// Defaults to 1.2 (20% larger on hover).
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    #[cfg_attr(feature = "egui-probe", egui_probe(range = 1.0..=2.0))]
    pub pin_hover_scale: Option<f32>,

    /// Controls when pin labels are visible.
    /// Defaults to [`PinLabelVisibility::Always`].
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub pin_label_visibility: Option<PinLabelVisibility>,

    #[doc(hidden)]
    #[cfg_attr(feature = "egui-probe", egui_probe(skip))]
    #[cfg_attr(feature = "serde", serde(skip_serializing, default))]
    /// Do not access other than with .., here to emulate `#[non_exhaustive(pub)]`
    pub _non_exhaustive: (),
}

impl SnarlStyle {
    fn node_layout(&self) -> NodeLayout {
        self.node_layout.unwrap_or_default()
    }

    fn pin_size(&self, style: &Style) -> f32 {
        self.pin_size.unwrap_or(style.spacing.interact_size.y * 0.6)
    }

    pub(crate) fn pin_fill(&self, style: &Style) -> Color32 {
        self.pin_fill
            .unwrap_or(style.visuals.widgets.active.bg_fill)
    }

    pub(crate) fn pin_stroke(&self, style: &Style) -> Stroke {
        self.pin_stroke.unwrap_or_else(|| {
            Stroke::new(
                style.visuals.widgets.active.bg_stroke.width,
                style.visuals.widgets.active.bg_stroke.color,
            )
        })
    }

    pub(crate) fn pin_shape(&self) -> PinShape {
        self.pin_shape.unwrap_or(PinShape::Circle)
    }

    fn pin_placement(&self) -> PinPlacement {
        self.pin_placement.unwrap_or_default()
    }

    fn wire_width(&self, style: &Style) -> f32 {
        self.wire_width
            .unwrap_or_else(|| self.pin_size(style) * 0.1)
    }

    fn wire_frame_size(&self, style: &Style) -> f32 {
        self.wire_frame_size
            .unwrap_or_else(|| self.pin_size(style) * 3.0)
    }

    fn downscale_wire_frame(&self) -> bool {
        self.downscale_wire_frame.unwrap_or(true)
    }

    fn upscale_wire_frame(&self) -> bool {
        self.upscale_wire_frame.unwrap_or(false)
    }

    pub(crate) fn wire_style(&self) -> WireStyle {
        self.wire_style.unwrap_or(WireStyle::Bezier5)
    }

    fn vertical_wires(&self) -> bool {
        self.vertical_wires.unwrap_or(true)
    }
    fn vertical_wire(&self, from_pos: Pos2, to_pos: Pos2) -> bool {
        self.vertical_wires() && {
            let dx = (to_pos.x - from_pos.x).abs();
            let dy = (to_pos.y - from_pos.y).abs();
            dy > dx
        }
    }

    fn wire_layer(&self) -> WireLayer {
        self.wire_layer.unwrap_or(WireLayer::BehindNodes)
    }

    fn header_drag_space(&self, style: &Style) -> Vec2 {
        self.header_drag_space
            .unwrap_or_else(|| vec2(style.spacing.icon_width, style.spacing.icon_width))
    }

    fn collapsible(&self) -> bool {
        self.collapsible.unwrap_or(true)
    }

    fn bg_frame(&self, style: &Style) -> Frame {
        self.bg_frame.unwrap_or_else(|| Frame::canvas(style))
    }

    pub(crate) fn bg_pattern_stroke(&self, style: &Style) -> Stroke {
        self.bg_pattern_stroke
            .unwrap_or(style.visuals.widgets.noninteractive.bg_stroke)
    }

    fn min_scale(&self) -> f32 {
        self.min_scale.unwrap_or(0.2)
    }

    fn max_scale(&self) -> f32 {
        self.max_scale.unwrap_or(2.0)
    }

    fn node_frame(&self, style: &Style) -> Frame {
        self.node_frame.unwrap_or_else(|| Frame::window(style))
    }

    fn header_frame(&self, style: &Style) -> Frame {
        self.header_frame
            .unwrap_or_else(|| self.node_frame(style).shadow(Shadow::NONE))
    }

    fn centering(&self) -> bool {
        self.centering.unwrap_or(true)
    }

    fn select_stroke(&self, style: &Style) -> Stroke {
        self.select_stoke.unwrap_or_else(|| {
            Stroke::new(
                style.visuals.selection.stroke.width,
                style.visuals.selection.stroke.color.gamma_multiply(0.5),
            )
        })
    }

    fn select_fill(&self, style: &Style) -> Color32 {
        self.select_fill
            .unwrap_or_else(|| style.visuals.selection.bg_fill.gamma_multiply(0.3))
    }

    fn select_rect_contained(&self) -> bool {
        self.select_rect_contained.unwrap_or(false)
    }

    fn select_style(&self, style: &Style) -> SelectionStyle {
        self.select_style.unwrap_or_else(|| SelectionStyle {
            margin: style.spacing.window_margin,
            rounding: style.visuals.window_corner_radius,
            fill: self.select_fill(style),
            stroke: self.select_stroke(style),
        })
    }

    fn crisp_magnified_text(&self) -> bool {
        self.crisp_magnified_text.unwrap_or(false)
    }

    fn wire_smoothness(&self) -> f32 {
        self.wire_smoothness.unwrap_or(1.0)
    }

    fn pin_hover_scale(&self) -> f32 {
        self.pin_hover_scale.unwrap_or(1.2)
    }

    fn pin_label_visibility(&self) -> PinLabelVisibility {
        self.pin_label_visibility.unwrap_or_default()
    }
}

#[cfg(feature = "serde")]
mod serde_frame_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    pub struct Frame {
        pub inner_margin: egui::Margin,
        pub outer_margin: egui::Margin,
        pub rounding: egui::CornerRadius,
        pub shadow: egui::epaint::Shadow,
        pub fill: egui::Color32,
        pub stroke: egui::Stroke,
    }

    #[allow(clippy::ref_option)]
    pub fn serialize<S>(frame: &Option<egui::Frame>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match frame {
            Some(frame) => Frame {
                inner_margin: frame.inner_margin,
                outer_margin: frame.outer_margin,
                rounding: frame.corner_radius,
                shadow: frame.shadow,
                fill: frame.fill,
                stroke: frame.stroke,
            }
            .serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<egui::Frame>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let frame_opt = Option::<Frame>::deserialize(deserializer)?;
        Ok(frame_opt.map(|frame| egui::Frame {
            inner_margin: frame.inner_margin,
            outer_margin: frame.outer_margin,
            corner_radius: frame.rounding,
            shadow: frame.shadow,
            fill: frame.fill,
            stroke: frame.stroke,
        }))
    }
}

impl SnarlStyle {
    /// Creates new [`SnarlStyle`] filled with default values.
    #[must_use]
    pub const fn new() -> Self {
        SnarlStyle {
            node_layout: None,
            pin_size: None,
            pin_fill: None,
            pin_stroke: None,
            pin_shape: None,
            pin_placement: None,
            wire_width: None,
            wire_frame_size: None,
            downscale_wire_frame: None,
            upscale_wire_frame: None,
            wire_style: None,
            vertical_wires: None,
            wire_layer: None,
            header_drag_space: None,
            collapsible: None,

            bg_frame: None,
            bg_pattern: None,
            bg_pattern_stroke: None,

            min_scale: None,
            max_scale: None,
            node_frame: None,
            header_frame: None,
            centering: None,
            select_stoke: None,
            select_fill: None,
            select_rect_contained: None,
            select_style: None,
            crisp_magnified_text: None,
            wire_smoothness: None,
            wire_widget_gap: None,
            wire_widget_align: None,
            pin_hover_scale: None,
            pin_label_visibility: None,

            _non_exhaustive: (),
        }
    }
}

impl Default for SnarlStyle {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
struct Input {
    hover_pos: Option<Pos2>,
    interact_pos: Option<Pos2>,
    zoom_delta: f32,
    // primary_pressed: bool,
    secondary_pressed: bool,
    modifiers: Modifiers,
    escape_pressed: bool,
}

struct DrawNodeResponse {
    node_moved: Option<(NodeId, Vec2)>,
    node_to_top: Option<NodeId>,
    drag_released: bool,
    pin_hovered: Option<AnyPin>,
    final_rect: Rect,
    in_pins: Vec<InPin>,
    out_pins: Vec<OutPin>,
}

struct DrawPinsResponse {
    drag_released: bool,
    pin_hovered: Option<AnyPin>,
    final_rect: Rect,
    new_heights: RowHeights,
}

struct DrawPinsHorizontalResponse {
    drag_released: bool,
    pin_hovered: Option<AnyPin>,
    final_rect: Rect,
    new_widths: RowHeights, // Reusing RowHeights type for widths
}

struct DrawBodyResponse {
    final_rect: Rect,
}

#[derive(Debug)]
struct PinResponse {
    pos: Pos2,
    wire_color: Color32,
    wire_style: WireStyle,
}

/// Widget to display [`Snarl`] graph in [`Ui`].
#[derive(Clone, Copy, Debug)]
pub struct SnarlWidget {
    id_salt: Id,
    id: Option<Id>,
    style: SnarlStyle,
    config: SnarlConfig,
    min_size: Vec2,
    max_size: Vec2,
}

impl Default for SnarlWidget {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SnarlWidget {
    /// Returns new [`SnarlWidget`] with default parameters.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        SnarlWidget {
            id_salt: Id::new(":snarl:"),
            id: None,
            style: SnarlStyle::new(),
            config: SnarlConfig::new(),
            min_size: Vec2::ZERO,
            max_size: Vec2::INFINITY,
        }
    }

    /// Assign an explicit and globally unique [`Id`].
    ///
    /// Use this if you want to persist the state of the widget
    /// when it changes position in the widget hierarchy.
    ///
    /// Prefer using [`SnarlWidget::id_salt`] otherwise.
    #[inline]
    #[must_use]
    pub const fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Assign a source for the unique [`Id`]
    ///
    /// It must be locally unique for the current [`Ui`] hierarchy position.
    ///
    /// Ignored if [`SnarlWidget::id`] was set.
    #[inline]
    #[must_use]
    pub fn id_salt(mut self, id_salt: impl Hash) -> Self {
        self.id_salt = Id::new(id_salt);
        self
    }

    /// Set style parameters for the [`Snarl`] widget.
    #[inline]
    #[must_use]
    pub const fn style(mut self, style: SnarlStyle) -> Self {
        self.style = style;
        self
    }

    /// Set config parameters for the [`Snarl`] widget.
    #[inline]
    #[must_use]
    pub fn config(mut self, config: SnarlConfig) -> Self {
        self.config = config;
        self
    }

    /// Set minimum size of the [`Snarl`] widget.
    #[inline]
    #[must_use]
    pub const fn min_size(mut self, min_size: Vec2) -> Self {
        self.min_size = min_size;
        self
    }

    /// Set maximum size of the [`Snarl`] widget.
    #[inline]
    #[must_use]
    pub const fn max_size(mut self, max_size: Vec2) -> Self {
        self.max_size = max_size;
        self
    }

    #[inline]
    fn get_id(&self, ui_id: Id) -> Id {
        self.id.unwrap_or_else(|| ui_id.with(self.id_salt))
    }

    /// Render [`Snarl`] using given viewer and style into the [`Ui`].
    #[inline]
    pub fn show<T, G, V>(&self, snarl: &mut Snarl<T, G>, viewer: &mut V, ui: &mut Ui) -> egui::Response
    where
        V: SnarlViewer<T, G>,
    {
        let snarl_id = self.get_id(ui.id());

        show_snarl(
            snarl_id,
            self.style,
            &self.config,
            self.min_size,
            self.max_size,
            snarl,
            viewer,
            ui,
        )
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn show_snarl<T, G, V>(
    snarl_id: Id,
    mut style: SnarlStyle,
    config: &SnarlConfig,
    min_size: Vec2,
    max_size: Vec2,
    snarl: &mut Snarl<T, G>,
    viewer: &mut V,
    ui: &mut Ui,
) -> egui::Response
where
    V: SnarlViewer<T, G>,
{
    let (mut latest_pos, input) = ui.ctx().input(|i| {
        (
            i.pointer.latest_pos(),
            Input {
                zoom_delta: i.zoom_delta(),
                hover_pos: i.pointer.hover_pos(),
                interact_pos: i.pointer.interact_pos(),
                modifiers: i.modifiers,
                // primary_pressed: i.pointer.primary_pressed(),
                secondary_pressed: i.pointer.secondary_pressed(),
                escape_pressed: i.key_pressed(Key::Escape),
            },
        )
    });

    let bg_frame = style.bg_frame(ui.style());

    let outer_size_bounds = ui.available_size_before_wrap().max(min_size).min(max_size);

    let outer_resp = ui.allocate_response(outer_size_bounds, Sense::hover());

    ui.painter().add(bg_frame.paint(outer_resp.rect));

    let mut content_rect = outer_resp.rect - bg_frame.total_margin();

    // Make sure we don't shrink to the negative:
    content_rect.max.x = content_rect.max.x.max(content_rect.min.x);
    content_rect.max.y = content_rect.max.y.max(content_rect.min.y);

    let snarl_layer_id = LayerId::new(ui.layer_id().order, snarl_id);
    let group_layer_id = LayerId::new(ui.layer_id().order, snarl_id.with("groups"));
    let wire_layer_id = LayerId::new(ui.layer_id().order, snarl_id.with("wires"));
    let node_layer_id = LayerId::new(ui.layer_id().order, snarl_id.with("nodes"));

    ui.ctx().set_sublayer(ui.layer_id(), snarl_layer_id);
    ui.ctx().set_sublayer(snarl_layer_id, group_layer_id);
    match style.wire_layer() {
        WireLayer::BehindNodes => {
            ui.ctx().set_sublayer(snarl_layer_id, wire_layer_id);
            ui.ctx().set_sublayer(snarl_layer_id, node_layer_id);
        }
        WireLayer::AboveNodes => {
            ui.ctx().set_sublayer(snarl_layer_id, node_layer_id);
            ui.ctx().set_sublayer(snarl_layer_id, wire_layer_id);
        }
    }

    let mut min_scale = style.min_scale();
    let mut max_scale = style.max_scale();

    let ui_rect = content_rect;

    let mut snarl_state =
        SnarlState::load(ui.ctx(), snarl_id, snarl, ui_rect, min_scale, max_scale);

    // Allow viewer to update selection before rendering
    if let Some(selection) = viewer.update_selection(snarl_state.selected_nodes()) {
        snarl_state.select_many_nodes(true, selection.iter().copied());
    }

    // Apply automatic layout if requested
    if viewer.apply_layout(snarl) {
        let layout = viewer.compute_layout(snarl);
        for (node_id, new_pos) in layout {
            if let Some(node) = snarl.nodes.get_mut(node_id.0) {
                node.pos = new_pos;
            }
        }
    }

    let mut to_global = snarl_state.to_global();

    let clip_rect = ui.clip_rect();

    let mut ui = ui.new_child(
        UiBuilder::new()
            .ui_stack_info(UiStackInfo::new(UiKind::Frame).with_frame(bg_frame))
            .layer_id(snarl_layer_id)
            .max_rect(Rect::EVERYTHING)
            .sense(Sense::click_and_drag()),
    );

    if style.crisp_magnified_text() {
        style.scale(max_scale);
        ui.style_mut().scale(max_scale);

        min_scale /= max_scale;
        max_scale = 1.0;
    }

    clamp_scale(&mut to_global, min_scale, max_scale, ui_rect);

    let mut snarl_resp = ui.response();
    Scene::new()
        .zoom_range(min_scale..=max_scale)
        .register_pan_and_zoom(&ui, &mut snarl_resp, &mut to_global);

    if snarl_resp.changed() {
        ui.ctx().request_repaint();
    }

    // Inform viewer about current transform.
    viewer.current_transform(&mut to_global, snarl);

    snarl_state.set_to_global(to_global);

    let to_global = to_global;
    let from_global = to_global.inverse();

    // Graph viewport
    let viewport = (from_global * ui_rect).round_ui();
    let viewport_clip = from_global * clip_rect;

    ui.set_clip_rect(viewport.intersect(viewport_clip));
    ui.expand_to_include_rect(viewport);

    // Set transform for snarl layer and sublayers.
    ui.ctx().set_transform_layer(snarl_layer_id, to_global);
    ui.ctx().set_transform_layer(group_layer_id, to_global);
    ui.ctx().set_transform_layer(wire_layer_id, to_global);
    ui.ctx().set_transform_layer(node_layer_id, to_global);

    // Map latest pointer position to graph space.
    latest_pos = latest_pos.map(|pos| from_global * pos);

    viewer.draw_background(
        style.bg_pattern.as_ref(),
        &viewport,
        &style,
        ui.style(),
        ui.painter(),
        snarl,
    );

    // Draw snap grid if visible
    if let Some(ref grid) = config.grid_snap {
        grid.draw(&viewport, ui.painter());
    }

    // --- Draw groups ---
    let dragged_nodes = draw_groups(snarl_id, snarl, viewer, config, &input, group_layer_id, &mut snarl_state, &mut ui);

    let mut node_moved = None;
    let mut node_to_top = None;

    // Process selection rect.
    let mut rect_selection_ended = None;
    if input.modifiers == config.rect_select.modifiers || snarl_state.is_rect_selection() {
        let select_resp = ui.interact(snarl_resp.rect, snarl_id.with("select"), Sense::drag());

        if select_resp.dragged_by(PointerButton::Primary)
            && let Some(pos) = select_resp.interact_pointer_pos()
        {
            if snarl_state.is_rect_selection() {
                snarl_state.update_rect_selection(pos);
            } else {
                snarl_state.start_rect_selection(pos);
            }
        }

        if select_resp.drag_stopped_by(config.rect_select.mouse_button) {
            if let Some(select_rect) = snarl_state.rect_selection() {
                rect_selection_ended = Some(select_rect);
            }
            snarl_state.stop_rect_selection();
        }
    }

    let wire_frame_size = style.wire_frame_size(ui.style());
    let wire_width = style.wire_width(ui.style());
    let wire_threshold = style.wire_smoothness();

    let mut input_info = HashMap::new();
    let mut output_info = HashMap::new();
    let mut input_pins = HashMap::new();
    let mut output_pins = HashMap::new();

    let mut pin_hovered = None;

    let draw_order = snarl_state.update_draw_order(snarl);
    let mut drag_released = false;

    let mut nodes_bb = Rect::NOTHING;
    let mut node_rects = Vec::new();

    let mut node_ui = ui.new_child(
        UiBuilder::new()
            .layer_id(node_layer_id)
            .max_rect(ui.max_rect()),
    );
    node_ui.set_clip_rect(ui.clip_rect());

    for node_idx in draw_order {
        if !snarl.nodes.contains(node_idx.0) {
            continue;
        }

        // Skip nodes hidden by collapsed groups
        if snarl.is_node_hidden(node_idx) {
            continue;
        }
        let response = draw_node(
            snarl,
            &mut node_ui,
            node_idx,
            viewer,
            &mut snarl_state,
            config,
            &style,
            snarl_id,
            &mut input_info,
            input.modifiers,
            &mut output_info,
        );

        if let Some(response) = response {
            if let Some(v) = response.node_to_top {
                node_to_top = Some(v);
            }
            if let Some(v) = response.node_moved {
                node_moved = Some(v);
            }
            if let Some(v) = response.pin_hovered {
                pin_hovered = Some(v);
            }
            drag_released |= response.drag_released;

            nodes_bb = nodes_bb.union(response.final_rect);
            if rect_selection_ended.is_some() {
                node_rects.push((node_idx, response.final_rect));
            }
            for pin in response.in_pins {
                input_pins.insert(pin.id, pin);
            }
            for pin in response.out_pins {
                output_pins.insert(pin.id, pin);
            }
        }
    }

    let mut wire_resp = WireResponse::default();
    let wire_info = wire::WireInfo {
        output_pins,
        output_info,
        input_pins,
        input_info,
        wire_frame_size,
        wire_width,
        latest_pos,
        wire_threshold,
    };
    // Draw and interact with wires
    for wire in snarl.wires.iter() {
        let _ = wire::show_wire(wire, snarl, viewer, &mut ui, snarl_id, &snarl_state, &style, config, &snarl_resp, &wire_info, &mut wire_resp);
    }

    // Remove hovered wire by second click
    if wire_resp.hovered_wire_disconnect && let Some(wire) = wire_resp.hovered_wire {
        let out_pin = OutPin::new(snarl, wire.out_pin);
        let in_pin = InPin::new(snarl, wire.in_pin);
        viewer.disconnect(&out_pin, &in_pin, snarl);
    }

    if let Some(select_rect) = rect_selection_ended {
        let mut select_nodes: Vec<NodeId> = node_rects
            .into_iter()
            .filter_map(|(id, rect)| {
                let select = if style.select_rect_contained() {
                    select_rect.contains_rect(rect)
                } else {
                    select_rect.intersects(rect)
                };

                if select { Some(id) } else { None }
            })
            .collect();

        // In single_select mode, only select one node (the last one found)
        if config.single_select && select_nodes.len() > 1 {
            select_nodes = select_nodes.into_iter().last().into_iter().collect();
        }

        if input
            .modifiers
            .contains(config.deselect_all_nodes.modifiers)
        {
            snarl_state.deselect_many_nodes(select_nodes.into_iter());
        } else {
            // In single_select mode, always reset (clear previous selection)
            let reset =
                config.single_select || !input.modifiers.contains(config.rect_select.modifiers);
            snarl_state.select_many_nodes(reset, select_nodes.into_iter());
        }
    }

    if let Some(select_rect) = snarl_state.rect_selection() {
        ui.painter().rect(
            select_rect,
            0.0,
            style.select_fill(ui.style()),
            style.select_stroke(ui.style()),
            StrokeKind::Inside,
        );
    }

    // If right button is clicked while new wire is being dragged, cancel it.
    // This is to provide way to 'not open' the link graph node menu, but just
    // releasing the new wire to empty space.
    //
    // This uses `button_down` directly, instead of `clicked_by` to improve
    // responsiveness of the cancel action.
    if snarl_state.has_new_wires()
        && ui.input(|x| x.pointer.button_down(config.cancel_wire_drag.mouse_button))
    {
        let _ = snarl_state.take_new_wires();
        snarl_resp.flags.remove(Flags::CLICKED);
    }

    // Do centering unless no nodes are present.
    if style.centering() && snarl_resp.double_clicked() && nodes_bb.is_finite() {
        let nodes_bb = nodes_bb.expand(100.0);
        snarl_state.look_at(nodes_bb, ui_rect, min_scale, max_scale);
    }

    if input.modifiers == config.deselect_all_nodes.modifiers
        && snarl_resp.clicked_by(config.deselect_all_nodes.mouse_button)
    {
        snarl_state.deselect_all_nodes();
    }

    // Wire end position will be overridden when link graph menu is opened.
    let mut wire_end_pos = latest_pos.unwrap_or_else(|| snarl_resp.rect.center());

    if drag_released {
        let new_wires = snarl_state.take_new_wires();
        if new_wires.is_some() {
            ui.ctx().request_repaint();
        }
        match (new_wires, pin_hovered) {
            (Some(NewWires::In(in_pins)), Some(AnyPin::Out(out_pin))) => {
                for in_pin in in_pins {
                    viewer.connect(
                        &OutPin::new(snarl, out_pin),
                        &InPin::new(snarl, in_pin),
                        snarl,
                    );
                }
            }
            (Some(NewWires::Out(out_pins)), Some(AnyPin::In(in_pin))) => {
                for out_pin in out_pins {
                    viewer.connect(
                        &OutPin::new(snarl, out_pin),
                        &InPin::new(snarl, in_pin),
                        snarl,
                    );
                }
            }
            (Some(new_wires), None) if snarl_resp.hovered() => {
                let pins = match &new_wires {
                    NewWires::In(x) => AnyPins::In(x),
                    NewWires::Out(x) => AnyPins::Out(x),
                };

                if viewer.has_dropped_wire_menu(pins, snarl) {
                    // A wire is dropped without connecting to a pin.
                    // Show context menu for the wire drop.
                    snarl_state.set_new_wires_menu(new_wires);

                    // Force open context menu.
                    snarl_resp.flags.insert(Flags::LONG_TOUCHED);
                }
            }
            _ => {}
        }
    }

    // If right button is clicked while new wire is being dragged, cancel it.
    // This is to provide way to 'not open' the link graph node menu, but just
    // releasing the new wire to empty space.
    //
    // This uses `button_down` directly, instead of `clicked_by` to improve
    // responsiveness of the cancel action.
    if snarl_state.has_new_wires() && ui.input(|x| x.pointer.button_down(PointerButton::Secondary))
    {
        let _ = snarl_state.take_new_wires();
    }

    if input.escape_pressed {
        snarl_state.deselect_all_nodes();
        snarl_state.deselect_all_groups();
    }

    if let Some(interact_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
        if let Some(new_wires) = snarl_state.take_new_wires_menu() {
            let pins = match &new_wires {
                NewWires::In(x) => AnyPins::In(x),
                NewWires::Out(x) => AnyPins::Out(x),
            };

            if viewer.has_dropped_wire_menu(pins, snarl) {
                snarl_resp.context_menu(|ui| {
                    let pins = match &new_wires {
                        NewWires::In(x) => AnyPins::In(x),
                        NewWires::Out(x) => AnyPins::Out(x),
                    };

                    let menu_pos = from_global * ui.cursor().min;

                    // Override wire end position when the wire-drop context menu is opened.
                    wire_end_pos = menu_pos;

                    // The context menu is opened as *link* graph menu.
                    viewer.show_dropped_wire_menu(menu_pos, ui, pins, snarl);

                    // Even though menu could be closed in `show_dropped_wire_menu`,
                    // we need to revert the new wires here, because menu state is inaccessible.
                    // Next frame context menu won't be shown and wires will be removed.
                    snarl_state.set_new_wires_menu(new_wires);
                });
            }
        } else if viewer.has_graph_menu(interact_pos, snarl) {
            snarl_resp.context_menu(|ui| {
                let menu_pos = from_global * ui.cursor().min;

                viewer.show_graph_menu(menu_pos, ui, snarl);
            });
        }
    }

    match snarl_state.new_wires() {
        None => {}
        Some(NewWires::In(in_pins)) => {
            for &in_pin in in_pins {
                let from_pos = wire_end_pos;
                let to_r = &wire_info.input_info[&in_pin];

                // Use vertical wire drawing when Y distance > X distance
                draw_wire(
                    &ui,
                    WireId::NewInput { snarl_id, in_pin },
                    &mut wire_resp.wire_shapes,
                    wire_frame_size,
                    style.upscale_wire_frame(),
                    style.downscale_wire_frame(),
                    from_pos,
                    to_r.pos,
                    Stroke::new(wire_width, to_r.wire_color),
                    wire_threshold,
                    to_r.wire_style,
                    style.vertical_wire(from_pos, to_r.pos),
                );
            }
        }
        Some(NewWires::Out(out_pins)) => {
            for &out_pin in out_pins {
                let from_r = &wire_info.output_info[&out_pin];
                let to_pos = wire_end_pos;

                // Use vertical wire drawing when Y distance > X distance
                let vertical_wire = style.vertical_wire(from_r.pos, to_pos);

                draw_wire(
                    &ui,
                    WireId::NewOutput { snarl_id, out_pin },
                    &mut wire_resp.wire_shapes,
                    wire_frame_size,
                    style.upscale_wire_frame(),
                    style.downscale_wire_frame(),
                    from_r.pos,
                    to_pos,
                    Stroke::new(wire_width, from_r.wire_color),
                    wire_threshold,
                    from_r.wire_style,
                    vertical_wire,
                );
            }
        }
    }

    let wire_painter = ui.painter().clone().with_layer_id(wire_layer_id);
    for shape in wire_resp.wire_shapes {
        wire_painter.add(shape);
    }

    for info in wire_resp.wire_widgets {
        let wire_x_length = (info.from_pos.x - info.to_pos.x).abs();

        for (index, descriptor) in info.descriptors.into_iter().enumerate() {
            let center = point_on_wire(
                wire_frame_size,
                style.upscale_wire_frame(),
                style.downscale_wire_frame(),
                info.from_pos,
                info.to_pos,
                info.wire_style,
                info.vertical,
                descriptor.t,
            );

            let id = Id::new("wire-widget").with((info.out_pin.id, info.in_pin.id, index));
            let cached = ui
                .ctx()
                .memory(|mem| mem.data.get_temp::<WireWidgetCache>(id));
            let child_size = match cached {
                Some(cached) => cached.widget_size,
                None => Vec2::new(wire_x_length, 0.0),
            };
            let ctx = wire::WireWidgetContext {
                t: descriptor.t,
                pos: center,
                align: descriptor
                    .align
                    .unwrap_or_else(|| style.wire_widget_align.unwrap_or(Align2::CENTER_CENTER)),
                gap: descriptor
                    .gap
                    .unwrap_or_else(|| style.wire_widget_gap.unwrap_or(0.0)),
            };
            let widget_rect = RectAlign {
                parent: Align2::CENTER_CENTER,
                child: ctx.align,
            }
            .align_rect(
                &Rect::from_center_size(center, [wire_x_length, 0.0].into()),
                child_size,
                ctx.gap,
            );
            let builder = UiBuilder::new()
                .layer_id(wire_layer_id)
                .max_rect(widget_rect)
                .layout(Layout::default())
                .id_salt(id);
            let mut wire_ui = ui.new_child(builder);
            viewer.show_wire_widget(
                index,
                &ctx,
                info.out_pin,
                info.in_pin,
                &mut wire_ui,
                snarl,
            );
            ui.ctx().memory_mut(|mem| {
                mem.data.insert_temp(
                    id,
                    WireWidgetCache {
                        widget_size: wire_ui.min_rect().size(),
                    },
                );
            });
        }
    }

    ui.advance_cursor_after_rect(Rect::from_min_size(snarl_resp.rect.min, Vec2::ZERO));

    if let Some(node) = node_to_top
        && snarl.nodes.contains(node.0)
    {
        snarl_state.node_to_top(node);
    }

    if let Some((node, delta)) = node_moved
        && snarl.nodes.contains(node.0)
    {
        ui.ctx().request_repaint();

        // Helper to snap position to grid if enabled
        let snap_to_grid = |pos: Pos2| -> Pos2 {
            if let Some(ref grid) = config.grid_snap {
                grid.snap(pos)
            } else {
                pos
            }
        };

        if snarl_state.selected_nodes().contains(&node) {
            for node_id in snarl_state.selected_nodes() {
                let node_data = &mut snarl.nodes[node_id.0];
                node_data.pos += delta;
                node_data.pos = snap_to_grid(node_data.pos);
                let new_pos = node_data.pos;
                viewer.node_moved(*node_id, new_pos, snarl);
            }
        } else {
            let node_data = &mut snarl.nodes[node.0];
            node_data.pos += delta;
            node_data.pos = snap_to_grid(node_data.pos);
            let new_pos = node_data.pos;
            viewer.node_moved(node, new_pos, snarl);
        }
    }

    // Update dragged nodes for next frame's group rendering.
    // When nodes are being dragged, they're excluded from group bounding rect computation
    // so they can be visually dragged out of a group.
    if let Some((node, _)) = node_moved {
        let nodes = if drag_released {
            // Drag ended — clear dragged nodes
            SmallVec::new()
        } else {
            let nodes: SmallVec<[NodeId; 4]> = if snarl_state.selected_nodes().contains(&node) {
                snarl_state.selected_nodes().iter().copied().collect()
            } else {
                smallvec::smallvec![node]
            };
            nodes
        };
        snarl_state.set_dragged_nodes(ui.ctx(), nodes);
    } else if !drag_released {
        // No node moved and no drag released — check if we should clear stale dragged state.
        // If dragged_nodes is non-empty but nothing is being dragged, clear it.
        if !dragged_nodes.is_empty() {
            snarl_state.set_dragged_nodes(ui.ctx(), SmallVec::new());
        }
    }

    // Detect group membership changes after node drag
    if drag_released {
        let nodes_to_check: &[NodeId] = if let Some((node, _)) = node_moved {
            if snarl_state.selected_nodes().contains(&node) {
                snarl_state.selected_nodes()
            } else {
                &[node]
            }
        } else {
            &[]
        };

        for &node_id in nodes_to_check {
            if !snarl.nodes.contains(node_id.0) {
                continue;
            }

            let node_pos = snarl.nodes[node_id.0].pos;
            let old_group = snarl.nodes[node_id.0].group;

            // Find the deepest group whose cached rect contains this node's position
            let mut best_group: Option<(crate::GroupId, u32)> = None;
            for (gid, group) in snarl.groups_ids_data() {
                if !group.open {
                    continue;
                }
                let gs = GroupState::load(ui.ctx(), snarl_id, gid);
                if gs.rect().is_finite() && gs.rect().contains(node_pos) {
                    let mut depth = 0u32;
                    let mut current = group.parent;
                    while let Some(pid) = current {
                        depth += 1;
                        current = snarl.group_info(pid).and_then(|g| g.parent);
                    }
                    if best_group.is_none_or(|(_, d)| depth > d) {
                        best_group = Some((gid, depth));
                    }
                }
                gs.store(ui.ctx());
            }

            let new_group = best_group.map(|(gid, _)| gid);

            if new_group != old_group {
                let accepted = new_group.is_none_or(|gid| {
                    viewer.accept_node_in_group(node_id, gid, snarl)
                });
                if accepted {
                    snarl.set_node_group(node_id, new_group);
                    viewer.node_group_changed(node_id, old_group, new_group, snarl);
                }
            }
        }
    }

    // Draw foreground elements (comments, annotations, overlays)
    viewer.draw_foreground(&viewport, &style, ui.style(), ui.painter(), snarl);

    snarl_state.store(snarl, ui.ctx());

    snarl_resp
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn draw_groups<T, G, V>(
    snarl_id: Id,
    snarl: &mut Snarl<T, G>,
    viewer: &mut V,
    config: &SnarlConfig,
    input: &Input,
    group_layer_id: LayerId,
    snarl_state: &mut SnarlState,
    ui: &mut Ui
) -> SmallVec<[NodeId; 4]>
where
    V: SnarlViewer<T, G>
{
    let group_draw_order = snarl_state.update_group_draw_order(snarl);
    let dragged_nodes = snarl_state.dragged_nodes(ui.ctx());
    let dragged_groups = snarl_state.dragged_groups(ui.ctx());
    let mut group_dragged: Option<GroupId> = None;
    let mut group_drag_released = false;

    {
        let mut group_ui = ui.new_child(
            UiBuilder::new()
                .layer_id(group_layer_id)
                .max_rect(ui.max_rect()),
        );
        group_ui.set_clip_rect(ui.clip_rect());

        let group_painter = group_ui.painter().clone().with_layer_id(group_layer_id);
        let group_padding = 20.0;
        let group_header_height = 24.0;

        for group_id in group_draw_order {
            let Some(group) = snarl.group_info(group_id) else {
                continue;
            };

            if !group.open {
                // Draw collapsed group
                let group_state = GroupState::load(group_ui.ctx(), snarl_id, group_id);
                let collapsed_pos = group_state.collapsed_pos()
                    .unwrap_or_else(|| {
                        if group_state.rect().is_finite() {
                            group_state.rect().min
                        } else {
                            group.pos
                        }
                    });
                let collapsed_size = egui::vec2(150.0, group_header_height + 8.0);
                let collapsed_rect = Rect::from_min_size(collapsed_pos, collapsed_size);

                let title = viewer.group_title(group_id, snarl);
                let default_frame = egui::Frame::group(group_ui.style())
                    .fill(group_ui.style().visuals.faint_bg_color)
                    .corner_radius(4.0);
                let frame = viewer.group_frame(default_frame, group_id, snarl);

                let resp = group_ui.interact(collapsed_rect, snarl_id.with(("group_interact", group_id.0)), Sense::click_and_drag());

                group_painter.add(frame.paint(collapsed_rect));

                group_painter.text(
                    collapsed_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("▶ {title}"),
                    egui::FontId::proportional(14.0),
                    group_ui.style().visuals.text_color(),
                );

                // Handle double-click to expand
                if resp.double_clicked() {
                    snarl.open_group(group_id, true);
                }

                // Handle drag
                if config.drag_group.dragged(input, &resp) {
                    let delta = resp.drag_delta();
                    // Move collapsed position
                    let mut gs = GroupState::load(group_ui.ctx(), snarl_id, group_id);
                    gs.set_collapsed_pos(collapsed_pos + delta);
                    // Move the group's own pos
                    snarl.groups[group_id.0].pos += delta;
                    // Also move all descendant nodes so they stay in place when expanded
                    let descendants = snarl.group_all_descendants(group_id);
                    for node_id in descendants {
                        snarl.nodes[node_id.0].pos += delta;
                    }
                    gs.store(group_ui.ctx());
                    group_dragged = Some(group_id);
                }

                if config.drag_group.drag_stopped(input, &resp) {
                    group_drag_released = true;
                    if group_dragged.is_none() {
                        group_dragged = Some(group_id);
                    }
                }

                group_state.store(group_ui.ctx());
                continue;
            }

            // Determine whether any current child (node or sub-group) of this
            // group is being dragged. If so, the group's geometry is "frozen":
            // it keeps its previous-frame content rect instead of following the
            // dragged child around. Dragging a child only changes group geometry
            // once the drag is released and membership is recomputed.
            let child_being_dragged = dragged_nodes
                .iter()
                .any(|id| snarl.nodes.contains(id.0) && snarl.nodes[id.0].group == Some(group_id))
                || dragged_groups
                    .iter()
                    .any(|&gid| snarl.group_info(gid).is_some_and(|g| g.parent == Some(group_id)));

            // Raw union of child geometry (before padding/header). While a child
            // is being dragged we reuse the frozen content rect from last frame.
            let mut content_rect = Rect::NOTHING;

            if child_being_dragged {
                let frozen = GroupState::load(group_ui.ctx(), snarl_id, group_id).content_rect();
                if frozen.is_finite() {
                    content_rect = frozen;
                }
            } else {
                // Compute bounding rect from child nodes
                for (idx, node) in &snarl.nodes {
                    if node.group == Some(group_id) {
                        let node_state = NodeState::load(group_ui.ctx(), snarl_id, NodeId(idx), group_ui.spacing());
                        let node_rect = node_state.node_rect(node.pos, if node.open { 1.0 } else { 0.0 });
                        content_rect = content_rect.union(node_rect);
                        node_state.store(group_ui.ctx());
                    }
                }

                // Include child group rects
                for (idx, child_group) in &snarl.groups {
                    let idx = GroupId(idx);
                    if child_group.parent == Some(group_id) {
                        let child_state = GroupState::load(group_ui.ctx(), snarl_id, idx);
                        if child_state.rect().is_finite() {
                            content_rect = content_rect.union(child_state.rect());
                        }
                        child_state.store(group_ui.ctx());
                    }
                }
            }

            let mut group_rect = content_rect;

            // For empty groups, use the default rect as the base for hit-testing.
            let base_rect = if content_rect.is_finite() {
                content_rect
            } else {
                let default_size = egui::vec2(150.0, 60.0);
                Rect::from_min_size(group.pos, default_size)
            };

            // Include dragged nodes/groups that are NOT current children but are
            // hovering over this group's base rect. This makes the group grow to
            // indicate that, when dropped, the item will become a child — and
            // shrink back when dragged out again. Current children are skipped:
            // their geometry is frozen above and must not follow the drag.
            //
            // `expanded` is set on the first hover-in. When it is, we seed the
            // rect with `base_rect` so the group only GROWS toward the incoming
            // item — the anchor corner stays pinned instead of the whole box
            // chasing the node (which matters most for an empty group, whose
            // `content_rect` is otherwise empty and would track the node alone).
            let mut expanded = false;
            for &dragged_id in dragged_nodes.iter() {
                if !snarl.nodes.contains(dragged_id.0) {
                    continue;
                }
                let node = &snarl.nodes[dragged_id.0];
                if node.group == Some(group_id) {
                    continue;
                }
                if base_rect.contains(node.pos) {
                    let node_state = NodeState::load(group_ui.ctx(), snarl_id, dragged_id, group_ui.spacing());
                    let node_rect = node_state.node_rect(node.pos, if node.open { 1.0 } else { 0.0 });
                    if !expanded {
                        group_rect = group_rect.union(base_rect);
                        expanded = true;
                    }
                    group_rect = group_rect.union(node_rect);
                    node_state.store(group_ui.ctx());
                }
            }

            for &dragged_gid in dragged_groups.iter() {
                if dragged_gid == group_id {
                    continue;
                }
                if snarl.group_info(dragged_gid).is_some_and(|g| g.parent == Some(group_id)) {
                    continue;
                }
                let child_state = GroupState::load(group_ui.ctx(), snarl_id, dragged_gid);
                if child_state.rect().is_finite() && base_rect.contains(snarl.groups[dragged_gid.0].pos) {
                    if !expanded {
                        group_rect = group_rect.union(base_rect);
                        expanded = true;
                    }
                    group_rect = group_rect.union(child_state.rect());
                }
                child_state.store(group_ui.ctx());
            }

            // Anchor the group's persistent position to its committed children
            // only. Transient hover-expansion (a non-member being dragged in)
            // grows the drawn rect below but must NOT move the anchor, or an
            // empty group would chase the node being dragged into it.
            if content_rect.is_finite() {
                snarl.groups[group_id.0].pos = content_rect.expand(group_padding).min;
            }

            if group_rect.is_finite() {
                // Expand rect for padding and header
                group_rect = group_rect.expand(group_padding);
            } else {
                // Empty group with nothing dragged in: use default rect
                group_rect = base_rect;
            }
            group_rect.min.y -= group_header_height;

            // Cache the computed rect, and persist the frozen child-union for
            // next frame. (When frozen, content_rect is unchanged; the hover
            // expansion is NOT baked in, so it can shrink back when a dragged-in
            // item leaves.)
            let mut group_state = GroupState::load(group_ui.ctx(), snarl_id, group_id);
            group_state.set_rect(group_rect);
            group_state.set_content_rect(content_rect);

            // Draw group background
            let title = viewer.group_title(group_id, snarl);
            let default_frame = egui::Frame::group(group_ui.style())
                .fill(group_ui.style().visuals.faint_bg_color.linear_multiply(0.5))
                .corner_radius(8.0);
            let frame = viewer.group_frame(default_frame, group_id, snarl);

            group_painter.add(frame.paint(group_rect));

            // Draw header with title
            let header_rect = Rect::from_min_size(
                group_rect.min,
                egui::vec2(group_rect.width(), group_header_height),
            );
            group_painter.text(
                header_rect.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("▼ {title}"),
                egui::FontId::proportional(14.0),
                group_ui.style().visuals.text_color(),
            );

            // Handle interaction
            let resp = group_ui.interact(header_rect, snarl_id.with(("group_header", group_id.0)), Sense::click_and_drag());

            // Double-click to collapse
            if resp.double_clicked() {
                // Save current rect center as collapsed position
                group_state.set_collapsed_pos(group_rect.center_top());
                snarl.open_group(group_id, false);
            }

            // Drag group header to move all children
            if config.drag_group.dragged(input, &resp) {
                let delta = resp.drag_delta();
                // Move the group's own pos
                snarl.groups[group_id.0].pos += delta;
                // Move all descendant nodes
                let descendants = snarl.group_all_descendants(group_id);
                for node_id in descendants {
                    snarl.nodes[node_id.0].pos += delta;
                }
                // Also move child groups' positions and collapsed positions
                for (idx, child_group) in &snarl.groups {
                    if child_group.parent == Some(group_id) {
                        let mut cs = GroupState::load(group_ui.ctx(), snarl_id, GroupId(idx));
                        if let Some(pos) = cs.collapsed_pos() {
                            cs.set_collapsed_pos(pos + delta);
                        }
                        cs.store(group_ui.ctx());
                    }
                }
                group_dragged = Some(group_id);
            }

            if config.drag_group.drag_stopped(input, &resp) {
                group_drag_released = true;
                if group_dragged.is_none() {
                    group_dragged = Some(group_id);
                }
            }

            // Selection highlight
            if snarl_state.selected_groups().contains(&group_id) {
                let highlight_rect = group_rect.expand(2.0);
                group_painter.rect_stroke(
                    highlight_rect,
                    8.0,
                    egui::Stroke::new(2.0_f32, group_ui.style().visuals.selection.stroke.color),
                    StrokeKind::Outside,
                );
            }

            // Click to select group
            if config.select_group.clicked(input, &resp) {
                if snarl_state.selected_groups().contains(&group_id) {
                    snarl_state.deselect_group(group_id);
                } else {
                    snarl_state.select_group(group_id);
                }
            } else if config.click_group.clicked(input, &resp) {
                snarl_state.deselect_all_groups();
                snarl_state.select_group(group_id);
            }

            // Context menu
            if resp.secondary_clicked() && viewer.has_group_menu(group_id, snarl) {
                resp.context_menu(|ui| {
                    viewer.show_group_menu(group_id, ui, snarl);
                });
            }

            group_state.store(group_ui.ctx());
        }
    }

    // Update dragged groups for next frame's parent group rendering.
    if let Some(gid) = group_dragged {
        let groups = if group_drag_released {
            SmallVec::new()
        } else {
            smallvec::smallvec![gid]
        };
        snarl_state.set_dragged_groups(ui.ctx(), groups);
    } else if !dragged_groups.is_empty() {
        snarl_state.set_dragged_groups(ui.ctx(), SmallVec::new());
    }

    // Detect group parent changes after group drag release
    if group_drag_released && let Some(dragged_gid) = group_dragged {
        let group_pos = snarl.groups[dragged_gid.0].pos;
        let old_parent = snarl.groups[dragged_gid.0].parent;

        // Find the deepest group whose cached rect contains this group's position,
        // excluding the dragged group itself and its descendants.
        let mut descendant_groups = Vec::new();
        collect_descendant_groups(snarl, dragged_gid, &mut descendant_groups);

        let mut best_parent: Option<(crate::GroupId, u32)> = None;
        for (gid, group) in snarl.groups_ids_data() {
            if gid == dragged_gid || descendant_groups.contains(&gid) {
                continue;
            }
            if !group.open {
                continue;
            }
            let gs = GroupState::load(ui.ctx(), snarl_id, gid);
            if gs.rect().is_finite() && gs.rect().contains(group_pos) {
                let mut depth = 0u32;
                let mut current = group.parent;
                while let Some(pid) = current {
                    depth += 1;
                    current = snarl.group_info(pid).and_then(|g| g.parent);
                }
                if best_parent.is_none_or(|(_, d)| depth > d) {
                    best_parent = Some((gid, depth));
                }
            }
            gs.store(ui.ctx());
        }

        let new_parent = best_parent.map(|(gid, _)| gid);
        if new_parent != old_parent {
            snarl.set_group_parent(dragged_gid, new_parent);
        }
    }
    dragged_nodes
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn draw_inputs<T, G, V>(
    snarl: &mut Snarl<T, G>,
    viewer: &mut V,
    node: NodeId,
    inputs: &[InPin],
    pin_size: f32,
    style: &SnarlStyle,
    node_ui: &mut Ui,
    inputs_rect: Rect,
    payload_clip_rect: Rect,
    input_x: f32,
    min_pin_y_top: f32,
    min_pin_y_bottom: f32,
    input_spacing: Option<f32>,
    snarl_state: &mut SnarlState,
    config: &SnarlConfig,
    modifiers: Modifiers,
    input_positions: &mut HashMap<InPinId, PinResponse>,
    heights: Heights,
) -> DrawPinsResponse
where
    V: SnarlViewer<T, G>,
{
    let mut drag_released = false;
    let mut pin_hovered = None;

    // Input pins on the left.
    let mut inputs_ui = node_ui.new_child(
        UiBuilder::new()
            .max_rect(inputs_rect.round_ui())
            .layout(Layout::top_down(Align::Min))
            .id_salt("inputs"),
    );

    let snarl_clip_rect = node_ui.clip_rect();
    inputs_ui.shrink_clip_rect(payload_clip_rect);

    let pin_layout = Layout::left_to_right(Align::Min);
    let mut new_heights = SmallVec::with_capacity(inputs.len());

    for in_pin in inputs {
        // Show input pin.
        let cursor = inputs_ui.cursor();
        let (height, height_outer) = heights.get(in_pin.id.input);

        let margin = (height_outer - height) / 2.0;
        let outer_rect = cursor.with_max_y(cursor.top() + height_outer);
        let inner_rect = outer_rect.shrink2(vec2(0.0, margin));

        let builder = UiBuilder::new().layout(pin_layout).max_rect(inner_rect);

        inputs_ui.scope_builder(builder, |pin_ui| {
            if let Some(input_spacing) = input_spacing {
                let min = pin_ui.next_widget_position();
                pin_ui.advance_cursor_after_rect(Rect::from_min_size(
                    min,
                    vec2(input_spacing, pin_size),
                ));
            }

            // Compute PinContext based on style.pin_label_visibility()
            let is_connecting = snarl_state.has_new_wires();
            let label_visible = match style.pin_label_visibility() {
                PinLabelVisibility::Always => true,
                PinLabelVisibility::OnHover => false, // Will show via hover tooltip instead
                PinLabelVisibility::WhileConnecting => is_connecting,
                PinLabelVisibility::NearWire { .. } => is_connecting, // Simplified: show all when connecting
            };
            let pin_context = PinContext {
                label_visible,
                is_connecting,
                is_hovered: false,             // Updated after interaction
                wire_near_node: is_connecting, // Simplified for now
            };
            let snarl_pin = viewer.show_input(in_pin, pin_ui, pin_context, snarl);
            if !snarl.nodes.contains(node.0) {
                // If removed
                return;
            }

            // Use min_rect after content is shown to get actual content bounds.
            // This ensures pin is aligned with actual content, not pre-allocated space.
            let content_rect = pin_ui.min_rect();
            let y0 = content_rect.min.y;
            let y1 = content_rect.max.y;

            let pin_rect = snarl_pin.pin_rect(
                input_x,
                min_pin_y_top.max(y0),
                min_pin_y_bottom.max(y1),
                pin_size,
            );

            // Interact with pin shape.
            pin_ui.set_clip_rect(snarl_clip_rect);

            let r = pin_ui.interact(pin_rect, pin_ui.next_auto_id(), Sense::click_and_drag());

            pin_ui.skip_ahead_auto_ids(1);

            if r.clicked_by(config.click_pin.mouse_button) {
                if snarl_state.has_new_wires() {
                    snarl_state.remove_new_wire_in(in_pin.id);
                } else {
                    viewer.drop_inputs(in_pin, snarl);
                    if !snarl.nodes.contains(node.0) {
                        // If removed
                        return;
                    }
                }
            }
            if r.drag_started_by(config.drag_pin.mouse_button) {
                // TODO: I am not sure what start_new_wires_out() does?
                // Is that for a future feature where you can draw several wires at once?
                if modifiers.command {
                    snarl_state.start_new_wires_out(&in_pin.remotes);
                    if modifiers != config.drag_pin.modifiers {
                        snarl.drop_inputs(in_pin.id);
                        if !snarl.nodes.contains(node.0) {
                            // If removed
                            return;
                        }
                    }
                } else {
                    snarl_state.start_new_wire_in(in_pin.id);
                }
            }

            if r.drag_stopped() {
                drag_released = true;
            }

            let mut visual_pin_rect = r.rect;

            if r.contains_pointer() {
                if snarl_state.has_new_wires_in() {
                    // TODO: And here I am also not sure what these modifiers do
                    if modifiers.shift && !modifiers.command {
                        snarl_state.add_new_wire_in(in_pin.id);
                    }
                    if !modifiers.shift && modifiers.command {
                        snarl_state.remove_new_wire_in(in_pin.id);
                    }
                }
                pin_hovered = Some(AnyPin::In(in_pin.id));
                visual_pin_rect = visual_pin_rect.scale_from_center(style.pin_hover_scale());
            }

            let wire_info =
                snarl_pin.draw(style, pin_ui.style(), visual_pin_rect, pin_ui.painter());

            input_positions.insert(
                in_pin.id,
                PinResponse {
                    pos: r.rect.center(),
                    wire_color: wire_info.color,
                    wire_style: wire_info.style,
                },
            );

            new_heights.push(pin_ui.min_rect().height());

            pin_ui.expand_to_include_y(outer_rect.bottom());
        });
    }

    let final_rect = inputs_ui.min_rect();
    node_ui.expand_to_include_rect(final_rect.intersect(payload_clip_rect));

    DrawPinsResponse {
        drag_released,
        pin_hovered,
        final_rect,
        new_heights,
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn draw_outputs<T, G, V>(
    snarl: &mut Snarl<T, G>,
    viewer: &mut V,
    node: NodeId,
    outputs: &[OutPin],
    pin_size: f32,
    style: &SnarlStyle,
    node_ui: &mut Ui,
    outputs_rect: Rect,
    payload_clip_rect: Rect,
    output_x: f32,
    min_pin_y_top: f32,
    min_pin_y_bottom: f32,
    output_spacing: Option<f32>,
    snarl_state: &mut SnarlState,
    config: &SnarlConfig,
    modifiers: Modifiers,
    output_positions: &mut HashMap<OutPinId, PinResponse>,
    heights: Heights,
) -> DrawPinsResponse
where
    V: SnarlViewer<T, G>,
{
    let mut drag_released = false;
    let mut pin_hovered = None;

    let mut outputs_ui = node_ui.new_child(
        UiBuilder::new()
            .max_rect(outputs_rect.round_ui())
            .layout(Layout::top_down(Align::Max))
            .id_salt("outputs"),
    );

    let snarl_clip_rect = node_ui.clip_rect();
    outputs_ui.shrink_clip_rect(payload_clip_rect);

    let pin_layout = Layout::right_to_left(Align::Min);
    let mut new_heights = SmallVec::with_capacity(outputs.len());

    // Output pins on the right.
    for out_pin in outputs {
        // Show output pin.
        let cursor = outputs_ui.cursor();
        let (height, height_outer) = heights.get(out_pin.id.output);

        let margin = (height_outer - height) / 2.0;
        let outer_rect = cursor.with_max_y(cursor.top() + height_outer);
        let inner_rect = outer_rect.shrink2(vec2(0.0, margin));

        let builder = UiBuilder::new().layout(pin_layout).max_rect(inner_rect);

        outputs_ui.scope_builder(builder, |pin_ui| {
            // Allocate space for pin shape.
            if let Some(output_spacing) = output_spacing {
                let min = pin_ui.next_widget_position();
                pin_ui.advance_cursor_after_rect(Rect::from_min_size(
                    min,
                    vec2(output_spacing, pin_size),
                ));
            }

            // Compute PinContext based on style.pin_label_visibility()
            let is_connecting = snarl_state.has_new_wires();
            let label_visible = match style.pin_label_visibility() {
                PinLabelVisibility::Always => true,
                PinLabelVisibility::OnHover => false, // Will show via hover tooltip instead
                PinLabelVisibility::WhileConnecting => is_connecting,
                PinLabelVisibility::NearWire { .. } => is_connecting, // Simplified: show all when connecting
            };
            let pin_context = PinContext {
                label_visible,
                is_connecting,
                is_hovered: false,             // Updated after interaction
                wire_near_node: is_connecting, // Simplified for now
            };
            let snarl_pin = viewer.show_output(out_pin, pin_ui, pin_context, snarl);
            if !snarl.nodes.contains(node.0) {
                // If removed
                return;
            }

            // Use min_rect after content is shown to get actual content bounds.
            // This ensures pin is aligned with actual content, not pre-allocated space.
            let content_rect = pin_ui.min_rect();
            let y0 = content_rect.min.y;
            let y1 = content_rect.max.y;

            let pin_rect = snarl_pin.pin_rect(
                output_x,
                min_pin_y_top.max(y0),
                min_pin_y_bottom.max(y1),
                pin_size,
            );

            pin_ui.set_clip_rect(snarl_clip_rect);

            let r = pin_ui.interact(pin_rect, pin_ui.next_auto_id(), Sense::click_and_drag());

            pin_ui.skip_ahead_auto_ids(1);

            if r.clicked_by(config.click_pin.mouse_button) {
                if snarl_state.has_new_wires() {
                    snarl_state.remove_new_wire_out(out_pin.id);
                } else {
                    viewer.drop_outputs(out_pin, snarl);
                    if !snarl.nodes.contains(node.0) {
                        // If removed
                        return;
                    }
                }
            }
            if r.drag_started_by(config.drag_pin.mouse_button) {
                if modifiers.contains(config.drag_pin.modifiers) {
                    snarl_state.start_new_wires_in(&out_pin.remotes);

                    if !modifiers.contains(config.no_menu.modifiers) {
                        snarl.drop_outputs(out_pin.id);
                        if !snarl.nodes.contains(node.0) {
                            // If removed
                            return;
                        }
                    }
                } else {
                    snarl_state.start_new_wire_out(out_pin.id);
                }
            }

            if r.drag_stopped() {
                drag_released = true;
            }

            let mut visual_pin_rect = r.rect;

            if r.contains_pointer() {
                // TODO: What is happening here?
                if snarl_state.has_new_wires_out() {
                    if modifiers.shift && !modifiers.command {
                        snarl_state.add_new_wire_out(out_pin.id);
                    }
                    if !modifiers.shift && modifiers.command {
                        snarl_state.remove_new_wire_out(out_pin.id);
                    }
                }
                pin_hovered = Some(AnyPin::Out(out_pin.id));
                visual_pin_rect = visual_pin_rect.scale_from_center(style.pin_hover_scale());
            }

            let wire_info =
                snarl_pin.draw(style, pin_ui.style(), visual_pin_rect, pin_ui.painter());

            output_positions.insert(
                out_pin.id,
                PinResponse {
                    pos: r.rect.center(),
                    wire_color: wire_info.color,
                    wire_style: wire_info.style,
                },
            );

            new_heights.push(pin_ui.min_rect().height());

            pin_ui.expand_to_include_y(outer_rect.bottom());
        });
    }
    let final_rect = outputs_ui.min_rect();
    node_ui.expand_to_include_rect(final_rect.intersect(payload_clip_rect));

    DrawPinsResponse {
        drag_released,
        pin_hovered,
        final_rect,
        new_heights,
    }
}

/// Draws input pins in a horizontal row (for vertical sandwich layouts).
/// Pins are arranged left-to-right with pin icons at the edge and labels below/above.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn draw_inputs_horizontal<T, G, V>(
    snarl: &mut Snarl<T, G>,
    viewer: &mut V,
    node: NodeId,
    inputs: &[InPin],
    pin_size: f32,
    style: &SnarlStyle,
    node_ui: &mut Ui,
    inputs_rect: Rect,
    payload_clip_rect: Rect,
    input_y: f32,
    min_pin_x_left: f32,
    min_pin_x_right: f32,
    snarl_state: &mut SnarlState,
    config: &SnarlConfig,
    modifiers: Modifiers,
    input_positions: &mut HashMap<InPinId, PinResponse>,
    widths: Widths,
    pins_at_top: bool,
) -> DrawPinsHorizontalResponse
where
    V: SnarlViewer<T, G>,
{
    let mut drag_released = false;
    let mut pin_hovered = None;

    // Input pins in horizontal row
    let mut inputs_ui = node_ui.new_child(
        UiBuilder::new()
            .max_rect(inputs_rect.round_ui())
            .layout(Layout::left_to_right(Align::Center))
            .id_salt("inputs_horizontal"),
    );

    let snarl_clip_rect = node_ui.clip_rect();
    inputs_ui.shrink_clip_rect(payload_clip_rect);

    // Pin layout: vertical stack with pin icon at edge, label on other side
    let pin_layout = if pins_at_top {
        Layout::top_down(Align::Center) // Pin at top, label below
    } else {
        Layout::bottom_up(Align::Center) // Pin at bottom, label above
    };

    let mut new_widths = SmallVec::with_capacity(inputs.len());

    for in_pin in inputs {
        let cursor = inputs_ui.cursor();
        let (width, width_outer) = widths.get(in_pin.id.input);

        let margin = (width_outer - width) / 2.0;
        let outer_rect = cursor.with_max_x(cursor.left() + width_outer);
        let inner_rect = outer_rect.shrink2(vec2(margin, 0.0));

        let builder = UiBuilder::new().layout(pin_layout).max_rect(inner_rect);

        inputs_ui.scope_builder(builder, |pin_ui| {
            // Compute PinContext based on style.pin_label_visibility()
            let is_connecting = snarl_state.has_new_wires();
            let label_visible = match style.pin_label_visibility() {
                PinLabelVisibility::Always => true,
                PinLabelVisibility::OnHover => false,
                PinLabelVisibility::WhileConnecting => is_connecting,
                PinLabelVisibility::NearWire { .. } => is_connecting,
            };
            let pin_context = PinContext {
                label_visible,
                is_connecting,
                is_hovered: false,
                wire_near_node: is_connecting,
            };
            let snarl_pin = viewer.show_input(in_pin, pin_ui, pin_context, snarl);
            if !snarl.nodes.contains(node.0) {
                return;
            }

            // Use min_rect after content is shown to get actual content bounds.
            let content_rect = pin_ui.min_rect();
            let x0 = content_rect.min.x;
            let x1 = content_rect.max.x;

            let pin_rect = snarl_pin.pin_rect_horizontal(
                input_y,
                min_pin_x_left.max(x0),
                min_pin_x_right.max(x1),
                pin_size,
            );

            // Interact with pin shape.
            pin_ui.set_clip_rect(snarl_clip_rect);

            let r = pin_ui.interact(pin_rect, pin_ui.next_auto_id(), Sense::click_and_drag());
            pin_ui.skip_ahead_auto_ids(1);

            if r.clicked_by(config.click_pin.mouse_button) {
                if snarl_state.has_new_wires() {
                    snarl_state.remove_new_wire_in(in_pin.id);
                } else {
                    viewer.drop_inputs(in_pin, snarl);
                    if !snarl.nodes.contains(node.0) {
                        return;
                    }
                }
            }
            if r.drag_started_by(config.drag_pin.mouse_button) {
                if modifiers.command {
                    snarl_state.start_new_wires_out(&in_pin.remotes);
                    if modifiers != config.drag_pin.modifiers {
                        snarl.drop_inputs(in_pin.id);
                        if !snarl.nodes.contains(node.0) {
                            return;
                        }
                    }
                } else {
                    snarl_state.start_new_wire_in(in_pin.id);
                }
            }

            if r.drag_stopped() {
                drag_released = true;
            }

            let mut visual_pin_rect = r.rect;

            if r.contains_pointer() {
                if snarl_state.has_new_wires_in() {
                    if modifiers.shift && !modifiers.command {
                        snarl_state.add_new_wire_in(in_pin.id);
                    }
                    if !modifiers.shift && modifiers.command {
                        snarl_state.remove_new_wire_in(in_pin.id);
                    }
                }
                pin_hovered = Some(AnyPin::In(in_pin.id));
                visual_pin_rect = visual_pin_rect.scale_from_center(style.pin_hover_scale());
            }

            let wire_info =
                snarl_pin.draw(style, pin_ui.style(), visual_pin_rect, pin_ui.painter());

            input_positions.insert(
                in_pin.id,
                PinResponse {
                    pos: r.rect.center(),
                    wire_color: wire_info.color,
                    wire_style: wire_info.style,
                },
            );

            new_widths.push(pin_ui.min_rect().width());

            pin_ui.expand_to_include_x(outer_rect.right());
        });
    }

    let final_rect = inputs_ui.min_rect();
    node_ui.expand_to_include_rect(final_rect.intersect(payload_clip_rect));

    DrawPinsHorizontalResponse {
        drag_released,
        pin_hovered,
        final_rect,
        new_widths,
    }
}

/// Draws output pins in a horizontal row (for vertical sandwich layouts).
/// Pins are arranged left-to-right with pin icons at the edge and labels below/above.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn draw_outputs_horizontal<T, G, V>(
    snarl: &mut Snarl<T, G>,
    viewer: &mut V,
    node: NodeId,
    outputs: &[OutPin],
    pin_size: f32,
    style: &SnarlStyle,
    node_ui: &mut Ui,
    outputs_rect: Rect,
    payload_clip_rect: Rect,
    output_y: f32,
    min_pin_x_left: f32,
    min_pin_x_right: f32,
    snarl_state: &mut SnarlState,
    config: &SnarlConfig,
    modifiers: Modifiers,
    output_positions: &mut HashMap<OutPinId, PinResponse>,
    widths: Widths,
    pins_at_top: bool,
) -> DrawPinsHorizontalResponse
where
    V: SnarlViewer<T, G>,
{
    let mut drag_released = false;
    let mut pin_hovered = None;

    let mut outputs_ui = node_ui.new_child(
        UiBuilder::new()
            .max_rect(outputs_rect.round_ui())
            .layout(Layout::left_to_right(Align::Center))
            .id_salt("outputs_horizontal"),
    );

    let snarl_clip_rect = node_ui.clip_rect();
    outputs_ui.shrink_clip_rect(payload_clip_rect);

    // Pin layout: vertical stack with pin icon at edge, label on other side
    let pin_layout = if pins_at_top {
        Layout::top_down(Align::Center) // Pin at top, label below
    } else {
        Layout::bottom_up(Align::Center) // Pin at bottom, label above
    };

    let mut new_widths = SmallVec::with_capacity(outputs.len());

    for out_pin in outputs {
        let cursor = outputs_ui.cursor();
        let (width, width_outer) = widths.get(out_pin.id.output);

        let margin = (width_outer - width) / 2.0;
        let outer_rect = cursor.with_max_x(cursor.left() + width_outer);
        let inner_rect = outer_rect.shrink2(vec2(margin, 0.0));

        let builder = UiBuilder::new().layout(pin_layout).max_rect(inner_rect);

        outputs_ui.scope_builder(builder, |pin_ui| {
            // Compute PinContext based on style.pin_label_visibility()
            let is_connecting = snarl_state.has_new_wires();
            let label_visible = match style.pin_label_visibility() {
                PinLabelVisibility::Always => true,
                PinLabelVisibility::OnHover => false,
                PinLabelVisibility::WhileConnecting => is_connecting,
                PinLabelVisibility::NearWire { .. } => is_connecting,
            };
            let pin_context = PinContext {
                label_visible,
                is_connecting,
                is_hovered: false,
                wire_near_node: is_connecting,
            };
            let snarl_pin = viewer.show_output(out_pin, pin_ui, pin_context, snarl);
            if !snarl.nodes.contains(node.0) {
                return;
            }

            let content_rect = pin_ui.min_rect();
            let x0 = content_rect.min.x;
            let x1 = content_rect.max.x;

            let pin_rect = snarl_pin.pin_rect_horizontal(
                output_y,
                min_pin_x_left.max(x0),
                min_pin_x_right.max(x1),
                pin_size,
            );

            pin_ui.set_clip_rect(snarl_clip_rect);

            let r = pin_ui.interact(pin_rect, pin_ui.next_auto_id(), Sense::click_and_drag());
            pin_ui.skip_ahead_auto_ids(1);

            if r.clicked_by(config.click_pin.mouse_button) {
                if snarl_state.has_new_wires() {
                    snarl_state.remove_new_wire_out(out_pin.id);
                } else {
                    viewer.drop_outputs(out_pin, snarl);
                    if !snarl.nodes.contains(node.0) {
                        return;
                    }
                }
            }
            if r.drag_started_by(config.drag_pin.mouse_button) {
                if modifiers.contains(config.drag_pin.modifiers) {
                    snarl_state.start_new_wires_in(&out_pin.remotes);
                    if !modifiers.contains(config.no_menu.modifiers) {
                        snarl.drop_outputs(out_pin.id);
                        if !snarl.nodes.contains(node.0) {
                            return;
                        }
                    }
                } else {
                    snarl_state.start_new_wire_out(out_pin.id);
                }
            }

            if r.drag_stopped() {
                drag_released = true;
            }

            let mut visual_pin_rect = r.rect;

            if r.contains_pointer() {
                if snarl_state.has_new_wires_out() {
                    if modifiers.shift && !modifiers.command {
                        snarl_state.add_new_wire_out(out_pin.id);
                    }
                    if !modifiers.shift && modifiers.command {
                        snarl_state.remove_new_wire_out(out_pin.id);
                    }
                }
                pin_hovered = Some(AnyPin::Out(out_pin.id));
                visual_pin_rect = visual_pin_rect.scale_from_center(style.pin_hover_scale());
            }

            let wire_info =
                snarl_pin.draw(style, pin_ui.style(), visual_pin_rect, pin_ui.painter());

            output_positions.insert(
                out_pin.id,
                PinResponse {
                    pos: r.rect.center(),
                    wire_color: wire_info.color,
                    wire_style: wire_info.style,
                },
            );

            new_widths.push(pin_ui.min_rect().width());

            pin_ui.expand_to_include_x(outer_rect.right());
        });
    }

    let final_rect = outputs_ui.min_rect();
    node_ui.expand_to_include_rect(final_rect.intersect(payload_clip_rect));

    DrawPinsHorizontalResponse {
        drag_released,
        pin_hovered,
        final_rect,
        new_widths,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_body<T, G, V>(
    snarl: &mut Snarl<T, G>,
    viewer: &mut V,
    node: NodeId,
    inputs: &[InPin],
    outputs: &[OutPin],
    ui: &mut Ui,
    body_rect: Rect,
    payload_clip_rect: Rect,
    _snarl_state: &SnarlState,
) -> DrawBodyResponse
where
    V: SnarlViewer<T, G>,
{
    let mut body_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(body_rect.round_ui())
            .layout(Layout::left_to_right(Align::Min))
            .id_salt("body"),
    );

    body_ui.shrink_clip_rect(payload_clip_rect);

    viewer.show_body(node, inputs, outputs, &mut body_ui, snarl);

    let final_rect = body_ui.min_rect();
    ui.expand_to_include_rect(final_rect.intersect(payload_clip_rect));
    // node_state.set_body_width(body_size.x);

    DrawBodyResponse { final_rect }
}

//First step for split big function to parts
/// Draw one node. Return Pins info
#[inline]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn draw_node<T, G, V>(
    snarl: &mut Snarl<T, G>,
    ui: &mut Ui,
    node: NodeId,
    viewer: &mut V,
    snarl_state: &mut SnarlState,
    config: &SnarlConfig,
    style: &SnarlStyle,
    snarl_id: Id,
    input_positions: &mut HashMap<InPinId, PinResponse>,
    modifiers: Modifiers,
    output_positions: &mut HashMap<OutPinId, PinResponse>,
) -> Option<DrawNodeResponse>
where
    V: SnarlViewer<T, G>,
{
    let Node {
        pos,
        open,
        ref value,
        ..
    } = snarl.nodes[node.0];

    // Collect pins
    let inputs_count = viewer.inputs(value);
    let outputs_count = viewer.outputs(value);

    let inputs = (0..inputs_count)
        .map(|idx| InPin::new(snarl, InPinId { node, input: idx }))
        .collect::<Vec<_>>();

    let outputs = (0..outputs_count)
        .map(|idx| OutPin::new(snarl, OutPinId { node, output: idx }))
        .collect::<Vec<_>>();

    let node_pos = pos.round_ui();

    // Generate persistent id for the node.
    let node_id = snarl_id.with(("snarl-node", node));

    let openness = ui.ctx().animate_bool(node_id, open);

    let mut node_state = NodeState::load(ui.ctx(), snarl_id, node, ui.spacing());

    let node_rect = node_state.node_rect(node_pos, openness);

    let mut node_to_top = None;
    let mut node_moved = None;
    let mut drag_released = false;
    let mut pin_hovered = None;

    let node_frame =
        viewer.node_frame(style.node_frame(ui.style()), node, &inputs, &outputs, snarl);

    let header_frame = viewer.header_frame(
        style.header_frame(ui.style()),
        node,
        &inputs,
        &outputs,
        snarl,
    );

    // Rect for node + frame margin.
    let node_frame_rect = node_rect + node_frame.total_margin();

    if snarl_state.selected_nodes().contains(&node) {
        let select_style = style.select_style(ui.style());

        let select_rect = node_frame_rect + select_style.margin;

        ui.painter().rect(
            select_rect,
            select_style.rounding,
            select_style.fill,
            select_style.stroke,
            StrokeKind::Inside,
        );
    }

    // Size of the pin.
    // Side of the square or diameter of the circle.
    let pin_size = style.pin_size(ui.style()).max(0.0);

    let pin_placement = style.pin_placement();

    let header_drag_space = style.header_drag_space(ui.style()).max(Vec2::ZERO);

    // Interact with node frame.
    let r = ui.interact(
        node_frame_rect,
        node_id.with("frame"),
        Sense::click_and_drag(),
    );

    if !modifiers.contains(config.select_node.modifiers)
        && !modifiers.contains(config.deselect_node.modifiers)
        && r.dragged_by(config.click_node.mouse_button)
    {
        node_moved = Some((node, r.drag_delta()));
    }

    if r.drag_stopped_by(config.click_node.mouse_button) {
        drag_released = true;
        // Ensure node_moved is set on the release frame so group membership detection works
        if node_moved.is_none() {
            node_moved = Some((node, Vec2::ZERO));
        }
    }

    if r.clicked_by(config.click_node.mouse_button) || r.dragged_by(config.drag_node.mouse_button) {
        if modifiers.contains(config.select_node.modifiers) {
            // In single_select mode, always clear previous selection
            let reset = config.single_select || modifiers.command;
            snarl_state.select_one_node(reset, node);
        } else if modifiers.contains(config.deselect_node.modifiers) {
            // Cmd+click: deselect
            snarl_state.deselect_one_node(node);
        } else {
            // Plain click: select exclusively (deselect all others first)
            snarl_state.select_one_node(true, node);
        }
    }

    if r.clicked() || r.dragged() {
        node_to_top = Some(node);
    }

    if viewer.has_node_menu(&snarl.nodes[node.0].value) {
        r.context_menu(|ui| {
            viewer.show_node_menu(node, &inputs, &outputs, ui, snarl);
        });
    }

    if !snarl.nodes.contains(node.0) {
        node_state.clear(ui.ctx());
        // If removed
        return None;
    }

    if viewer.has_on_hover_popup(&snarl.nodes[node.0].value) {
        r.on_hover_ui_at_pointer(|ui| {
            viewer.show_on_hover_popup(node, &inputs, &outputs, ui, snarl);
        });
    }

    if !snarl.nodes.contains(node.0) {
        node_state.clear(ui.ctx());
        // If removed
        return None;
    }

    let node_ui = &mut ui.new_child(
        UiBuilder::new()
            .max_rect(node_frame_rect.round_ui())
            .layout(Layout::top_down(Align::Center))
            .id_salt(node_id),
    );

    let mut new_pins_size = Vec2::ZERO;

    let r = node_frame.show(node_ui, |ui| {
        if viewer.has_node_style(node, &inputs, &outputs, snarl) {
            viewer.apply_node_style(ui.style_mut(), node, &inputs, &outputs, snarl);
        }

        // Input pins' center side by X axis.
        let input_x = match pin_placement {
            PinPlacement::Inside => pin_size.mul_add(
                0.5,
                node_frame_rect.left() + node_frame.inner_margin.leftf(),
            ),
            PinPlacement::Edge => node_frame_rect.left(),
            PinPlacement::Outside { margin } => {
                pin_size.mul_add(-0.5, node_frame_rect.left() - margin)
            }
        };

        // Input pins' spacing required.
        let input_spacing = match pin_placement {
            PinPlacement::Inside => Some(pin_size),
            PinPlacement::Edge => Some(
                pin_size
                    .mul_add(0.5, -node_frame.inner_margin.leftf())
                    .max(0.0),
            ),
            PinPlacement::Outside { .. } => None,
        };

        // Output pins' center side by X axis.
        let output_x = match pin_placement {
            PinPlacement::Inside => pin_size.mul_add(
                -0.5,
                node_frame_rect.right() - node_frame.inner_margin.rightf(),
            ),
            PinPlacement::Edge => node_frame_rect.right(),
            PinPlacement::Outside { margin } => {
                pin_size.mul_add(0.5, node_frame_rect.right() + margin)
            }
        };

        // Output pins' spacing required.
        let output_spacing = match pin_placement {
            PinPlacement::Inside => Some(pin_size),
            PinPlacement::Edge => Some(
                pin_size
                    .mul_add(0.5, -node_frame.inner_margin.rightf())
                    .max(0.0),
            ),
            PinPlacement::Outside { .. } => None,
        };

        // Input/output pin block

        if (openness < 1.0 && open) || (openness > 0.0 && !open) {
            ui.ctx().request_repaint();
        }

        // Pins are placed under the header and must not go outside of the header frame.
        let payload_rect = Rect::from_min_max(
            pos2(
                node_rect.min.x,
                node_rect.min.y
                    + node_state.header_height()
                    + header_frame.total_margin().bottom
                    + ui.spacing().item_spacing.y
                    - node_state.payload_offset(openness),
            ),
            node_rect.max,
        );

        let node_layout = viewer.node_layout(style.node_layout(), node, &inputs, &outputs, snarl);

        let payload_clip_rect =
            Rect::from_min_max(node_rect.min, pos2(node_rect.max.x, f32::INFINITY));

        let pins_rect = match node_layout.kind {
            NodeLayoutKind::Coil => {
                // Show input pins.
                let r = draw_inputs(
                    snarl,
                    viewer,
                    node,
                    &inputs,
                    pin_size,
                    style,
                    ui,
                    payload_rect,
                    payload_clip_rect,
                    input_x,
                    node_rect.min.y,
                    node_rect.min.y + node_state.header_height(),
                    input_spacing,
                    snarl_state,
                    config,
                    modifiers,
                    input_positions,
                    node_layout.input_heights(&node_state),
                );

                let new_input_heights = r.new_heights;

                drag_released |= r.drag_released;

                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let inputs_rect = r.final_rect;
                let inputs_size = inputs_rect.size();

                if !snarl.nodes.contains(node.0) {
                    // If removed
                    return;
                }

                // Show output pins.

                let r = draw_outputs(
                    snarl,
                    viewer,
                    node,
                    &outputs,
                    pin_size,
                    style,
                    ui,
                    payload_rect,
                    payload_clip_rect,
                    output_x,
                    node_rect.min.y,
                    node_rect.min.y + node_state.header_height(),
                    output_spacing,
                    snarl_state,
                    config,
                    modifiers,
                    output_positions,
                    node_layout.output_heights(&node_state),
                );

                let new_output_heights = r.new_heights;

                drag_released |= r.drag_released;

                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let outputs_rect = r.final_rect;
                let outputs_size = outputs_rect.size();

                if !snarl.nodes.contains(node.0) {
                    // If removed
                    return;
                }

                node_state.set_input_heights(new_input_heights);
                node_state.set_output_heights(new_output_heights);

                new_pins_size = vec2(
                    inputs_size.x + outputs_size.x + ui.spacing().item_spacing.x,
                    f32::max(inputs_size.y, outputs_size.y),
                );

                let mut pins_rect = inputs_rect.union(outputs_rect);

                // Show body if there's one.
                if viewer.has_body(&snarl.nodes.get(node.0).unwrap().value) {
                    let body_rect = Rect::from_min_max(
                        pos2(
                            inputs_rect.right() + ui.spacing().item_spacing.x,
                            payload_rect.top(),
                        ),
                        pos2(
                            outputs_rect.left() - ui.spacing().item_spacing.x,
                            payload_rect.bottom(),
                        ),
                    );

                    let r = draw_body(
                        snarl,
                        viewer,
                        node,
                        &inputs,
                        &outputs,
                        ui,
                        body_rect,
                        payload_clip_rect,
                        snarl_state,
                    );

                    new_pins_size.x += r.final_rect.width() + ui.spacing().item_spacing.x;
                    new_pins_size.y = f32::max(new_pins_size.y, r.final_rect.height());

                    pins_rect = pins_rect.union(body_rect);

                    if !snarl.nodes.contains(node.0) {
                        // If removed
                        return;
                    }
                }

                pins_rect
            }
            NodeLayoutKind::Sandwich => {
                // Show input pins.

                let r = draw_inputs(
                    snarl,
                    viewer,
                    node,
                    &inputs,
                    pin_size,
                    style,
                    ui,
                    payload_rect,
                    payload_clip_rect,
                    input_x,
                    node_rect.min.y,
                    node_rect.min.y + node_state.header_height(),
                    input_spacing,
                    snarl_state,
                    config,
                    modifiers,
                    input_positions,
                    node_layout.input_heights(&node_state),
                );

                let new_input_heights = r.new_heights;

                drag_released |= r.drag_released;

                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let inputs_rect = r.final_rect;

                new_pins_size = inputs_rect.size();

                let mut next_y = inputs_rect.bottom() + ui.spacing().item_spacing.y;

                if !snarl.nodes.contains(node.0) {
                    // If removed
                    return;
                }

                let mut pins_rect = inputs_rect;

                // Show body if there's one.
                if viewer.has_body(&snarl.nodes.get(node.0).unwrap().value) {
                    let body_rect = payload_rect.intersect(Rect::everything_below(next_y));

                    let r = draw_body(
                        snarl,
                        viewer,
                        node,
                        &inputs,
                        &outputs,
                        ui,
                        body_rect,
                        payload_clip_rect,
                        snarl_state,
                    );

                    let body_rect = r.final_rect;

                    new_pins_size.x = f32::max(new_pins_size.x, body_rect.width());
                    new_pins_size.y += body_rect.height() + ui.spacing().item_spacing.y;

                    if !snarl.nodes.contains(node.0) {
                        // If removed
                        return;
                    }

                    pins_rect = pins_rect.union(body_rect);
                    next_y = body_rect.bottom() + ui.spacing().item_spacing.y;
                }

                // Show output pins.

                let outputs_rect = payload_rect.intersect(Rect::everything_below(next_y));

                let r = draw_outputs(
                    snarl,
                    viewer,
                    node,
                    &outputs,
                    pin_size,
                    style,
                    ui,
                    outputs_rect,
                    payload_clip_rect,
                    output_x,
                    node_rect.min.y,
                    node_rect.min.y + node_state.header_height(),
                    output_spacing,
                    snarl_state,
                    config,
                    modifiers,
                    output_positions,
                    node_layout.output_heights(&node_state),
                );

                let new_output_heights = r.new_heights;

                drag_released |= r.drag_released;

                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let outputs_rect = r.final_rect;

                if !snarl.nodes.contains(node.0) {
                    // If removed
                    return;
                }

                node_state.set_input_heights(new_input_heights);
                node_state.set_output_heights(new_output_heights);

                new_pins_size.x = f32::max(new_pins_size.x, outputs_rect.width());
                new_pins_size.y += outputs_rect.height() + ui.spacing().item_spacing.y;

                pins_rect = pins_rect.union(outputs_rect);

                pins_rect
            }
            NodeLayoutKind::FlippedSandwich => {
                // Show input pins.

                let outputs_rect = payload_rect;
                let r = draw_outputs(
                    snarl,
                    viewer,
                    node,
                    &outputs,
                    pin_size,
                    style,
                    ui,
                    outputs_rect,
                    payload_clip_rect,
                    output_x,
                    node_rect.min.y,
                    node_rect.min.y + node_state.header_height(),
                    output_spacing,
                    snarl_state,
                    config,
                    modifiers,
                    output_positions,
                    node_layout.output_heights(&node_state),
                );

                let new_output_heights = r.new_heights;

                drag_released |= r.drag_released;

                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let outputs_rect = r.final_rect;

                new_pins_size = outputs_rect.size();

                let mut next_y = outputs_rect.bottom() + ui.spacing().item_spacing.y;

                if !snarl.nodes.contains(node.0) {
                    // If removed
                    return;
                }

                let mut pins_rect = outputs_rect;

                // Show body if there's one.
                if viewer.has_body(&snarl.nodes.get(node.0).unwrap().value) {
                    let body_rect = payload_rect.intersect(Rect::everything_below(next_y));

                    let r = draw_body(
                        snarl,
                        viewer,
                        node,
                        &inputs,
                        &outputs,
                        ui,
                        body_rect,
                        payload_clip_rect,
                        snarl_state,
                    );

                    let body_rect = r.final_rect;

                    new_pins_size.x = f32::max(new_pins_size.x, body_rect.width());
                    new_pins_size.y += body_rect.height() + ui.spacing().item_spacing.y;

                    if !snarl.nodes.contains(node.0) {
                        // If removed
                        return;
                    }

                    pins_rect = pins_rect.union(body_rect);
                    next_y = body_rect.bottom() + ui.spacing().item_spacing.y;
                }

                // Show output pins.

                let inputs_rect = payload_rect.intersect(Rect::everything_below(next_y));

                let r = draw_inputs(
                    snarl,
                    viewer,
                    node,
                    &inputs,
                    pin_size,
                    style,
                    ui,
                    inputs_rect,
                    payload_clip_rect,
                    input_x,
                    node_rect.min.y,
                    node_rect.min.y + node_state.header_height(),
                    input_spacing,
                    snarl_state,
                    config,
                    modifiers,
                    input_positions,
                    node_layout.input_heights(&node_state),
                );

                let new_input_heights = r.new_heights;

                drag_released |= r.drag_released;

                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let inputs_rect = r.final_rect;

                if !snarl.nodes.contains(node.0) {
                    // If removed
                    return;
                }

                node_state.set_input_heights(new_input_heights);
                node_state.set_output_heights(new_output_heights);

                new_pins_size.x = f32::max(new_pins_size.x, inputs_rect.width());
                new_pins_size.y += inputs_rect.height() + ui.spacing().item_spacing.y;

                pins_rect = pins_rect.union(inputs_rect);

                pins_rect
            }

            NodeLayoutKind::VerticalSandwichDown => {
                // VerticalSandwichDown: inputs at top (horizontal), body, outputs at bottom (horizontal)
                // Pin Y positions at top and bottom edges
                let top_pin_y = match pin_placement {
                    PinPlacement::Inside => node_rect.min.y + pin_size / 2.0,
                    PinPlacement::Edge => node_rect.min.y,
                    PinPlacement::Outside { margin } => node_rect.min.y - margin - pin_size / 2.0,
                };
                let bottom_pin_y = match pin_placement {
                    PinPlacement::Inside => node_rect.max.y - pin_size / 2.0,
                    PinPlacement::Edge => node_rect.max.y,
                    PinPlacement::Outside { margin } => node_rect.max.y + margin + pin_size / 2.0,
                };

                // Draw inputs at top (horizontal row)
                let r = draw_inputs_horizontal(
                    snarl,
                    viewer,
                    node,
                    &inputs,
                    pin_size,
                    style,
                    ui,
                    payload_rect,
                    payload_clip_rect,
                    top_pin_y,
                    node_rect.min.x,
                    node_rect.max.x,
                    snarl_state,
                    config,
                    modifiers,
                    input_positions,
                    node_layout.input_widths(&node_state),
                    true, // pins at top
                );

                let new_input_widths = r.new_widths;
                drag_released |= r.drag_released;
                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let inputs_rect = r.final_rect;
                new_pins_size = inputs_rect.size();
                let mut next_y = inputs_rect.bottom() + ui.spacing().item_spacing.y;

                if !snarl.nodes.contains(node.0) {
                    return;
                }

                let mut pins_rect = inputs_rect;

                // Show body
                if viewer.has_body(&snarl.nodes[node.0].value) {
                    let body_rect = payload_rect.intersect(Rect::everything_below(next_y));

                    let r = draw_body(
                        snarl,
                        viewer,
                        node,
                        &inputs,
                        &outputs,
                        ui,
                        body_rect,
                        payload_clip_rect,
                        snarl_state,
                    );

                    new_pins_size.x = f32::max(new_pins_size.x, r.final_rect.width());
                    new_pins_size.y += r.final_rect.height() + ui.spacing().item_spacing.y;

                    pins_rect = pins_rect.union(r.final_rect);
                    next_y = r.final_rect.bottom() + ui.spacing().item_spacing.y;
                }

                // Draw outputs at bottom (horizontal row)
                let outputs_area = payload_rect.intersect(Rect::everything_below(next_y));

                let r = draw_outputs_horizontal(
                    snarl,
                    viewer,
                    node,
                    &outputs,
                    pin_size,
                    style,
                    ui,
                    outputs_area,
                    payload_clip_rect,
                    bottom_pin_y,
                    node_rect.min.x,
                    node_rect.max.x,
                    snarl_state,
                    config,
                    modifiers,
                    output_positions,
                    node_layout.output_widths(&node_state),
                    false, // pins at bottom
                );

                let new_output_widths = r.new_widths;
                drag_released |= r.drag_released;
                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let outputs_rect = r.final_rect;

                if !snarl.nodes.contains(node.0) {
                    return;
                }

                node_state.set_input_widths(new_input_widths);
                node_state.set_output_widths(new_output_widths);

                new_pins_size.x = f32::max(new_pins_size.x, outputs_rect.width());
                new_pins_size.y += outputs_rect.height() + ui.spacing().item_spacing.y;

                pins_rect = pins_rect.union(outputs_rect);

                pins_rect
            }

            NodeLayoutKind::VerticalSandwichUp => {
                // VerticalSandwichUp: outputs at top (horizontal), body, inputs at bottom (horizontal)
                // Pin Y positions at top and bottom edges
                let top_pin_y = match pin_placement {
                    PinPlacement::Inside => node_rect.min.y + pin_size / 2.0,
                    PinPlacement::Edge => node_rect.min.y,
                    PinPlacement::Outside { margin } => node_rect.min.y - margin - pin_size / 2.0,
                };
                let bottom_pin_y = match pin_placement {
                    PinPlacement::Inside => node_rect.max.y - pin_size / 2.0,
                    PinPlacement::Edge => node_rect.max.y,
                    PinPlacement::Outside { margin } => node_rect.max.y + margin + pin_size / 2.0,
                };

                // Draw outputs at top (horizontal row)
                let r = draw_outputs_horizontal(
                    snarl,
                    viewer,
                    node,
                    &outputs,
                    pin_size,
                    style,
                    ui,
                    payload_rect,
                    payload_clip_rect,
                    top_pin_y,
                    node_rect.min.x,
                    node_rect.max.x,
                    snarl_state,
                    config,
                    modifiers,
                    output_positions,
                    node_layout.output_widths(&node_state),
                    true, // pins at top
                );

                let new_output_widths = r.new_widths;
                drag_released |= r.drag_released;
                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let outputs_rect = r.final_rect;
                new_pins_size = outputs_rect.size();
                let mut next_y = outputs_rect.bottom() + ui.spacing().item_spacing.y;

                if !snarl.nodes.contains(node.0) {
                    return;
                }

                let mut pins_rect = outputs_rect;

                // Show body
                if viewer.has_body(&snarl.nodes[node.0].value) {
                    let body_rect = payload_rect.intersect(Rect::everything_below(next_y));

                    let r = draw_body(
                        snarl,
                        viewer,
                        node,
                        &inputs,
                        &outputs,
                        ui,
                        body_rect,
                        payload_clip_rect,
                        snarl_state,
                    );

                    new_pins_size.x = f32::max(new_pins_size.x, r.final_rect.width());
                    new_pins_size.y += r.final_rect.height() + ui.spacing().item_spacing.y;

                    pins_rect = pins_rect.union(r.final_rect);
                    next_y = r.final_rect.bottom() + ui.spacing().item_spacing.y;
                }

                // Draw inputs at bottom (horizontal row)
                let inputs_area = payload_rect.intersect(Rect::everything_below(next_y));

                let r = draw_inputs_horizontal(
                    snarl,
                    viewer,
                    node,
                    &inputs,
                    pin_size,
                    style,
                    ui,
                    inputs_area,
                    payload_clip_rect,
                    bottom_pin_y,
                    node_rect.min.x,
                    node_rect.max.x,
                    snarl_state,
                    config,
                    modifiers,
                    input_positions,
                    node_layout.input_widths(&node_state),
                    false, // pins at bottom
                );

                let new_input_widths = r.new_widths;
                drag_released |= r.drag_released;
                if r.pin_hovered.is_some() {
                    pin_hovered = r.pin_hovered;
                }

                let inputs_rect = r.final_rect;

                if !snarl.nodes.contains(node.0) {
                    return;
                }

                node_state.set_input_widths(new_input_widths);
                node_state.set_output_widths(new_output_widths);

                new_pins_size.x = f32::max(new_pins_size.x, inputs_rect.width());
                new_pins_size.y += inputs_rect.height() + ui.spacing().item_spacing.y;

                pins_rect = pins_rect.union(inputs_rect);

                pins_rect
            }
        };

        if viewer.has_footer(&snarl.nodes[node.0].value) {
            let footer_rect = Rect::from_min_max(
                pos2(
                    node_rect.left(),
                    pins_rect.bottom() + ui.spacing().item_spacing.y,
                ),
                pos2(node_rect.right(), node_rect.bottom()),
            );

            let mut footer_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(footer_rect.round_ui())
                    .layout(Layout::left_to_right(Align::Min))
                    .id_salt("footer"),
            );
            footer_ui.shrink_clip_rect(payload_clip_rect);

            viewer.show_footer(node, &inputs, &outputs, &mut footer_ui, snarl);

            let final_rect = footer_ui.min_rect();
            ui.expand_to_include_rect(final_rect.intersect(payload_clip_rect));
            let footer_size = final_rect.size();

            new_pins_size.x = f32::max(new_pins_size.x, footer_size.x);
            new_pins_size.y += footer_size.y + ui.spacing().item_spacing.y;

            if !snarl.nodes.contains(node.0) {
                // If removed
                return;
            }
        }

        // Render header frame.
        let mut header_rect = Rect::NAN;

        let mut header_frame_rect = Rect::NAN; //node_rect + header_frame.total_margin();

        // Show node's header
        let header_ui: &mut Ui = &mut ui.new_child(
            UiBuilder::new()
                .max_rect(node_rect.round_ui() + header_frame.total_margin())
                .layout(Layout::top_down(Align::Center))
                .id_salt("header"),
        );

        header_frame.show(header_ui, |ui: &mut Ui| {
            ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                if style.collapsible() {
                    let (_, r) = ui.allocate_exact_size(
                        vec2(ui.spacing().icon_width, ui.spacing().icon_width),
                        Sense::click(),
                    );
                    paint_default_icon(ui, openness, &r);

                    if r.clicked_by(config.click_header.mouse_button) {
                        // Toggle node's openness.
                        snarl.open_node(node, !open);
                    }
                }

                ui.allocate_exact_size(header_drag_space, Sense::hover());

                viewer.show_header(node, &inputs, &outputs, ui, snarl);

                header_rect = ui.min_rect();
            });

            header_frame_rect = header_rect + header_frame.total_margin();

            ui.advance_cursor_after_rect(Rect::from_min_max(
                header_rect.min,
                pos2(
                    f32::max(header_rect.max.x, node_rect.max.x),
                    header_rect.min.y,
                ),
            ));
        });

        ui.expand_to_include_rect(header_rect);
        let header_size = header_rect.size();
        node_state.set_header_height(header_size.y);
        node_state.set_header_width(header_size.x);

        node_state.set_size(vec2(
            f32::max(header_size.x, new_pins_size.x),
            header_size.y
                + header_frame.total_margin().bottom
                + ui.spacing().item_spacing.y
                + new_pins_size.y,
        ));
    });

    if !snarl.nodes.contains(node.0) {
        ui.ctx().request_repaint();
        node_state.clear(ui.ctx());
        // If removed
        return None;
    }

    viewer.final_node_rect(node, r.response.rect, ui, snarl);

    node_state.store(ui.ctx());
    Some(DrawNodeResponse {
        node_moved,
        node_to_top,
        drag_released,
        pin_hovered,
        final_rect: r.response.rect,
        in_pins: inputs,
        out_pins: outputs,
    })
}

const fn mix_colors(a: Color32, b: Color32) -> Color32 {
    #![allow(clippy::cast_possible_truncation)]

    Color32::from_rgba_premultiplied(
        u8::midpoint(a.r(), b.r()),
        u8::midpoint(a.g(), b.g()),
        u8::midpoint(a.b(), b.b()),
        u8::midpoint(a.a(), b.a()),
    )
}

// fn mix_colors(mut colors: impl Iterator<Item = Color32>) -> Option<Color32> {
//     let color = colors.next()?;

//     let mut r = color.r() as u32;
//     let mut g = color.g() as u32;
//     let mut b = color.b() as u32;
//     let mut a = color.a() as u32;
//     let mut w = 1;

//     for c in colors {
//         r += c.r() as u32;
//         g += c.g() as u32;
//         b += c.b() as u32;
//         a += c.a() as u32;
//         w += 1;
//     }

//     Some(Color32::from_rgba_premultiplied(
//         (r / w) as u8,
//         (g / w) as u8,
//         (b / w) as u8,
//         (a / w) as u8,
//     ))
// }

// fn mix_sizes(mut sizes: impl Iterator<Item = f32>) -> Option<f32> {
//     let mut size = sizes.next()?;
//     let mut w = 1;

//     for s in sizes {
//         size += s;
//         w += 1;
//     }

//     Some(size / w as f32)
// }

// fn mix_strokes(mut strokes: impl Iterator<Item = Stroke>) -> Option<Stroke> {
//     let stoke = strokes.next()?;

//     let mut width = stoke.width;
//     let mut r = stoke.color.r() as u32;
//     let mut g = stoke.color.g() as u32;
//     let mut b = stoke.color.b() as u32;
//     let mut a = stoke.color.a() as u32;

//     let mut w = 1;

//     for s in strokes {
//         width += s.width;
//         r += s.color.r() as u32;
//         g += s.color.g() as u32;
//         b += s.color.b() as u32;
//         a += s.color.a() as u32;
//         w += 1;
//     }

//     Some(Stroke {
//         width: width / w as f32,
//         color: Color32::from_rgba_premultiplied(
//             (r / w) as u8,
//             (g / w) as u8,
//             (b / w) as u8,
//             (a / w) as u8,
//         ),
//     })
// }

impl<T, G> Snarl<T, G> {
    /// Render [`Snarl`] using given viewer and style into the [`Ui`].
    #[inline]
    pub fn show<V>(
        &mut self,
        viewer: &mut V,
        style: &SnarlStyle,
        config: &SnarlConfig,
        id_salt: impl Hash,
        ui: &mut Ui,
    ) where
        V: SnarlViewer<T, G>,
    {
        show_snarl(
            ui.make_persistent_id(id_salt),
            *style,
            config,
            Vec2::ZERO,
            Vec2::INFINITY,
            self,
            viewer,
            ui,
        );
    }
}

/// Collects all descendant group IDs of a group (recursive).
fn collect_descendant_groups<T, G>(snarl: &Snarl<T, G>, group: crate::GroupId, result: &mut Vec<crate::GroupId>) {
    for (idx, g) in &snarl.groups {
        if g.parent == Some(group) {
            let child_id = crate::GroupId(idx);
            result.push(child_id);
            collect_descendant_groups(snarl, child_id, result);
        }
    }
}

/// Find the nearest point on the edge of `rect` to the external point `target`.
/// Used to route wires to the edge of collapsed groups.
fn nearest_rect_edge(rect: Rect, target: Pos2) -> Pos2 {
    let center = rect.center();
    let dx = target.x - center.x;
    let dy = target.y - center.y;

    if dx == 0.0 && dy == 0.0 {
        // Target is at the center — pick the right edge arbitrarily
        return pos2(rect.max.x, center.y);
    }

    let half_w = rect.width() * 0.5;
    let half_h = rect.height() * 0.5;

    // Scale factor to reach rect edge along the direction from center to target
    let scale_x = if dx == 0.0 { f32::INFINITY } else { half_w / dx.abs() };
    let scale_y = if dy == 0.0 { f32::INFINITY } else { half_h / dy.abs() };
    let scale = scale_x.min(scale_y);

    pos2(center.x + dx * scale, center.y + dy * scale)
}

#[inline]
fn clamp_scale(to_global: &mut TSTransform, min_scale: f32, max_scale: f32, ui_rect: Rect) {
    if to_global.scaling >= min_scale && to_global.scaling <= max_scale {
        return;
    }

    let new_scaling = to_global.scaling.clamp(min_scale, max_scale);
    *to_global = scale_transform_around(to_global, new_scaling, ui_rect.center());
}

#[inline]
#[must_use]
fn transform_matching_points(from: Pos2, to: Pos2, scaling: f32) -> TSTransform {
    TSTransform {
        scaling,
        translation: to.to_vec2() - from.to_vec2() * scaling,
    }
}

#[inline]
#[must_use]
fn scale_transform_around(transform: &TSTransform, scaling: f32, point: Pos2) -> TSTransform {
    let from = (point - transform.translation) / transform.scaling;
    transform_matching_points(from, point, scaling)
}

#[test]
const fn snarl_style_is_send_sync() {
    const fn is_send_sync<T: Send + Sync>() {}
    is_send_sync::<SnarlStyle>();
}
