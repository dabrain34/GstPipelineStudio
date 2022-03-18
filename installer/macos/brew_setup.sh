#!/bin/bash

HOMEBREW_NO_INSTALL_CLEANUP=1

/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

brew install gtk4 gstreamer gst-plugins-base gst-plugins-bad gst-plugins-good

brew install npm

npm install -g appdmg

exit 0
