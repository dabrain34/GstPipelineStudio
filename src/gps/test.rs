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
            // Identity element with all defaults shouldn't have any properties
            assert!(
                properties.is_empty(),
                "Expected no properties for identity with defaults, got: {:?}",
                properties.keys().collect::<Vec<_>>()
            );
        });
    }

    #[test]
    fn test_create_properties_for_videotestsrc_with_defaults() {
        // Regression test for issue #33: videotestsrc with default properties
        // should not have any properties stored (enum properties like pattern=smpte
        // were incorrectly being added because default value comparison failed)
        test_synced(|| {
            use crate::graphmanager as GM;
            use crate::graphmanager::PropertyExt;

            let player = Player::new().unwrap();

            let pipeline = player
                .create_pipeline("videotestsrc name=vts_test ! fakesink")
                .unwrap();
            let bin = pipeline.dynamic_cast::<gst::Bin>().unwrap();
            let videotestsrc = bin.by_name("vts_test").unwrap();

            let node = GM::Node::new(99, "videotestsrc", GM::NodeType::Source);

            player.create_properties_for_element(&videotestsrc, &node);

            let properties = node.properties();

            // videotestsrc with all defaults (pattern=smpte, animation-mode=frames, etc.)
            // should not have any properties stored
            assert!(
                properties.is_empty(),
                "Expected no properties for videotestsrc with defaults, got: {:?}",
                properties
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
            );
        });
    }

    // Note: fakesrc and fakesink have buggy ParamSpec default values in GStreamer:
    // - fakesrc filltype: ParamSpec says 2/zero but element initializes to 1/nothing
    // - fakesink sync: ParamSpec says true but element initializes to false
    // These tests are skipped because of GStreamer bugs, not GPS bugs.
    // See videotestsrc test which works correctly.

    #[test]
    fn test_create_pads_for_element_requires_app() {
        test_synced(|| {
            use crate::graphmanager as GM;

            let player = Player::new().unwrap();

            // Create a pipeline with a named element to avoid environment-dependent naming
            let pipeline = player
                .create_pipeline("videotestsrc name=vts_pads_test ! fakesink")
                .unwrap();
            let bin = pipeline.dynamic_cast::<gst::Bin>().unwrap();
            let videotestsrc = bin.by_name("vts_pads_test").unwrap();

            let node = GM::Node::new(3, "videotestsrc", GM::NodeType::Source);

            // create_pads_for_element requires app to be initialized
            // It will fail gracefully with GPS_ERROR logs but won't crash
            player.create_pads_for_element(&videotestsrc, &node);

            // The method should complete without panicking
            // (errors are logged but not returned)
        });
    }

    #[test]
    fn test_parse_launch_with_quoted_path() {
        test_synced(|| {
            // Test that GStreamer parse_launch correctly handles quoted paths with spaces
            let description = "filesrc location=\"/path/with spaces/file.mp4\" ! fakesink";
            let result = gst::parse::launch(description);
            assert!(
                result.is_ok(),
                "Failed to parse pipeline with quoted path: {} - error: {:?}",
                description,
                result.err()
            );
        });
    }

    #[test]
    fn test_parse_launch_with_quoted_special_characters() {
        test_synced(|| {
            // Test that GStreamer parse_launch correctly handles quoted paths with special chars
            let description =
                "filesrc location=\"/path/with spaces (and parens)/file.mp4\" ! fakesink";
            let result = gst::parse::launch(description);
            assert!(
                result.is_ok(),
                "Failed to parse pipeline with special characters: {} - error: {:?}",
                description,
                result.err()
            );
        });
    }

    #[test]
    fn test_parse_launch_unquoted_path_fails() {
        test_synced(|| {
            // Verify that unquoted paths with spaces fail to parse correctly
            // This documents the bug that the quoting fix addresses
            let description = "filesrc location=/path/with spaces/file.mp4 ! fakesink";
            let result = gst::parse::launch(description);
            // This should fail because GStreamer will interpret "spaces/file.mp4" as a separate element
            assert!(
                result.is_err(),
                "Expected parse to fail with unquoted path containing spaces"
            );
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

// =============================================================================
// Caps compatibility tests (used by auto-connect feature)
// =============================================================================

#[cfg(test)]
mod caps_test {
    use super::*;
    use crate::gps::PadInfo;

    #[test]
    fn test_caps_compatible_same_type() {
        test_synced(|| {
            // Same caps should be compatible
            assert!(PadInfo::caps_compatible("video/x-raw", "video/x-raw"));
            assert!(PadInfo::caps_compatible("audio/x-raw", "audio/x-raw"));
        });
    }

    #[test]
    fn test_caps_compatible_any() {
        test_synced(|| {
            // ANY caps should be compatible with anything
            assert!(PadInfo::caps_compatible("ANY", "video/x-raw"));
            assert!(PadInfo::caps_compatible("video/x-raw", "ANY"));
            assert!(PadInfo::caps_compatible("ANY", "ANY"));
            assert!(PadInfo::caps_compatible("ANY", "audio/x-raw"));
        });
    }

    #[test]
    fn test_caps_compatible_different_types() {
        test_synced(|| {
            // Different media types should not be compatible
            assert!(!PadInfo::caps_compatible("video/x-raw", "audio/x-raw"));
            assert!(!PadInfo::caps_compatible("audio/x-raw", "video/x-raw"));
        });
    }

    #[test]
    fn test_caps_compatible_with_format() {
        test_synced(|| {
            // Caps with matching formats should be compatible
            assert!(PadInfo::caps_compatible(
                "video/x-raw,format=I420",
                "video/x-raw,format=I420"
            ));

            // video/x-raw should intersect with specific format
            assert!(PadInfo::caps_compatible(
                "video/x-raw",
                "video/x-raw,format=I420"
            ));
        });
    }

    #[test]
    fn test_caps_compatible_incompatible_formats() {
        test_synced(|| {
            // Different specific formats that can't intersect
            // Note: This depends on GStreamer's caps intersection logic
            // Some format combinations may still be compatible through negotiation
            assert!(!PadInfo::caps_compatible(
                "video/x-raw,format=I420,width=640,height=480",
                "video/x-raw,format=RGBA,width=1920,height=1080"
            ));
        });
    }

    #[test]
    fn test_caps_compatible_invalid_caps() {
        test_synced(|| {
            // Invalid caps strings should return false
            assert!(!PadInfo::caps_compatible(
                "invalid/caps/string!",
                "video/x-raw"
            ));
            assert!(!PadInfo::caps_compatible(
                "video/x-raw",
                "invalid/caps/string!"
            ));
            assert!(!PadInfo::caps_compatible("", "video/x-raw"));
            assert!(!PadInfo::caps_compatible("video/x-raw", ""));
        });
    }

    #[test]
    fn test_caps_compatible_encoded_types() {
        test_synced(|| {
            // H264 and VP8 are both video but shouldn't intersect
            assert!(!PadInfo::caps_compatible("video/x-h264", "video/x-vp8"));

            // Same encoded type should be compatible
            assert!(PadInfo::caps_compatible("video/x-h264", "video/x-h264"));
        });
    }

    #[test]
    fn test_caps_compatible_audio_formats() {
        test_synced(|| {
            // Audio caps with different channel counts
            assert!(PadInfo::caps_compatible(
                "audio/x-raw",
                "audio/x-raw,channels=2"
            ));
            assert!(PadInfo::caps_compatible(
                "audio/x-raw,channels=2",
                "audio/x-raw,channels=2"
            ));
        });
    }
}

/// Tests for GStreamer element default value validation.
///
/// These tests scan all elements and check that ParamSpec defaults match actual values.
/// Known GStreamer bugs are in a skip list - the test only fails on NEW issues.
#[cfg(test)]
mod gstreamer_default_value_tests {
    use super::*;
    use gst::glib;

    /// Known GStreamer bugs: (element, property) pairs where ParamSpec != actual value.
    /// These are skipped to avoid test failures. When GStreamer fixes them, they'll
    /// show as "fixed" and can be removed from this list.
    const KNOWN_MISMATCHES: &[(&str, &str)] = &[
        // sync property (true -> false): GstBaseSink subclasses
        ("fakesink", "sync"),
        ("filesink", "sync"),
        ("fdsink", "sync"),
        ("multifilesink", "sync"),
        ("giosink", "sync"),
        ("giostreamsink", "sync"),
        ("splitmuxsink", "sync"),
        ("shout2send", "sync"),
        ("checksumsink", "sync"),
        ("glsinkbin", "sync"),
        ("videocodectestsink", "sync"),
        // qos property (false -> true): GstBaseTransform and GstBaseSink subclasses
        ("videoconvert", "qos"),
        ("videoscale", "qos"),
        ("videoconvertscale", "qos"),
        ("videoflip", "qos"),
        ("gamma", "qos"),
        ("videobalance", "qos"),
        ("alpha", "qos"),
        ("alphacolor", "qos"),
        ("videomedian", "qos"),
        ("edgetv", "qos"),
        ("agingtv", "qos"),
        ("dicetv", "qos"),
        ("warptv", "qos"),
        ("shagadelictv", "qos"),
        ("vertigotv", "qos"),
        ("revtv", "qos"),
        ("quarktv", "qos"),
        ("optv", "qos"),
        ("radioactv", "qos"),
        ("streaktv", "qos"),
        ("rippletv", "qos"),
        ("burn", "qos"),
        ("chromium", "qos"),
        ("dilate", "qos"),
        ("dodge", "qos"),
        ("exclusion", "qos"),
        ("solarize", "qos"),
        ("gaussianblur", "qos"),
        ("deinterlace", "qos"),
        ("videobox", "qos"),
        ("videocrop", "qos"),
        ("videoanalyse", "qos"),
        ("navigationtest", "qos"),
        ("coloreffects", "qos"),
        ("chromahold", "qos"),
        ("glupload", "qos"),
        ("gldownload", "qos"),
        ("glcolorconvert", "qos"),
        ("glcolorbalance", "qos"),
        ("gltransformation", "qos"),
        ("glvideoflip", "qos"),
        ("gleffects", "qos"),
        ("gldifferencematte", "qos"),
        ("glfilterglass", "qos"),
        ("gloverlay", "qos"),
        ("gloverlaycompositor", "qos"),
        ("glalpha", "qos"),
        ("gldeinterlace", "qos"),
        ("glviewconvert", "qos"),
        ("glfilterapp", "qos"),
        ("glshader", "qos"),
        ("glcolorscale", "qos"),
        ("glfiltercube", "qos"),
        ("glimagesinkelement", "qos"),
        ("glimagesink", "qos"),
        // GL effects variants
        ("gleffects_laplacian", "qos"),
        ("gleffects_blur", "qos"),
        ("gleffects_sobel", "qos"),
        ("gleffects_glow", "qos"),
        ("gleffects_sin", "qos"),
        ("gleffects_xray", "qos"),
        ("gleffects_lumaxpro", "qos"),
        ("gleffects_xpro", "qos"),
        ("gleffects_sepia", "qos"),
        ("gleffects_heat", "qos"),
        ("gleffects_square", "qos"),
        ("gleffects_bulge", "qos"),
        ("gleffects_twirl", "qos"),
        ("gleffects_fisheye", "qos"),
        ("gleffects_tunnel", "qos"),
        ("gleffects_stretch", "qos"),
        ("gleffects_squeeze", "qos"),
        ("gleffects_mirror", "qos"),
        ("gleffects_identity", "qos"),
        // Geometric transforms
        ("perspective", "qos"),
        ("fisheye", "qos"),
        ("mirror", "qos"),
        ("square", "qos"),
        ("tunnel", "qos"),
        ("bulge", "qos"),
        ("stretch", "qos"),
        ("waterripple", "qos"),
        ("twirl", "qos"),
        ("sphere", "qos"),
        ("rotate", "qos"),
        ("pinch", "qos"),
        ("marble", "qos"),
        ("kaleidoscope", "qos"),
        ("diffuse", "qos"),
        ("circle", "qos"),
        // Other video filters
        ("smooth", "qos"),
        ("objectdetectionoverlay", "qos"),
        ("rsvgoverlay", "qos"),
        ("gdkpixbufoverlay", "qos"),
        ("lcms", "qos"),
        ("cacatv", "qos"),
        ("aatv", "qos"),
        ("zbar", "qos"),
        ("zxing", "qos"),
        ("combdetect", "qos"),
        ("line21encoder", "qos"),
        ("line21decoder", "qos"),
        ("simplevideomark", "qos"),
        ("simplevideomarkdetect", "qos"),
        ("videodiff", "qos"),
        ("zebrastripe", "qos"),
        ("scenechange", "qos"),
        ("smptealpha", "qos"),
        // Sinks with qos
        ("aasink", "qos"),
        ("ximagesink", "qos"),
        ("xvimagesink", "qos"),
        ("vulkansink", "qos"),
        ("waylandsink", "qos"),
        ("gtkglsink", "qos"),
        ("gtksink", "qos"),
        ("gtkwaylandsink", "qos"),
        ("dfbvideosink", "qos"),
        ("fbdevsink", "qos"),
        ("kmssink", "qos"),
        ("intervideosink", "qos"),
        ("fakevideosink", "qos"),
        ("fakeaudiosink", "qos"),
        ("v4l2sink", "qos"),
        ("vadeinterlace", "qos"),
        ("vapostproc", "qos"),
        // do-timestamp property (false -> true)
        ("udpsrc", "do-timestamp"),
        ("dv1394src", "do-timestamp"),
        ("dc1394src", "do-timestamp"),
        ("dvbsrc", "do-timestamp"),
        ("avdtpsrc", "do-timestamp"),
        // enable-last-sample property (true -> false)
        ("glsinkbin", "enable-last-sample"),
        ("alsasink", "enable-last-sample"),
        ("jackaudiosink", "enable-last-sample"),
        ("openalsink", "enable-last-sample"),
        ("pulsesink", "enable-last-sample"),
        ("oss4sink", "enable-last-sample"),
        ("osssink", "enable-last-sample"),
        // Other known mismatches
        ("glsinkbin", "force-aspect-ratio"),
        ("glsinkbin", "async"),
        ("assrender", "wait-text"),
        ("rsvgoverlay", "fit-to-frame"),
        ("curlhttpsrc", "automatic-eos"),
        ("curlftpsink", "epsv-mode"),
        ("srtserversink", "authentication"),
        ("srtclientsink", "authentication"),
        ("srtserversrc", "authentication"),
        ("srtclientsrc", "authentication"),
        ("srtsink", "authentication"),
        ("srtsrc", "authentication"),
        ("flacenc", "perfect-timestamp"),
        ("speexenc", "perfect-timestamp"),
        ("wavpackenc", "perfect-timestamp"),
        ("vorbisenc", "perfect-timestamp"),
        ("videoparse", "top-field-first"),
        ("aasink", "inversion"),
        ("xvimagesink", "double-buffer"),
        ("modplug", "oversamp"),
        ("a52dec", "lfe"),
        ("souphttpsrc", "automatic-eos"),
        ("souphttpsrc", "ssl-use-system-ca-file"),
        ("giosink", "close-on-stop"),
        ("tsdemux", "parse-private-sections"),
        // frei0r filters - all have qos mismatch (false -> true)
        ("frei0r-filter-mask0mate", "qos"),
        ("frei0r-filter-keyspillm0pup", "qos"),
        ("frei0r-filter-delaygrab", "qos"),
        ("frei0r-filter-nervous", "qos"),
        ("frei0r-filter-alphagrad", "qos"),
        ("frei0r-filter-sobel", "qos"),
        ("frei0r-filter-delay0r", "qos"),
        ("frei0r-filter-bw0r", "qos"),
        ("frei0r-filter-levels", "qos"),
        ("frei0r-filter-tint0r", "qos"),
        ("frei0r-filter-g", "qos"),
        ("frei0r-filter-pr0file", "qos"),
        ("frei0r-filter-coloradj-rgb", "qos"),
        ("frei0r-filter-cairoimagegrid", "qos"),
        ("frei0r-filter-hqdn3d", "qos"),
        ("frei0r-filter-edgeglow", "qos"),
        ("frei0r-filter-vectorscope", "qos"),
        ("frei0r-filter-defish0r", "qos"),
        ("frei0r-filter-medians", "qos"),
        ("frei0r-filter-scale0tilt", "qos"),
        ("frei0r-filter-white-balance--lms-space-", "qos"),
        ("frei0r-filter-rgb-parade", "qos"),
        ("frei0r-filter-facebl0r", "qos"),
        ("frei0r-filter-cartoon", "qos"),
        ("frei0r-filter-white-balance", "qos"),
        ("frei0r-filter-primaries", "qos"),
        ("frei0r-filter-normaliz0r", "qos"),
        ("frei0r-filter-cairogradient", "qos"),
        ("frei0r-filter-iir-blur", "qos"),
        ("frei0r-filter-transparency", "qos"),
        ("frei0r-filter-rgbnoise", "qos"),
        ("frei0r-filter-bgsubtract0r", "qos"),
        ("frei0r-filter-softglow", "qos"),
        ("frei0r-filter-ndvi-filter", "qos"),
        ("frei0r-filter-k-means-clustering", "qos"),
        ("frei0r-filter-alpha0ps", "qos"),
        ("frei0r-filter-opencvfacedetect", "qos"),
        ("frei0r-filter-tehroxx0r", "qos"),
        ("frei0r-filter-color-distance", "qos"),
        ("frei0r-filter-glow", "qos"),
        ("frei0r-filter-saturat0r", "qos"),
        ("frei0r-filter-dither", "qos"),
        ("frei0r-filter-distort0r", "qos"),
        ("frei0r-filter-pr0be", "qos"),
        ("frei0r-filter-colorhalftone", "qos"),
        ("frei0r-filter-r", "qos"),
        ("frei0r-filter-pixeliz0r", "qos"),
        ("frei0r-filter-colorize", "qos"),
        ("frei0r-filter-contrast0r", "qos"),
        ("frei0r-filter-aech0r", "qos"),
        ("frei0r-filter-lens-correction", "qos"),
        ("frei0r-filter-premultiply-or-unpremultiply", "qos"),
        ("frei0r-filter-colortap", "qos"),
        ("frei0r-filter-threelay0r", "qos"),
        ("frei0r-filter-spillsupress", "qos"),
        ("frei0r-filter-brightness", "qos"),
        ("frei0r-filter-light-graffiti", "qos"),
        ("frei0r-filter-select0r", "qos"),
        ("frei0r-filter-threshold0r", "qos"),
        ("frei0r-filter-sharpness", "qos"),
        ("frei0r-filter-baltan", "qos"),
        ("frei0r-filter-equaliz0r", "qos"),
        ("frei0r-filter-emboss", "qos"),
        ("frei0r-filter-curves", "qos"),
        ("frei0r-filter-vertigo", "qos"),
        ("frei0r-filter-twolay0r", "qos"),
        ("frei0r-filter-alphaspot", "qos"),
        ("frei0r-filter-timeout-indicator", "qos"),
        ("frei0r-filter-c0rners", "qos"),
        ("frei0r-filter-squareblur", "qos"),
        ("frei0r-filter-posterize", "qos"),
        ("frei0r-filter-gamma", "qos"),
        ("frei0r-filter-nosync0r", "qos"),
        ("frei0r-filter-3-point-color-balance", "qos"),
        ("frei0r-filter-scanline0r", "qos"),
        ("frei0r-filter-flippo", "qos"),
        ("frei0r-filter-sigmoidaltransfer", "qos"),
        ("frei0r-filter-glitch0r", "qos"),
        ("frei0r-filter-b", "qos"),
        ("frei0r-filter-rgbsplit0r", "qos"),
        ("frei0r-filter-hueshift0r", "qos"),
        ("frei0r-filter-sop-sat", "qos"),
        ("frei0r-filter-nikon-d90-stairstepping-fix", "qos"),
        ("frei0r-filter-letterb0xed", "qos"),
        ("frei0r-filter-perspective", "qos"),
        ("frei0r-filter-luminance", "qos"),
        ("frei0r-filter-vignette", "qos"),
        ("frei0r-filter-invert0r", "qos"),
        ("frei0r-filter-3dflippo", "qos"),
        ("frei0r-filter-bluescreen0r", "qos"),
        ("frei0r-filter-elastic-scale-filter", "qos"),
    ];

    /// Elements to skip entirely (can't be instantiated in test environment)
    const SKIP_ELEMENTS: &[&str] = &[
        "ipcpipelinesink",
        "ipcpipelinesrc",
        "ipcslavepipeline",
        "shmsink",
        "shmsrc",
        "decklinkvideosrc",
        "decklinkvideosink",
        "decklinkaudiosrc",
        "decklinkaudiosink",
    ];

    /// Check if a (element, property) pair is in the known mismatch list
    fn is_known_mismatch(element: &str, property: &str) -> bool {
        KNOWN_MISMATCHES
            .iter()
            .any(|(e, p)| *e == element && *p == property)
    }

    /// Get ParamSpec default for boolean property
    fn get_bool_default(pspec: &glib::ParamSpec) -> Option<bool> {
        if pspec.value_type() == glib::Type::BOOL {
            pspec.default_value().get::<bool>().ok()
        } else {
            None
        }
    }

    #[test]
    fn test_element_default_values() {
        test_synced(|| {
            let registry = gst::Registry::get();
            let factories = registry.features(gst::ElementFactory::static_type());

            let mut tested_elements = 0;
            let mut tested_properties = 0;
            let mut known_issues = Vec::new();
            let mut fixed_issues = Vec::new();
            let mut new_issues = Vec::new();

            for feature in factories.iter() {
                let factory = match feature.clone().downcast::<gst::ElementFactory>() {
                    Ok(f) => f,
                    Err(_) => continue,
                };

                let element_name = factory.name().to_string();

                // Skip problematic elements
                if SKIP_ELEMENTS.iter().any(|e| *e == element_name) {
                    continue;
                }

                // Create element
                let element = match factory.create().build() {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                tested_elements += 1;

                // Check all boolean properties
                for pspec in element.list_properties() {
                    let prop_name = pspec.name().to_string();

                    // Skip name/parent
                    if prop_name == "name" || prop_name == "parent" {
                        continue;
                    }

                    // Only check readable+writable boolean properties
                    let flags = pspec.flags();
                    if !flags.contains(glib::ParamFlags::READABLE)
                        || !flags.contains(glib::ParamFlags::WRITABLE)
                    {
                        continue;
                    }

                    // Only test boolean properties (main source of issues)
                    let paramspec_default = match get_bool_default(&pspec) {
                        Some(v) => v,
                        None => continue,
                    };

                    let actual_value: bool = element.property(&prop_name);
                    tested_properties += 1;

                    if paramspec_default != actual_value {
                        let issue = format!(
                            "{}::{} (ParamSpec: {}, Actual: {})",
                            element_name, prop_name, paramspec_default, actual_value
                        );

                        if is_known_mismatch(&element_name, &prop_name) {
                            known_issues.push(issue);
                        } else {
                            new_issues.push(issue);
                        }
                    } else if is_known_mismatch(&element_name, &prop_name) {
                        // Was in skip list but now matches - GStreamer fixed it!
                        fixed_issues.push(format!("{}::{}", element_name, prop_name));
                    }
                }
            }

            // Print summary
            println!("\n=== GStreamer Default Value Test ===");
            println!("Elements tested: {}", tested_elements);
            println!("Properties tested: {}", tested_properties);
            println!("Known issues (skipped): {}", known_issues.len());
            println!("Fixed issues: {}", fixed_issues.len());
            println!("New issues: {}", new_issues.len());

            if !fixed_issues.is_empty() {
                println!("\nFIXED (remove from KNOWN_MISMATCHES):");
                for issue in &fixed_issues {
                    println!("  - {}", issue);
                }
            }

            if !new_issues.is_empty() {
                println!("\nNEW ISSUES (add to KNOWN_MISMATCHES or report to GStreamer):");
                for issue in &new_issues {
                    println!("  - {}", issue);
                }
            }

            // Only fail on NEW issues
            assert!(
                new_issues.is_empty(),
                "Found {} new default value mismatches! Add to KNOWN_MISMATCHES or report to GStreamer.",
                new_issues.len()
            );
        });
    }
}

// =============================================================================
// GStreamer-specific DOT file parsing tests
// =============================================================================

#[cfg(test)]
mod gstreamer_dot_test {
    use crate::gps::dot_parsing;
    use crate::graphmanager::dot_parser::{DotGraph, DotLoader};
    use std::collections::HashMap;

    /// GStreamer-specific test loader for DOT parsing tests.
    /// Uses shared parsing functions from dot_parsing module to avoid code duplication.
    struct GstreamerTestLoader;

    impl DotLoader for GstreamerTestLoader {
        fn class_to_type_name(&self, class_name: &str) -> String {
            dot_parsing::class_to_type_name(class_name)
        }

        fn parse_node_label(&self, label: &str) -> HashMap<String, String> {
            // Test loader accepts all properties (no security validation needed in tests)
            dot_parsing::parse_node_label(label, |_, _| true)
        }

        fn extract_port_name_from_id(&self, dot_id: &str) -> Option<String> {
            dot_parsing::extract_port_name_from_id(dot_id)
        }

        fn extract_node_instance_from_id(&self, port_id: &str) -> Option<String> {
            dot_parsing::extract_node_instance_from_id(port_id)
        }

        fn extract_graph_metadata(
            &self,
            attributes: &[(String, String)],
        ) -> HashMap<String, String> {
            dot_parsing::extract_graph_metadata(attributes)
        }

        fn is_node_subgraph(&self, id: &str) -> bool {
            dot_parsing::is_node_subgraph(id)
        }

        fn is_port_subgraph(&self, id: &str) -> bool {
            dot_parsing::is_port_subgraph(id)
        }
    }

    // Helper to get the test loader
    fn test_loader() -> GstreamerTestLoader {
        GstreamerTestLoader
    }

    #[test]
    fn dot_parse_node_label_basic() {
        let loader = test_loader();
        let label = "GstVideoTestSrc\nvideotestsrc0\n[>]";
        let metadata = loader.parse_node_label(label);
        assert_eq!(
            metadata.get("class_name"),
            Some(&"GstVideoTestSrc".to_string())
        );
        assert_eq!(
            metadata.get("instance_name"),
            Some(&"videotestsrc0".to_string())
        );
        assert_eq!(metadata.get("state"), Some(&">".to_string()));
    }

    #[test]
    fn dot_parse_node_label_with_escaped_newlines() {
        let loader = test_loader();
        let label = "GstVideoTestSrc\\nvideotestsrc0\\n[>]";
        let metadata = loader.parse_node_label(label);
        assert_eq!(
            metadata.get("class_name"),
            Some(&"GstVideoTestSrc".to_string())
        );
        assert_eq!(
            metadata.get("instance_name"),
            Some(&"videotestsrc0".to_string())
        );
        assert_eq!(metadata.get("state"), Some(&">".to_string()));
    }

    #[test]
    fn dot_parse_node_label_with_properties() {
        let loader = test_loader();
        let label = "GstAutoVideoSink\\nautovideosink0\\n[>]\\nfilter-caps=video/x-raw";
        let metadata = loader.parse_node_label(label);
        assert_eq!(
            metadata.get("class_name"),
            Some(&"GstAutoVideoSink".to_string())
        );
        assert_eq!(
            metadata.get("instance_name"),
            Some(&"autovideosink0".to_string())
        );
        assert_eq!(metadata.get("state"), Some(&">".to_string()));
        assert_eq!(
            metadata.get("filter-caps"),
            Some(&"video/x-raw".to_string())
        );
    }

    #[test]
    fn dot_parse_node_label_with_html_newlines() {
        let loader = test_loader();
        // Tooltips use &#10; as newline separator
        let tooltip = "GstFileSrc&#10;filesrc0&#10;[>]&#10;location=\"/path/with spaces/file.mkv\"";
        let metadata = loader.parse_node_label(tooltip);
        assert_eq!(metadata.get("class_name"), Some(&"GstFileSrc".to_string()));
        assert_eq!(metadata.get("instance_name"), Some(&"filesrc0".to_string()));
        assert_eq!(metadata.get("state"), Some(&">".to_string()));
        assert_eq!(
            metadata.get("location"),
            Some(&"/path/with spaces/file.mkv".to_string())
        );
    }

    #[test]
    fn dot_parse_tooltip_with_full_path() {
        let loader = test_loader();
        // This simulates what GStreamer outputs in DOT files with tooltips
        // Tests that paths with spaces, parentheses, and special chars are preserved
        let tooltip = "GstFileSrc&#10;filesrc0&#10;[>]&#10;location=\"/home/user/Videos/Movie Title (2023) 1080p H264.mkv\"";
        let metadata = loader.parse_node_label(tooltip);
        assert_eq!(metadata.get("class_name"), Some(&"GstFileSrc".to_string()));
        assert_eq!(metadata.get("instance_name"), Some(&"filesrc0".to_string()));
        assert_eq!(
            metadata.get("location"),
            Some(&"/home/user/Videos/Movie Title (2023) 1080p H264.mkv".to_string())
        );
    }

    #[test]
    fn dot_parse_tooltip_properties_only() {
        let loader = test_loader();
        // Some GStreamer DOT tooltips only contain properties (no class/instance/state)
        // They start with &#10; (newline) followed by property=value pairs
        let tooltip = "&#10;location=\"/home/user/Videos/test.mkv\"&#10;caps=video/x-raw";
        let metadata = loader.parse_node_label(tooltip);
        // Properties-only format doesn't have class_name or instance_name in metadata
        assert_eq!(metadata.get("class_name"), None);
        assert_eq!(metadata.get("instance_name"), None);
        assert_eq!(metadata.get("state"), None);
        // But properties should be parsed correctly
        assert_eq!(
            metadata.get("location"),
            Some(&"/home/user/Videos/test.mkv".to_string())
        );
        assert_eq!(metadata.get("caps"), Some(&"video/x-raw".to_string()));
    }

    #[test]
    fn dot_parse_tooltip_direct_property() {
        let loader = test_loader();
        // Some tooltips contain only properties directly (no leading newline)
        // e.g., tooltip="location=\"/path/to/file.mkv\""
        let tooltip = "location=\"/home/user/Videos/Big Buck Bunny.m4v\"";
        let metadata = loader.parse_node_label(tooltip);
        // Direct property format doesn't have class_name or instance_name in metadata
        assert_eq!(metadata.get("class_name"), None);
        assert_eq!(metadata.get("instance_name"), None);
        assert_eq!(metadata.get("state"), None);
        // Property should be parsed correctly
        assert_eq!(
            metadata.get("location"),
            Some(&"/home/user/Videos/Big Buck Bunny.m4v".to_string())
        );
    }

    #[test]
    fn dot_parse_file_with_tooltip() {
        let loader = test_loader();
        let dot_content = include_str!("../../data/dots/gst126_filesrc_special_chars_tooltip.dot");
        let graph = DotGraph::parse(dot_content, &loader).expect("Failed to parse DOT file");

        // Find the filesrc node
        let filesrc = graph
            .nodes
            .iter()
            .find(|n| n.instance_name == "filesrc0")
            .expect("Should find filesrc0 node");

        // The location property should be the full path from the tooltip, not truncated
        let location = filesrc.metadata.get("location");
        assert!(location.is_some(), "filesrc should have location property");

        let loc_value = location.unwrap();
        // The full path should contain special characters (spaces, parentheses, ampersand)
        assert!(
            loc_value.contains("Special & Chars.mkv"),
            "Location should contain full filename with special chars from tooltip, got: {}",
            loc_value
        );
        assert!(
            loc_value.contains("(2023)"),
            "Location should contain parentheses, got: {}",
            loc_value
        );
        assert!(
            !loc_value.contains("…"),
            "Location should not be truncated, got: {}",
            loc_value
        );
    }

    #[test]
    fn dot_extract_port_name_old_format() {
        let loader = test_loader();
        // Old GStreamer 1.24 format: ELEMENT_ADDR_PORTNAME_ADDR
        let port_id = "videotestsrc0_0x5d75d6505510_src_0x5d75d65059f0";
        let result = loader.extract_port_name_from_id(port_id);
        assert_eq!(result, Some("src".to_string()));

        let port_id = "autovideosink0_0x5d75d6507b00_sink_0x5d75d65089f0";
        let result = loader.extract_port_name_from_id(port_id);
        assert_eq!(result, Some("sink".to_string()));
    }

    #[test]
    fn dot_extract_port_name_new_format() {
        let loader = test_loader();
        // New GStreamer format: node_ELEMENT_ADDR_node_PORTNAME_ADDR
        let port_id = "node_videotestsrc0_0x123456_node_src_0x789abc";
        let result = loader.extract_port_name_from_id(port_id);
        assert_eq!(result, Some("src".to_string()));

        let port_id = "node_autovideosink0_0x123456_node_sink_0x789abc";
        let result = loader.extract_port_name_from_id(port_id);
        assert_eq!(result, Some("sink".to_string()));
    }

    #[test]
    fn dot_extract_node_instance_basic() {
        let loader = test_loader();
        // Standard DOT port ID format
        let port_id = "node_videotestsrc0_0x123456_node_src_0x789abc";
        let result = loader.extract_node_instance_from_id(port_id);
        assert_eq!(result, Some("videotestsrc0".to_string()));
    }

    #[test]
    fn dot_extract_instance_no_collision() {
        let loader = test_loader();
        // Ensure "videotestsrc0" doesn't match "videotestsrc01"
        // This tests the substring collision fix
        let port_id_0 = "node_videotestsrc0_0x123456_node_src_0x789abc";
        let port_id_01 = "node_videotestsrc01_0x123456_node_src_0x789abc";

        let result_0 = loader.extract_node_instance_from_id(port_id_0);
        let result_01 = loader.extract_node_instance_from_id(port_id_01);

        assert_eq!(result_0, Some("videotestsrc0".to_string()));
        assert_eq!(result_01, Some("videotestsrc01".to_string()));

        // They should be different - this is the key assertion for the collision fix
        assert_ne!(result_0, result_01);
    }

    #[test]
    fn dot_extract_instance_with_hyphen_normalized() {
        let loader = test_loader();
        // Instance names with hyphens are normalized to underscores in DOT format
        let port_id = "node_avdec_h264_0_0x123456_node_sink_0x789abc";
        let result = loader.extract_node_instance_from_id(port_id);
        assert_eq!(result, Some("avdec_h264_0".to_string()));
    }

    #[test]
    fn dot_extract_instance_from_sink_port() {
        let loader = test_loader();
        // Ghost/proxy port format
        let port_id = "_node_proxypad0_0x13b93d0e0";
        let result = loader.extract_node_instance_from_id(port_id);
        assert_eq!(result, Some("proxypad0".to_string()));
    }

    #[test]
    fn dot_parse_old_gstreamer_124_format() {
        let loader = test_loader();
        // Test parsing old GStreamer 1.24 format which uses "cluster_" instead of "cluster_node_"
        // and includes proxypad nodes that should be filtered out
        let dot_content = include_str!("../../data/dots/gst124_videotestsrc_autovideosink.dot");
        let graph = DotGraph::parse(dot_content, &loader).expect("Failed to parse DOT file");

        // Should have 2 top-level nodes
        assert_eq!(
            graph.nodes.len(),
            2,
            "Should have 2 top-level nodes (videotestsrc0, autovideosink0)"
        );

        // Verify nodes
        assert!(
            graph
                .nodes
                .iter()
                .any(|n| n.instance_name == "videotestsrc0"),
            "Should find videotestsrc0"
        );
        assert!(
            graph
                .nodes
                .iter()
                .any(|n| n.instance_name == "autovideosink0"),
            "Should find autovideosink0"
        );

        // Verify videotestsrc0 has exactly 1 port (src)
        let videotestsrc_ports: Vec<_> = graph
            .ports
            .iter()
            .filter(|p| p.node_dot_id.contains("videotestsrc0"))
            .collect();
        assert_eq!(
            videotestsrc_ports.len(),
            1,
            "videotestsrc0 should have exactly 1 port, got: {:?}",
            videotestsrc_ports
        );
        assert_eq!(videotestsrc_ports[0].name, "src");

        // Verify autovideosink0 has exactly 1 port (sink) - proxypad should be filtered out
        let autovideosink_ports: Vec<_> = graph
            .ports
            .iter()
            .filter(|p| {
                p.node_dot_id.contains("autovideosink0") && !p.node_dot_id.contains("actual_sink")
            })
            .collect();
        assert_eq!(
            autovideosink_ports.len(),
            1,
            "autovideosink0 should have exactly 1 port (sink), proxypad should be filtered. Got: {:?}",
            autovideosink_ports
        );
        assert_eq!(autovideosink_ports[0].name, "sink");

        // Verify no proxypad nodes were parsed
        let proxypad_count = graph
            .ports
            .iter()
            .filter(|p| p.name.contains("proxypad") || p.dot_id.contains("proxypad"))
            .count();
        assert_eq!(
            proxypad_count, 0,
            "Proxypads should be filtered out, found: {}",
            proxypad_count
        );

        // Verify no links involving proxypads were parsed
        let proxypad_link_count = graph
            .links
            .iter()
            .filter(|l| l.from_port_id.contains("proxypad") || l.to_port_id.contains("proxypad"))
            .count();
        assert_eq!(
            proxypad_link_count, 0,
            "Links involving proxypads should be filtered out, found: {}",
            proxypad_link_count
        );

        // Should have exactly 1 link (videotestsrc0 -> autovideosink0)
        assert_eq!(
            graph.links.len(),
            1,
            "Should have exactly 1 link between top-level elements"
        );
    }

    // Malformed input tests - verify error handling for invalid DOT content

    #[test]
    fn dot_parse_invalid_syntax() {
        let loader = test_loader();
        // Completely invalid DOT syntax
        let result = DotGraph::parse("this is not valid dot syntax", &loader);
        assert!(result.is_err(), "Should fail on invalid DOT syntax");
    }

    #[test]
    fn dot_parse_empty_string() {
        let loader = test_loader();
        let result = DotGraph::parse("", &loader);
        assert!(result.is_err(), "Should fail on empty string");
    }

    #[test]
    fn dot_parse_unclosed_graph() {
        let loader = test_loader();
        // Missing closing brace
        let result = DotGraph::parse("digraph pipeline { node1", &loader);
        assert!(result.is_err(), "Should fail on unclosed graph");
    }

    #[test]
    fn dot_parse_empty_graph() {
        let loader = test_loader();
        // Valid syntax but empty - should succeed with empty result
        let result = DotGraph::parse("digraph pipeline { }", &loader);
        assert!(result.is_ok(), "Empty graph should parse successfully");
        let graph = result.unwrap();
        assert!(graph.nodes.is_empty(), "Empty graph should have no nodes");
        assert!(graph.ports.is_empty(), "Empty graph should have no ports");
        assert!(graph.links.is_empty(), "Empty graph should have no links");
    }

    #[test]
    fn dot_parse_non_digraph() {
        let loader = test_loader();
        // GStreamer DOT files are always digraphs, but undirected graphs are valid DOT.
        // The parser should handle this gracefully without panicking.
        let result = DotGraph::parse("graph pipeline { a -- b }", &loader);
        // Undirected edges (--) are not processed as directed edges, so result should
        // be ok but with no links extracted
        assert!(result.is_ok(), "Should parse without error");
        let graph = result.unwrap();
        assert!(
            graph.links.is_empty(),
            "Undirected edges should not create links"
        );
    }

    #[test]
    fn dot_parse_gst_version_present() {
        let loader = test_loader();
        // DOT file with gst_version attribute declared
        let dot_content = r#"digraph pipeline {
            gst_version="1.26.0";
            rankdir=LR;
        }"#;
        let result = DotGraph::parse(dot_content, &loader);
        assert!(result.is_ok(), "Should parse successfully");
        let graph = result.unwrap();
        assert_eq!(
            graph.metadata.get("gst_version"),
            Some(&"1.26.0".to_string()),
            "Should extract gst_version attribute"
        );
    }

    #[test]
    fn dot_parse_gst_version_absent() {
        let loader = test_loader();
        // Standard DOT file without gst_version attribute
        let dot_content = r#"digraph pipeline {
            rankdir=LR;
            fontname="sans";
        }"#;
        let result = DotGraph::parse(dot_content, &loader);
        assert!(result.is_ok(), "Should parse successfully");
        let graph = result.unwrap();
        assert_eq!(
            graph.metadata.get("gst_version"),
            None,
            "Should be None when gst_version not present"
        );
    }

    #[test]
    fn dot_parse_gst_version_124_format() {
        let loader = test_loader();
        // DOT file declaring older GStreamer version
        let dot_content = r#"digraph pipeline {
            gst_version="1.24.3";
            label="test pipeline";
        }"#;
        let result = DotGraph::parse(dot_content, &loader);
        assert!(result.is_ok(), "Should parse successfully");
        let graph = result.unwrap();
        assert_eq!(
            graph.metadata.get("gst_version"),
            Some(&"1.24.3".to_string()),
            "Should extract 1.24.x version"
        );
    }
}

// =============================================================================
// Auto-connect integration tests (GStreamer-specific caps compatibility)
// =============================================================================

#[cfg(test)]
mod auto_connect_test {
    use super::*;
    use crate::gps::PadInfo;
    use crate::graphmanager::{GraphView, NodeType, PortDirection, PortPresence, PropertyExt};

    #[test]
    fn auto_connect_find_compatible_port_by_caps() {
        test_synced(|| {
            let graphview = GraphView::new();

            // Create source node with video output port
            let source = graphview.create_node("video_source", NodeType::Source);
            graphview.add_node(source);
            let src_port =
                graphview.create_port("src", PortDirection::Output, PortPresence::Always);
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

            // Use GStreamer PadInfo::caps_compatible instead of string matching
            let compatible_port = target_node
                .all_ports(PortDirection::Input)
                .into_iter()
                .filter(|p| graphview.port_is_linked(p.id()).is_none())
                .find(|p| {
                    let port_caps =
                        PropertyExt::property(p, "_caps").unwrap_or_else(|| "ANY".to_string());
                    PadInfo::caps_compatible(&from_caps, &port_caps)
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
            let src_port =
                graphview.create_port("src", PortDirection::Output, PortPresence::Always);
            src_port.add_property("_caps", "video/x-raw");
            let mut source = graphview.node(1).unwrap();
            graphview.add_port_to_node(&mut source, src_port);

            // Create sink node with only audio input port
            let sink = graphview.create_node("audio_sink", NodeType::Sink);
            graphview.add_node(sink);

            let audio_sink =
                graphview.create_port("sink", PortDirection::Input, PortPresence::Always);
            audio_sink.add_property("_caps", "audio/x-raw");
            let mut sink = graphview.node(2).unwrap();
            graphview.add_port_to_node(&mut sink, audio_sink);

            // Find free input ports on the sink node
            let target_node = graphview.node(2).unwrap();
            let from_port = graphview.node(1).unwrap().port(1).unwrap();
            let from_caps =
                PropertyExt::property(&from_port, "_caps").unwrap_or_else(|| "ANY".to_string());

            // Use GStreamer PadInfo::caps_compatible instead of string matching
            let compatible_port = target_node
                .all_ports(PortDirection::Input)
                .into_iter()
                .filter(|p| graphview.port_is_linked(p.id()).is_none())
                .find(|p| {
                    let port_caps =
                        PropertyExt::property(p, "_caps").unwrap_or_else(|| "ANY".to_string());
                    PadInfo::caps_compatible(&from_caps, &port_caps)
                });

            assert!(
                compatible_port.is_none(),
                "Should not find a compatible port when caps don't match"
            );
        });
    }

    #[test]
    fn auto_connect_with_any_caps() {
        test_synced(|| {
            let graphview = GraphView::new();

            // Create source node with ANY caps (permissive)
            let source = graphview.create_node("source", NodeType::Source);
            graphview.add_node(source);
            let src_port =
                graphview.create_port("src", PortDirection::Output, PortPresence::Always);
            src_port.add_property("_caps", "ANY");
            let mut source = graphview.node(1).unwrap();
            graphview.add_port_to_node(&mut source, src_port);

            // Create sink node with specific audio caps
            let sink = graphview.create_node("audio_sink", NodeType::Sink);
            graphview.add_node(sink);
            let audio_sink =
                graphview.create_port("sink", PortDirection::Input, PortPresence::Always);
            audio_sink.add_property("_caps", "audio/x-raw");
            let mut sink = graphview.node(2).unwrap();
            graphview.add_port_to_node(&mut sink, audio_sink);

            // ANY should be compatible with anything
            let from_caps = "ANY";
            let target_node = graphview.node(2).unwrap();

            // Use GStreamer PadInfo::caps_compatible instead of string matching
            let compatible_port = target_node
                .all_ports(PortDirection::Input)
                .into_iter()
                .filter(|p| graphview.port_is_linked(p.id()).is_none())
                .find(|p| {
                    let port_caps =
                        PropertyExt::property(p, "_caps").unwrap_or_else(|| "ANY".to_string());
                    PadInfo::caps_compatible(from_caps, &port_caps)
                });

            assert!(
                compatible_port.is_some(),
                "ANY caps should be compatible with any port"
            );
        });
    }
}

// =============================================================================
// WebSocket tests
// =============================================================================

#[cfg(test)]
mod websocket_test {
    use crate::gps::websocket::{
        run_server_blocking, ServerHandle, SnapshotPipeline, SnapshotRequest, SnapshotResponse,
        TypedMessage, WebSocketError, WsAddress,
    };
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;
    use tungstenite::{connect, Message};

    // ========================================================================
    // Protocol structure tests
    // ========================================================================

    #[test]
    fn test_snapshot_request_serialization_without_id() {
        let req = SnapshotRequest {
            id: None,
            msg_type: "Snapshot".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"type":"Snapshot"}"#);
    }

    #[test]
    fn test_snapshot_request_serialization_with_id() {
        let req = SnapshotRequest {
            id: Some("test-id-1".to_string()),
            msg_type: "Snapshot".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"id":"test-id-1","type":"Snapshot"}"#);
    }

    #[test]
    fn test_typed_message_deserialization() {
        let json = r#"{"type":"Hello"}"#;
        let msg: TypedMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msg_type, "Hello");
    }

    // ========================================================================
    // URL parsing tests
    // ========================================================================

    #[test]
    fn test_ws_address_parse_valid_localhost() {
        let addr = WsAddress::parse("ws://localhost:8080").unwrap();
        assert_eq!(addr.host, "localhost");
        assert_eq!(addr.port, 8080);
        assert_eq!(addr.bind_addr(), "localhost:8080");
    }

    #[test]
    fn test_ws_address_parse_default_port() {
        // ws:// scheme has a standard default port of 80
        let addr = WsAddress::parse("ws://localhost").unwrap();
        assert_eq!(addr.host, "localhost");
        assert_eq!(addr.port, 80);
    }

    #[test]
    fn test_ws_address_parse_wss_scheme_rejected() {
        // wss:// is not supported (TLS not implemented)
        let result = WsAddress::parse("wss://secure.example.com:443");
        assert!(result.is_err());
        if let Err(WebSocketError::InvalidUrl(_, msg)) = result {
            assert!(msg.contains("TLS not implemented"));
        } else {
            panic!("Expected InvalidUrl error");
        }
    }

    #[test]
    fn test_ws_address_parse_invalid_scheme() {
        let result = WsAddress::parse("http://localhost:8080");
        assert!(result.is_err());
        if let Err(WebSocketError::InvalidUrl(url, msg)) = result {
            assert!(url.contains("http://"));
            assert!(msg.contains("scheme"));
        } else {
            panic!("Expected InvalidUrl error");
        }
    }

    #[test]
    fn test_ws_address_parse_missing_host() {
        let result = WsAddress::parse("ws://");
        assert!(result.is_err());
    }

    #[test]
    fn test_snapshot_response_deserialization() {
        let json = r#"{
            "type": "SnapshotResponse",
            "pipelines": [
                {"name": "pipeline0", "dot": "digraph { a -> b }"}
            ]
        }"#;
        let response: SnapshotResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.msg_type, "SnapshotResponse");
        assert_eq!(response.pipelines.len(), 1);
        assert_eq!(response.pipelines[0].name, Some("pipeline0".to_string()));
        assert_eq!(
            response.pipelines[0].dot,
            Some("digraph { a -> b }".to_string())
        );
    }

    #[test]
    fn test_snapshot_pipeline_without_dot() {
        let json = r#"{"name": "pipeline0"}"#;
        let pipeline: SnapshotPipeline = serde_json::from_str(json).unwrap();
        assert_eq!(pipeline.name, Some("pipeline0".to_string()));
        assert!(pipeline.dot.is_none());
    }

    // ========================================================================
    // Integration tests - Server mode (GPS listens for connections)
    // ========================================================================

    /// Helper to find an available port for testing
    fn find_available_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    /// Creates a mock tracer client that sends Hello and responds with DOT
    fn start_mock_tracer_client(
        port: u16,
        dot_content: &str,
    ) -> thread::JoinHandle<Result<(), String>> {
        let dot_content = dot_content.to_string();
        thread::spawn(move || {
            // Give server time to start listening
            thread::sleep(Duration::from_millis(200));

            let ws_url = format!("ws://127.0.0.1:{}", port);
            let (mut socket, _) = connect(&ws_url).map_err(|e| e.to_string())?;

            // Send Hello
            socket
                .send(Message::Text(r#"{"type":"Hello"}"#.to_string()))
                .map_err(|e| e.to_string())?;

            // Wait for Snapshot request
            loop {
                let msg = socket.read().map_err(|e| e.to_string())?;
                if let Message::Text(text) = msg {
                    if let Ok(typed) = serde_json::from_str::<TypedMessage>(&text) {
                        if typed.msg_type == "Snapshot" {
                            // Send SnapshotResponse
                            let response = format!(
                                r#"{{"type":"SnapshotResponse","pipelines":[{{"name":"test","dot":"{}"}}]}}"#,
                                dot_content.replace('"', "\\\"")
                            );
                            socket
                                .send(Message::Text(response))
                                .map_err(|e| e.to_string())?;
                            break;
                        }
                    }
                }
            }
            Ok(())
        })
    }

    #[test]
    fn test_server_mode_receives_dot_from_tracer() {
        let port = find_available_port();
        let expected_dot = "digraph { videosrc -> videosink }";

        // Start mock tracer client (will connect after server starts)
        let client_handle = start_mock_tracer_client(port, expected_dot);

        // Run server blocking (this simulates run_server_blocking)
        let bind_addr = format!("127.0.0.1:{}", port);
        let handle = ServerHandle::new();
        let result = run_server_blocking(&bind_addr, &handle);

        // Verify we got the DOT content
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_dot);

        // Wait for client to finish
        let client_result = client_handle.join().unwrap();
        assert!(client_result.is_ok());
    }

    #[test]
    fn test_server_mode_handles_extra_messages_before_hello() {
        let port = find_available_port();
        let expected_dot = "digraph { a -> b }";

        // Start a custom client that sends extra messages before Hello
        let client_handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));

            let ws_url = format!("ws://127.0.0.1:{}", port);
            let (mut socket, _) = connect(&ws_url).unwrap();

            // Send some non-Hello messages first
            socket
                .send(Message::Text(r#"{"type":"Ping"}"#.to_string()))
                .unwrap();
            socket
                .send(Message::Text(
                    r#"{"type":"Status","ready":true}"#.to_string(),
                ))
                .unwrap();

            // Now send Hello
            socket
                .send(Message::Text(r#"{"type":"Hello"}"#.to_string()))
                .unwrap();

            // Wait for Snapshot request and respond
            loop {
                let msg = socket.read().unwrap();
                if let Message::Text(text) = msg {
                    if let Ok(typed) = serde_json::from_str::<TypedMessage>(&text) {
                        if typed.msg_type == "Snapshot" {
                            let response = format!(
                                r#"{{"type":"SnapshotResponse","pipelines":[{{"name":"test","dot":"{}"}}]}}"#,
                                expected_dot
                            );
                            socket.send(Message::Text(response)).unwrap();
                            break;
                        }
                    }
                }
            }
        });

        let bind_addr = format!("127.0.0.1:{}", port);
        let handle = ServerHandle::new();
        let result = run_server_blocking(&bind_addr, &handle);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_dot);

        client_handle.join().unwrap();
    }

    // ========================================================================
    // Cancellation tests
    // ========================================================================

    #[test]
    fn test_server_cancellation_while_waiting_for_connection() {
        let port = find_available_port();
        let bind_addr = format!("127.0.0.1:{}", port);

        let handle = ServerHandle::new();
        let handle_clone = handle.clone();

        // Spawn server in a thread
        let server_thread = thread::spawn(move || run_server_blocking(&bind_addr, &handle_clone));

        // Give server time to start listening
        thread::sleep(Duration::from_millis(100));

        // Cancel the server
        handle.cancel();

        // Server should return Cancelled error
        let result = server_thread.join().unwrap();
        assert!(matches!(result, Err(WebSocketError::Cancelled)));
    }
}
