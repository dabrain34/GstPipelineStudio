
## 0.5.0

### New Features
  - [x] Remote pipeline introspection
  - [x] Crash recovery dialog to display previous session log on startup
  - [x] Open Dot Folder menu entry for loading dot files
  - [x] Trait-based DOT file loading (GstreamerDotLoader)
  - [x] Auto-connect on node click (node-link-request)
  - [x] Auto-arrange pipeline layout
  - [x] File selector button for location property
  - [x] Logger copy to clipboard with multi-selection support

### User Interface
  - [x] Semi-transparent background on nodes
  - [x] Dynamic node width in auto-arrange
  - [x] Improved node positioning and auto-scroll on add
  - [x] Sort ports by name when saving to XML

### Build and Platforms
  - [x] GStreamer 1.28.0
  - [x] App ID renamed to dev.mooday.GstPipelineStudio
  - [x] Legacy config directory migration on app ID rename
  - [x] RPM build script and CI jobs for Fedora
  - [x] Simplified version management (single source in VERSION file)
  - [x] macOS CI updated to Tahoe 26
  - [x] Commit message linting with gitlint

### Stability
  - [x] Fix port counter decrement in remove_port
  - [x] Fix window visibility using opacity instead of realize-only
  - [x] Fix window visibility during splash screen display
  - [x] Player position/duration changed to ms precision

## 0.4.0

### New Features
  - [x] Splash screen during GStreamer initialization
  - [x] Recent files menu
  - [x] Context menus on elements list and graph links
  - [x] Graph zoom support

### User Interface
  - [x] New application logo with dark theme support
  - [x] Modernized dialogs (preferences, properties, alerts)
  - [x] GTK4 widget migration (ColumnView, DropDown, FileDialog)
  - [x] Resizable columns in element browser and loggers
  - [x] Element search with plugin name and rank display
  - [x] Compact player controls layout
  - [x] Port name tooltips

### Build and Platforms
  - [x] GStreamer 1.26 and GTK 4.16
  - [x] Rust 1.85
  - [x] macOS/Windows: libav enabled, optimized installers
  - [x] CI: AppImage and deb package jobs

### Stability
  - [x] Error dialogs on pipeline failures
  - [x] Improved error handling throughout codebase
  - [x] Unit test infrastructure

## 0.3.6

### app
  - [x] gtk: 4.13.9
  - [x] gstreamer: 1.26
  - [x] graphview: can now zoom on graph
  - [x] add duplicate node
  - [x] app: enhance element uri handler
  - [x] macos: installer with gstreamer 1.24
  - [x] Control the connection between element
  - [x] unable to connect element with incompatible caps.

## 0.3.5

### app
  - [x] logs: receive multiple log sources such as GST logs and messages.
  - [x] settings: add a log level selection
  - [x] rename gst_pipeline_studio to gst-pipeline-studio
  - [x] can open a pipeline from the command line

## 0.3.4

### app
  - [x] Fix first run when application folder has not been created, fixes #23
  - [x] Fix windows installer to bring share folder and let filesrc work properly, fixes #24

## 0.3.3

### app
 - [x] Fix MacOs GTK runtime dependencies
 - [x] Fix the maximize call with MacOS
 - [x] Fix the default size at GTK save/load state

## 0.3.2
### app
- [x] check that element exists before creating it on file load.

## 0.3.1
### app
 - [x] Add multiple graphviews with tabs.
 - [x] handle the caps setter element

## 0.3.0

### CI/Infra
- [x] Create a macos installer
- [x] Create a windows installer

### Graphview
- [x] set/get the file format version

### GStreamer
- [x] Display GStreamer version in the about dialog

## 0.2.2

### app

- [x] Remove quit as it's unnecessary with close button
- [x] Remove the close button in dialogs (properties etc.)
- [x] Unable to use flags in playbin3
- [x] the desktop icon execs gps_pipeline_studio
- [x] move burger menu on the right

### Graphview

- [x] Update node description on property removal

## 0.2.1

### app

- [x] Can set pad properties to be used during the pipeline generation. See videomixer_alpha.xml
- [x] Support gtk4paintablesink with playbin
- [x] Display a pipeline properties dialog (list elements)

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