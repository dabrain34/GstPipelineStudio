#!/bin/bash
#
# Build script for GstPipelineStudio AppImage
#
# Requirements:
#   - Ubuntu 24.04 or compatible system
#   - curl, meson, ninja, cargo installed
#   - System development libraries (see apt-get install below)
#
# This script is designed to run in CI but can also be used locally.
# Set CLEANUP_BUILD=1 to remove build directory after successful build.
#

set -e

test_ok() {
  "$@"
  if [ $? != 0 ]; then
    echo "Command failed: $*"
    exit 1
  fi
}

SCRIPT_DIR="$( cd "$(dirname "$0")" ; pwd -P )"
PROJECTDIR="$( cd "${SCRIPT_DIR}/../../" ; pwd -P )"
BUILD_DIR="${PROJECTDIR}/builddir-appimage"
APPDIR="${BUILD_DIR}/AppDir"

# Extract version from Cargo.toml (same as build_deb.sh)
VERSION=$(grep '^version' "${PROJECTDIR}/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
export VERSION
echo "Building GstPipelineStudio AppImage version ${VERSION}"

# Architecture detection
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)
    ARCH_LABEL="x86_64"
    LIB_ARCH="x86_64-linux-gnu"
    ;;
  aarch64)
    ARCH_LABEL="aarch64"
    LIB_ARCH="aarch64-linux-gnu"
    ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

GSTREAMER_OPTS="
        -Dforce_fallback_for=gstreamer-1.0,gtk,glib
        -Dglib:introspection=disabled
        -Dglib:tests=false
        -Dgstreamer-1.0:libav=disabled
        -Dgstreamer-1.0:examples=disabled
        -Dgstreamer-1.0:introspection=disabled
        -Dgstreamer-1.0:rtsp_server=disabled
        -Dgstreamer-1.0:devtools=disabled
        -Dgstreamer-1.0:ges=disabled
        -Dgstreamer-1.0:python=disabled
        -Dgstreamer-1.0:tests=disabled
        -Dgstreamer-1.0:gtk=enabled
        -Dgstreamer:tests=disabled
        -Dgst-plugins-base:tests=disabled
        -Dgst-plugins-good:tests=disabled
        -Dgst-plugins-bad:openexr=disabled
        -Dgstreamer-1.0:gst-examples=disabled
        -Dgst-plugins-bad:vulkan=disabled
        -Dgst-plugins-bad:webrtc=disabled
        -Dgst-plugins-bad:webrtcdsp=disabled
        -Dgst-plugins-bad:tests=disabled
        -Dorc:gtk_doc=disabled
        -Dgtk4:introspection=disabled
        -Dgtk4:build-examples=false
        -Dgtk4:build-tests=false
        -Dgtk4:media-gstreamer=enabled
        -Dgtk4:x11-backend=true
        -Dgtk4:wayland-backend=true
        -Dgtk4:print-cups=disabled
        -Dgtk4:vulkan=disabled
        -Dgtk4:build-demos=false
        -Djson-glib:introspection=disabled
        "
# Install system packages
echo "Installing system dependencies..."
test_ok apt-get update
test_ok apt-get install -y --no-install-recommends \
      libfuse2 \
      libx11-dev \
      libxext-dev \
      libxrandr-dev \
      libxi-dev \
      libxcursor-dev \
      libxdamage-dev \
      libxinerama-dev \
      libxkbcommon-dev \
      libwayland-dev \
      wayland-protocols \
      libepoxy-dev \
      libegl-dev \
      libdrm-dev \
      libgbm-dev \
      libpng-dev \
      libjpeg-dev \
      libtiff-dev \
      libfontconfig1-dev \
      libfreetype-dev \
      libfribidi-dev \
      libharfbuzz-dev \
      libcairo2-dev \
      libpango1.0-dev \
      libatk1.0-dev \
      libsqlite3-dev \
      libxml2-dev \
      libasound2-dev \
      libpulse-dev \
      libogg-dev \
      libvorbis-dev \
      libflac-dev \
      libopus-dev \
      libv4l-dev \
      libudev-dev \
      libgudev-1.0-dev \
      libvulkan-dev \
      shared-mime-info \
      glslc \
      gettext

echo "Updating subprojects..."
test_ok meson subprojects update --reset

echo "Configuring meson build..."
test_ok meson setup --prefix=/usr --buildtype=release "${BUILD_DIR}" ${GSTREAMER_OPTS}

echo "Building..."
test_ok ninja -C "${BUILD_DIR}"

echo "Installing to AppDir..."
DESTDIR="${APPDIR}" test_ok ninja -C "${BUILD_DIR}" install

# Download linuxdeploy if not present
# Pin to specific version for security and reproducibility
LINUXDEPLOY_VERSION="1-alpha-20240109-1"
LINUXDEPLOY_URL="https://github.com/linuxdeploy/linuxdeploy/releases/download/${LINUXDEPLOY_VERSION}/linuxdeploy-${ARCH_LABEL}.AppImage"
LINUXDEPLOY="${BUILD_DIR}/linuxdeploy-${ARCH_LABEL}.AppImage"
if [ ! -f "${LINUXDEPLOY}" ]; then
    echo "Downloading linuxdeploy..."
    test_ok curl -L -o "${LINUXDEPLOY}" "${LINUXDEPLOY_URL}"
    test_ok chmod +x "${LINUXDEPLOY}"
fi

# Copy desktop file (same as build_deb.sh for consistency)
DESKTOP_FILE="${PROJECTDIR}/data/org.freedesktop.dabrain34.GstPipelineStudio.desktop.in"
test_ok mkdir -p "${APPDIR}/usr/share/applications"
if [ -f "${DESKTOP_FILE}" ]; then
    test_ok sed 's/@icon@/org.freedesktop.dabrain34.GstPipelineStudio/' "${DESKTOP_FILE}" > \
        "${APPDIR}/usr/share/applications/org.freedesktop.dabrain34.GstPipelineStudio.desktop"
else
    echo "Error: Desktop file not found at ${DESKTOP_FILE}"
    exit 1
fi

# Ensure icon is in the right place
test_ok mkdir -p "${APPDIR}/usr/share/icons/hicolor/scalable/apps"
if [ -f "${PROJECTDIR}/data/icons/org.freedesktop.dabrain34.GstPipelineStudio.svg" ]; then
    test_ok cp "${PROJECTDIR}/data/icons/org.freedesktop.dabrain34.GstPipelineStudio.svg" \
       "${APPDIR}/usr/share/icons/hicolor/scalable/apps/"
fi

# Copy GStreamer plugins to the AppDir
echo "Setting up GStreamer plugins..."
GST_PLUGIN_PATH="${APPDIR}/usr/lib/${LIB_ARCH}/gstreamer-1.0"
test_ok mkdir -p "${GST_PLUGIN_PATH}"

# Set up environment wrapper script
test_ok mkdir -p "${APPDIR}/usr/bin"
test_ok cat > "${APPDIR}/AppRun" << EOF
#!/bin/bash
SELF=\$(readlink -f "\$0")
HERE=\${SELF%/*}
export PATH="\${HERE}/usr/bin:\${PATH}"
export LD_LIBRARY_PATH="\${HERE}/usr/lib:\${HERE}/usr/lib64:\${HERE}/usr/lib/${LIB_ARCH}:\${LD_LIBRARY_PATH}"
export GST_PLUGIN_PATH="\${HERE}/usr/lib/${LIB_ARCH}/gstreamer-1.0"
export GST_PLUGIN_SCANNER="\${HERE}/usr/libexec/gstreamer-1.0/gst-plugin-scanner"
exec "\${HERE}/usr/bin/gst-pipeline-studio" "\$@"
EOF
test_ok chmod +x "${APPDIR}/AppRun"


# Clean unnecessary files to reduce size
echo "Cleaning unnecessary files..."
# Keep only gst-pipeline-studio in bin directory
find "${APPDIR}/usr/bin" -type f ! -name "gst-pipeline-studio" -delete 2>/dev/null || true
rm -rf "${APPDIR}/usr/include"
rm -rf "${APPDIR}/usr/lib/pkgconfig"
rm -rf "${APPDIR}/usr/lib/cmake"
rm -rf "${APPDIR}/usr/share/doc"
rm -rf "${APPDIR}/usr/share/man"
rm -rf "${APPDIR}/usr/share/gtk-doc"
rm -rf "${APPDIR}/usr/share/aclocal"
find "${APPDIR}" -name "*.a" -delete 2>/dev/null || true
find "${APPDIR}" -name "*.la" -delete 2>/dev/null || true

# Create the AppImage
echo "Creating AppImage..."
cd "${BUILD_DIR}"

# Use linuxdeploy to bundle libraries and create AppImage
export ARCH="${ARCH_LABEL}"
export LDAI_OUTPUT="gst-pipeline-studio-${VERSION}-${ARCH_LABEL}.AppImage"
LD_LIBRARY_PATH="${APPDIR}/usr/lib:${APPDIR}/usr/lib64:${APPDIR}/usr/lib/${LIB_ARCH}:${LD_LIBRARY_PATH}" \
"${LINUXDEPLOY}" --appimage-extract-and-run \
    --appdir "${APPDIR}" \
    --desktop-file "${APPDIR}/usr/share/applications/org.freedesktop.dabrain34.GstPipelineStudio.desktop" \
    --icon-file "${APPDIR}/usr/share/icons/hicolor/scalable/apps/org.freedesktop.dabrain34.GstPipelineStudio.svg" \
    --output appimage

# Move AppImage to installer directory
test_ok mv "${LDAI_OUTPUT}" "${PROJECTDIR}/installer/linux/"
test_ok chmod a+x "${PROJECTDIR}/installer/linux/${LDAI_OUTPUT}"

# Optional: Cleanup build directory
if [ "${CLEANUP_BUILD:-0}" = "1" ]; then
    echo "Cleaning up build directory..."
    rm -rf "${BUILD_DIR}"
fi

echo ""
echo "AppImage created successfully: installer/linux/${LDAI_OUTPUT}"
echo ""
