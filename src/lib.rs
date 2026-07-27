//!
//! # egui-snarl
//!
//! Provides a node-graph container for egui.
//!
//!

#![deny(missing_docs, non_ascii_idents, unsafe_code)]
#![deny(
    clippy::correctness,
    clippy::complexity,
    clippy::perf,
    clippy::style,
    clippy::suspicious
)]
#![warn(clippy::pedantic, clippy::dbg_macro, clippy::must_use_candidate, missing_docs)]
#![allow(clippy::range_plus_one, clippy::inline_always, clippy::use_self)]

pub mod ui;

use std::{collections::HashSet, hash::RandomState, iter::FusedIterator, ops::{Index, IndexMut}};

use egui::Pos2;
use slab::Slab;

impl<T, G> Default for Snarl<T, G> {
    fn default() -> Self {
        Snarl::new()
    }
}

/// Node identifier.
///
/// This is newtype wrapper around [`usize`] that implements
/// necessary traits, but omits arithmetic operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
pub struct NodeId(pub usize);

/// Group identifier.
///
/// This is newtype wrapper around [`usize`] that implements
/// necessary traits, but omits arithmetic operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct GroupId(pub usize);

/// Node of the graph.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Node<T> {
    /// Node generic value.
    pub value: T,

    /// Position of the top-left corner of the node.
    /// This does not include frame margin.
    pub pos: egui::Pos2,

    /// Flag indicating that the node is open - not collapsed.
    pub open: bool,

    /// Group this node belongs to, if any.
    /// `None` means the node is at the root level.
    #[cfg_attr(feature = "serde", serde(default))]
    pub group: Option<GroupId>,
}

/// Group of nodes.
///
/// Groups can contain nodes and other groups, forming a hierarchy.
/// Groups can be collapsed to hide their contents.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Group<G> {
    /// Group generic value.
    pub value: G,

    /// Flag indicating that the group is open - not collapsed.
    pub open: bool,

    /// Parent group, if nested.
    /// `None` means the group is at the root level.
    pub parent: Option<GroupId>,

    /// Position of the group when it has no children.
    /// When the group has children, the position is computed from the bounding box of children.
    pub pos: egui::Pos2,
}

/// Output pin identifier.
/// Cosists of node id and pin index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
pub struct OutPinId {
    /// Node id.
    pub node: NodeId,

    /// Output pin index.
    pub output: usize,
}

/// Input pin identifier. Cosists of node id and pin index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "facet", derive(facet::Facet))]
pub struct InPinId {
    /// Node id.
    pub node: NodeId,

    /// Input pin index.
    pub input: usize,
}

/// Connection between two nodes.
///
/// Nodes may support multiple connections to the same input or output.
/// But duplicate connections between same input and the same output are not allowed.
/// Attempt to insert existing connection will be ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Wire {
    out_pin: OutPinId,
    in_pin: InPinId,
}

#[derive(Clone, Debug)]
struct Wires {
    wires: HashSet<Wire>,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Wires {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;

        let mut seq = serializer.serialize_seq(Some(self.wires.len()))?;
        for wire in &self.wires {
            seq.serialize_element(&wire)?;
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Wires {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = HashSet<Wire>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence of wires")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut wires = HashSet::with_hasher(RandomState::new());
                while let Some(wire) = seq.next_element()? {
                    wires.insert(wire);
                }
                Ok(wires)
            }
        }

        let wires = deserializer.deserialize_seq(Visitor)?;
        Ok(Wires { wires })
    }
}

impl Wires {
    fn new() -> Self {
        Wires {
            wires: HashSet::with_hasher(RandomState::new()),
        }
    }

    fn insert(&mut self, wire: Wire) -> bool {
        self.wires.insert(wire)
    }

    fn remove(&mut self, wire: &Wire) -> bool {
        self.wires.remove(wire)
    }

    fn drop_node(&mut self, node: NodeId) -> usize {
        let count = self.wires.len();
        self.wires
            .retain(|wire| wire.out_pin.node != node && wire.in_pin.node != node);
        count - self.wires.len()
    }

    fn drop_inputs(&mut self, pin: InPinId) -> usize {
        let count = self.wires.len();
        self.wires.retain(|wire| wire.in_pin != pin);
        count - self.wires.len()
    }

    fn drop_outputs(&mut self, pin: OutPinId) -> usize {
        let count = self.wires.len();
        self.wires.retain(|wire| wire.out_pin != pin);
        count - self.wires.len()
    }

    fn wired_inputs(&self, out_pin: OutPinId) -> impl Iterator<Item = InPinId> + '_ {
        self.wires
            .iter()
            .filter(move |wire| wire.out_pin == out_pin)
            .map(|wire| wire.in_pin)
    }

    fn wired_outputs(&self, in_pin: InPinId) -> impl Iterator<Item = OutPinId> + '_ {
        self.wires
            .iter()
            .filter(move |wire| wire.in_pin == in_pin)
            .map(|wire| wire.out_pin)
    }

    fn iter(&self) -> impl Iterator<Item = Wire> + '_ {
        self.wires.iter().copied()
    }
}

/// Snarl is generic node-graph container.
///
/// It holds graph state - positioned nodes and wires between their pins.
/// It can be rendered using [`Snarl::show`].
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Snarl<T, G = ()> {
    // #[cfg_attr(feature = "serde", serde(with = "serde_nodes"))]
    nodes: Slab<Node<T>>,
    wires: Wires,
    // We have to provide the path here so it doesn't the unnecessary bound G: Default
    #[cfg_attr(feature = "serde", serde(default = "Default::default"))]
    groups: Slab<Group<G>>,
}

impl<T, G> Snarl<T, G> {
    /// Create a new empty Snarl.
    ///
    /// # Examples
    ///
    /// ```
    /// # use egui_snarl::Snarl;
    /// let snarl = Snarl::<()>::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Snarl {
            nodes: Slab::new(),
            wires: Wires::new(),
            groups: Slab::new(),
        }
    }

    /// Adds a node to the Snarl.
    /// Returns the index of the node.
    ///
    /// # Examples
    ///
    /// ```
    /// # use egui_snarl::Snarl;
    /// let mut snarl = Snarl::<()>::new();
    /// snarl.insert_node(egui::pos2(0.0, 0.0), ());
    /// ```
    pub fn insert_node(&mut self, pos: egui::Pos2, node: T) -> NodeId {
        let idx = self.nodes.insert(Node {
            value: node,
            pos,
            open: true,
            group: None,
        });

        NodeId(idx)
    }

    /// Adds a node to the Snarl in collapsed state.
    /// Returns the index of the node.
    ///
    /// # Examples
    ///
    /// ```
    /// # use egui_snarl::Snarl;
    /// let mut snarl = Snarl::<()>::new();
    /// snarl.insert_node_collapsed(egui::pos2(0.0, 0.0), ());
    /// ```
    pub fn insert_node_collapsed(&mut self, pos: egui::Pos2, node: T) -> NodeId {
        let idx = self.nodes.insert(Node {
            value: node,
            pos,
            open: false,
            group: None,
        });

        NodeId(idx)
    }

    /// Opens or collapses a node.
    ///
    /// # Panics
    ///
    /// Panics if the node does not exist.
    #[track_caller]
    pub fn open_node(&mut self, node: NodeId, open: bool) {
        self.nodes[node.0].open = open;
    }

    /// Removes a node from the Snarl.
    /// Returns the node if it was removed.
    ///
    /// # Panics
    ///
    /// Panics if the node does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// # use egui_snarl::Snarl;
    /// let mut snarl = Snarl::<()>::new();
    /// let node = snarl.insert_node(egui::pos2(0.0, 0.0), ());
    /// snarl.remove_node(node);
    /// ```
    #[track_caller]
    pub fn remove_node(&mut self, idx: NodeId) -> T {
        let value = self.nodes.remove(idx.0).value;
        self.wires.drop_node(idx);
        value
    }

    /// Connects two nodes.
    /// Returns true if the connection was successful.
    /// Returns false if the connection already exists.
    ///
    /// # Panics
    ///
    /// Panics if either node does not exist.
    #[track_caller]
    pub fn connect(&mut self, from: OutPinId, to: InPinId) -> bool {
        assert!(self.nodes.contains(from.node.0));
        assert!(self.nodes.contains(to.node.0));

        let wire = Wire {
            out_pin: from,
            in_pin: to,
        };
        self.wires.insert(wire)
    }

    /// Disconnects two nodes.
    /// Returns true if the connection was removed.
    ///
    /// # Panics
    ///
    /// Panics if either node does not exist.
    #[track_caller]
    pub fn disconnect(&mut self, from: OutPinId, to: InPinId) -> bool {
        assert!(self.nodes.contains(from.node.0));
        assert!(self.nodes.contains(to.node.0));

        let wire = Wire {
            out_pin: from,
            in_pin: to,
        };

        self.wires.remove(&wire)
    }

    /// Removes all connections to the node's pin.
    ///
    /// Returns number of removed connections.
    ///
    /// # Panics
    ///
    /// Panics if the node does not exist.
    #[track_caller]
    pub fn drop_inputs(&mut self, pin: InPinId) -> usize {
        assert!(self.nodes.contains(pin.node.0));
        self.wires.drop_inputs(pin)
    }

    /// Removes all connections from the node's pin.
    /// Returns number of removed connections.
    ///
    /// # Panics
    ///
    /// Panics if the node does not exist.
    #[track_caller]
    pub fn drop_outputs(&mut self, pin: OutPinId) -> usize {
        assert!(self.nodes.contains(pin.node.0));
        self.wires.drop_outputs(pin)
    }

    /// Removes all connections to and from the node.
    /// Returns number of removed connections.
    ///
    /// # Panics
    ///
    /// Panics if the node does not exist.
    #[track_caller]
    pub fn disconnect_all(&mut self, node: NodeId) -> usize {
        assert!(self.nodes.contains(node.0));
        self.wires.drop_node(node)
    }

    /// Returns reference to the node.
    #[must_use]
    pub fn node(&self, idx: NodeId) -> Option<&T> {
        self.nodes.get(idx.0).map(|node| &node.value)
    }

    /// Returns mutable reference to the node.
    pub fn node_mut(&mut self, idx: NodeId) -> Option<&mut T> {
        match self.nodes.get_mut(idx.0) {
            Some(node) => Some(&mut node.value),
            None => None,
        }
    }

    /// Returns reference to the node data.
    #[must_use]
    pub fn node_info(&self, idx: NodeId) -> Option<&Node<T>> {
        self.nodes.get(idx.0)
    }

    /// Returns mutable reference to the node data.
    #[must_use]
    pub fn node_info_mut(&mut self, idx: NodeId) -> Option<&mut Node<T>> {
        self.nodes.get_mut(idx.0)
    }

    /// Deprecated: Use [`node`](Self::node) instead.
    #[deprecated(since = "0.8.0", note = "renamed to `node` per Rust API guidelines")]
    #[must_use]
    pub fn get_node(&self, idx: NodeId) -> Option<&T> {
        self.node(idx)
    }

    /// Deprecated: Use [`node_mut`](Self::node_mut) instead.
    #[deprecated(
        since = "0.8.0",
        note = "renamed to `node_mut` per Rust API guidelines"
    )]
    pub fn get_node_mut(&mut self, idx: NodeId) -> Option<&mut T> {
        self.node_mut(idx)
    }

    /// Deprecated: Use [`node_info`](Self::node_info) instead.
    #[deprecated(
        since = "0.8.0",
        note = "renamed to `node_info` per Rust API guidelines"
    )]
    #[must_use]
    pub fn get_node_info(&self, idx: NodeId) -> Option<&Node<T>> {
        self.node_info(idx)
    }

    /// Deprecated: Use [`node_info_mut`](Self::node_info_mut) instead.
    #[deprecated(
        since = "0.8.0",
        note = "renamed to `node_info_mut` per Rust API guidelines"
    )]
    pub fn get_node_info_mut(&mut self, idx: NodeId) -> Option<&mut Node<T>> {
        self.node_info_mut(idx)
    }

    /// Iterates over shared references to each node, `&T`.
    pub fn nodes(&self) -> NodesIter<'_, T> {
        NodesIter(self.nodes.iter())
    }

    /// Iterates over mutable references to each node, `&mut T`.
    pub fn nodes_mut(&mut self) -> NodesIterMut<'_, T> {
        NodesIterMut(self.nodes.iter_mut())
    }

    /// Iterates over shared references to each node and its position, `(Pos2, &T)`.
    pub fn nodes_pos(&self) -> NodesPosIter<'_, T> {
        NodesPosIter(self.nodes.iter())
    }

    /// Iterates over mutable references to each node and its position, `(Pos2, &mut T)`.
    pub fn nodes_pos_mut(&mut self) -> NodesPosIterMut<'_, T> {
        NodesPosIterMut(self.nodes.iter_mut())
    }

    /// Iterates over shared references to each node and its identifier, `(NodeId, &T)`.
    pub fn node_ids(&self) -> NodesIdsIter<'_, T> {
        NodesIdsIter(self.nodes.iter())
    }

    /// Iterates over mutable references to each node and its identifier, `(NodeId, &mut T)`.
    pub fn nodes_ids_mut(&mut self) -> NodesIdsIterMut<'_, T> {
        NodesIdsIterMut(self.nodes.iter_mut())
    }

    /// Iterates over shared references to each node, its position and its identifier, `(NodeId, Pos2, &T)`.
    pub fn nodes_pos_ids(&self) -> NodesPosIdsIter<'_, T> {
        NodesPosIdsIter(self.nodes.iter())
    }

    /// Iterates over mutable references to each node, its position and its identifier, `(NodeId, Pos2, &mut T)`.
    pub fn nodes_pos_ids_mut(&mut self) -> NodesPosIdsIterMut<'_, T> {
        NodesPosIdsIterMut(self.nodes.iter_mut())
    }

    /// Iterates over shared references to each node data, `&Node<T>`.
    pub fn nodes_info(&self) -> NodeInfoIter<'_, T> {
        NodeInfoIter(self.nodes.iter())
    }

    /// Iterates over mutable references to each node data, `&mut Node<T>`.
    pub fn nodes_info_mut(&mut self) -> NodeInfoIterMut<'_, T> {
        NodeInfoIterMut(self.nodes.iter_mut())
    }

    /// Iterates over shared references to each node id and data, yielding `(NodeId, &Node<T>)` for each.
    pub fn nodes_ids_data(&self) -> NodeIdsDataIter<'_, T> {
        NodeIdsDataIter(self.nodes.iter())
    }

    /// Iterates over mutable references to each node id and data, yielding `(NodeId, &mut Node<T>)` for each.
    pub fn nodes_ids_data_mut(&mut self) -> NodeIdsDataIterMut<'_, T> {
        NodeIdsDataIterMut(self.nodes.iter_mut())
    }

    /// Iterates over wires.
    pub fn wires(&self) -> impl Iterator<Item = (OutPinId, InPinId)> + '_ {
        self.wires.iter().map(|wire| (wire.out_pin, wire.in_pin))
    }

    /// Returns input pin of the node.
    #[must_use]
    pub fn in_pin(&self, pin: InPinId) -> InPin {
        InPin::new(self, pin)
    }

    /// Returns output pin of the node.
    #[must_use]
    pub fn out_pin(&self, pin: OutPinId) -> OutPin {
        OutPin::new(self, pin)
    }

    // --- Group methods ---

    /// Adds a group to the Snarl.
    /// Returns the identifier of the group.
    pub fn insert_group(&mut self, pos: egui::Pos2, value: G) -> GroupId {
        let idx = self.groups.insert(Group {
            value,
            open: true,
            parent: None,
            pos,
        });
        GroupId(idx)
    }

    /// Removes a group from the Snarl.
    /// All child nodes and subgroups are moved to the parent group (or root).
    ///
    /// Returns the group value if the group existed.
    ///
    /// # Panics
    ///
    /// Panics if the group does not exist.
    #[track_caller]
    pub fn remove_group(&mut self, id: GroupId) -> G {
        let group = self.groups.remove(id.0);
        let parent = group.parent;

        // Move child nodes to parent group
        for (_, node) in &mut self.nodes {
            if node.group == Some(id) {
                node.group = parent;
            }
        }

        // Move child groups to parent group
        for (_, g) in &mut self.groups {
            if g.parent == Some(id) {
                g.parent = parent;
            }
        }

        group.value
    }

    /// Returns reference to the group.
    #[must_use]
    pub fn group(&self, id: GroupId) -> Option<&G> {
        self.group_info(id)
            .map(|info| &info.value)
    }

    /// Returns mutable reference to the group.
    #[must_use]
    pub fn group_mut(&mut self, id: GroupId) -> Option<&mut G> {
        self.group_info_mut(id)
            .map(|info| &mut info.value)
    }

    /// Returns reference to the group data.
    #[must_use]
    pub fn group_info(&self, id: GroupId) -> Option<&Group<G>> {
        self.groups.get(id.0)
    }

    /// Returns mutable reference to the group data.
    pub fn group_info_mut(&mut self, id: GroupId) -> Option<&mut Group<G>> {
        self.groups.get_mut(id.0)
    }

    /// Sets the group a node belongs to.
    ///
    /// # Panics
    ///
    /// Panics if the node does not exist.
    #[track_caller]
    pub fn set_node_group(&mut self, node: NodeId, group: Option<GroupId>) {
        self.nodes[node.0].group = group;
    }

    /// Sets the parent group of a group.
    /// Validates that this does not create a cycle.
    ///
    /// # Panics
    ///
    /// Panics if the group does not exist or if the assignment would create a cycle.
    #[track_caller]
    pub fn set_group_parent(&mut self, group: GroupId, parent: Option<GroupId>) {
        if let Some(parent_id) = parent {
            // Walk the parent chain to check for cycles
            let mut current = Some(parent_id);
            while let Some(id) = current {
                assert!(id != group, "setting this parent would create a cycle");
                current = self.groups[id.0].parent;
            }
        }
        self.groups[group.0].parent = parent;
    }

    /// Opens or collapses a group.
    ///
    /// # Panics
    ///
    /// Panics if the group does not exist.
    #[track_caller]
    pub fn open_group(&mut self, id: GroupId, open: bool) {
        self.groups[id.0].open = open;
    }

    /// Iterates over direct child nodes of a group.
    #[must_use]
    pub fn group_nodes(&self, group: GroupId) -> impl DoubleEndedIterator<Item = NodeId> + '_ {
        self.nodes
            .iter()
            .filter(move |(_, node)| node.group == Some(group))
            .map(|(idx, _)| NodeId(idx))
    }

    /// Iterates over direct child groups of a group.
    #[must_use]
    pub fn group_subgroups(&self, group: GroupId) -> impl DoubleEndedIterator<Item = GroupId> + '_ {
        self.groups
            .iter()
            .filter(move |(_, g)| g.parent == Some(group))
            .map(|(idx, _)| GroupId(idx))
    }

    /// Returns all descendant node IDs of a group (recursive).
    #[must_use]
    pub fn group_all_descendants(&self, group: GroupId) -> Vec<NodeId> {
        let mut result = Vec::new();
        self.collect_descendants(group, &mut result);
        result
    }

    fn collect_descendants(&self, group: GroupId, result: &mut Vec<NodeId>) {
        for (idx, node) in &self.nodes {
            if node.group == Some(group) {
                result.push(NodeId(idx));
            }
        }
        for (idx, g) in &self.groups {
            if g.parent == Some(group) {
                self.collect_descendants(GroupId(idx), result);
            }
        }
    }

    /// Iterates over groups, `&G`.
    #[must_use]
    pub fn groups(&self) -> GroupsIter<'_, G> {
        GroupsIter(self.groups.iter())
    }

    /// Iterates over mutable references to each node, `&mut G`.
    #[must_use]
    pub fn groups_mut(&mut self) -> GroupsIterMut<'_, G> {
        GroupsIterMut(self.groups.iter_mut())
    }

    /// Iterates over groups with their IDs, yielding `(GroupId, &G)` for each.
    pub fn groups_ids(&self) -> GroupsIdsIter<'_, G> {
        GroupsIdsIter(self.groups.iter())
    }

    /// Iterates over groups with their IDs, yielding `(GroupId, &mut G)` for each.
    pub fn groups_ids_mut(&mut self) -> GroupsIdsIterMut<'_, G> {
        GroupsIdsIterMut(self.groups.iter_mut())
    }

    /// Iterates over group data, yielding `&Group<G>` for each.
    pub fn groups_info(&self) -> GroupInfoIter<'_, G> {
        GroupInfoIter(self.groups.iter())
    }

    /// Iterates over group data, yielding `&mut Group<G>` for each.
    pub fn groups_info_mut(&mut self) -> GroupInfoIterMut<'_, G> {
        GroupInfoIterMut(self.groups.iter_mut())
    }

    /// Iterates over group data with their IDs, yielding `(GroupId, &Group<G>)` for each.
    pub fn groups_ids_data(&self) -> GroupIdsDataIter<'_, G> {
        GroupIdsDataIter(self.groups.iter())
    }

    /// Iterates over group data with their IDs, yielding `(GroupId, &mut Group<G>)` for each.
    pub fn groups_ids_data_mut(&mut self) -> GroupIdsDataIterMut<'_, G> {
        GroupIdsDataIterMut(self.groups.iter_mut())
    }

    /// Returns true if the group is an ancestor of the given group.
    #[must_use]
    pub fn is_ancestor(&self, ancestor: GroupId, descendant: GroupId) -> bool {
        self.group_ancestor_ids(Some(descendant))
            .any(|candidate| ancestor == candidate)
    }

    /// Returns true if the node is inside the given group or any of its descendants.
    #[must_use]
    pub fn node_in_group_recursive(&self, node: NodeId, group: GroupId) -> bool {
        self.group_ancestor_ids(self.node_info(node).and_then(|node| node.group))
            .any(|ancestor_id| group == ancestor_id)
    }

    /// Returns true if the given group (or any ancestor) is collapsed.
    #[must_use]
    pub fn is_group_collapsed(&self, group: GroupId) -> bool {
        self.group_ancestors(Some(group))
            .any(|group| !group.open)
    }

    /// Iterate over the [`GroupId`]s ancestors of `group`, starting with `group` if present
    fn group_ancestor_ids(&self, group: Option<GroupId>) -> impl FusedIterator<Item = GroupId> {
        std::iter::successors(
            group,
            |group_id| self.groups.get(group_id.0)?.parent
        )
    }
    fn group_ancestors(&self, group: Option<GroupId>) -> impl Iterator<Item = &Group<G>> {
        self.group_ancestor_ids(group)
            .map_while(|group_id| self.group_info(group_id))
    }

    /// Returns true if the node is hidden because its group (or an ancestor group) is collapsed.
    #[must_use]
    pub fn is_node_hidden(&self, node: NodeId) -> bool {
        self.node_info(node)
            .and_then(|n| n.group)
            .is_some_and(|group| self.is_group_collapsed(group))
    }

    /// Returns the outermost collapsed group that hides this node, if any.
    /// This is the group whose visual rect should be used as a wire endpoint.
    #[must_use]
    pub(crate) fn node_collapsed_group(&self, node: NodeId) -> Option<GroupId> {
        let group = self.node_info(node)?.group?;
        // Walk up the ancestor chain, tracking the outermost collapsed group
        self.group_ancestor_ids(Some(group))
            .filter(|group_id| self.group_info(*group_id).is_some_and(|group| !group.open))
            .last()
    }
}

impl<T, G> Index<NodeId> for Snarl<T, G> {
    type Output = T;

    #[inline]
    #[track_caller]
    fn index(&self, idx: NodeId) -> &Self::Output {
        &self.nodes[idx.0].value
    }
}

impl<T, G> IndexMut<NodeId> for Snarl<T, G> {
    #[inline]
    #[track_caller]
    fn index_mut(&mut self, idx: NodeId) -> &mut Self::Output {
        &mut self.nodes[idx.0].value
    }
}

impl<T, G> Index<GroupId> for Snarl<T, G> {
    type Output = G;

    #[inline]
    #[track_caller]
    fn index(&self, idx: GroupId) -> &Self::Output {
        &self.groups[idx.0].value
    }
}

impl<T, G> IndexMut<GroupId> for Snarl<T, G> {
    #[inline]
    #[track_caller]
    fn index_mut(&mut self, idx: GroupId) -> &mut Self::Output {
        &mut self.groups[idx.0].value
    }
}

/// Defines a iterator struct wrapping `slab::Iter` or `slab::IterMut`,
/// mapping each item.
/// 
/// Use `ref` for a cloneable shared-reference iterator and `mut` for a mutable-reference iterator.
macro_rules! define_iter {
    ($(#[$($attrss:tt)*])* ref $name:ident<$ty:ident>($inner:ty) -> $item:ty; |$arg:pat_param| $value:expr) => {
        $(#[$($attrss)*])*
        #[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
        #[derive(Clone)]
        pub struct $name<'a, $ty>(slab::Iter<'a, $inner>);
        impl<'a, $ty> Iterator for $name<'a, $ty> {
            type Item = $item;

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }

            fn next(&mut self) -> Option<Self::Item> {
                let $arg = self.0.next()?;
                Some($value)
            }

            fn nth(&mut self, n: usize) -> Option<Self::Item> {
                let $arg = self.0.nth(n)?;
                Some($value)
            }
        }

        impl<T> DoubleEndedIterator for $name<'_, T> {
            fn next_back(&mut self) -> Option<Self::Item> {
                let $arg = self.0.next_back()?;
                Some($value)
            }
        }

        impl<T> ExactSizeIterator for $name<'_, T> {
            fn len(&self) -> usize {
                self.0.len()
            }
        }

        impl<T> FusedIterator for $name<'_, T> {}
    };
    ($(#[$($attrss:tt)*])* mut $name:ident<$ty:ident>($inner:ty) -> $item:ty; |$arg:pat_param| $value:expr) => {
        $(#[$($attrss)*])*
        #[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
        // No derive(Clone)
        pub struct $name<'a, $ty>(slab::IterMut<'a, $inner>);
        
        impl<'a, $ty> Iterator for $name<'a, $ty> {
            type Item = $item;

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }

            fn next(&mut self) -> Option<Self::Item> {
                let $arg = self.0.next()?;
                Some($value)
            }

            fn nth(&mut self, n: usize) -> Option<Self::Item> {
                let $arg = self.0.nth(n)?;
                Some($value)
            }
        }

        impl<T> DoubleEndedIterator for $name<'_, T> {
            fn next_back(&mut self) -> Option<Self::Item> {
                let $arg = self.0.next_back()?;
                Some($value)
            }
        }

        impl<T> ExactSizeIterator for $name<'_, T> {
            fn len(&self) -> usize {
                self.0.len()
            }
        }
        
        impl<T> FusedIterator for $name<'_, T> {}
    }
}

define_iter!(
    /// Iterator over shared references to nodes.
    ref NodesIter<T>(Node<T>) -> &'a T;
    |(_, node)| &node.value
);
define_iter!(
    /// Iterator over mutable references to nodes.
    mut NodesIterMut<T>(Node<T>) -> &'a mut T;
    |(_, node)| &mut node.value
);

define_iter!(
    /// Iterator over shared references to nodes and their positions.
    ref NodesPosIter<T>(Node<T>) -> (Pos2, &'a T);
    |(_, node)| (node.pos, &node.value)
);
define_iter!(
    /// Iterator over mutable references to nodes and their positions.
    mut NodesPosIterMut<T>(Node<T>) -> (Pos2, &'a mut T);
    |(_, node)| (node.pos, &mut node.value)
);

define_iter!(
    /// Iterator over shared references to nodes and their IDs.
    ref NodesIdsIter<T>(Node<T>) -> (NodeId, &'a T);
    |(idx, node)| (NodeId(idx), &node.value)
);
define_iter!(
    /// Iterator over mutable references to nodes and their IDs.
    mut NodesIdsIterMut<T>(Node<T>) -> (NodeId, &'a mut T);
    |(idx, node)| (NodeId(idx), &mut node.value)
);

define_iter!(
    /// Iterator over shared references to nodes, their positions, and their IDs.
    ref NodesPosIdsIter<T>(Node<T>) -> (NodeId, Pos2, &'a T);
    |(idx, node)| (NodeId(idx), node.pos, &node.value)
);
define_iter!(
    /// Iterator over mutable references to nodes, their positions, and their IDs.
    mut NodesPosIdsIterMut<T>(Node<T>) -> (NodeId, Pos2, &'a mut T);
    |(idx, node)| (NodeId(idx), node.pos, &mut node.value)
);

define_iter!(
    /// Iterator over shared references to node data.
    ref NodeInfoIter<T>(Node<T>) -> &'a Node<T>;
    |(_, node)| node
);
define_iter!(
    /// Iterator over mutable references to node data.
    mut NodeInfoIterMut<T>(Node<T>) -> &'a mut Node<T>;
    |(_, node)| node
);

define_iter!(
    /// Iterator over shared references to node data and their IDs.
    ref NodeIdsDataIter<T>(Node<T>) -> (NodeId, &'a Node<T>);
    |(idx, node)| (NodeId(idx), node)
);
define_iter!(
    /// Iterator over mutable references to node data and their IDs.
    mut NodeIdsDataIterMut<T>(Node<T>) -> (NodeId, &'a mut Node<T>);
    |(idx, node)| (NodeId(idx), node)
);




define_iter!(
    /// Iterator over shared references to groups.
    ref GroupsIter<G>(Group<G>) -> &'a G;
    |(_, group)| &group.value
);
define_iter!(
    /// Iterator over mutable references to groups.
    mut GroupsIterMut<G>(Group<G>) -> &'a mut G;
    |(_, group)| &mut group.value
);

define_iter!(
    /// Iterator over shared references to groups and their IDs.
    ref GroupsIdsIter<G>(Group<G>) -> (GroupId, &'a G);
    |(idx, group)| (GroupId(idx), &group.value)
);
define_iter!(
    /// Iterator over mutable references to groups and their IDs.
    mut GroupsIdsIterMut<G>(Group<G>) -> (GroupId, &'a mut G);
    |(idx, group)| (GroupId(idx), &mut group.value)
);

define_iter!(
    /// Iterator over shared references to group data.
    ref GroupInfoIter<G>(Group<G>) -> &'a Group<G>;
    |(_, group)| group
);
define_iter!(
    /// Iterator over mutable references to group data.
    mut GroupInfoIterMut<G>(Group<G>) -> &'a Group<G>;
    |(_, group)| group
);

define_iter!(
    /// Iterator over shared references to group data and their IDs.
    ref GroupIdsDataIter<G>(Group<G>) -> (GroupId, &'a Group<G>);
    |(idx, node)| (GroupId(idx), node)
);
define_iter!(
    /// Iterator over mutable references to group data and their IDs.
    mut GroupIdsDataIterMut<G>(Group<G>) -> (GroupId, &'a mut Group<G>);
    |(idx, node)| (GroupId(idx), node)
);


/// Node and its output pin.
#[derive(Clone, Debug)]
pub struct OutPin {
    /// Output pin identifier.
    pub id: OutPinId,

    /// List of input pins connected to this output pin.
    pub remotes: Vec<InPinId>,
}

/// Node and its output pin.
#[derive(Clone, Debug)]
pub struct InPin {
    /// Input pin identifier.
    pub id: InPinId,

    /// List of output pins connected to this input pin.
    pub remotes: Vec<OutPinId>,
}

impl OutPin {
    fn new<T, G>(snarl: &Snarl<T, G>, pin: OutPinId) -> Self {
        OutPin {
            id: pin,
            remotes: snarl.wires.wired_inputs(pin).collect(),
        }
    }
}

impl InPin {
    fn new<T, G>(snarl: &Snarl<T, G>, pin: InPinId) -> Self {
        InPin {
            id: pin,
            remotes: snarl.wires.wired_outputs(pin).collect(),
        }
    }
}
