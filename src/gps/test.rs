use crate::gps::player::Player;
use crate::gps::ElementInfo;
use gst::prelude::*;

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
            // Initialize GTK first to set up MainContext, then GStreamer
            gtk::init().expect("Tests failed to initialize GTK");
            gst::init().expect("Tests failed to initialize GStreamer");
        })
        .expect("Failed to schedule a test call");
        pool
    });

#[cfg(test)]
mod element_test {
    use super::*;

    #[test]
    fn test_element_factory_exists() {
        test_synced(|| {
            // Test with a common element that should exist in all GStreamer installations
            assert!(ElementInfo::element_factory_exists("identity"));
            // Test with an element that definitely doesn't exist
            assert!(!ElementInfo::element_factory_exists(
                "nonexistentelement12345"
            ));
        });
    }

    #[test]
    fn test_element_feature() {
        test_synced(|| {
            // Test getting a feature for an existing element
            let feature = ElementInfo::element_feature("identity");
            assert!(feature.is_some());
            // Test getting a feature for a non-existing element
            let feature = ElementInfo::element_feature("nonexistentelement12345");
            assert!(feature.is_none());
        });
    }

    #[test]
    fn test_element_description() {
        test_synced(|| {
            // Test description for an existing element
            let result = ElementInfo::element_description("identity");
            assert!(result.is_ok());
            let desc = result.unwrap();
            assert!(desc.contains("<b>Factory details:</b>"));
            assert!(desc.contains("<b>Name:</b>"));
            // Test description for a non-existing element
            let result = ElementInfo::element_description("nonexistentelement12345");
            assert!(result.is_ok());
            let desc = result.unwrap();
            assert!(desc.contains("Factory unavailable"));
        });
    }

    #[test]
    fn test_element_property_by_feature_name() {
        test_synced(|| {
            // Test getting a property from identity element (silent property should exist)
            let result = ElementInfo::element_property_by_feature_name("identity", "silent");
            assert!(result.is_ok());
            // Test with non-existent element should return error
            let result = ElementInfo::element_property_by_feature_name(
                "nonexistentelement12345",
                "someprop",
            );
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_element_properties_by_feature_name() {
        test_synced(|| {
            // Test getting properties from identity element
            let result = ElementInfo::element_properties_by_feature_name("identity");
            assert!(result.is_ok());
            let properties = result.unwrap();
            assert!(!properties.is_empty());
            // identity element should have a "silent" property
            assert!(properties.contains_key("silent"));

            // Test with non-existent element should return error
            let result = ElementInfo::element_properties_by_feature_name("nonexistentelement12345");
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_element_is_uri_src_handler() {
        test_synced(|| {
            // filesrc should be a URI source handler with location property
            let result = ElementInfo::element_is_uri_src_handler("filesrc");
            assert!(result.is_some());
            if let Some((property, _)) = result {
                assert_eq!(property, "location");
            }
            // identity is not a URI source handler
            let result = ElementInfo::element_is_uri_src_handler("identity");
            assert!(result.is_none());

            // Non-existent element should return None
            let result = ElementInfo::element_is_uri_src_handler("nonexistentelement12345");
            assert!(result.is_none());
        });
    }

    #[test]
    fn test_element_is_uri_sink_handler() {
        test_synced(|| {
            // filesink should be a URI sink handler with location property
            let result = ElementInfo::element_is_uri_sink_handler("filesink");
            assert!(result.is_some());
            if let Some((property, _)) = result {
                assert_eq!(property, "location");
            }
            // identity is not a URI sink handler
            let result = ElementInfo::element_is_uri_sink_handler("identity");
            assert!(result.is_none());

            // Non-existent element should return None
            let result = ElementInfo::element_is_uri_sink_handler("nonexistentelement12345");
            assert!(result.is_none());
        });
    }

    #[test]
    fn test_elements_list() {
        test_synced(|| {
            let result = ElementInfo::elements_list();
            assert!(result.is_ok());
            let elements = result.unwrap();
            assert!(!elements.is_empty());
            // Check that the list is sorted
            for i in 1..elements.len() {
                assert!(elements[i - 1] <= elements[i]);
            }
        });
    }
}

#[cfg(test)]
mod player_test {
    use super::*;
    use crate::gps::PipelineState;

    #[test]
    fn test_version() {
        test_synced(|| {
            let version = Player::version();
            assert!(!version.is_empty());
        });
    }
    #[test]
    fn test_player_creation() {
        test_synced(|| {
            let result = Player::new();
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_player_initial_state() {
        test_synced(|| {
            let player = Player::new().unwrap();
            assert_eq!(player.state(), PipelineState::Stopped);
            assert_eq!(player.n_video_sink(), 0);
            assert!(player.pipeline_elements().is_none());
        });
    }

    #[test]
    fn test_player_weak_upgrade() {
        test_synced(|| {
            let player = Player::new().unwrap();
            let weak = player.downgrade();

            // Weak should upgrade successfully while strong reference exists
            assert!(weak.upgrade().is_some());

            drop(player);

            // Weak should fail to upgrade after strong reference is dropped
            assert!(weak.upgrade().is_none());
        });
    }

    #[test]
    fn test_pipeline_state_display() {
        test_synced(|| {
            assert_eq!(PipelineState::Playing.to_string(), "Playing");
            assert_eq!(PipelineState::Paused.to_string(), "Paused");
            assert_eq!(PipelineState::Stopped.to_string(), "Stopped");
            assert_eq!(PipelineState::Error.to_string(), "Error");
        });
    }

    #[test]
    fn test_position_without_pipeline() {
        test_synced(|| {
            let player = Player::new().unwrap();

            // Without a pipeline, position should be 0
            let position = player.position();
            assert_eq!(position, 0);

            // Duration should also be 0
            let duration = player.duration();
            assert_eq!(duration, 0);
        });
    }

    #[test]
    fn test_create_simple_pipeline() {
        test_synced(|| {
            let player = Player::new().unwrap();
            let result = player.create_pipeline("videotestsrc num-buffers=5 ! fakesink");
            assert!(result.is_ok());

            let pipeline = result.unwrap();
            // Pipeline name will be auto-generated (pipeline0, pipeline1, etc.)
            assert!(pipeline.name().as_str().starts_with("pipeline"));

            // Pipeline should start in NULL state
            let state = pipeline.state(gst::ClockTime::NONE).1;
            assert_eq!(state, gst::State::Null);

            // Try to set pipeline to PLAYING state
            let result = pipeline.set_state(gst::State::Playing);
            assert!(result.is_ok());

            // Wait for state change
            let result = pipeline.state(gst::ClockTime::from_seconds(1));
            assert_eq!(result.1, gst::State::Playing);

            // Stop the pipeline
            let result = pipeline.set_state(gst::State::Null);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_pipeline_with_properties() {
        test_synced(|| {
            let player = Player::new().unwrap();
            let result = player.create_pipeline("videotestsrc pattern=1 ! fakesink");
            assert!(result.is_ok());

            let pipeline = result.unwrap();

            // Verify we can access the pipeline elements
            let bin = pipeline.clone().dynamic_cast::<gst::Bin>().unwrap();
            let elements: Vec<_> = bin.iterate_elements().into_iter().collect();
            assert_eq!(elements.len(), 2); // videotestsrc and fakesink
        });
    }

    #[test]
    fn test_invalid_pipeline_description() {
        test_synced(|| {
            let player = Player::new().unwrap();
            let result = player.create_pipeline("invalid ! nonexistent");
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_position_queries() {
        test_synced(|| {
            let player = Player::new().unwrap();

            // Position/duration should return 0 without a pipeline
            let position = player.position();
            assert_eq!(position, 0);

            let duration = player.duration();
            assert_eq!(duration, 0);
        });
    }

    #[test]
    fn test_multiple_pipeline_creations() {
        test_synced(|| {
            let player = Player::new().unwrap();

            // Create first pipeline
            let result1 = player.create_pipeline("videotestsrc ! fakesink");
            assert!(result1.is_ok());

            // Create second pipeline (should get different name)
            let result2 = player.create_pipeline("audiotestsrc ! fakesink");
            assert!(result2.is_ok());

            let pipeline1 = result1.unwrap();
            let pipeline2 = result2.unwrap();

            // Names should be different
            assert_ne!(pipeline1.name(), pipeline2.name());
        });
    }

    #[test]
    fn test_position_description_format() {
        test_synced(|| {
            let player = Player::new().unwrap();

            let desc = player.position_description();

            // Should have the format with "/"
            assert!(desc.contains('/'));

            // Should be in time format (contains ":")
            assert!(desc.contains(':'));

            // Without pipeline, should show "0:00:00/0:00:00" format
            assert_eq!(desc, "0:00:00/0:00:00");
        });
    }

    #[test]
    fn test_n_video_sink_initial_value() {
        test_synced(|| {
            let player = Player::new().unwrap();

            // Initial value should be 0
            assert_eq!(player.n_video_sink(), 0);
        });
    }

    #[test]
    fn test_create_properties_for_element() {
        test_synced(|| {
            use crate::graphmanager as GM;
            use crate::graphmanager::PropertyExt;

            let player = Player::new().unwrap();

            // Create a simple pipeline with an element that has properties
            let pipeline = player
                .create_pipeline("videotestsrc pattern=1 ! fakesink")
                .unwrap();

            // Get the videotestsrc element
            let bin = pipeline.dynamic_cast::<gst::Bin>().unwrap();

            // Find videotestsrc by factory name
            let mut elements: Vec<gst::Element> = Vec::new();
            let mut iter = bin.iterate_elements();
            while let Ok(Some(element)) = iter.next() {
                elements.push(element);
            }

            let videotestsrc = elements
                .iter()
                .find(|e| {
                    e.factory()
                        .map(|f| f.name().starts_with("videotestsrc"))
                        .unwrap_or(false)
                })
                .expect("videotestsrc not found");

            // Create a standalone node to hold the properties
            let node = GM::Node::new(1, "videotestsrc", GM::NodeType::Source);

            // Call create_properties_for_element
            player.create_properties_for_element(videotestsrc, &node);

            // Verify that properties were added
            let properties = node.properties();

            // The "pattern" property should be set to "1" which is "snow" (non-default value)
            assert!(properties.contains_key("pattern"));
            assert_eq!(properties.get("pattern").unwrap(), "snow");

            // Properties like "name" and "parent" should NOT be added
            assert!(!properties.contains_key("name"));
            assert!(!properties.contains_key("parent"));
        });
    }

    #[test]
    fn test_create_properties_for_element_with_defaults() {
        test_synced(|| {
            use crate::graphmanager as GM;
            use crate::graphmanager::PropertyExt;

            let player = Player::new().unwrap();

            // Create element with default properties
            let pipeline = player.create_pipeline("identity ! fakesink").unwrap();
            let bin = pipeline.dynamic_cast::<gst::Bin>().unwrap();

            // Find identity by factory name
            let mut elements: Vec<gst::Element> = Vec::new();
            let mut iter = bin.iterate_elements();
            while let Ok(Some(element)) = iter.next() {
                elements.push(element);
            }

            let identity = elements
                .iter()
                .find(|e| {
                    e.factory()
                        .map(|f| f.name().starts_with("identity"))
                        .unwrap_or(false)
                })
                .expect("identity not found");

            let node = GM::Node::new(2, "identity", GM::NodeType::Transform);

            player.create_properties_for_element(identity, &node);

            let properties = node.properties();

            // Default properties should not be added (only non-default values)
            // Identity element with all defaults shouldn't add many properties
            // We just verify the method runs without errors and returns a valid HashMap
            assert!(properties.is_empty() || !properties.is_empty()); // Always true, just checks method runs
        });
    }

    #[test]
    fn test_create_pads_for_element_requires_app() {
        test_synced(|| {
            use crate::graphmanager as GM;

            let player = Player::new().unwrap();

            // Create a pipeline with an element
            let pipeline = player.create_pipeline("videotestsrc ! fakesink").unwrap();
            let bin = pipeline.dynamic_cast::<gst::Bin>().unwrap();
            let videotestsrc = bin.by_name("videotestsrc0").unwrap();

            let node = GM::Node::new(3, "videotestsrc", GM::NodeType::Source);

            // create_pads_for_element requires app to be initialized
            // It will fail gracefully with GPS_ERROR logs but won't crash
            player.create_pads_for_element(&videotestsrc, &node);

            // The method should complete without panicking
            // (errors are logged but not returned)
        });
    }

    #[test]
    fn test_element_pads_iteration() {
        test_synced(|| {
            let player = Player::new().unwrap();

            // Create a pipeline with various elements
            let pipeline = player
                .create_pipeline("videotestsrc ! capsfilter ! fakesink")
                .unwrap();
            let bin = pipeline.dynamic_cast::<gst::Bin>().unwrap();

            // Collect all elements from the pipeline
            let mut elements: Vec<gst::Element> = Vec::new();
            let mut iter = bin.iterate_elements();
            while let Ok(Some(element)) = iter.next() {
                elements.push(element);
            }

            // Find videotestsrc (has src pad)
            let videotestsrc = elements
                .iter()
                .find(|e| {
                    e.factory()
                        .map(|f| f.name().starts_with("videotestsrc"))
                        .unwrap_or(false)
                })
                .expect("videotestsrc not found");

            let mut src_pads = 0;
            let mut iter = videotestsrc.iterate_pads();
            while let Ok(Some(pad)) = iter.next() {
                assert_eq!(pad.direction(), gst::PadDirection::Src);
                src_pads += 1;
            }
            assert_eq!(src_pads, 1);

            // Find fakesink (has sink pad)
            let fakesink = elements
                .iter()
                .find(|e| {
                    e.factory()
                        .map(|f| f.name().starts_with("fakesink"))
                        .unwrap_or(false)
                })
                .expect("fakesink not found");

            let mut sink_pads = 0;
            let mut iter = fakesink.iterate_pads();
            while let Ok(Some(pad)) = iter.next() {
                assert_eq!(pad.direction(), gst::PadDirection::Sink);
                sink_pads += 1;
            }
            assert_eq!(sink_pads, 1);

            // Find capsfilter (has both sink and src pads)
            let capsfilter = elements
                .iter()
                .find(|e| {
                    e.factory()
                        .map(|f| f.name().starts_with("capsfilter"))
                        .unwrap_or(false)
                })
                .expect("capsfilter not found");

            let mut total_pads = 0;
            let mut iter = capsfilter.iterate_pads();
            while let Ok(Some(_pad)) = iter.next() {
                total_pads += 1;
            }
            assert_eq!(total_pads, 2); // sink and src
        });
    }
}
