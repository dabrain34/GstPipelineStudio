#!/bin/bash

HOMEBREW_NO_INSTALL_CLEANUP=1

/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

eval "$(/opt/homebrew/bin/brew shellenv)"

pip3 install --upgrade pip
# Make sure meson is up to date
pip3 install -U meson==1.10.1
# Need to install certificates for python
pip3 install --upgrade certifi

# # Another way to install certificates
# - open /Applications/Python\ 3.8/Install\ Certificates.command
# Get ninja
pip3 install -U ninja
# Get tomlib
pip3 install -U tomli

brew update

brew install pkg-config

brew install m4

echo 'export PATH="/opt/homebrew/opt/m4/bin:$PATH"' >> ~/.zshrc

brew install bash

curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash

source ~/.nvm/nvm.sh

nvm install node

nvm install 20

nvm alias default 20

nvm install-latest-npm

npm install -g appdmg

exit 0
