mod graphview;
mod link;
mod node;
mod port;
mod property;
mod selection;

pub use graphview::GraphView;
pub use link::Link;
pub use node::Node;
pub use node::NodeType;
pub use port::Port;
pub use port::{PortDirection, PortPresence};
pub use property::PropertyExt;
pub use selection::SelectionExt;

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

#[cfg(test)]
mod test {
    use super::*;
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
            let link = graphview.create_link(1, 2, 1, 2, true);
            graphview.add_link(link);

            //Create link between node2 and node 3
            let link = graphview.create_link(2, 3, 3, 4, true);
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
}
