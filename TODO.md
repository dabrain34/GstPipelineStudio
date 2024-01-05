## TODO

### Graphview

- [ ] create a crate for graphview/node/port


### GStreamer:

- [ ] Implement pipeline unit test

### app

- [ ] Control the connection between element
- [ ] unable to connect element with incompatible caps.
- [ ] Implement graph dot render/load
- [ ] Add probes on each pad to monitor the pipeline
- [ ] Render a media file
- [ ] Offer compatible element to a pad (autorender)
- [ ] Display tags/meta/message detected
- [ ] Change TreeView to ListView
- [ ] Implement zoom on the view (https://gitlab.gnome.org/World/obfuscate/-/blob/master/src/widgets/drawing_area.rs)
- [ ] reopen the last log on prematured exit (crash)
- [ ] Play/pause should be prevented until the pipeline is ready
- [ ] Filter the elements by class/rank etc.
- [ ] double click on node/pad open the properties



## bugs

- [ ] Combo box is not well selected if the value is not linear such as flags. See flags in playbin
- [ ] opening a graph file can lead a different behavior in the pipeline. See videomixer graph where the zorder
      on pads is not correctly set to right one.
