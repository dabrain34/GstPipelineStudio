#!/bin/bash
#
# Build script for GstPipelineStudio Debian package
#
# Requirements:
#   - Ubuntu 24.04 or compatible Debian-based system
#   - meson, cargo, dpkg-deb installed
#   - System GStreamer and GTK4 development packages
#
# This script is designed to run in CI but can also be used locally.
#

set -e

test_ok() {
  "$@"
  if [ $? != 0 ]; then
    echo "Command failed: $*"
    exit 1
  fi
}

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Extract version from Cargo.toml
VERSION=$(grep '^version' "${PROJECT_DIR}/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
PACKAGE_NAME="gst-pipeline-studio"
MAINTAINER="Stéphane Cerveau <scerveau@igalia.com>"
DESCRIPTION="A graphical user interface for GStreamer pipelines"

# Architecture detection
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)
    ARCHITECTURE="amd64"
    ;;
  aarch64)
    ARCHITECTURE="arm64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

# Install system packages (only in CI environment)
echo "Installing system dependencies..."
test_ok apt-get update
test_ok apt-get install -y --no-install-recommends \
        libgtk-4-dev \
        libunwind-dev \
        libgstreamer1.0-dev \
        libgstreamer-plugins-base1.0-dev \
        libgstreamer-plugins-bad1.0-dev

# Build GstPipelineStudio
echo "Building ${PACKAGE_NAME} version ${VERSION}"

test_ok meson setup --buildtype=release builddir
test_ok ninja -C builddir

echo "Building deb package for ${PACKAGE_NAME} version ${VERSION}"

# Create package directory structure
PKG_DIR="${SCRIPT_DIR}/${PACKAGE_NAME}_${VERSION}_${ARCHITECTURE}"
rm -rf "${PKG_DIR}"
test_ok mkdir -p "${PKG_DIR}/DEBIAN"
test_ok mkdir -p "${PKG_DIR}/usr/bin"
test_ok mkdir -p "${PKG_DIR}/usr/share/applications"
test_ok mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps"
test_ok mkdir -p "${PKG_DIR}/usr/share/icons/hicolor/symbolic/apps"

# Copy binary
if [ -f "${PROJECT_DIR}/builddir/target/release/gst-pipeline-studio" ]; then
    test_ok cp "${PROJECT_DIR}/builddir/target/release/gst-pipeline-studio" "${PKG_DIR}/usr/bin/"
else
    echo "Error: Release binary not found."
    exit 1
fi

# Copy desktop file
DESKTOP_FILE="${PROJECT_DIR}/data/org.freedesktop.dabrain34.GstPipelineStudio.desktop.in"
if [ -f "${DESKTOP_FILE}" ]; then
    test_ok sed 's/@icon@/org.freedesktop.dabrain34.GstPipelineStudio/' "${DESKTOP_FILE}" > \
        "${PKG_DIR}/usr/share/applications/org.freedesktop.dabrain34.GstPipelineStudio.desktop"
else
    echo "Error: Desktop file not found at ${DESKTOP_FILE}"
    exit 1
fi

# Copy icons
ICONS_DIR="${PROJECT_DIR}/data/icons"
if [ -f "${ICONS_DIR}/org.freedesktop.dabrain34.GstPipelineStudio.svg" ]; then
    test_ok cp "${ICONS_DIR}/org.freedesktop.dabrain34.GstPipelineStudio.svg" \
        "${PKG_DIR}/usr/share/icons/hicolor/scalable/apps/"
fi
if [ -f "${ICONS_DIR}/org.freedesktop.dabrain34.GstPipelineStudio-symbolic.svg" ]; then
    test_ok cp "${ICONS_DIR}/org.freedesktop.dabrain34.GstPipelineStudio-symbolic.svg" \
        "${PKG_DIR}/usr/share/icons/hicolor/symbolic/apps/"
fi

# Create control file
cat > "${PKG_DIR}/DEBIAN/control" << EOF
Package: ${PACKAGE_NAME}
Version: ${VERSION}
Section: video
Priority: optional
Architecture: ${ARCHITECTURE}
Depends: libgtk-4-1 (>= 4.0.0), libgstreamer1.0-0 (>= 1.20), libgstreamer-plugins-base1.0-0 (>= 1.20), gstreamer1.0-plugins-base, gstreamer1.0-plugins-good
Recommends: gstreamer1.0-plugins-bad, gstreamer1.0-plugins-ugly
Suggests: gstreamer1.0-libav
Maintainer: ${MAINTAINER}
Homepage: https://gitlab.freedesktop.org/dabrain34/GstPipelineStudio
Description: ${DESCRIPTION}
 GstPipelineStudio is a graphical user interface for the GStreamer framework
 that allows users to visually create, edit, and debug GStreamer pipelines.
 The application provides a drag-and-drop interface for building complex
 multimedia pipelines from individual GStreamer elements.
EOF

# Set permissions
test_ok chmod 755 "${PKG_DIR}/usr/bin/gst-pipeline-studio"

# Build the package
test_ok dpkg-deb --build --root-owner-group "${PKG_DIR}"

# Cleanup
rm -rf "${PKG_DIR}"

echo "Package created: ${SCRIPT_DIR}/${PACKAGE_NAME}_${VERSION}_${ARCHITECTURE}.deb"
