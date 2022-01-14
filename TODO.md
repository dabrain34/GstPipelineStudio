## 0.1.0

- [x] Fix c.fill issue
- [x] Create Element structure with pads and connections
- [x] Get a list of GStreamer elements in dialog add plugin
- [x] Add plugin details in the element dialog
- [x] Draw element with its pad
- [x] Be able to move the element on Screen
- [x] Create connection between element
- [x] create contextual menu on pad or element
- [x] save/load pipeline
- [x] Run a pipeline with GStreamer
- [x] Run the pipeline with GStreamer
- [x] Control the pipeline with GStreamer
- [x] select nodes/links with a Trait Selectable
- [x] be able to remove a link by selecting it
- [x] Connect the logs to the window
- [x] Define the license
- [x] crash with x11 on contextual menu
- [x] open multiple times dialog (About) prevent to close it.
- [x] remove useless code from graphview
- [x] Move render to a specific module
- [x] Move GST render to a specific module

## TODO

- [ ] Control the connection between element
  - [x] unable to connect in and in out and out
  - [ ] unable to connect element with incompatible caps.
  - [x] unable to connect a port which is already connected
- [ ] create a crate for graphview/node/port
- [ ] check that a node accept to create a port on request (input/output)
- [ ] Create a window for the video output
- [ ] Add multiple graphviews with tabs.
- [ ] Property window in the main window
- [ ] Connect the GPS status to GST status
- [ ] Implement graph dot render/load
- [ ] Implement a command line parser to graph
- [ ] Unable to create a pad in an element without the template
- [ ] Remove a pad from a node
- [ ] Implement graphview unit test
- [ ] Implement pipeline unit test
- [x] Save node position in XML
- [x] Autosave the graph
- [ ] Check the pîpeline live
- [ ] Add probes on each pad to monitor the pipeline
- [ ] Display pad properties with tooltip hover
- [ ] Render a media file
- [ ] Offer compatible element to a pad (autorender)
- [ ] Display tags/meta/message detected
- [ ] Seek to position
- [x] Icon install
- [ ] Flatpak infrastructure
- [ ] handle the caps setter
- [x] Logger in file/app all over the app

## bugs

- [ ] check that element exists before creating it on file load.
