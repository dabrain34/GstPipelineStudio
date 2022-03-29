#!/bin/bash

HOMEBREW_NO_INSTALL_CLEANUP=1

/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

brew install pkg-config gtk4

brew install npm

npm install -g appdmg

exit 0
