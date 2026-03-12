use egui::{Painter, Pos2, Rect, Response, Style, Ui, emath::TSTransform};

use crate::{GroupId, InPin, InPinId, NodeId, OutPin, OutPinId, Snarl};

use super::{
    BackgroundPattern, NodeLayout, SnarlStyle,
    pin::{AnyPins, PinContext, SnarlPin},
    wire::{WireWidgetContext, WireWidgetDescriptor},
};

/// `SnarlViewer` is a trait for viewing a Snarl.
///
/// It can extract necessary data from the nodes and controls their
/// response to certain events.
pub trait SnarlViewer<T, G = ()> {
    /// Returns title of the node.
    fn title(&mut self, node: &T) -> String;

    /// Returns the node's frame.
    /// All node's elements will be rendered inside this frame.
    /// Except for pins if they are configured to be rendered outside of the frame.
    ///
    /// Returns `default` by default.
    /// `default` frame is taken from the [`SnarlStyle::node_frame`] or constructed if it's `None`.
    ///
    /// Override this method to customize the frame for specific nodes.
    fn node_frame(
        &mut self,
        default: egui::Frame,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        snarl: &Snarl<T, G>,
    ) -> egui::Frame {
        let _ = (node, inputs, outputs, snarl);
        default
    }

    /// Returns the node's header frame.
    ///
    /// This frame would be placed on top of the node's frame.
    /// And header UI (see [`show_header`]) will be placed inside this frame.
    ///
    /// Returns `default` by default.
    /// `default` frame is taken from the [`SnarlStyle::header_frame`],
    /// or [`SnarlStyle::node_frame`] with removed shadow if `None`,
    /// or constructed if both are `None`.
    fn header_frame(
        &mut self,
        default: egui::Frame,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        snarl: &Snarl<T, G>,
    ) -> egui::Frame {
        let _ = (node, inputs, outputs, snarl);
        default
    }
    /// Checks if node has a custom egui style.
    #[inline]
    fn has_node_style(
        &mut self,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        snarl: &Snarl<T, G>,
    ) -> bool {
        let _ = (node, inputs, outputs, snarl);
        false
    }

    /// Modifies the node's egui style
    fn apply_node_style(
        &mut self,
        style: &mut Style,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        snarl: &Snarl<T, G>,
    ) {
        let _ = (style, node, inputs, outputs, snarl);
    }

    /// Returns elements layout for the node.
    ///
    /// Node consists of 5 parts: header, body, footer, input pins and output pins.
    /// See [`NodeLayout`] for available placements.
    ///
    /// Returns `default` by default.
    /// `default` layout is taken from the [`SnarlStyle::node_layout`] or constructed if it's `None`.
    /// Override this method to customize the layout for specific nodes.
    #[inline]
    fn node_layout(
        &mut self,
        default: NodeLayout,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        snarl: &Snarl<T, G>,
    ) -> NodeLayout {
        let _ = (node, inputs, outputs, snarl);
        default
    }

    /// Renders elements inside the node's header frame.
    ///
    /// This is the good place to show the node's title and controls related to the whole node.
    ///
    /// By default it shows the node's title on the left and calls [`show_header_buttons`]
    /// on the right side.
    #[inline]
    fn show_header(
        &mut self,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        ui.horizontal(|ui| {
            ui.label(self.title(&snarl[node]));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.show_header_buttons(node, inputs, outputs, ui, snarl);
            });
        });
    }

    /// Renders buttons/icons in the node's header (right side by default).
    ///
    /// Override this method to add custom buttons, flags, or status icons to nodes.
    /// This is useful for Houdini-style node flags (template, preview, bypass, etc.).
    ///
    /// By default this method does nothing.
    #[inline]
    fn show_header_buttons(
        &mut self,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (node, inputs, outputs, ui, snarl);
    }

    /// Returns number of input pins of the node.
    ///
    /// [`SnarlViewer::show_input`] will be called for each input in range `0..inputs()`.
    fn inputs(&mut self, node: &T) -> usize;

    /// Renders one specified node's input element and returns drawer for the corresponding pin.
    ///
    /// The `context` parameter provides information about the current UI state,
    /// including whether the label should be visible based on [`SnarlStyle::pin_label_visibility`].
    /// Use `context.label_visible` to conditionally show or hide the pin's label.
    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut Ui,
        context: PinContext,
        snarl: &mut Snarl<T, G>,
    ) -> impl SnarlPin + 'static;

    /// Returns number of output pins of the node.
    ///
    /// [`SnarlViewer::show_output`] will be called for each output in range `0..outputs()`.
    fn outputs(&mut self, node: &T) -> usize;

    /// Renders the node's output.
    ///
    /// The `context` parameter provides information about the current UI state,
    /// including whether the label should be visible based on [`SnarlStyle::pin_label_visibility`].
    /// Use `context.label_visible` to conditionally show or hide the pin's label.
    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut Ui,
        context: PinContext,
        snarl: &mut Snarl<T, G>,
    ) -> impl SnarlPin + 'static;

    /// Checks if node has something to show in body - between input and output pins.
    #[inline]
    fn has_body(&mut self, node: &T) -> bool {
        let _ = node;
        false
    }

    /// Renders the node's body.
    #[inline]
    fn show_body(
        &mut self,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (node, inputs, outputs, ui, snarl);
    }

    /// Checks if node has something to show in footer - below pins and body.
    #[inline]
    fn has_footer(&mut self, node: &T) -> bool {
        let _ = node;
        false
    }

    /// Renders the node's footer.
    #[inline]
    fn show_footer(
        &mut self,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (node, inputs, outputs, ui, snarl);
    }

    /// Reports the final node's rect after rendering.
    ///
    /// It aimed to be used for custom positioning of nodes that requires node dimensions for calculations.
    /// Node's position can be modified directly in this method.
    #[inline]
    fn final_node_rect(&mut self, node: NodeId, rect: Rect, ui: &mut Ui, snarl: &mut Snarl<T, G>) {
        let _ = (node, rect, ui, snarl);
    }

    /// Checks if node has something to show in on-hover popup.
    #[inline]
    fn has_on_hover_popup(&mut self, node: &T) -> bool {
        let _ = node;
        false
    }

    /// Renders the node's on-hover popup.
    #[inline]
    fn show_on_hover_popup(
        &mut self,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (node, inputs, outputs, ui, snarl);
    }

    /// Returns optional waypoints for a wire to route through.
    ///
    /// Override this method to specify intermediate waypoints for wires,
    /// allowing for custom wire routing (e.g., to avoid crossing other nodes).
    ///
    /// Note: This is a hook for future wire routing customization. Currently,
    /// for complex wire routing, you can hide the default wire using `wire_style`
    /// and draw custom wire segments in [`draw_foreground`].
    ///
    /// By default returns `None` (direct wire using default rendering).
    #[inline]
    fn wire_waypoints(
        &mut self,
        from: &OutPinId,
        to: &InPinId,
        snarl: &Snarl<T, G>,
    ) -> Option<Vec<Pos2>> {
        let _ = (from, to, snarl);
        None
    }

    /// Returns descriptors for wire widgets to display on this wire.
    /// Each descriptor specifies a parametric position `t` along the curve
    /// and optional per-widget alignment overrides.
    ///
    /// Return an empty [`Vec`] for no widgets (the default).
    /// This may not be called if the wire is invisible.
    #[inline]
    fn wire_widgets(
        &mut self,
        from: &OutPinId,
        to: &InPinId,
        snarl: &Snarl<T, G>,
    ) -> Vec<WireWidgetDescriptor> {
        let _ = (from, to, snarl);
        vec![]
    }

    /// Renders the wire widget at the given `index`.
    ///
    /// `index` corresponds to the position in the [`Vec`] returned by
    /// [`Self::wire_widgets`]. The [`WireWidgetContext`] contains the resolved
    /// position, alignment, and gap after applying style defaults.
    ///
    /// This may not be called if the wire is invisible.
    #[inline]
    fn show_wire_widget(
        &mut self,
        index: usize,
        context: &WireWidgetContext,
        from: &OutPin,
        to: &InPin,
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (index, context, from, to, ui, snarl);
    }

    /// Called when a wire is hovered or clicked.
    ///
    /// Use the [`Response`] to query interaction state (e.g.
    /// [`Response::clicked`], [`Response::secondary_clicked`],
    /// [`Response::hovered`]).
    ///
    /// Return `true` to suppress the default disconnect-on-click behavior.
    #[inline]
    fn wire_interact(
        &mut self,
        from: &OutPinId,
        to: &InPinId,
        response: &Response,
        snarl: &Snarl<T, G>,
    ) -> bool {
        let _ = (from, to, response, snarl);
        false
    }

    /// Checks if the snarl has something to show in context menu if right-clicked or long-touched on empty space at `pos`.
    #[inline]
    fn has_graph_menu(&mut self, pos: Pos2, snarl: &mut Snarl<T, G>) -> bool {
        let _ = (pos, snarl);
        false
    }

    /// Show context menu for the snarl.
    ///
    /// This can be used to implement menu for adding new nodes.
    #[inline]
    fn show_graph_menu(&mut self, pos: Pos2, ui: &mut Ui, snarl: &mut Snarl<T, G>) {
        let _ = (pos, ui, snarl);
    }

    /// Checks if the snarl has something to show in context menu if wire drag is stopped at `pos`.
    #[inline]
    fn has_dropped_wire_menu(&mut self, src_pins: AnyPins, snarl: &mut Snarl<T, G>) -> bool {
        let _ = (src_pins, snarl);
        false
    }

    /// Show context menu for the snarl. This menu is opened when releasing a pin to empty
    /// space. It can be used to implement menu for adding new node, and directly
    /// connecting it to the released wire.
    #[inline]
    fn show_dropped_wire_menu(
        &mut self,
        pos: Pos2,
        ui: &mut Ui,
        src_pins: AnyPins,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (pos, ui, src_pins, snarl);
    }

    /// Checks if the node has something to show in context menu if right-clicked or long-touched on the node.
    #[inline]
    fn has_node_menu(&mut self, node: &T) -> bool {
        let _ = node;
        false
    }

    /// Show context menu for the snarl.
    ///
    /// This can be used to implement menu for adding new nodes.
    #[inline]
    fn show_node_menu(
        &mut self,
        node: NodeId,
        inputs: &[InPin],
        outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (node, inputs, outputs, ui, snarl);
    }

    /// Asks the viewer to connect two pins.
    ///
    /// This is usually happens when user drags a wire from one node's output pin to another node's input pin or vice versa.
    /// By default this method connects the pins and returns `Ok(())`.
    #[inline]
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<T, G>) {
        snarl.connect(from.id, to.id);
    }

    /// Asks the viewer to disconnect two pins.
    #[inline]
    fn disconnect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<T, G>) {
        snarl.disconnect(from.id, to.id);
    }

    /// Asks the viewer to disconnect all wires from the output pin.
    ///
    /// This is usually happens when right-clicking on an output pin.
    /// By default this method disconnects the pins and returns `Ok(())`.
    #[inline]
    fn drop_outputs(&mut self, pin: &OutPin, snarl: &mut Snarl<T, G>) {
        snarl.drop_outputs(pin.id);
    }

    /// Asks the viewer to disconnect all wires from the input pin.
    ///
    /// This is usually happens when right-clicking on an input pin.
    /// By default this method disconnects the pins and returns `Ok(())`.
    #[inline]
    fn drop_inputs(&mut self, pin: &InPin, snarl: &mut Snarl<T, G>) {
        snarl.drop_inputs(pin.id);
    }

    /// Called when a node has been moved (dragged to a new position).
    ///
    /// This callback is invoked after the node's position has been updated in the snarl.
    /// Override this method to react to node position changes without polling.
    ///
    /// The `new_pos` parameter contains the node's new position in graph coordinates.
    #[inline]
    fn node_moved(&mut self, node: NodeId, new_pos: Pos2, snarl: &mut Snarl<T, G>) {
        let _ = (node, new_pos, snarl);
    }

    /// Draws background of the snarl view.
    ///
    /// By default it draws the background pattern using [`BackgroundPattern::draw`].
    ///
    /// If you want to draw the background yourself, you can override this method.
    /// This is also a good place to draw grouping rectangles behind nodes.
    #[inline]
    fn draw_background(
        &mut self,
        background: Option<&BackgroundPattern>,
        viewport: &Rect,
        snarl_style: &SnarlStyle,
        style: &Style,
        painter: &Painter,
        snarl: &Snarl<T, G>,
    ) {
        let _ = snarl;

        if let Some(background) = background {
            background.draw(viewport, snarl_style, style, painter);
        }
    }

    /// Draws foreground elements on top of nodes and wires.
    ///
    /// Override this method to draw comments, annotations, overlays, or any
    /// custom elements that should appear on top of the graph.
    ///
    /// By default this method does nothing.
    #[inline]
    fn draw_foreground(
        &mut self,
        viewport: &Rect,
        snarl_style: &SnarlStyle,
        style: &Style,
        painter: &Painter,
        snarl: &Snarl<T, G>,
    ) {
        let _ = (viewport, snarl_style, style, painter, snarl);
    }

    /// Called to compute automatic layout positions for nodes.
    ///
    /// Override this method to implement automatic layout algorithms
    /// (e.g., force-directed, hierarchical, tree layouts).
    ///
    /// Return a map of node IDs to their new positions. Only nodes present
    /// in the returned map will be repositioned.
    ///
    /// This method is called when [`apply_layout`](Self::apply_layout) returns true.
    ///
    /// By default returns an empty map (no layout changes).
    #[inline]
    fn compute_layout(&mut self, snarl: &Snarl<T, G>) -> std::collections::HashMap<NodeId, Pos2> {
        let _ = snarl;
        std::collections::HashMap::new()
    }

    /// Returns whether to apply automatic layout this frame.
    ///
    /// Override this method to trigger layout computation based on user input
    /// (e.g., a button press) or other conditions.
    ///
    /// By default returns false (no automatic layout).
    #[inline]
    fn apply_layout(&mut self, snarl: &Snarl<T, G>) -> bool {
        let _ = snarl;
        false
    }

    /// Informs the viewer what is the current transform of the snarl view
    /// and allows viewer to override it.
    ///
    /// This method is called in the beginning of the graph rendering.
    ///
    /// By default it does nothing.
    #[inline]
    fn current_transform(&mut self, to_global: &mut TSTransform, snarl: &mut Snarl<T, G>) {
        let _ = (to_global, snarl);
    }

    /// Allows last-minute updates to the selected nodes.
    ///
    /// This method is called at the beginning of graph rendering.
    /// Return `Some(Vec<NodeId>)` to override the current selection,
    /// or `None` to keep the current selection unchanged.
    #[inline]
    fn update_selection(&mut self, selected_nodes: &[NodeId]) -> Option<Vec<NodeId>> {
        let _ = selected_nodes;
        None
    }

    // --- Group methods ---

    /// Returns the title of a group.
    ///
    /// By default returns the group's stored title.
    #[inline]
    fn group_title(&mut self, group: GroupId, snarl: &Snarl<T, G>) -> String {
        snarl
            .group_info(group)
            .map_or_else(String::new, |g| g.title.clone())
    }

    /// Returns the frame used to draw the group background.
    ///
    /// Override this to customize group appearance per-group.
    #[inline]
    fn group_frame(
        &mut self,
        default: egui::Frame,
        group: GroupId,
        snarl: &Snarl<T, G>,
    ) -> egui::Frame {
        let _ = (group, snarl);
        default
    }

    /// Renders custom content in the group header.
    ///
    /// By default shows the group title.
    #[inline]
    fn show_group_header(
        &mut self,
        group: GroupId,
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        let title = self.group_title(group, snarl);
        ui.label(title);
    }

    /// Called when a node is about to be added to a group by drag.
    /// Return `false` to prevent the addition.
    #[inline]
    fn accept_node_in_group(
        &mut self,
        node: NodeId,
        group: GroupId,
        snarl: &Snarl<T, G>,
    ) -> bool {
        let _ = (node, group, snarl);
        true
    }

    /// Called after a node's group membership changes.
    #[inline]
    fn node_group_changed(
        &mut self,
        node: NodeId,
        old_group: Option<GroupId>,
        new_group: Option<GroupId>,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (node, old_group, new_group, snarl);
    }

    /// Checks if the group has a context menu.
    #[inline]
    fn has_group_menu(&mut self, group: GroupId, snarl: &Snarl<T, G>) -> bool {
        let _ = (group, snarl);
        false
    }

    /// Shows the context menu for a group.
    #[inline]
    fn show_group_menu(
        &mut self,
        group: GroupId,
        ui: &mut Ui,
        snarl: &mut Snarl<T, G>,
    ) {
        let _ = (group, ui, snarl);
    }
}
