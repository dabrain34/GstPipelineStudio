#!/bin/bash

HOMEBREW_NO_INSTALL_CLEANUP=1

/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

eval "$(/opt/homebrew/bin/brew shellenv)"

brew update

brew install pkg-config

brew install glib

brew install m4

echo 'export PATH="/opt/homebrew/opt/m4/bin:$PATH"' >> ~/.zshrc

brew install bash

bash -c "$(curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.5/install.sh)"

source ~/.nvm/nvm.sh

nvm install node

nvm install-latest-npm

npm install -g appdmg

exit 0
