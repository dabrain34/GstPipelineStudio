// dot.rs
//
// Copyright 2025 Stéphane Cerveau <scerveau@igalia.com>
//
// This file is part of GraphManager
//
// SPDX-License-Identifier: GPL-3.0-only

//! DOT file parser and loader for graph visualization.
//!
//! This module provides generic DOT file parsing that can be customized
//! via the `DotLoader` trait for domain-specific enhancements.
//!
//! # Architecture
//!
//! The parser uses a trait-based design to separate generic parsing from
//! domain-specific logic:
//!
//! - **Generic layer** (this module): Parses DOT structure, identifies subgraphs,
//!   nodes, ports, and links based on configurable prefixes/suffixes.
//!
//! - **Domain layer** (via `DotLoader` trait): Provides domain-specific
//!   knowledge like node type detection, ID parsing, and metadata extraction.
//!
//! # Supported Conventions
//!
//! - Node subgraphs: Identified by `DotLoader::is_node_subgraph()` (default: "cluster_" prefix)
//! - Port subgraphs: Identified by `DotLoader::is_port_subgraph()` (domain-specific)
//! - Special nodes (legend, tracers, proxypads) are filtered out

use anyhow::{anyhow, Result};
use graphviz_rust::dot_structures::*;
use graphviz_rust::parse;
use log::{info, trace};
use std::collections::{HashMap, HashSet};

use super::node::NodeType;
use super::port::PortDirection;

/// Special node IDs to skip during parsing.
mod skip_nodes {
    pub const LEGEND: &str = "legend";
    pub const TRACERS: &str = "tracers";
    pub const PROXYPAD: &str = "proxypad";
}

/// Parsed node information from a DOT cluster
#[derive(Debug, Clone)]
pub struct DotNode {
    /// Original DOT cluster ID (includes memory address)
    pub dot_id: String,
    /// Instance name
    pub instance_name: String,
    /// Type name derived from class
    pub type_name: String,
    /// Node metadata (filtered for non-runtime values)
    /// May include "class_name" and "state" if parsed from the DOT label.
    pub metadata: HashMap<String, String>,
    /// Nesting depth (0 = top-level, 1+ = nested)
    pub depth: usize,
}

/// Parsed port information from a DOT node
#[derive(Debug, Clone)]
pub struct DotPort {
    /// Original DOT node ID
    pub dot_id: String,
    /// Port name
    pub name: String,
    /// Parent node's DOT cluster ID
    pub node_dot_id: String,
    /// Port direction (Input/Output)
    pub direction: PortDirection,
}

/// Parsed link information from a DOT edge
#[derive(Debug, Clone)]
pub struct DotLink {
    /// Source port DOT ID
    pub from_port_id: String,
    /// Target port DOT ID
    pub to_port_id: String,
    /// Capabilities/label (optional)
    #[allow(dead_code)]
    pub caps: Option<String>,
}

/// Result of parsing a DOT file
#[derive(Debug, Default)]
pub struct DotGraph {
    /// Top-level nodes only (depth = 0)
    pub nodes: Vec<DotNode>,
    /// Ports belonging to top-level nodes
    pub ports: Vec<DotPort>,
    /// Links between ports
    pub links: Vec<DotLink>,
    /// Metadata extracted from graph-level attributes.
    /// The loader decides which attributes to extract via `extract_graph_metadata()`.
    pub metadata: HashMap<String, String>,
}

/// Trait for customizing DOT loading behavior.
///
/// Implementations can provide domain-specific knowledge about nodes,
/// such as node types and available ports.
pub trait DotLoader {
    /// Get the node type for a given type name.
    /// Default: `NodeType::All`
    fn node_type(&self, _type_name: &str) -> NodeType {
        NodeType::All
    }

    /// Check if a node type exists in the system.
    /// Default: `true` (assume all nodes exist)
    fn node_exists(&self, _type_name: &str) -> bool {
        true
    }

    /// Get static port names for a node type.
    /// Returns (input_port_names, output_port_names).
    /// Default: empty vectors (use ports from DOT file only)
    fn get_static_ports(&self, _type_name: &str) -> (Vec<String>, Vec<String>) {
        (vec![], vec![])
    }

    /// Convert class name to type name.
    /// Default: lowercase the class name
    fn class_to_type_name(&self, class_name: &str) -> String {
        class_name.to_lowercase()
    }

    /// Parse a node label into metadata.
    /// Returns a HashMap that should contain at least "class_name" and "instance_name".
    /// May also contain "state" and other domain-specific metadata.
    /// Default: basic parsing that extracts lines as class_name and instance_name
    fn parse_node_label(&self, label: &str) -> HashMap<String, String> {
        // Default implementation: simple line-based parsing
        let label = label.trim_matches('"');
        let lines: Vec<&str> = label.lines().collect();

        let mut metadata = HashMap::new();
        metadata.insert(
            "class_name".to_string(),
            lines.first().unwrap_or(&"Unknown").to_string(),
        );
        metadata.insert(
            "instance_name".to_string(),
            lines.get(1).unwrap_or(&"unknown").to_string(),
        );
        metadata
    }

    /// Extract port name from DOT node ID.
    /// Default: None
    fn extract_port_name_from_id(&self, _dot_id: &str) -> Option<String> {
        None
    }

    /// Extract node instance name from a DOT port ID.
    /// Default: None
    fn extract_node_instance_from_id(&self, _port_id: &str) -> Option<String> {
        None
    }

    /// Extract metadata from graph-level attributes.
    /// Called with all graph-level attribute (name, value) pairs.
    /// Return a map of metadata key-value pairs to store.
    /// Default: empty map (no metadata extraction)
    fn extract_graph_metadata(&self, _attributes: &[(String, String)]) -> HashMap<String, String> {
        HashMap::new()
    }

    /// Check if a DOT subgraph ID represents a graph node.
    /// Default: checks for "cluster_" prefix (standard DOT convention)
    fn is_node_subgraph(&self, id: &str) -> bool {
        id.starts_with("cluster_")
    }

    /// Check if a DOT subgraph ID represents a port grouping.
    /// Default: false (no port subgraph detection)
    fn is_port_subgraph(&self, _id: &str) -> bool {
        false
    }
}

impl DotGraph {
    /// Maximum allowed DOT file size (10 MB) to prevent DoS via memory exhaustion
    const MAX_DOT_SIZE: usize = 10 * 1024 * 1024;

    /// Maximum allowed nesting depth to prevent DoS via deeply nested structures
    const MAX_NESTING_DEPTH: usize = 100;

    /// Parse a DOT file content into structured data using the provided loader
    pub fn parse<L: DotLoader>(content: &str, loader: &L) -> Result<Self> {
        // Validate size to prevent DoS attacks
        if content.len() > Self::MAX_DOT_SIZE {
            return Err(anyhow!(
                "DOT file too large: {} bytes (maximum allowed: {} bytes)",
                content.len(),
                Self::MAX_DOT_SIZE
            ));
        }

        let graph = parse(content)
            .map_err(|e| anyhow!("Failed to parse DOT file ({} bytes): {}", content.len(), e))?;

        let mut result = DotGraph::default();
        let mut all_nodes: Vec<DotNode> = Vec::new();
        let mut all_ports: Vec<DotPort> = Vec::new();

        // Parse the graph structure
        if let Graph::DiGraph { stmts, .. } = graph {
            // Extract graph-level attributes and let the loader decide which to keep as metadata
            let graph_attributes = Self::extract_graph_attributes(&stmts);
            result.metadata = loader.extract_graph_metadata(&graph_attributes);
            if !result.metadata.is_empty() {
                info!("DOT file metadata: {:?}", result.metadata);
            }

            Self::parse_statements(
                &stmts,
                &mut all_nodes,
                &mut all_ports,
                &mut result.links,
                0,
                loader,
            )?;
        }

        // Filter to only top-level nodes (depth = 0)
        result.nodes = all_nodes.into_iter().filter(|n| n.depth == 0).collect();

        // Filter ports to only those belonging to top-level nodes
        // Use HashSet for O(1) lookup instead of O(n) iteration through Vec
        let top_level_ids: HashSet<&str> = result.nodes.iter().map(|n| n.dot_id.as_str()).collect();
        result.ports = all_ports
            .into_iter()
            .filter(|p| top_level_ids.iter().any(|id| p.node_dot_id.starts_with(id)))
            .collect();

        // Infer port directions from edge analysis
        // DOT edges always go from src (output) to sink (input) in GStreamer
        Self::infer_port_directions(&mut result.ports, &result.links, &result.nodes, loader);

        // Log parsed nodes with class names for debugging
        for node in &result.nodes {
            trace!(
                "Node: {} (class={}, type={})",
                node.instance_name,
                node.metadata
                    .get("class_name")
                    .map(|s| s.as_str())
                    .unwrap_or("?"),
                node.type_name
            );
        }

        // Log parsed ports for debugging
        for port in &result.ports {
            trace!(
                "Port: {} on {} ({:?}) [{}]",
                port.name,
                port.node_dot_id,
                port.direction,
                port.dot_id
            );
        }

        info!(
            "Parsed DOT: {} nodes, {} ports, {} links",
            result.nodes.len(),
            result.ports.len(),
            result.links.len()
        );

        Ok(result)
    }

    /// Infer port directions from edge analysis and static port information.
    ///
    /// This is more reliable than color-based detection because:
    /// 1. DOT edges always go from src (output) to sink (input)
    /// 2. Static port info from node factories is authoritative
    /// 3. Port naming conventions (src*, sink*) provide fallback
    fn infer_port_directions<L: DotLoader>(
        ports: &mut [DotPort],
        links: &[DotLink],
        nodes: &[DotNode],
        loader: &L,
    ) {
        // Build a map from port DOT ID to port index for efficient lookup
        // Clone the keys to avoid borrowing from ports while we mutate it
        let port_id_to_index: HashMap<String, usize> = ports
            .iter()
            .enumerate()
            .map(|(idx, p)| (p.dot_id.clone(), idx))
            .collect();

        // Phase 1: Infer direction from edges
        // DOT edges always go: from_port (output) -> to_port (input)
        for link in links {
            if let Some(&idx) = port_id_to_index.get(&link.from_port_id) {
                ports[idx].direction = PortDirection::Output;
            }
            if let Some(&idx) = port_id_to_index.get(&link.to_port_id) {
                ports[idx].direction = PortDirection::Input;
            }
        }

        // Phase 2: For ports still unknown, use static port info from node factory
        // Build node type lookup map
        let node_types: HashMap<&str, &str> = nodes
            .iter()
            .map(|n| (n.dot_id.as_str(), n.type_name.as_str()))
            .collect();

        for port in ports.iter_mut() {
            if port.direction != PortDirection::Unknown {
                continue;
            }

            // Find the node this port belongs to
            let node_type = node_types
                .iter()
                .find(|(dot_id, _)| port.node_dot_id.starts_with(*dot_id))
                .map(|(_, type_name)| *type_name);

            if let Some(type_name) = node_type {
                let (inputs, outputs) = loader.get_static_ports(type_name);

                // Check if port name matches known input or output
                if inputs.iter().any(|name| name == &port.name) {
                    port.direction = PortDirection::Input;
                } else if outputs.iter().any(|name| name == &port.name) {
                    port.direction = PortDirection::Output;
                }
            }

            // Phase 3: Fall back to naming convention if still unknown
            if port.direction == PortDirection::Unknown {
                if port.name.starts_with("sink") {
                    port.direction = PortDirection::Input;
                } else if port.name.starts_with("src") {
                    port.direction = PortDirection::Output;
                }
            }
        }
    }

    fn parse_statements<L: DotLoader>(
        stmts: &[Stmt],
        nodes: &mut Vec<DotNode>,
        ports: &mut Vec<DotPort>,
        links: &mut Vec<DotLink>,
        depth: usize,
        loader: &L,
    ) -> Result<()> {
        // Validate nesting depth to prevent DoS via deeply nested structures
        if depth > Self::MAX_NESTING_DEPTH {
            return Err(anyhow!(
                "DOT file nesting too deep: depth {} exceeds maximum allowed depth {}",
                depth,
                Self::MAX_NESTING_DEPTH
            ));
        }

        for stmt in stmts {
            match stmt {
                Stmt::Subgraph(subgraph) => {
                    Self::parse_subgraph(subgraph, nodes, ports, links, depth, loader)?;
                }
                Stmt::Edge(edge) => {
                    if let Some(link) = Self::parse_edge(edge) {
                        links.push(link);
                    }
                }
                Stmt::Node(node) => {
                    // Nodes at this level are typically ports
                    if let Some(port) = Self::parse_port_node(node, "") {
                        ports.push(port);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_subgraph<L: DotLoader>(
        subgraph: &Subgraph,
        nodes: &mut Vec<DotNode>,
        ports: &mut Vec<DotPort>,
        links: &mut Vec<DotLink>,
        depth: usize,
        loader: &L,
    ) -> Result<()> {
        let id = match &subgraph.id {
            Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.clone(),
            Id::Anonymous(_) => return Ok(()),
        };

        // Check if this subgraph represents a graph node.
        // The loader determines the ID format conventions (prefixes, suffixes).
        let is_port_subgraph = loader.is_port_subgraph(&id);
        let is_node_subgraph = !is_port_subgraph && loader.is_node_subgraph(&id);

        if is_node_subgraph {
            // Parse node from subgraph
            if let Some(node) = Self::parse_node_subgraph(&id, &subgraph.stmts, depth, loader) {
                trace!("Found node: {} (depth={})", node.instance_name, depth);
                nodes.push(node.clone());

                // Parse nested content (ports and child nodes)
                for stmt in &subgraph.stmts {
                    match stmt {
                        Stmt::Subgraph(sub) => {
                            let sub_id = match &sub.id {
                                Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.clone(),
                                Id::Anonymous(_) => continue,
                            };

                            // Check for port subgraphs
                            if loader.is_port_subgraph(&sub_id) {
                                Self::parse_port_subgraph(sub, &id, ports);
                            } else if loader.is_node_subgraph(&sub_id) {
                                // Nested node - recurse with increased depth
                                Self::parse_subgraph(sub, nodes, ports, links, depth + 1, loader)?;
                            }
                        }
                        Stmt::Edge(edge) => {
                            if let Some(link) = Self::parse_edge(edge) {
                                links.push(link);
                            }
                        }
                        Stmt::Node(dot_node) => {
                            if let Some(port) = Self::parse_port_node(dot_node, &id) {
                                ports.push(port);
                            }
                        }
                        _ => {}
                    }
                }
            }
        } else {
            // Not a node cluster, recurse into statements
            Self::parse_statements(&subgraph.stmts, nodes, ports, links, depth, loader)?;
        }
        Ok(())
    }

    fn parse_node_subgraph<L: DotLoader>(
        cluster_id: &str,
        stmts: &[Stmt],
        depth: usize,
        loader: &L,
    ) -> Option<DotNode> {
        // Find the label attribute (required for basic info)
        let label = Self::find_attribute(stmts, "label")?;

        // Prefer tooltip over label for metadata since tooltip contains full values
        // (label may have truncated property values like "location=...path…")
        let tooltip = Self::find_attribute(stmts, "tooltip");
        let content_for_metadata = tooltip.as_ref().unwrap_or(&label);

        // Parse label for class/instance (needed for skip logic and type_name)
        let label_metadata = loader.parse_node_label(&label);
        let class_name = label_metadata
            .get("class_name")
            .cloned()
            .unwrap_or_default();
        let instance_name = label_metadata
            .get("instance_name")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        // Skip the legend node and nodes with missing class names
        if instance_name == "Legend" {
            trace!("Skipping legend node in cluster: {}", cluster_id);
            return None;
        }
        if class_name.is_empty() {
            trace!(
                "Skipping node with empty class name (cluster: {}, instance: {})",
                cluster_id,
                instance_name
            );
            return None;
        }

        // Parse tooltip (or label) for full metadata
        let mut metadata = loader.parse_node_label(content_for_metadata);

        // Ensure class_name is in metadata (may come from label if tooltip lacks it)
        if !metadata.contains_key("class_name") {
            metadata.insert("class_name".to_string(), class_name.clone());
        }

        // Convert class name to type name
        let type_name = loader.class_to_type_name(&class_name);

        Some(DotNode {
            dot_id: cluster_id.to_string(),
            instance_name,
            type_name,
            metadata,
            depth,
        })
    }

    fn parse_port_subgraph(subgraph: &Subgraph, element_id: &str, ports: &mut Vec<DotPort>) {
        for stmt in &subgraph.stmts {
            if let Stmt::Node(node) = stmt {
                if let Some(mut port) = Self::parse_port_node(node, element_id) {
                    // Override element_id since we know the parent
                    port.node_dot_id = element_id.to_string();
                    ports.push(port);
                }
            }
        }
    }

    fn parse_port_node(
        node: &graphviz_rust::dot_structures::Node,
        element_id: &str,
    ) -> Option<DotPort> {
        let node_id = match &node.id.0 {
            Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.clone(),
            Id::Anonymous(_) => return None,
        };

        // Skip non-port nodes (like legend, tracers)
        // Port nodes contain memory addresses (0x...) and are not special nodes
        if node_id == skip_nodes::LEGEND
            || node_id == skip_nodes::TRACERS
            || !node_id.contains("0x")
        {
            return None;
        }

        // Skip proxypad nodes - these are internal implementation details, not actual ports
        // Proxypad IDs can appear in various formats:
        // - Old format: "_proxypad0_0x5d75d6508d70" or "proxypad0_0x..."
        // - New format: "_node_proxypad0_0x..." or "node_proxypad0_0x..."
        if node_id.contains(skip_nodes::PROXYPAD) {
            return None;
        }

        // Find label attribute
        let mut label = None;

        for attr in &node.attributes {
            let attr_name = match &attr.0 {
                Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.as_str(),
                Id::Anonymous(_) => continue,
            };
            let attr_value = match &attr.1 {
                Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.clone(),
                Id::Anonymous(_) => continue,
            };

            if attr_name == "label" {
                label = Some(attr_value);
                break;
            }
        }

        let label = label?;

        // Parse port name from label (first line before \n)
        // Handle both actual newlines and literal "\n" (backslash-n) from DOT files
        let port_name = label
            .split("\\n")
            .next()
            .and_then(|s| s.split('\n').next())?
            .trim_matches('"')
            .to_string();

        // Direction will be inferred from edge analysis after parsing
        Some(DotPort {
            dot_id: node_id,
            name: port_name,
            node_dot_id: element_id.to_string(),
            direction: PortDirection::Unknown,
        })
    }

    fn parse_edge(edge: &Edge) -> Option<DotLink> {
        // Get source and target IDs
        match &edge.ty {
            EdgeTy::Pair(from, to) => {
                let from_id = Self::vertex_id(from)?;
                let to_id = Self::vertex_id(to)?;

                // Extract caps and style from attributes
                let mut caps = None;
                let mut is_invisible = false;

                for attr in &edge.attributes {
                    let attr_name = match &attr.0 {
                        Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.as_str(),
                        Id::Anonymous(_) => continue,
                    };
                    let attr_value = match &attr.1 {
                        Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.clone(),
                        Id::Anonymous(_) => continue,
                    };

                    match attr_name {
                        "label" => caps = Some(attr_value),
                        "style" => {
                            // Skip invisible edges - these are layout hints, not real connections
                            if attr_value.contains("invis") {
                                is_invisible = true;
                            }
                        }
                        _ => {}
                    }
                }

                // Skip invisible (layout) edges
                if is_invisible {
                    return None;
                }

                // Skip edges involving proxypad nodes (internal implementation details)
                if from_id.contains("proxypad") || to_id.contains("proxypad") {
                    return None;
                }

                Some(DotLink {
                    from_port_id: from_id,
                    to_port_id: to_id,
                    caps,
                })
            }
            EdgeTy::Chain(_) => None,
        }
    }

    fn vertex_id(vertex: &Vertex) -> Option<String> {
        match vertex {
            Vertex::N(node_id) => match &node_id.0 {
                Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => Some(s.clone()),
                Id::Anonymous(_) => None,
            },
            Vertex::S(_) => None,
        }
    }

    /// Extract all graph-level attributes as (name, value) pairs.
    fn extract_graph_attributes(stmts: &[Stmt]) -> Vec<(String, String)> {
        let mut attributes = Vec::new();
        for stmt in stmts {
            if let Stmt::Attribute(attr) = stmt {
                let attr_name = match &attr.0 {
                    Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.clone(),
                    Id::Anonymous(_) => continue,
                };
                let attr_value = match &attr.1 {
                    Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.trim_matches('"').to_string(),
                    Id::Anonymous(_) => continue,
                };
                attributes.push((attr_name, attr_value));
            }
        }
        attributes
    }

    fn find_attribute(stmts: &[Stmt], name: &str) -> Option<String> {
        for stmt in stmts {
            if let Stmt::Attribute(attr) = stmt {
                let attr_name = match &attr.0 {
                    Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => s.as_str(),
                    Id::Anonymous(_) => continue,
                };
                if attr_name == name {
                    return match &attr.1 {
                        Id::Plain(s) | Id::Escaped(s) | Id::Html(s) => Some(s.clone()),
                        Id::Anonymous(_) => None,
                    };
                }
            }
        }
        None
    }
}

// Tests: graphmanager/test.rs
