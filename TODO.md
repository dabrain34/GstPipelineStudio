TODO:

- [x] Fix c.fill issue
- [x] Create Element structure with pads and connections
- [x] Get a list of GStreamer elements in dialog add plugin
- [x] Add plugin details in the element dialog
- [x] Draw element with its pad
- [x] Be able to move the element on Screen
- [x] Create connection between element
- [] Control the connection between element
  - [x] unable to connect in and in out and out
  - [] unable to connect element with incompatible caps.
  - [x] unable to connect a port which is already connected
- [x] create contextual menu on pad or element
- [] upclass the element
- [] create a crate for graphview/node/port
- [x] save/load pipeline
- [x] Run a pipeline with GStreamer
- [x] Run the pipeline with GStreamer
- [x] Control the pipeline with GStreamer
- [x] Define the license
- [] check that a node accept to create a port on request (input/output)
- [x] select nodes/links with a Trait Selectable
- [x] be able to remove a link by selecting it
- [x] Connect the logs to the window
- [] Create a window for the video output
- [] Add multiple graphviews with tabs.

## bugs

- [x] crash with x11 on contextual menu
- [] check that element exists before creating it on file load.
- [x] open multiple times dialog (About) prevent to close it.

## Code cleanup

[] remove useless code from graphview
[] Move render to a specific module
[x] Move GST render to a specific module
