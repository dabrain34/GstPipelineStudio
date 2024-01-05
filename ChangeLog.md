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

## 0.2.0

### Graphview

- [x] Remove a port from a node when its possible (Presence support)
- [x] Implement graphview unit test
- [x] Add a css class for pad (presence always or sometimes)
- [x] Add properties to Port to store some specific value (ie Caps)
- [x] Unable to connect a port which is already connected
- [x] Unable to connect port with same directions (in/in, out/out)

### GStreamer:

- [x] Add seek support
- [x] Use of gtk4paintablesink

### app

- [x] Check that a node accepts to create a port on request (input/output)
- [x] Render the parse launch line in a message box
- [x] Prevent to create a pad in an element without the template
- [x] Check the pipeline validity
- [x] Save node position in XML
- [x] Auto-save the graph
- [x] Logger in file/app all over the app
- [x] Property window in the main window
- [x] Connect the GPS status to GST status
- [x] Display position and duration
- [x] Seek to position with slider
- [x] One listbox with elements and one listbox with favorites in the app dashboard
- [x] See the link creation with a dashed line
- [x] Display pad properties with tooltip hover
- [x] Add preferences dialog
- [x] Create a window for the video output

### infra

- [x] Icon install
- [x] Flatpak infrastructure

## 0.2.1

### app

- [x] Can set pad properties to be used during the pipeline generation. See videomixer_alpha.xml
- [x] Support gtk4paintablesink with playbin
- [x] Display a pipeline properties dialog (list elements)

## 0.2.2

### app

- [x] Remove quit as it's unnecessary with close button
- [x] Remove the close button in dialogs (properties etc.)
- [x] Unable to use flags in playbin3
- [x] the desktop icon execs gps_pipeline_studio
- [x] move burger menu on the right

### Graphview

- [x] Update node description on property removal

## 0.3.0

### CI/Infra
- [x] Create a macos installer
- [x] Create a windows installer

### Graphview
- [x] set/get the file format version

### GStreamer
- [x] Display GStreamer version in the about dialog

## 0.3.1
### app
 - [x] Add multiple graphviews with tabs.
 - [x] handle the caps setter element

## 0.3.2
### app
- [x] check that element exists before creating it on file load.

## 0.3.3

### app
 - [x] Fix MacOs GTK runtime dependencies
 - [x] Fix the maximize call with MacOS
 - [x] Fix the default size at GTK save/load state

## 0.3.4

### app
  - [x] Fix first run when application folder has not been created, fixes #23
  - [x] Fix windows installer to bring share folder and let filesrc work properly, fixes #24

## 0.3.5

### app
  - [x] logs: receive multiple log sources such as GST logs and messages.
  - [x] settings: add a log level selection
  - [x] rename gst_pipeline_studio to gst-pipeline-studio
  - [x] can open a pipeline from the command line
