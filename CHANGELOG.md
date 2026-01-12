# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.0] - 26.10.2025

### Changed

- Upgraded to egui 0.33
- Changed MSRV to 1.88

## [0.8.0] - 14.07.2025

### Added

- `SnarlViewer::current_transform` method to receive and possibly modify the current transform of the snarl's UI layer.
- `SnarlStyle::pin_hover_scale` option to control or disable pin hover scaling effect. Set to `1.0` to disable hover effect.
- `Snarl::disconnect_all(node_id)` method to remove all connections to and from a node.
- `Effects::disconnect_all(node_id)` for deferred disconnection of all node wires.
- `SnarlViewer::node_moved` callback invoked when a node is dragged to a new position.
- `SnarlConfig::single_select` option to limit selection to a single node at a time.
- `SnapGrid` and `SnapGridType` types for configurable snap grids with quad and hex (pointy/flat) variants.
- `SnarlConfig::grid_snap` now accepts `Option<SnapGrid>` for full grid configuration including type and visibility.
- `SnarlViewer::show_header_buttons` method for adding buttons/flags to node headers (Houdini-style).
- `SnarlViewer::draw_foreground` method for drawing comments, annotations, and overlays on top of the graph.
- `SnarlViewer::wire_waypoints` method for specifying custom wire routing waypoints.
- `SnarlViewer::compute_layout` and `apply_layout` methods for implementing automatic node layout algorithms.

### Changed

- `SnarlViewer::show_header` now uses horizontal layout with title on left and header buttons on right.
- Collapsed nodes now shrink their width to match the header width instead of retaining full content width.
- Snarl is now scaled using egui's layer scaling mechanism.
  Removes layout twitching when zooming in and out.
  Simplifies interface of `SnarlViewer`.
- Renamed `Snarl::get_node` to `node`, `get_node_mut` to `node_mut`, `get_node_info` to `node_info`, and `get_node_info_mut` to `node_info_mut` per Rust API guidelines.
  The old names are deprecated but still available for backwards compatibility.

## [0.7.1] - 19.02.2025

### Added

- `SnarlPin::pin_rect` method allows custom type to override default pin position and size.

## [0.7.0] - 19.02.2025

### Changed

- `SnarlViewer::show_input` and `SnarlViewer::show_output` now returns `impl SnarlPin`
  that allows fully custom pin drawing.
  This replaces `SnarlViewer::draw_input_pin` and `SnarlViewer::draw_output_pin`.

### Removed

- `SnarlViewer::draw_input_pin` and `SnarlViewer::draw_output_pin` are removed.
  Return custom type from `SnarlViewer::show_input` and `SnarlViewer::show_output` instead.

## [0.6.0] - 20.12.2024

### Changed

- Zooming now uses egui's zoom_delta from Input.
  This means that zooming is now performed by ctrl+scroll (cmd+scroll on mac) or pinching.
  Previously it was performed by scrolling alone, which collided with scrolling through content in the node's widgets and didn't work for touch controls.

### Added

- NodeLayout enum
  To control layout of nodes in the graph
  Can be set globally in SnarlStyle and overridden per node with SnarlViewer::node_layout
  Defaults to NodeLayout::Basic which is the previous layout
  NodeLayout::Sandwich is a new layout that places inputs, body and outputs vertically with inputs on top and outputs on bottom
  NodeLayout::FlippedSandwich is the same as Sandwich but with outputs on top and inputs on bottom

- SnarlViewer::draw_input_pin/draw_output_pin can be used to override how pins are drawn.
  Default implementation matches old behavior.
  This mechanism is meant to replace PinShape::Custom that was removed.

- SnarlViewer::draw_node_background can be used to override how node background is drawn.
  Default implementation matches old behavior.
  This mechanism is meant to replace BackgroundPattern::Custom that was removed.

- PinPlacement style option in SnarlStyle
  This option controls how pins are placed in the node
  Inside - pins are placed inside the node frame - default, old behavior
  Edge - pin centers are placed on the edge of the node frame
  Outside - pins are placed outside the node frame with specified margin

### Removed

- BackgroundPattern::Custom is removed.
  It contained opaque function to draw custom background pattern
  and permitted !Send and !Sync captures which made SnarlStyle !Send and !Sync as well

- PinShape::Custom is removed.
  It is replaced by SnarlViewer::draw_input_pin/draw_output_pin which is more flexible.

- BasicPinShape is removed. SnarlStyle::pin_shape has PinShape type now.

### Fixed

- Crash after centering graph when no nodes are present and adding a node afterwards