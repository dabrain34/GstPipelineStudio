#!/bin/bash

HOMEBREW_NO_INSTALL_CLEANUP=1

/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

brew install pkg-config

# GTK4 support
brew install gtk4
# brew install cairo libxrandr libxi libxcursor libxdamage libxinerama

brew install npm

npm install -g appdmg

exit 0
