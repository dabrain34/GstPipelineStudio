// undo.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GraphManager
//
// SPDX-License-Identifier: GPL-3.0-only

//! Undo/Redo functionality for graph operations.
//!
//! This module provides a command pattern implementation for tracking and
//! reversing graph modifications. It maintains separate undo and redo stacks
//! with configurable depth limits.
//!
//! # Supported Operations
//!
//! The following graph operations are automatically tracked:
//!
//! - **Add/Remove Node**: Complete node state including all ports and properties
//! - **Add/Remove Link**: Connection data between nodes
//! - **Move Node**: Position changes from drag operations
//! - **Add/Remove Port**: Dynamic port modifications
//! - **Modify Property**: Node and port property changes with old/new values
//!
//! # API Usage
//!
//! From the application side, use [`GraphView`](super::GraphView) methods:
//!
//! ```ignore
//! // Perform undo/redo
//! graphview.undo();  // Returns true if action was undone
//! graphview.redo();  // Returns true if action was redone
//!
//! // Check availability
//! graphview.can_undo();
//! graphview.can_redo();
//!
//! // Manage history
//! graphview.clear_undo_history();
//! graphview.set_max_undo_depth(50);
//!
//! // Temporarily disable recording (e.g., during file load)
//! graphview.set_undo_recording(false);
//! // ... perform operations that shouldn't be recorded ...
//! graphview.set_undo_recording(true);
//! ```
//!
//! # Behavior Notes
//!
//! - New actions clear the redo stack (creates a new timeline)
//! - File load operations clear all history automatically
//! - Removing a node captures connected links for atomic restoration
//! - Maximum depth defaults to 100 operations

use super::{Node, NodeType, Port, PortDirection, PortPresence, PropertyExt};
use gtk::graphene;
use std::collections::{HashMap, VecDeque};

/// Maximum number of undo/redo operations to keep in history by default
const DEFAULT_MAX_UNDO_DEPTH: usize = 100;

/// Serialized node data for undo/redo operations
#[derive(Debug, Clone)]
pub struct NodeData {
    pub id: u32,
    pub name: String,
    pub node_type: NodeType,
    pub position: (f32, f32),
    pub light: bool,
    pub unique_name: String,
    pub properties: HashMap<String, String>,
    pub ports: Vec<PortData>,
}

impl NodeData {
    /// Create NodeData from a Node widget
    pub fn from_node(node: &Node) -> Self {
        let ports: Vec<PortData> = node.ports().values().map(PortData::from_port).collect();

        Self {
            id: node.id(),
            name: node.name(),
            node_type: node.node_type().cloned().unwrap_or(NodeType::Unknown),
            position: node.position(),
            light: node.light(),
            unique_name: node.unique_name(),
            properties: node.properties().clone(),
            ports,
        }
    }
}

/// Serialized port data for undo/redo operations
#[derive(Debug, Clone)]
pub struct PortData {
    pub id: u32,
    pub name: String,
    pub direction: PortDirection,
    pub presence: PortPresence,
    pub properties: HashMap<String, String>,
}

impl PortData {
    /// Create PortData from a Port widget
    pub fn from_port(port: &Port) -> Self {
        Self {
            id: port.id(),
            name: port.name(),
            direction: port.direction(),
            presence: port.presence(),
            properties: port.properties().clone(),
        }
    }
}

/// Serialized link data for undo/redo operations
#[derive(Debug, Clone)]
pub struct LinkData {
    pub id: u32,
    pub node_from: u32,
    pub node_to: u32,
    pub port_from: u32,
    pub port_to: u32,
    pub active: bool,
    pub name: String,
}

/// Represents a reversible action on the graph
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum UndoAction {
    /// A node was added to the graph
    AddNode {
        node_data: NodeData,
        position: graphene::Point,
    },
    /// A node was removed from the graph
    RemoveNode {
        node_data: NodeData,
        position: graphene::Point,
        /// Links that were connected to this node
        connected_links: Vec<LinkData>,
    },
    /// A link was added to the graph
    AddLink { link_data: LinkData },
    /// A link was removed from the graph
    RemoveLink { link_data: LinkData },
    /// A node was moved to a new position
    MoveNode {
        node_id: u32,
        old_position: graphene::Point,
        new_position: graphene::Point,
    },
    /// A port was added to a node
    AddPort { node_id: u32, port_data: PortData },
    /// A port was removed from a node
    RemovePort { node_id: u32, port_data: PortData },
    /// A property was modified
    ModifyProperty {
        node_id: u32,
        /// None for node properties, Some(port_id) for port properties
        port_id: Option<u32>,
        property_name: String,
        old_value: String,
        new_value: String,
    },
}

/// Manages undo/redo history for graph operations
pub struct UndoStack {
    /// Stack of actions that can be undone (front = oldest, back = newest)
    undo_stack: VecDeque<UndoAction>,
    /// Stack of actions that can be redone (front = oldest, back = newest)
    redo_stack: VecDeque<UndoAction>,
    /// Maximum number of actions to keep in history
    max_depth: usize,
    /// Flag to prevent recording actions during undo/redo
    recording_enabled: bool,
}

impl UndoStack {
    /// Create a new undo stack with default depth
    pub fn new() -> Self {
        Self::with_depth(DEFAULT_MAX_UNDO_DEPTH)
    }

    /// Create a new undo stack with specified maximum depth
    pub fn with_depth(max_depth: usize) -> Self {
        Self {
            undo_stack: VecDeque::with_capacity(max_depth),
            redo_stack: VecDeque::with_capacity(max_depth),
            max_depth,
            recording_enabled: true,
        }
    }

    /// Push an action onto the undo stack
    ///
    /// This clears the redo stack since we're creating a new timeline.
    pub fn push(&mut self, action: UndoAction) {
        if !self.recording_enabled {
            return;
        }

        // Clear redo stack when new action is performed
        self.redo_stack.clear();

        // Add to undo stack
        self.undo_stack.push_back(action);

        // Enforce max depth by removing oldest action (O(1) with VecDeque)
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.pop_front();
        }
    }

    /// Pop an action from the undo stack (most recent action)
    pub fn pop_undo(&mut self) -> Option<UndoAction> {
        self.undo_stack.pop_back()
    }

    /// Pop an action from the redo stack (most recent action)
    pub fn pop_redo(&mut self) -> Option<UndoAction> {
        self.redo_stack.pop_back()
    }

    /// Push an action onto the redo stack
    pub fn push_redo(&mut self, action: UndoAction) {
        self.redo_stack.push_back(action);
        // Enforce max depth (O(1) with VecDeque)
        if self.redo_stack.len() > self.max_depth {
            self.redo_stack.pop_front();
        }
    }

    /// Push an action directly onto the undo stack
    ///
    /// Unlike `push()`, this does not clear the redo stack.
    /// Used during redo operations.
    pub fn push_undo(&mut self, action: UndoAction) {
        self.undo_stack.push_back(action);
        // Enforce max depth (O(1) with VecDeque)
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.pop_front();
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all undo/redo history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Set maximum undo depth
    pub fn set_max_depth(&mut self, depth: usize) {
        self.max_depth = depth;
        // Trim stacks if needed (O(1) per removal with VecDeque)
        while self.undo_stack.len() > depth {
            self.undo_stack.pop_front();
        }
        while self.redo_stack.len() > depth {
            self.redo_stack.pop_front();
        }
    }

    /// Disable recording of new actions (used during undo/redo)
    pub fn disable_recording(&mut self) {
        self.recording_enabled = false;
    }

    /// Enable recording of new actions
    pub fn enable_recording(&mut self) {
        self.recording_enabled = true;
    }

    /// Get number of actions in undo stack
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get number of actions in redo stack
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}
