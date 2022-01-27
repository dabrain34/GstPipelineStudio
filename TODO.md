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

### Graphview

- [ ] create a crate for graphview/node/port
- [x] Remove a port from a node if possible
- [x] Implement graphview unit test
- [x] add a css class for pad (presence always or sometimes)
- [x] Add property to port to store some specific value(Caps)

### GStreamer:

- [ ] Implement pipeline unit test

### app

- [x] check that a node accept to create a port on request (input/output)
- [ ] Control the connection between element
  - [x] unable to connect in and in out and out
  - [ ] unable to connect element with incompatible caps.
  - [x] unable to connect a port which is already connected
  - [ ] Create a window for the video output
- [ ] Add multiple graphviews with tabs.
- [x] Property window in the main window
- [x] Connect the GPS status to GST status
- [ ] Implement graph dot render/load
- [ ] Implement a command line parser to graph
- [x] Render the parse launch line in a message box
- [x] Prevent to create a pad in an element without the template
- [x] Check the pipeline validity
- [x] Save node position in XML
- [x] Autosave the graph
- [x] Logger in file/app all over the app
- [ ] handle the caps setter element
- [ ] Add probes on each pad to monitor the pipeline
- [x] Display pad properties with tooltip hover
- [ ] Render a media file
- [ ] Offer compatible element to a pad (autorender)
- [ ] Display tags/meta/message detected
- [ ] Seek to position
- [ ] Use one listbox with name, favorites and rank (sort list)
- [x] See the link creation with a dashed line

### CI/Infra

- [x] Icon install
- [x] Flatpak infrastructure
- [ ] Create a macos/windows job

## bugs

- [ ] check that element exists before creating it on file load.
