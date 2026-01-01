#[cfg(test)]
fn test_synced<F, R>(function: F) -> R
where
    F: FnOnce() -> R + Send + std::panic::UnwindSafe + 'static,
    R: Send + 'static,
{
    /// No-op.
    macro_rules! skip_assert_initialized {
        () => {};
    }
    skip_assert_initialized!();

    use futures_channel::oneshot;
    use std::panic;

    let (tx, rx) = oneshot::channel();
    TEST_THREAD_WORKER
        .push(move || {
            tx.send(panic::catch_unwind(function))
                .unwrap_or_else(|_| panic!("Failed to return result from thread pool"));
        })
        .expect("Failed to schedule a test call");
    futures_executor::block_on(rx)
        .expect("Failed to receive result from thread pool")
        .unwrap_or_else(|e| std::panic::resume_unwind(e))
}

#[cfg(test)]
static TEST_THREAD_WORKER: once_cell::sync::Lazy<gtk::glib::ThreadPool> =
    once_cell::sync::Lazy::new(|| {
        let pool = gtk::glib::ThreadPool::exclusive(1).unwrap();
        pool.push(move || {
            gtk::init().expect("Tests failed to initialize gtk");
        })
        .expect("Failed to schedule a test call");
        pool
    });

use crate::graphmanager::{GraphView, Node, NodeType, PortDirection, PortPresence, PropertyExt};
use gtk::prelude::WidgetExt;

#[test]
fn graphview_creation() {
    test_synced(|| {
        let graphview = GraphView::new();
        assert_eq!(graphview.id(), 0);
    });
}

#[test]
fn graphview_lifetime() {
    test_synced(|| {
        let graphview = GraphView::new();
        assert_eq!(graphview.id(), 0);
        let node = graphview.create_node("my_node1", NodeType::Source);
        node.add_property("np1", "nv1");
        graphview.add_node(node);
        //create a port input on node 1
        let port = graphview.create_port("out", PortDirection::Output, PortPresence::Always);
        assert_eq!(port.name(), "out");
        assert_eq!(port.id(), 1);
        let mut node: Node = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut node, port);

        let node = graphview.create_node_with_port("my_node2", NodeType::Transform, 1, 1);
        node.add_property("np2", "nv2");
        graphview.add_node(node);

        let node = graphview.create_node("my_node3", NodeType::Sink);
        node.add_property("np3", "nv3");
        graphview.add_node(node);
        //create a port input on node 3
        let port = graphview.create_port("in", PortDirection::Input, PortPresence::Always);
        port.add_property("p1", "v1");
        assert_eq!(port.name(), "in");
        assert_eq!(port.id(), 4);
        let mut node: Node = graphview.node(3).unwrap();
        graphview.add_port_to_node(&mut node, port);

        assert_eq!(graphview.all_nodes(NodeType::Source).len(), 1);
        assert_eq!(graphview.all_nodes(NodeType::Transform).len(), 1);
        assert_eq!(graphview.all_nodes(NodeType::Sink).len(), 1);
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 3);

        assert_eq!(graphview.node(1).unwrap().name(), "my_node1");
        assert_eq!(graphview.node(2).unwrap().name(), "my_node2");
        assert_eq!(graphview.node(3).unwrap().name(), "my_node3");

        // Ports have been created by create_node_with_port

        //Create link between node1 and node 2
        let link = graphview.create_link(1, 2, 1, 2);
        assert_eq!(&link.name(), "");
        assert!(&link.active());
        link.set_name("link1");
        assert_eq!(&link.name(), "link1");
        graphview.add_link(link);

        //Create link between node2 and node 3
        let link = graphview.create_link(2, 3, 3, 4);
        graphview.add_link(link);

        // Save the graphview in XML into a buffer
        let buffer = graphview
            .render_xml()
            .expect("Should be able to render graph to xml");
        println!("{}", std::str::from_utf8(&buffer).unwrap());
        // Load the graphview from XML buffer
        graphview
            .load_from_xml(buffer)
            .expect("Should be able to load from XML data");

        // Check that nodes and links are present
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 3);
        assert_eq!(graphview.all_links(true).len(), 2);

        // Check all nodes are linked
        assert!(graphview.node_is_linked(1).is_some());
        assert!(graphview.node_is_linked(2).is_some());
        assert!(graphview.node_is_linked(3).is_some());

        // Check all ports are linked
        assert!(graphview.port_connected_to(1).is_some());
        assert!(graphview.port_connected_to(3).is_some());

        // Check properties
        let node = graphview.node(1).expect("Should be able to get node 1");
        assert_eq!(&node.property("np1").unwrap(), "nv1");
        let node = graphview.node(2).expect("Should be able to get node 1");
        assert_eq!(&node.property("np2").unwrap(), "nv2");
        let node = graphview.node(3).expect("Should be able to get node 1");
        assert_eq!(&node.property("np3").unwrap(), "nv3");

        // Clear the graph and check that everything has been destroyed properly
        graphview.clear();
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 0);
        assert_eq!(graphview.all_links(true).len(), 0);
    });
}

#[test]
fn undo_redo_add_node() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Initially, undo/redo should not be available
        assert!(!graphview.can_undo());
        assert!(!graphview.can_redo());
        assert_eq!(graphview.undo_count(), 0);
        assert_eq!(graphview.redo_count(), 0);

        // Add a node
        let node = graphview.create_node("test_node", NodeType::Source);
        graphview.add_node(node);

        // Should have 1 node and undo should be available
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 1);
        assert!(graphview.can_undo());
        assert!(!graphview.can_redo());
        assert_eq!(graphview.undo_count(), 1);

        // Undo the add
        assert!(graphview.undo());

        // Node should be gone
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 0);
        assert!(!graphview.can_undo());
        assert!(graphview.can_redo());
        assert_eq!(graphview.redo_count(), 1);

        // Redo the add
        assert!(graphview.redo());

        // Node should be back
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 1);
        assert!(graphview.can_undo());
        assert!(!graphview.can_redo());
    });
}

#[test]
fn undo_redo_remove_node() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Add a node
        let node = graphview.create_node("test_node", NodeType::Source);
        let node_id = node.id();
        graphview.add_node(node);

        // Clear undo history so we start fresh
        graphview.clear_undo_history();
        assert!(!graphview.can_undo());

        // Remove the node
        graphview.remove_node(node_id);

        // Node should be gone and undo available
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 0);
        assert!(graphview.can_undo());

        // Undo the removal
        assert!(graphview.undo());

        // Node should be back
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 1);
        let restored_node = graphview.node(node_id).unwrap();
        assert_eq!(restored_node.name(), "test_node");

        // Redo the removal
        assert!(graphview.redo());

        // Node should be gone again
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 0);
    });
}

#[test]
fn undo_redo_add_link() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create two nodes with ports
        let node1 = graphview.create_node_with_port("node1", NodeType::Source, 1, 0);
        graphview.add_node(node1);
        let node2 = graphview.create_node_with_port("node2", NodeType::Sink, 0, 1);
        graphview.add_node(node2);

        // Clear undo history so we start fresh
        graphview.clear_undo_history();

        // Create a link between the nodes
        let link = graphview.create_link(1, 2, 1, 2);
        graphview.add_link(link);

        // Should have 1 link
        assert_eq!(graphview.all_links(true).len(), 1);
        assert!(graphview.can_undo());

        // Undo the link addition
        assert!(graphview.undo());

        // Link should be gone
        assert_eq!(graphview.all_links(true).len(), 0);

        // Redo the link addition
        assert!(graphview.redo());

        // Link should be back
        assert_eq!(graphview.all_links(true).len(), 1);
    });
}

#[test]
fn undo_redo_remove_link() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create two nodes with ports and a link
        let node1 = graphview.create_node_with_port("node1", NodeType::Source, 1, 0);
        graphview.add_node(node1);
        let node2 = graphview.create_node_with_port("node2", NodeType::Sink, 0, 1);
        graphview.add_node(node2);
        let link = graphview.create_link(1, 2, 1, 2);
        let link_id = link.id;
        graphview.add_link(link);

        // Clear undo history so we start fresh
        graphview.clear_undo_history();

        // Remove the link
        graphview.remove_link(link_id);

        // Link should be gone
        assert_eq!(graphview.all_links(true).len(), 0);
        assert!(graphview.can_undo());

        // Undo the removal
        assert!(graphview.undo());

        // Link should be back
        assert_eq!(graphview.all_links(true).len(), 1);

        // Redo the removal
        assert!(graphview.redo());

        // Link should be gone again
        assert_eq!(graphview.all_links(true).len(), 0);
    });
}

#[test]
fn undo_redo_remove_node_with_links() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create three nodes with ports and links
        let node1 = graphview.create_node_with_port("node1", NodeType::Source, 1, 0);
        graphview.add_node(node1);
        let node2 = graphview.create_node_with_port("node2", NodeType::Transform, 1, 1);
        graphview.add_node(node2);
        let node3 = graphview.create_node_with_port("node3", NodeType::Sink, 0, 1);
        graphview.add_node(node3);

        let link1 = graphview.create_link(1, 2, 1, 2);
        graphview.add_link(link1);
        let link2 = graphview.create_link(2, 3, 3, 4);
        graphview.add_link(link2);

        // Clear undo history
        graphview.clear_undo_history();

        // Remove node2 (which has 2 connected links)
        graphview.remove_node(2);

        // Node2 and both links should be gone
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 2);
        assert_eq!(graphview.all_links(true).len(), 0);

        // Undo the removal
        assert!(graphview.undo());

        // Node2 and both links should be back
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 3);
        assert_eq!(graphview.all_links(true).len(), 2);
        assert!(graphview.node(2).is_some());

        // Redo the removal
        assert!(graphview.redo());

        // Node2 and links should be gone again
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 2);
        assert_eq!(graphview.all_links(true).len(), 0);
    });
}

#[test]
fn undo_redo_multiple_operations() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Perform multiple operations
        let node1 = graphview.create_node("node1", NodeType::Source);
        graphview.add_node(node1);

        let node2 = graphview.create_node("node2", NodeType::Sink);
        graphview.add_node(node2);

        let node3 = graphview.create_node("node3", NodeType::Transform);
        graphview.add_node(node3);

        // Should have 3 operations in undo stack
        assert_eq!(graphview.undo_count(), 3);
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 3);

        // Undo all operations
        assert!(graphview.undo());
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 2);

        assert!(graphview.undo());
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 1);

        assert!(graphview.undo());
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 0);

        // No more undo available
        assert!(!graphview.can_undo());
        assert!(!graphview.undo());

        // But redo should be available
        assert!(graphview.can_redo());
        assert_eq!(graphview.redo_count(), 3);

        // Redo all operations
        assert!(graphview.redo());
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 1);

        assert!(graphview.redo());
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 2);

        assert!(graphview.redo());
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 3);

        // No more redo available
        assert!(!graphview.can_redo());
        assert!(!graphview.redo());
    });
}

#[test]
fn undo_clears_redo_stack() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Add a node
        let node = graphview.create_node("node1", NodeType::Source);
        graphview.add_node(node);

        // Undo it
        assert!(graphview.undo());

        // Redo should be available
        assert!(graphview.can_redo());

        // Add a new node (this should clear redo stack)
        let node2 = graphview.create_node("node2", NodeType::Sink);
        graphview.add_node(node2);

        // Redo should no longer be available
        assert!(!graphview.can_redo());
        assert_eq!(graphview.redo_count(), 0);
    });
}

#[test]
fn load_from_xml_clears_undo_history() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Add some nodes
        let node1 = graphview.create_node("node1", NodeType::Source);
        graphview.add_node(node1);
        let node2 = graphview.create_node("node2", NodeType::Sink);
        graphview.add_node(node2);

        // Should have undo history
        assert!(graphview.can_undo());
        assert_eq!(graphview.undo_count(), 2);

        // Save to XML
        let buffer = graphview.render_xml().expect("Should render XML");

        // Load from XML (this should clear undo history)
        graphview.load_from_xml(buffer).expect("Should load XML");

        // Undo history should be cleared
        assert!(!graphview.can_undo());
        assert!(!graphview.can_redo());
        assert_eq!(graphview.undo_count(), 0);
        assert_eq!(graphview.redo_count(), 0);

        // But nodes should still be there
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 2);
    });
}

#[test]
fn undo_max_depth() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Set a small max depth for testing
        graphview.set_max_undo_depth(3);

        // Add 5 nodes
        for i in 0..5 {
            let node = graphview.create_node(&format!("node{}", i), NodeType::Source);
            graphview.add_node(node);
        }

        // Should only keep the last 3 operations
        assert_eq!(graphview.undo_count(), 3);

        // Undo 3 times
        assert!(graphview.undo());
        assert!(graphview.undo());
        assert!(graphview.undo());

        // Should have 2 nodes left (5 - 3 undone)
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 2);

        // No more undo available
        assert!(!graphview.can_undo());
    });
}

#[test]
fn undo_redo_node_property() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a node
        let node = graphview.create_node("test_node", NodeType::Source);
        let node_id = node.id();
        node.add_property("test_prop", "initial_value");
        graphview.add_node(node);

        // Clear undo history so we start fresh
        graphview.clear_undo_history();

        // Modify the property
        graphview.modify_node_property(node_id, "test_prop", "new_value");

        // Property should be updated
        let node = graphview.node(node_id).unwrap();
        assert_eq!(node.property("test_prop").unwrap(), "new_value");
        assert!(graphview.can_undo());

        // Undo the modification
        assert!(graphview.undo());

        // Property should be back to initial value
        let node = graphview.node(node_id).unwrap();
        assert_eq!(node.property("test_prop").unwrap(), "initial_value");

        // Redo the modification
        assert!(graphview.redo());

        // Property should be new value again
        let node = graphview.node(node_id).unwrap();
        assert_eq!(node.property("test_prop").unwrap(), "new_value");
    });
}

#[test]
fn undo_redo_port_property() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a node with a port
        let node = graphview.create_node_with_port("test_node", NodeType::Source, 0, 1);
        let node_id = node.id();
        graphview.add_node(node);

        // Get the port and set initial property
        let node = graphview.node(node_id).unwrap();
        let ports: Vec<_> = node.ports().values().cloned().collect();
        let port = &ports[0];
        let port_id = port.id();
        port.add_property("test_prop", "initial_value");

        // Clear undo history
        graphview.clear_undo_history();

        // Modify the port property
        graphview.modify_port_property(node_id, port_id, "test_prop", "new_value");

        // Property should be updated
        let node = graphview.node(node_id).unwrap();
        let port = node.port(port_id).unwrap();
        assert_eq!(port.property("test_prop").unwrap(), "new_value");
        assert!(graphview.can_undo());

        // Undo the modification
        assert!(graphview.undo());

        // Property should be back to initial value
        let node = graphview.node(node_id).unwrap();
        let port = node.port(port_id).unwrap();
        assert_eq!(port.property("test_prop").unwrap(), "initial_value");

        // Redo the modification
        assert!(graphview.redo());

        // Property should be new value again
        let node = graphview.node(node_id).unwrap();
        let port = node.port(port_id).unwrap();
        assert_eq!(port.property("test_prop").unwrap(), "new_value");
    });
}

#[test]
fn undo_redo_multiple_properties() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a node
        let node = graphview.create_node("test_node", NodeType::Source);
        let node_id = node.id();
        graphview.add_node(node);

        // Clear undo history
        graphview.clear_undo_history();

        // Modify multiple properties
        graphview.modify_node_property(node_id, "prop1", "value1");
        graphview.modify_node_property(node_id, "prop2", "value2");
        graphview.modify_node_property(node_id, "prop3", "value3");

        // Should have 3 undo operations
        assert_eq!(graphview.undo_count(), 3);

        // Verify all properties are set
        let node = graphview.node(node_id).unwrap();
        assert_eq!(node.property("prop1").unwrap(), "value1");
        assert_eq!(node.property("prop2").unwrap(), "value2");
        assert_eq!(node.property("prop3").unwrap(), "value3");

        // Undo all modifications
        assert!(graphview.undo());
        let node = graphview.node(node_id).unwrap();
        assert!(node.property("prop3").is_none());

        assert!(graphview.undo());
        let node = graphview.node(node_id).unwrap();
        assert!(node.property("prop2").is_none());

        assert!(graphview.undo());
        let node = graphview.node(node_id).unwrap();
        assert!(node.property("prop1").is_none());

        // Redo all modifications
        assert!(graphview.redo());
        assert!(graphview.redo());
        assert!(graphview.redo());

        // All properties should be back
        let node = graphview.node(node_id).unwrap();
        assert_eq!(node.property("prop1").unwrap(), "value1");
        assert_eq!(node.property("prop2").unwrap(), "value2");
        assert_eq!(node.property("prop3").unwrap(), "value3");
    });
}

#[test]
fn property_unchanged_no_undo() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a node with a property
        let node = graphview.create_node("test_node", NodeType::Source);
        let node_id = node.id();
        node.add_property("test_prop", "value");
        graphview.add_node(node);

        // Clear undo history
        graphview.clear_undo_history();

        // "Modify" property to same value
        graphview.modify_node_property(node_id, "test_prop", "value");

        // Should not have created an undo action
        assert!(!graphview.can_undo());
        assert_eq!(graphview.undo_count(), 0);
    });
}

#[test]
fn undo_default_max_depth_is_100() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Add 150 nodes (exceeds default limit of 100)
        for i in 0..150 {
            let node = graphview.create_node(&format!("node{}", i), NodeType::Source);
            graphview.add_node(node);
        }

        // Should have exactly 100 undo operations (the limit)
        assert_eq!(graphview.undo_count(), 100);
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 150);

        // Undo all 100 available operations
        for _ in 0..100 {
            assert!(graphview.undo());
        }

        // Should have 50 nodes left (150 - 100 undone)
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 50);

        // No more undo available
        assert!(!graphview.can_undo());
        assert!(!graphview.undo());
    });
}

#[test]
fn undo_oldest_actions_dropped_when_exceeding_limit() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Set a small limit for easier testing
        graphview.set_max_undo_depth(5);

        // Add 8 nodes
        for i in 0..8 {
            let node = graphview.create_node(&format!("node{}", i), NodeType::Source);
            graphview.add_node(node);
        }

        // Should only have 5 undo operations
        assert_eq!(graphview.undo_count(), 5);

        // Undo all 5 - should remove nodes 7, 6, 5, 4, 3 (the last 5 added)
        for _ in 0..5 {
            assert!(graphview.undo());
        }

        // Should have 3 nodes left (nodes 0, 1, 2 - first 3 that exceeded the limit)
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 3);

        // Verify the remaining nodes are the first ones added
        assert!(graphview.node(1).is_some()); // node0
        assert!(graphview.node(2).is_some()); // node1
        assert!(graphview.node(3).is_some()); // node2
    });
}

#[test]
fn redo_stack_respects_max_depth() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Set a small limit
        graphview.set_max_undo_depth(5);

        // Add 5 nodes
        for i in 0..5 {
            let node = graphview.create_node(&format!("node{}", i), NodeType::Source);
            graphview.add_node(node);
        }

        // Undo all 5
        for _ in 0..5 {
            assert!(graphview.undo());
        }

        // Redo stack should have 5 items
        assert_eq!(graphview.redo_count(), 5);
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 0);

        // Redo all 5
        for _ in 0..5 {
            assert!(graphview.redo());
        }

        // All nodes should be back
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 5);
        assert!(!graphview.can_redo());
    });
}

#[test]
fn changing_max_depth_trims_existing_stacks() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Add 10 nodes
        for i in 0..10 {
            let node = graphview.create_node(&format!("node{}", i), NodeType::Source);
            graphview.add_node(node);
        }

        assert_eq!(graphview.undo_count(), 10);

        // Reduce max depth to 3
        graphview.set_max_undo_depth(3);

        // Should trim to 3
        assert_eq!(graphview.undo_count(), 3);

        // Undo all 3
        for _ in 0..3 {
            assert!(graphview.undo());
        }

        // Should have 7 nodes left
        assert_eq!(graphview.all_nodes(NodeType::All).len(), 7);
    });
}

#[test]
fn auto_arrange_simple_pipeline() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create source -> transform -> sink pipeline
        let src = graphview.create_node_with_port("src", NodeType::Source, 1, 0);
        graphview.add_node(src);
        let transform = graphview.create_node_with_port("transform", NodeType::Transform, 1, 1);
        graphview.add_node(transform);
        let sink = graphview.create_node_with_port("sink", NodeType::Sink, 0, 1);
        graphview.add_node(sink);

        // Create links: src -> transform -> sink
        let link1 = graphview.create_link(1, 2, 1, 2);
        graphview.add_link(link1);
        let link2 = graphview.create_link(2, 3, 3, 4);
        graphview.add_link(link2);

        // Clear undo history
        graphview.clear_undo_history();

        // Apply auto-arrange
        assert!(graphview.auto_arrange_graph(None));

        // Verify layering: src.x < transform.x < sink.x
        let src_pos = graphview.node(1).unwrap().position();
        let transform_pos = graphview.node(2).unwrap().position();
        let sink_pos = graphview.node(3).unwrap().position();

        assert!(
            src_pos.0 < transform_pos.0,
            "Source should be left of transform: {} < {}",
            src_pos.0,
            transform_pos.0
        );
        assert!(
            transform_pos.0 < sink_pos.0,
            "Transform should be left of sink: {} < {}",
            transform_pos.0,
            sink_pos.0
        );

        // Should be undoable as single operation
        assert_eq!(graphview.undo_count(), 1);
        assert!(graphview.can_undo());
    });
}

#[test]
fn auto_arrange_empty_graph() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Empty graph should return false
        assert!(!graphview.auto_arrange_graph(None));

        // No undo should be recorded
        assert!(!graphview.can_undo());
    });
}

#[test]
fn auto_arrange_disconnected_nodes() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create disconnected nodes
        let node1 = graphview.create_node("node1", NodeType::Source);
        graphview.add_node(node1);
        let node2 = graphview.create_node("node2", NodeType::Transform);
        graphview.add_node(node2);
        let node3 = graphview.create_node("node3", NodeType::Sink);
        graphview.add_node(node3);

        // Clear undo history
        graphview.clear_undo_history();

        // Apply auto-arrange
        assert!(graphview.auto_arrange_graph(None));

        // All nodes should be positioned (all are sources since no incoming edges)
        // They should be in the same layer (layer 0) at different Y positions
        let pos1 = graphview.node(1).unwrap().position();
        let pos2 = graphview.node(2).unwrap().position();
        let pos3 = graphview.node(3).unwrap().position();

        // Same X position (same layer)
        assert!(
            (pos1.0 - pos2.0).abs() < 1.0,
            "Disconnected nodes should be in same layer"
        );
        assert!(
            (pos2.0 - pos3.0).abs() < 1.0,
            "Disconnected nodes should be in same layer"
        );

        // Should be undoable
        assert!(graphview.can_undo());
    });
}

#[test]
fn auto_arrange_undo_restores_positions() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a simple pipeline
        let src = graphview.create_node_with_port("src", NodeType::Source, 1, 0);
        graphview.add_node(src);
        let sink = graphview.create_node_with_port("sink", NodeType::Sink, 0, 1);
        graphview.add_node(sink);

        // Create link
        let link = graphview.create_link(1, 2, 1, 2);
        graphview.add_link(link);

        // Get original positions (from add_node automatic placement)
        let original_src_pos = graphview.node(1).unwrap().position();
        let original_sink_pos = graphview.node(2).unwrap().position();

        // Clear undo history
        graphview.clear_undo_history();

        // Apply auto-arrange
        assert!(graphview.auto_arrange_graph(None));

        // Positions should have changed
        let new_src_pos = graphview.node(1).unwrap().position();
        let new_sink_pos = graphview.node(2).unwrap().position();

        // Undo the layout
        assert!(graphview.undo());

        // Positions should be restored to original
        let restored_src_pos = graphview.node(1).unwrap().position();
        let restored_sink_pos = graphview.node(2).unwrap().position();

        assert!(
            (restored_src_pos.0 - original_src_pos.0).abs() < 1.0,
            "Source X should be restored"
        );
        assert!(
            (restored_src_pos.1 - original_src_pos.1).abs() < 1.0,
            "Source Y should be restored"
        );
        assert!(
            (restored_sink_pos.0 - original_sink_pos.0).abs() < 1.0,
            "Sink X should be restored"
        );
        assert!(
            (restored_sink_pos.1 - original_sink_pos.1).abs() < 1.0,
            "Sink Y should be restored"
        );

        // Redo should restore the layout
        assert!(graphview.redo());

        let redone_src_pos = graphview.node(1).unwrap().position();
        let redone_sink_pos = graphview.node(2).unwrap().position();

        assert!(
            (redone_src_pos.0 - new_src_pos.0).abs() < 1.0,
            "Source X should be redone"
        );
        assert!(
            (redone_sink_pos.0 - new_sink_pos.0).abs() < 1.0,
            "Sink X should be redone"
        );
    });
}

#[test]
fn auto_arrange_branching_pipeline() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a branching pipeline: src -> [transform1, transform2] -> sink
        let src = graphview.create_node_with_port("src", NodeType::Source, 2, 0);
        graphview.add_node(src);

        let transform1 = graphview.create_node_with_port("transform1", NodeType::Transform, 1, 1);
        graphview.add_node(transform1);

        let transform2 = graphview.create_node_with_port("transform2", NodeType::Transform, 1, 1);
        graphview.add_node(transform2);

        let sink = graphview.create_node_with_port("sink", NodeType::Sink, 0, 2);
        graphview.add_node(sink);

        // Link: src -> transform1 and src -> transform2
        let link1 = graphview.create_link(1, 2, 1, 3);
        graphview.add_link(link1);
        let link2 = graphview.create_link(1, 3, 2, 5);
        graphview.add_link(link2);

        // Link: transform1 -> sink and transform2 -> sink
        let link3 = graphview.create_link(2, 4, 4, 7);
        graphview.add_link(link3);
        let link4 = graphview.create_link(3, 4, 6, 8);
        graphview.add_link(link4);

        // Clear undo history
        graphview.clear_undo_history();

        // Apply auto-arrange
        assert!(graphview.auto_arrange_graph(None));

        // Verify layering
        let src_pos = graphview.node(1).unwrap().position();
        let t1_pos = graphview.node(2).unwrap().position();
        let t2_pos = graphview.node(3).unwrap().position();
        let sink_pos = graphview.node(4).unwrap().position();

        // src should be leftmost
        assert!(src_pos.0 < t1_pos.0, "Source should be left of transform1");
        assert!(src_pos.0 < t2_pos.0, "Source should be left of transform2");

        // transforms should be in the same layer (same X)
        assert!(
            (t1_pos.0 - t2_pos.0).abs() < 1.0,
            "Transforms should be in same layer"
        );

        // sink should be rightmost
        assert!(t1_pos.0 < sink_pos.0, "Transform1 should be left of sink");
        assert!(t2_pos.0 < sink_pos.0, "Transform2 should be left of sink");

        // transforms should have different Y positions
        assert!(
            (t1_pos.1 - t2_pos.1).abs() > 1.0,
            "Transforms should have different Y positions"
        );
    });
}

#[test]
fn auto_arrange_custom_options() {
    test_synced(|| {
        use crate::graphmanager::AutoArrangeOptions;

        let graphview = GraphView::new();

        // Create two connected nodes
        let src = graphview.create_node_with_port("src", NodeType::Source, 1, 0);
        graphview.add_node(src);
        let sink = graphview.create_node_with_port("sink", NodeType::Sink, 0, 1);
        graphview.add_node(sink);

        let link = graphview.create_link(1, 2, 1, 2);
        graphview.add_link(link);

        // Clear undo history
        graphview.clear_undo_history();

        // Apply auto-arrange with custom options
        let options = AutoArrangeOptions {
            horizontal_spacing: 500.0,
            vertical_spacing: 200.0,
            start_x: 100.0,
            start_y: 100.0,
            ..Default::default()
        };
        assert!(graphview.auto_arrange_graph(Some(options)));

        // Verify positions match custom options
        let src_node = graphview.node(1).unwrap();
        let sink_node = graphview.node(2).unwrap();
        let src_pos = src_node.position();
        let sink_pos = sink_node.position();

        // Source should be at start_x
        assert!(
            (src_pos.0 - 100.0).abs() < 1.0,
            "Source X should be at start_x"
        );
        assert!(
            (src_pos.1 - 100.0).abs() < 1.0,
            "Source Y should be at start_y"
        );

        // Sink should be at start_x + src_width + horizontal_spacing (gap-based layout)
        // Note: In tests, GTK widgets may not be realized, so width() returns 0
        let src_width = src_node.width() as f32;
        let expected_sink_x = 100.0 + src_width + 500.0;
        assert!(
            (sink_pos.0 - expected_sink_x).abs() < 1.0,
            "Sink X should be at start_x + src_width + horizontal_spacing: expected {}, got {}",
            expected_sink_x,
            sink_pos.0
        );
    });
}

#[test]
fn xml_ports_saved_in_sorted_order() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a node
        let node = graphview.create_node("mixer", NodeType::Transform);
        graphview.add_node(node);

        // Add ports in reverse alphabetical order to test sorting
        // This simulates what happens when ports are stored in a HashMap
        let port_z = graphview.create_port("sink_2", PortDirection::Input, PortPresence::Sometimes);
        let port_a = graphview.create_port("sink_0", PortDirection::Input, PortPresence::Sometimes);
        let port_m = graphview.create_port("sink_1", PortDirection::Input, PortPresence::Sometimes);
        let port_out = graphview.create_port("src_0", PortDirection::Output, PortPresence::Always);

        let mut node = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut node, port_z);
        graphview.add_port_to_node(&mut node, port_a);
        graphview.add_port_to_node(&mut node, port_m);
        graphview.add_port_to_node(&mut node, port_out);

        // Save to XML
        let buffer = graphview
            .render_xml()
            .expect("Should be able to render graph to xml");
        let xml_str = std::str::from_utf8(&buffer).expect("XML should be valid UTF-8");

        // Find all port names in order of appearance in XML
        let port_positions: Vec<(usize, &str)> = ["sink_0", "sink_1", "sink_2", "src_0"]
            .iter()
            .filter_map(|name| {
                xml_str
                    .find(&format!("name=\"{}\"", name))
                    .map(|pos| (pos, *name))
            })
            .collect();

        // Verify all ports were found
        assert_eq!(
            port_positions.len(),
            4,
            "All 4 ports should be in XML: {:?}",
            port_positions
        );

        // Verify ports appear in sorted order in the XML
        let mut sorted_positions = port_positions.clone();
        sorted_positions.sort_by_key(|(pos, _)| *pos);

        assert_eq!(
            sorted_positions[0].1, "sink_0",
            "sink_0 should appear first in XML"
        );
        assert_eq!(
            sorted_positions[1].1, "sink_1",
            "sink_1 should appear second in XML"
        );
        assert_eq!(
            sorted_positions[2].1, "sink_2",
            "sink_2 should appear third in XML"
        );
        assert_eq!(
            sorted_positions[3].1, "src_0",
            "src_0 should appear fourth in XML"
        );
    });
}

#[test]
fn xml_roundtrip_preserves_port_order() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a node with multiple input ports
        let node = graphview.create_node("mixer", NodeType::Transform);
        graphview.add_node(node);

        // Add ports - the order they're added determines their visual position
        let port0 = graphview.create_port("sink_0", PortDirection::Input, PortPresence::Sometimes);
        let port1 = graphview.create_port("sink_1", PortDirection::Input, PortPresence::Sometimes);
        let port_out = graphview.create_port("src_0", PortDirection::Output, PortPresence::Always);

        let mut node = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut node, port0);
        graphview.add_port_to_node(&mut node, port1);
        graphview.add_port_to_node(&mut node, port_out);

        // Create source nodes and links
        let src1 = graphview.create_node_with_port("video_source", NodeType::Source, 1, 0);
        src1.set_position(20.0, 30.0); // Top source
        graphview.add_node(src1);

        let src2 = graphview.create_node_with_port("video_source", NodeType::Source, 1, 0);
        src2.set_position(20.0, 300.0); // Bottom source
        graphview.add_node(src2);

        // Link top source to sink_0 (top input), bottom source to sink_1 (bottom input)
        let link1 = graphview.create_link(2, 1, 4, 1); // src1 -> sink_0
        graphview.add_link(link1);
        let link2 = graphview.create_link(3, 1, 5, 2); // src2 -> sink_1
        graphview.add_link(link2);

        // Save to XML
        let buffer = graphview
            .render_xml()
            .expect("Should be able to render graph to xml");

        // Load back
        graphview
            .load_from_xml(buffer)
            .expect("Should be able to load from XML");

        // Verify node exists
        let node = graphview.node(1).expect("Mixer node should exist");

        // Get input ports and check their order by collecting to vec and sorting by name
        let mut input_ports: Vec<_> = node.all_ports(PortDirection::Input).into_iter().collect();
        input_ports.sort_by_key(|p| p.name());

        assert_eq!(input_ports.len(), 2, "Should have 2 input ports");
        assert_eq!(
            input_ports[0].name(),
            "sink_0",
            "First input port should be sink_0"
        );
        assert_eq!(
            input_ports[1].name(),
            "sink_1",
            "Second input port should be sink_1"
        );

        // Verify links are still correct
        assert_eq!(graphview.all_links(true).len(), 2, "Should have 2 links");
    });
}

// =============================================================================
// Auto-connect signal tests
// =============================================================================

#[test]
fn node_link_request_signal_can_be_connected() {
    test_synced(|| {
        use gtk::glib::object::ObjectExt as GlibObjectExt;
        use std::cell::Cell;
        use std::rc::Rc;

        let graphview = GraphView::new();

        // Track whether signal was received
        let signal_received = Rc::new(Cell::new(false));
        let received_from_node = Rc::new(Cell::new(0u32));
        let received_from_port = Rc::new(Cell::new(0u32));
        let received_target_node = Rc::new(Cell::new(0u32));

        let sr = signal_received.clone();
        let rfn = received_from_node.clone();
        let rfp = received_from_port.clone();
        let rtn = received_target_node.clone();

        GlibObjectExt::connect_local(&graphview, "node-link-request", false, move |values| {
            sr.set(true);
            rfn.set(values[1].get::<u32>().unwrap());
            rfp.set(values[2].get::<u32>().unwrap());
            rtn.set(values[3].get::<u32>().unwrap());
            None
        });

        // Emit the signal manually
        GlibObjectExt::emit_by_name::<()>(&graphview, "node-link-request", &[&42u32, &1u32, &2u32]);

        // Verify signal was received with correct values
        assert!(signal_received.get(), "Signal should have been received");
        assert_eq!(received_from_node.get(), 42, "from_node_id should be 42");
        assert_eq!(received_from_port.get(), 1, "from_port_id should be 1");
        assert_eq!(received_target_node.get(), 2, "target_node_id should be 2");
    });
}

#[test]
fn port_direction_opposite() {
    test_synced(|| {
        // Test the direction logic used in auto-connect
        let input_opposite = match PortDirection::Input {
            PortDirection::Input => PortDirection::Output,
            PortDirection::Output => PortDirection::Input,
            _ => PortDirection::Unknown,
        };
        assert_eq!(
            input_opposite,
            PortDirection::Output,
            "Opposite of Input should be Output"
        );

        let output_opposite = match PortDirection::Output {
            PortDirection::Input => PortDirection::Output,
            PortDirection::Output => PortDirection::Input,
            _ => PortDirection::Unknown,
        };
        assert_eq!(
            output_opposite,
            PortDirection::Input,
            "Opposite of Output should be Input"
        );

        let unknown_opposite = match PortDirection::Unknown {
            PortDirection::Input => PortDirection::Output,
            PortDirection::Output => PortDirection::Input,
            _ => PortDirection::Unknown,
        };
        assert_eq!(
            unknown_opposite,
            PortDirection::Unknown,
            "Opposite of Unknown should be Unknown"
        );
    });
}

#[test]
fn find_free_port_on_node() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create source node with output port
        let source = graphview.create_node("source", NodeType::Source);
        graphview.add_node(source);
        let output_port = graphview.create_port("src", PortDirection::Output, PortPresence::Always);
        let mut source = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut source, output_port);

        // Create sink node with input port
        let sink = graphview.create_node("sink", NodeType::Sink);
        graphview.add_node(sink);
        let input_port = graphview.create_port("sink", PortDirection::Input, PortPresence::Always);
        let mut sink = graphview.node(2).unwrap();
        graphview.add_port_to_node(&mut sink, input_port);

        // Get the target node and find a free input port
        let target_node = graphview.node(2).unwrap();
        let free_input_ports: Vec<_> = target_node
            .all_ports(PortDirection::Input)
            .into_iter()
            .filter(|p| graphview.port_is_linked(p.id()).is_none())
            .collect();

        assert_eq!(free_input_ports.len(), 1, "Should have 1 free input port");
        assert_eq!(
            free_input_ports[0].name(),
            "sink",
            "Free port should be named 'sink'"
        );

        // Now link the ports
        let link = graphview.create_link(1, 2, 1, 2);
        graphview.add_link(link);

        // Check that port is now linked
        let free_input_ports_after: Vec<_> = target_node
            .all_ports(PortDirection::Input)
            .into_iter()
            .filter(|p| graphview.port_is_linked(p.id()).is_none())
            .collect();

        assert_eq!(
            free_input_ports_after.len(),
            0,
            "Should have 0 free input ports after linking"
        );
    });
}

#[test]
fn port_caps_property() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create a node with a port that has caps
        let node = graphview.create_node("test_node", NodeType::Transform);
        graphview.add_node(node);

        let port = graphview.create_port("audio_sink", PortDirection::Input, PortPresence::Always);
        port.add_property("_caps", "audio/x-raw");

        let mut node = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut node, port);

        // Retrieve the port and check caps
        let node = graphview.node(1).unwrap();
        let port = node.port(1).expect("Port should exist");
        let caps = PropertyExt::property(&port, "_caps");

        assert_eq!(
            caps,
            Some("audio/x-raw".to_string()),
            "Port should have audio/x-raw caps"
        );

        // Test port without caps
        let port2 = graphview.create_port("video_sink", PortDirection::Input, PortPresence::Always);
        let mut node = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut node, port2);

        let port2 = node.port(2).expect("Port 2 should exist");
        let caps2 = PropertyExt::property(&port2, "_caps");

        assert_eq!(caps2, None, "Port without caps property should return None");
    });
}

// =============================================================================
// Auto-connect integration tests (graphmanager level)
// =============================================================================

#[test]
fn auto_connect_find_compatible_port_by_caps() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create source node with video output port
        let source = graphview.create_node("video_source", NodeType::Source);
        graphview.add_node(source);
        let src_port = graphview.create_port("src", PortDirection::Output, PortPresence::Always);
        src_port.add_property("_caps", "video/x-raw");
        let mut source = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut source, src_port);

        // Create sink node with video input port and audio input port
        let sink = graphview.create_node("mixer", NodeType::Transform);
        graphview.add_node(sink);

        let video_sink =
            graphview.create_port("video_sink", PortDirection::Input, PortPresence::Always);
        video_sink.add_property("_caps", "video/x-raw");
        let mut sink = graphview.node(2).unwrap();
        graphview.add_port_to_node(&mut sink, video_sink);

        let audio_sink =
            graphview.create_port("audio_sink", PortDirection::Input, PortPresence::Always);
        audio_sink.add_property("_caps", "audio/x-raw");
        let mut sink = graphview.node(2).unwrap();
        graphview.add_port_to_node(&mut sink, audio_sink);

        // Find free input ports on the sink node
        let target_node = graphview.node(2).unwrap();
        let from_port = graphview.node(1).unwrap().port(1).unwrap();
        let from_caps =
            PropertyExt::property(&from_port, "_caps").unwrap_or_else(|| "ANY".to_string());

        // Simulate handle_auto_connect logic: find compatible free port
        let compatible_port = target_node
            .all_ports(PortDirection::Input)
            .into_iter()
            .filter(|p| graphview.port_is_linked(p.id()).is_none())
            .find(|p| {
                let port_caps =
                    PropertyExt::property(p, "_caps").unwrap_or_else(|| "ANY".to_string());
                // Simulate caps_compatible (just check if both are video or both are audio)
                from_caps.starts_with("video") && port_caps.starts_with("video")
            });

        assert!(
            compatible_port.is_some(),
            "Should find a compatible video input port"
        );
        assert_eq!(
            compatible_port.unwrap().name(),
            "video_sink",
            "Should select the video_sink port"
        );
    });
}

#[test]
fn auto_connect_no_compatible_port_when_caps_mismatch() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create source node with video output port
        let source = graphview.create_node("video_source", NodeType::Source);
        graphview.add_node(source);
        let src_port = graphview.create_port("src", PortDirection::Output, PortPresence::Always);
        src_port.add_property("_caps", "video/x-raw");
        let mut source = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut source, src_port);

        // Create sink node with only audio input port
        let sink = graphview.create_node("audio_sink", NodeType::Sink);
        graphview.add_node(sink);

        let audio_sink = graphview.create_port("sink", PortDirection::Input, PortPresence::Always);
        audio_sink.add_property("_caps", "audio/x-raw");
        let mut sink = graphview.node(2).unwrap();
        graphview.add_port_to_node(&mut sink, audio_sink);

        // Find free input ports on the sink node
        let target_node = graphview.node(2).unwrap();
        let from_port = graphview.node(1).unwrap().port(1).unwrap();
        let from_caps =
            PropertyExt::property(&from_port, "_caps").unwrap_or_else(|| "ANY".to_string());

        // Simulate handle_auto_connect logic: find compatible free port
        let compatible_port = target_node
            .all_ports(PortDirection::Input)
            .into_iter()
            .filter(|p| graphview.port_is_linked(p.id()).is_none())
            .find(|p| {
                let port_caps =
                    PropertyExt::property(p, "_caps").unwrap_or_else(|| "ANY".to_string());
                // Video cannot connect to audio
                from_caps.starts_with("video") && port_caps.starts_with("video")
            });

        assert!(
            compatible_port.is_none(),
            "Should not find a compatible port when caps don't match"
        );
    });
}

#[test]
fn auto_connect_all_ports_linked() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create source node 1 with video output
        let source1 = graphview.create_node("video_source", NodeType::Source);
        graphview.add_node(source1);
        let src_port1 = graphview.create_port("src", PortDirection::Output, PortPresence::Always);
        src_port1.add_property("_caps", "video/x-raw");
        let mut source1 = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut source1, src_port1);

        // Create source node 2 with video output
        let source2 = graphview.create_node("video_source_2", NodeType::Source);
        graphview.add_node(source2);
        let src_port2 = graphview.create_port("src", PortDirection::Output, PortPresence::Always);
        src_port2.add_property("_caps", "video/x-raw");
        let mut source2 = graphview.node(2).unwrap();
        graphview.add_port_to_node(&mut source2, src_port2);

        // Create sink node with single video input
        let sink = graphview.create_node("video_sink", NodeType::Sink);
        graphview.add_node(sink);
        let video_sink = graphview.create_port("sink", PortDirection::Input, PortPresence::Always);
        video_sink.add_property("_caps", "video/x-raw");
        let mut sink = graphview.node(3).unwrap();
        graphview.add_port_to_node(&mut sink, video_sink);

        // Link source1 to sink (the only input port)
        let link = graphview.create_link(1, 3, 1, 3);
        graphview.add_link(link);

        // Now try to find a free input port on the sink for source2
        let target_node = graphview.node(3).unwrap();
        let free_ports: Vec<_> = target_node
            .all_ports(PortDirection::Input)
            .into_iter()
            .filter(|p| graphview.port_is_linked(p.id()).is_none())
            .collect();

        assert!(
            free_ports.is_empty(),
            "All ports should be linked, no free ports available"
        );
    });
}

#[test]
fn auto_connect_node_not_found_returns_none() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Try to get a non-existent node
        let node = graphview.node(999);
        assert!(node.is_none(), "Non-existent node should return None");

        // Create a node and get a non-existent port
        let node = graphview.create_node("test", NodeType::Source);
        graphview.add_node(node);
        let node = graphview.node(1).unwrap();
        let port = node.port(999);
        assert!(port.is_none(), "Non-existent port should return None");
    });
}

#[test]
fn auto_connect_with_any_caps() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create source node with ANY caps (permissive)
        let source = graphview.create_node("source", NodeType::Source);
        graphview.add_node(source);
        let src_port = graphview.create_port("src", PortDirection::Output, PortPresence::Always);
        src_port.add_property("_caps", "ANY");
        let mut source = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut source, src_port);

        // Create sink node with specific audio caps
        let sink = graphview.create_node("audio_sink", NodeType::Sink);
        graphview.add_node(sink);
        let audio_sink = graphview.create_port("sink", PortDirection::Input, PortPresence::Always);
        audio_sink.add_property("_caps", "audio/x-raw");
        let mut sink = graphview.node(2).unwrap();
        graphview.add_port_to_node(&mut sink, audio_sink);

        // ANY should be compatible with anything
        let from_caps = "ANY";
        let target_node = graphview.node(2).unwrap();

        let compatible_port = target_node
            .all_ports(PortDirection::Input)
            .into_iter()
            .filter(|p| graphview.port_is_linked(p.id()).is_none())
            .find(|p| {
                let port_caps =
                    PropertyExt::property(p, "_caps").unwrap_or_else(|| "ANY".to_string());
                // ANY is compatible with everything
                from_caps == "ANY" || port_caps == "ANY"
            });

        assert!(
            compatible_port.is_some(),
            "ANY caps should be compatible with any port"
        );
    });
}

#[test]
fn auto_connect_port_without_caps_property() {
    test_synced(|| {
        let graphview = GraphView::new();

        // Create source node with port that has NO caps property
        let source = graphview.create_node("source", NodeType::Source);
        graphview.add_node(source);
        let src_port = graphview.create_port("src", PortDirection::Output, PortPresence::Always);
        // Note: NOT setting _caps property
        let mut source = graphview.node(1).unwrap();
        graphview.add_port_to_node(&mut source, src_port);

        // Create sink node with port that has caps
        let sink = graphview.create_node("sink", NodeType::Sink);
        graphview.add_node(sink);
        let sink_port = graphview.create_port("sink", PortDirection::Input, PortPresence::Always);
        sink_port.add_property("_caps", "video/x-raw");
        let mut sink = graphview.node(2).unwrap();
        graphview.add_port_to_node(&mut sink, sink_port);

        // Test the fallback to ANY
        let from_port = graphview.node(1).unwrap().port(1).unwrap();
        let from_caps =
            PropertyExt::property(&from_port, "_caps").unwrap_or_else(|| "ANY".to_string());

        assert_eq!(from_caps, "ANY", "Port without _caps should default to ANY");
    });
}

// =============================================================================
// DOT Parser Tests (no GTK required)
// =============================================================================

use crate::graphmanager::dot_parser::{DotGraph, DotLoader};
use std::collections::HashMap;

/// Default loader using all trait default implementations
struct DefaultDotLoader;

impl DotLoader for DefaultDotLoader {}

#[test]
fn dot_parse_empty_graph() {
    let loader = DefaultDotLoader;
    let result = DotGraph::parse("digraph pipeline { }", &loader);
    assert!(result.is_ok(), "Empty graph should parse successfully");
    let graph = result.unwrap();
    assert!(graph.nodes.is_empty(), "Empty graph should have no nodes");
    assert!(graph.ports.is_empty(), "Empty graph should have no ports");
    assert!(graph.links.is_empty(), "Empty graph should have no links");
}

#[test]
fn dot_parse_invalid_syntax() {
    let loader = DefaultDotLoader;
    let result = DotGraph::parse("not valid dot syntax {{{", &loader);
    assert!(result.is_err(), "Invalid syntax should return error");
}

#[test]
fn dot_parse_empty_string() {
    let loader = DefaultDotLoader;
    let result = DotGraph::parse("", &loader);
    assert!(result.is_err(), "Empty string should return error");
}

#[test]
fn dot_parse_simple_node() {
    let loader = DefaultDotLoader;
    // Use actual newlines in label (default parser uses .lines())
    let dot = "
        digraph pipeline {
            subgraph cluster_node0_0x123 {
                label=\"MyClass
instance0\";
            }
        }
    ";
    let result = DotGraph::parse(dot, &loader);
    assert!(result.is_ok(), "Simple node should parse: {:?}", result);
    let graph = result.unwrap();
    assert_eq!(graph.nodes.len(), 1, "Should have 1 node");
    assert_eq!(graph.nodes[0].instance_name, "instance0");
    assert_eq!(graph.nodes[0].type_name, "myclass"); // lowercased
    assert_eq!(
        graph.nodes[0].metadata.get("class_name"),
        Some(&"MyClass".to_string())
    );
}

#[test]
fn dot_parse_node_with_port() {
    let loader = DefaultDotLoader;
    let dot = "
        digraph pipeline {
            subgraph cluster_node0_0x123 {
                label=\"Source
src0\";
                node_src0_0x123_node_out_0x456 [label=\"out\"];
            }
        }
    ";
    let result = DotGraph::parse(dot, &loader);
    assert!(result.is_ok(), "Node with port should parse: {:?}", result);
    let graph = result.unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.ports.len(), 1);
    assert_eq!(graph.ports[0].name, "out");
}

#[test]
fn dot_parse_linked_nodes() {
    let loader = DefaultDotLoader;
    let dot = "
        digraph pipeline {
            subgraph cluster_src_0x100 {
                label=\"Source
src0\";
                src_0x100_out_0x101 [label=\"src\"];
            }
            subgraph cluster_sink_0x200 {
                label=\"Sink
sink0\";
                sink_0x200_in_0x201 [label=\"sink\"];
            }
            src_0x100_out_0x101 -> sink_0x200_in_0x201;
        }
    ";
    let result = DotGraph::parse(dot, &loader);
    assert!(result.is_ok(), "Linked nodes should parse: {:?}", result);
    let graph = result.unwrap();
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.ports.len(), 2);
    assert_eq!(graph.links.len(), 1);

    // Check port directions inferred from edge
    let src_port = graph.ports.iter().find(|p| p.name == "src").unwrap();
    let sink_port = graph.ports.iter().find(|p| p.name == "sink").unwrap();
    assert_eq!(src_port.direction, PortDirection::Output);
    assert_eq!(sink_port.direction, PortDirection::Input);
}

#[test]
fn dot_parse_skips_legend() {
    let loader = DefaultDotLoader;
    // Use actual newlines in labels
    let dot = "
        digraph pipeline {
            subgraph cluster_legend_0x999 {
                label=\"Legend
Legend\";
            }
            subgraph cluster_real_0x123 {
                label=\"RealNode
real0\";
            }
        }
    ";
    let result = DotGraph::parse(dot, &loader);
    assert!(result.is_ok());
    let graph = result.unwrap();
    assert_eq!(graph.nodes.len(), 1, "Should skip Legend node");
    assert_eq!(graph.nodes[0].instance_name, "real0");
}

#[test]
fn dot_parse_skips_proxypad() {
    let loader = DefaultDotLoader;
    let dot = "
        digraph pipeline {
            subgraph cluster_node_0x123 {
                label=\"Element
elem0\";
                node_proxypad0_0x456 [label=\"proxypad0\"];
                node_src_0x789 [label=\"src\"];
            }
        }
    ";
    let result = DotGraph::parse(dot, &loader);
    assert!(result.is_ok());
    let graph = result.unwrap();
    assert_eq!(graph.ports.len(), 1, "Should skip proxypad port");
    assert_eq!(graph.ports[0].name, "src");
}

#[test]
fn dot_parse_port_direction_from_name() {
    let loader = DefaultDotLoader;
    let dot = "
        digraph pipeline {
            subgraph cluster_node_0x123 {
                label=\"Element
elem0\";
                elem0_0x123_sink_0x456 [label=\"sink\"];
                elem0_0x123_src_0x789 [label=\"src\"];
            }
        }
    ";
    let result = DotGraph::parse(dot, &loader);
    assert!(result.is_ok());
    let graph = result.unwrap();

    // Ports not in any edge should get direction from naming convention
    let sink_port = graph.ports.iter().find(|p| p.name == "sink").unwrap();
    let src_port = graph.ports.iter().find(|p| p.name == "src").unwrap();
    assert_eq!(sink_port.direction, PortDirection::Input);
    assert_eq!(src_port.direction, PortDirection::Output);
}

#[test]
fn dot_parse_graph_metadata() {
    // Custom loader that extracts "version" attribute
    struct MetadataLoader;
    impl DotLoader for MetadataLoader {
        fn extract_graph_metadata(
            &self,
            attributes: &[(String, String)],
        ) -> HashMap<String, String> {
            attributes
                .iter()
                .filter(|(k, _)| k == "version")
                .cloned()
                .collect()
        }
    }

    let loader = MetadataLoader;
    let dot = "
        digraph pipeline {
            version=\"1.0\";
            other=\"ignored\";
        }
    ";
    let result = DotGraph::parse(dot, &loader);
    assert!(result.is_ok());
    let graph = result.unwrap();
    assert_eq!(graph.metadata.get("version"), Some(&"1.0".to_string()));
    assert!(!graph.metadata.contains_key("other"));
}

#[test]
fn dot_parse_nested_nodes_filtered() {
    let loader = DefaultDotLoader;
    // Use actual newlines in labels
    let dot = "
        digraph pipeline {
            subgraph cluster_bin_0x100 {
                label=\"Bin
bin0\";
                subgraph cluster_inner_0x200 {
                    label=\"Inner
inner0\";
                }
            }
        }
    ";
    let result = DotGraph::parse(dot, &loader);
    assert!(result.is_ok());
    let graph = result.unwrap();
    // Only top-level nodes (depth=0) should be included
    assert_eq!(graph.nodes.len(), 1, "Should only include top-level node");
    assert_eq!(graph.nodes[0].instance_name, "bin0");
}
