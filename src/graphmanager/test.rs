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
