use egui::{Color32, Modifiers, Painter, PointerButton, Pos2, Rect, Stroke};

/// Struct holding keyboard modifiers and mouse button.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModifierClick {
    /// Keyboard modifiers for this action.
    pub modifiers: Modifiers,

    /// Mouse buttons for this action.
    pub mouse_button: PointerButton,
}

/// Type of snap grid for node positioning.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub enum SnapGridType {
    /// Square/rectangular grid.
    #[default]
    Quad,
    /// Hexagonal grid with pointy tops (vertical orientation).
    /// Rows are offset horizontally.
    HexPointy,
    /// Hexagonal grid with flat tops (horizontal orientation).
    /// Columns are offset vertically.
    HexFlat,
}

/// Configuration for snap grid.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SnapGrid {
    /// The size of each grid cell.
    pub size: f32,
    /// The type of grid (quad or hex).
    pub grid_type: SnapGridType,
    /// Whether to show the grid visually.
    pub visible: bool,
    /// Color for grid points/lines when visible.
    /// If None, uses a default semi-transparent color.
    pub color: Option<Color32>,
    /// Size of the snap point indicators when visible.
    /// Defaults to 3.0.
    pub point_size: f32,
}

impl Default for SnapGrid {
    fn default() -> Self {
        Self {
            size: 25.0,
            grid_type: SnapGridType::Quad,
            visible: false,
            color: None,
            point_size: 3.0,
        }
    }
}

impl SnapGrid {
    /// Create a new quad snap grid with the given size.
    #[must_use]
    pub const fn quad(size: f32) -> Self {
        Self {
            size,
            grid_type: SnapGridType::Quad,
            visible: false,
            color: None,
            point_size: 3.0,
        }
    }

    /// Create a new pointy-top hex snap grid with the given size.
    #[must_use]
    pub const fn hex_pointy(size: f32) -> Self {
        Self {
            size,
            grid_type: SnapGridType::HexPointy,
            visible: false,
            color: None,
            point_size: 3.0,
        }
    }

    /// Create a new flat-top hex snap grid with the given size.
    #[must_use]
    pub const fn hex_flat(size: f32) -> Self {
        Self {
            size,
            grid_type: SnapGridType::HexFlat,
            visible: false,
            color: None,
            point_size: 3.0,
        }
    }

    /// Set the grid to be visible.
    #[must_use]
    pub const fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Set the grid color.
    #[must_use]
    pub const fn with_color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Set the snap point indicator size.
    #[must_use]
    pub const fn with_point_size(mut self, size: f32) -> Self {
        self.point_size = size;
        self
    }

    /// Snap a position to the nearest grid point.
    #[must_use]
    pub fn snap(&self, pos: Pos2) -> Pos2 {
        match self.grid_type {
            SnapGridType::Quad => self.snap_quad(pos),
            SnapGridType::HexPointy => self.snap_hex_pointy(pos),
            SnapGridType::HexFlat => self.snap_hex_flat(pos),
        }
    }

    fn snap_quad(&self, pos: Pos2) -> Pos2 {
        Pos2::new(
            (pos.x / self.size).round() * self.size,
            (pos.y / self.size).round() * self.size,
        )
    }

    fn snap_hex_pointy(&self, pos: Pos2) -> Pos2 {
        // Pointy-top hex: horizontal spacing is size, vertical spacing is size * sqrt(3)/2
        // Odd rows are offset by size/2
        let vert_spacing = self.size * 0.866_025_4; // sqrt(3)/2
        let horiz_spacing = self.size;

        // Find the row
        let row = (pos.y / vert_spacing).round();
        let snapped_y = row * vert_spacing;

        // Determine if this is an odd row (offset)
        #[allow(clippy::cast_possible_truncation)]
        let is_odd_row = (row as i32).abs() % 2 == 1;
        let x_offset = if is_odd_row { horiz_spacing / 2.0 } else { 0.0 };

        let snapped_x = ((pos.x - x_offset) / horiz_spacing).round() * horiz_spacing + x_offset;

        Pos2::new(snapped_x, snapped_y)
    }

    fn snap_hex_flat(&self, pos: Pos2) -> Pos2 {
        // Flat-top hex: vertical spacing is size, horizontal spacing is size * sqrt(3)/2
        // Odd columns are offset by size/2
        let horiz_spacing = self.size * 0.866_025_4; // sqrt(3)/2
        let vert_spacing = self.size;

        // Find the column
        let col = (pos.x / horiz_spacing).round();
        let snapped_x = col * horiz_spacing;

        // Determine if this is an odd column (offset)
        #[allow(clippy::cast_possible_truncation)]
        let is_odd_col = (col as i32).abs() % 2 == 1;
        let y_offset = if is_odd_col { vert_spacing / 2.0 } else { 0.0 };

        let snapped_y = ((pos.y - y_offset) / vert_spacing).round() * vert_spacing + y_offset;

        Pos2::new(snapped_x, snapped_y)
    }

    /// Get the effective stroke for drawing the grid.
    #[must_use]
    pub fn stroke(&self) -> Stroke {
        let color = self
            .color
            .unwrap_or(Color32::from_rgba_unmultiplied(128, 128, 128, 80));
        Stroke::new(1.0, color)
    }

    /// Get the effective color for drawing points.
    #[must_use]
    pub fn point_color(&self) -> Color32 {
        self.color
            .unwrap_or(Color32::from_rgba_unmultiplied(128, 128, 128, 120))
    }

    /// Draw the snap grid within the given viewport.
    pub fn draw(&self, viewport: &Rect, painter: &Painter) {
        if !self.visible {
            return;
        }

        let color = self.point_color();
        let point_size = self.point_size;

        match self.grid_type {
            SnapGridType::Quad => self.draw_quad(viewport, painter, color, point_size),
            SnapGridType::HexPointy => self.draw_hex_pointy(viewport, painter, color, point_size),
            SnapGridType::HexFlat => self.draw_hex_flat(viewport, painter, color, point_size),
        }
    }

    fn draw_quad(&self, viewport: &Rect, painter: &Painter, color: Color32, point_size: f32) {
        #![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let min_x = (viewport.min.x / self.size).floor() as i32;
        let max_x = (viewport.max.x / self.size).ceil() as i32;
        let min_y = (viewport.min.y / self.size).floor() as i32;
        let max_y = (viewport.max.y / self.size).ceil() as i32;

        for xi in min_x..=max_x {
            for yi in min_y..=max_y {
                let x = xi as f32 * self.size;
                let y = yi as f32 * self.size;
                painter.circle_filled(Pos2::new(x, y), point_size, color);
            }
        }
    }

    fn draw_hex_pointy(&self, viewport: &Rect, painter: &Painter, color: Color32, point_size: f32) {
        #![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let vert_spacing = self.size * 0.866_025_4; // sqrt(3)/2
        let horiz_spacing = self.size;

        let min_row = (viewport.min.y / vert_spacing).floor() as i32 - 1;
        let max_row = (viewport.max.y / vert_spacing).ceil() as i32 + 1;
        let min_col = (viewport.min.x / horiz_spacing).floor() as i32 - 1;
        let max_col = (viewport.max.x / horiz_spacing).ceil() as i32 + 1;

        for row in min_row..=max_row {
            let y = row as f32 * vert_spacing;
            let x_offset = if row.abs() % 2 == 1 {
                horiz_spacing / 2.0
            } else {
                0.0
            };

            for col in min_col..=max_col {
                let x = col as f32 * horiz_spacing + x_offset;
                let pos = Pos2::new(x, y);
                if viewport.contains(pos) {
                    painter.circle_filled(pos, point_size, color);
                }
            }
        }
    }

    fn draw_hex_flat(&self, viewport: &Rect, painter: &Painter, color: Color32, point_size: f32) {
        #![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let horiz_spacing = self.size * 0.866_025_4; // sqrt(3)/2
        let vert_spacing = self.size;

        let min_col = (viewport.min.x / horiz_spacing).floor() as i32 - 1;
        let max_col = (viewport.max.x / horiz_spacing).ceil() as i32 + 1;
        let min_row = (viewport.min.y / vert_spacing).floor() as i32 - 1;
        let max_row = (viewport.max.y / vert_spacing).ceil() as i32 + 1;

        for col in min_col..=max_col {
            let x = col as f32 * horiz_spacing;
            let y_offset = if col.abs() % 2 == 1 {
                vert_spacing / 2.0
            } else {
                0.0
            };

            for row in min_row..=max_row {
                let y = row as f32 * vert_spacing + y_offset;
                let pos = Pos2::new(x, y);
                if viewport.contains(pos) {
                    painter.circle_filled(pos, point_size, color);
                }
            }
        }
    }
}

/// Configuration for the Snarl graph editor's interactions and behaviour.
///
/// # User Interface Reference
///
/// ## Viewport
///
/// | Action          | Default | Notes |
/// |-----------------|---------|-------|
/// | Pan             | Middle-click drag or primary-click drag on empty space | |
/// | Zoom            | Scroll wheel | |
/// | Fit all in view | Double-click on empty space | |
/// | Deselect all    | Escape | Not configurable |
///
/// ## Nodes
///
/// | Action                 | Default | Configured by |
/// |------------------------|---------|---------------|
/// | Select                 | Left-click | [`click_node`](Self::click_node) |
/// | Drag                   | Left-drag | [`drag_node`](Self::drag_node) |
/// | Add to selection       | Shift + Left-click | [`select_node`](Self::select_node) |
/// | Remove from selection  | Cmd/Ctrl + Left-click | [`deselect_node`](Self::deselect_node) |
/// | Rectangle select       | Shift + Left-drag on empty space | [`rect_select`](Self::rect_select) |
/// | Deselect all           | Cmd/Ctrl + Left-click on empty space | [`deselect_all_nodes`](Self::deselect_all_nodes) |
/// | Open node context menu | Right-click on node | Not configurable |
/// | Collapse/expand node   | Left-click on header | [`click_header`](Self::click_header) |
///
/// ## Wires
///
/// | Action                                 | Default | Configured by |
/// |----------------------------------------|---------|---------------|
/// | Start wire drag from pin               | Left-drag on pin | Default `drag_pin` (no modifier) |
/// | Start multi-wire drag                  | Cmd/Ctrl + Left-drag on pin | [`drag_pin`](Self::drag_pin) |
/// | Add pin to in-progress multi-drag      | Shift (hover over pin while dragging) | Not configurable |
/// | Remove pin from in-progress multi-drag | Cmd/Ctrl (hover over pin while dragging) | Not configurable |
/// | Drop wire without opening menu         | Shift + release on empty space | [`no_menu`](Self::no_menu) |
/// | Cancel wire drag                       | Right-click while dragging | [`cancel_wire_drag`](Self::cancel_wire_drag) |
/// | Remove hovered wire                    | Right-click on wire | [`remove_hovered_wire`](Self::remove_hovered_wire) |
/// | Click pin (e.g. disconnect all)        | Right-click on pin | [`click_pin`](Self::click_pin) |
/// | Open dropped-wire menu                 | Release wire on empty space | Via [`SnarlViewer::has_dropped_wire_menu()`](super::SnarlViewer::has_dropped_wire_menu) |
///
/// ## Grid Snapping
///
/// Node positions can be snapped to a grid via [`grid_snap`](Self::grid_snap).
/// Three grid types are supported: [`SnapGridType::Quad`] (rectangular),
/// [`SnapGridType::HexPointy`] (pointy-top hexagonal), and [`SnapGridType::HexFlat`]
/// (flat-top hexagonal).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SnarlConfig {
    /// Action used to draw a rectangle selection over multiple nodes.
    ///
    /// Defaults to [`Modifiers::SHIFT`] + [`PointerButton::Primary`].
    pub rect_select: ModifierClick,

    /// Action used to remove the hovered wire.
    ///
    /// Defaults to no modifiers + [`PointerButton::Secondary`].
    pub remove_hovered_wire: ModifierClick,

    /// Action used to deselect all nodes when clicking on empty space.
    ///
    /// Defaults to [`Modifiers::COMMAND`] + [`PointerButton::Primary`].
    pub deselect_all_nodes: ModifierClick,

    /// Action used to cancel an in-progress wire drag.
    ///
    /// Defaults to no modifiers + [`PointerButton::Secondary`].
    pub cancel_wire_drag: ModifierClick,

    /// Action used to click on a pin (e.g. to trigger disconnect-all).
    ///
    /// Defaults to no modifiers + [`PointerButton::Secondary`].
    pub click_pin: ModifierClick,

    /// Action used to start dragging multiple wires from a pin simultaneously.
    ///
    /// Defaults to [`Modifiers::COMMAND`] + [`PointerButton::Primary`].
    /// 
    /// (Without this modifier, a plain left-drag starts a single-wire drag.)
    pub drag_pin: ModifierClick,

    /// Action used to drop a wire on empty space without opening the context menu.
    ///
    /// Defaults to [`Modifiers::SHIFT`] + [`PointerButton::Primary`].
    pub no_menu: ModifierClick,

    /// Action used to click a node (e.g. to bring it to the front).
    ///
    /// Defaults to no modifiers + [`PointerButton::Primary`].
    pub click_node: ModifierClick,

    /// Action used to drag a node to a new position.
    ///
    /// Defaults to no modifiers + [`PointerButton::Primary`].
    pub drag_node: ModifierClick,

    /// Action used to add a node to the current selection.
    ///
    /// Defaults to [`Modifiers::SHIFT`] + [`PointerButton::Primary`].
    pub select_node: ModifierClick,

    /// Action used to remove a node from the current selection.
    ///
    /// Defaults to [`Modifiers::COMMAND`] + [`PointerButton::Primary`].
    pub deselect_node: ModifierClick,

    /// Action used to click the node header (e.g. to collapse/expand the node).
    ///
    /// Defaults to no modifiers + [`PointerButton::Primary`].
    pub click_header: ModifierClick,

    /// When `true`, only a single node can be selected at a time,
    /// and clicking a node will deselect any previously selected nodes.
    /// 
    /// Defaults to `false`.
    pub single_select: bool,

    /// Grid snapping configuration for node positions.
    ///
    /// When `Some(grid)`, nodes snap to the configured grid after being dragged.
    /// Supports rectangular ([`SnapGridType::Quad`]) and hexagonal grids
    /// ([`SnapGridType::HexPointy`], [`SnapGridType::HexFlat`]).
    /// 
    /// Set to `None` to disable snapping (the default).
    pub grid_snap: Option<SnapGrid>,

    #[doc(hidden)]
    #[cfg_attr(feature = "serde", serde(skip_serializing, default))]
    /// Do not access other than with .., here to emulate `#[non_exhaustive(pub)]`
    pub _non_exhaustive: (),
}

impl Default for SnarlConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl SnarlConfig {
    /// Creates new [`SnarlConfig`] filled with default values.
    #[must_use]
    pub const fn new() -> Self {
        SnarlConfig {
            rect_select: ModifierClick {
                modifiers: Modifiers::SHIFT,
                mouse_button: PointerButton::Primary,
            },
            remove_hovered_wire: ModifierClick {
                modifiers: Modifiers::NONE,
                mouse_button: PointerButton::Secondary,
            },
            deselect_all_nodes: ModifierClick {
                modifiers: Modifiers::COMMAND,
                mouse_button: PointerButton::Primary,
            },
            cancel_wire_drag: ModifierClick {
                modifiers: Modifiers::NONE,
                mouse_button: PointerButton::Secondary,
            },
            click_pin: ModifierClick {
                modifiers: Modifiers::NONE,
                mouse_button: PointerButton::Secondary,
            },
            drag_pin: ModifierClick {
                modifiers: Modifiers::COMMAND,
                mouse_button: PointerButton::Primary,
            },
            no_menu: ModifierClick {
                modifiers: Modifiers::SHIFT,
                mouse_button: PointerButton::Primary,
            },
            click_node: ModifierClick {
                modifiers: Modifiers::NONE,
                mouse_button: PointerButton::Primary,
            },
            drag_node: ModifierClick {
                modifiers: Modifiers::NONE,
                mouse_button: PointerButton::Primary,
            },
            select_node: ModifierClick {
                modifiers: Modifiers::SHIFT,
                mouse_button: PointerButton::Primary,
            },
            deselect_node: ModifierClick {
                modifiers: Modifiers::COMMAND,
                mouse_button: PointerButton::Primary,
            },
            click_header: ModifierClick {
                modifiers: Modifiers::NONE,
                mouse_button: PointerButton::Primary,
            },

            single_select: false,

            grid_snap: None,

            _non_exhaustive: (),
        }
    }
}
