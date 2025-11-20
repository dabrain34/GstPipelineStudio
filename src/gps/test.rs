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
    fn test_get_version() {
        test_synced(|| {
            let version = Player::get_version();
            assert!(!version.is_empty());
            // Version should not contain "GStreamer" prefix after trimming
            assert!(!version.starts_with("GStreamer"));
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
            assert!(!player.playing());
            assert_eq!(player.n_video_sink(), 0);
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
    fn test_pipeline_state_default() {
        test_synced(|| {
            let state: PipelineState = Default::default();
            assert_eq!(state, PipelineState::Stopped);
        });
    }

    #[test]
    fn test_pipeline_state_equality() {
        test_synced(|| {
            assert_eq!(PipelineState::Playing, PipelineState::Playing);
            assert_ne!(PipelineState::Playing, PipelineState::Paused);
            assert_ne!(PipelineState::Stopped, PipelineState::Error);
        });
    }

    #[test]
    fn test_playing_state_detection() {
        test_synced(|| {
            let player = Player::new().unwrap();

            // Default stopped state should not be "playing"
            assert!(!player.playing());

            // Note: We can't test Playing/Paused states without a valid pipeline
            // Those would require integration tests with actual GStreamer pipelines
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
    fn test_position_description_format() {
        test_synced(|| {
            let player = Player::new().unwrap();
            let desc = player.position_description();

            // Should have format "position/duration"
            assert!(desc.contains('/'));

            // Without pipeline, should show "0:00:00/0:00:00" format
            assert_eq!(desc, "0:00:00/0:00:00");
        });
    }

    #[test]
    fn test_pipeline_elements_without_pipeline() {
        test_synced(|| {
            let player = Player::new().unwrap();

            // Without a playing pipeline, should return None
            assert!(player.pipeline_elements().is_none());
        });
    }

    #[test]
    fn test_create_simple_pipeline() {
        test_synced(|| {
            let player = Player::new().unwrap();
            let result = player.create_pipeline("videotestsrc num-buffers=5 ! fakesink");
            assert!(result.is_ok());

            let pipeline = result.unwrap();
            assert_eq!(pipeline.name().as_str(), "pipeline0");

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
}
