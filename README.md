# GstPipelineStudio: Draw your own GStreamer pipeline ...

## Setup

Install the Rust toolchain via `rustup`

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Check https://rustup.rs for alternative installation options.

### Ubuntu/Debian/etc

```sh
apt install python3-pip ninja-build pkgconfig
pip3 install --user meson
apt install libgtk-4-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```

### Fedora/RedHat/SuSE/etc

```sh
dnf install python3-pip ninja-build pkgconfig
pip3 install meson
dnf install gtk4-devel gstreamer1-devel gstreamer1-plugins-base-devel python3-pip ninja-build pkgconfig
```

## Getting started

```sh
meson builddir
cargo run
```
