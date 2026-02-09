#!/bin/bash
#
# Build script for GstPipelineStudio RPM package (Fedora)
#
# Requirements:
#   - Fedora 39+ or compatible RPM-based system
#   - meson, cargo, rpm-build installed
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
LICENSE="GPL-3.0-or-later"

# Architecture detection
ARCH=$(uname -m)
case "$ARCH" in
  x86_64)
    RPM_ARCH="x86_64"
    ;;
  aarch64)
    RPM_ARCH="aarch64"
    ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

# Install system packages (skip in CI where dependencies are pre-installed)
if [ -z "${CI}" ]; then
    echo "Installing system dependencies..."
    test_ok dnf install -y \
            gtk4-devel \
            libunwind-devel \
            gstreamer1-devel \
            gstreamer1-plugins-base-devel \
            gstreamer1-plugins-bad-free-devel \
            rpm-build
else
    echo "Running in CI, skipping dependency installation..."
fi

# Build GstPipelineStudio
echo "Building ${PACKAGE_NAME} version ${VERSION}"

test_ok meson setup --buildtype=release builddir
test_ok ninja -C builddir

echo "Building RPM package for ${PACKAGE_NAME} version ${VERSION}"

# Create RPM build directory structure
RPM_BUILD_DIR="${SCRIPT_DIR}/rpmbuild"
rm -rf "${RPM_BUILD_DIR}"
test_ok mkdir -p "${RPM_BUILD_DIR}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
test_ok mkdir -p "${RPM_BUILD_DIR}/BUILDROOT/${PACKAGE_NAME}-${VERSION}-1.${RPM_ARCH}"

BUILDROOT="${RPM_BUILD_DIR}/BUILDROOT/${PACKAGE_NAME}-${VERSION}-1.${RPM_ARCH}"

# Create directory structure in BUILDROOT
test_ok mkdir -p "${BUILDROOT}/usr/bin"
test_ok mkdir -p "${BUILDROOT}/usr/share/applications"
test_ok mkdir -p "${BUILDROOT}/usr/share/icons/hicolor/scalable/apps"
test_ok mkdir -p "${BUILDROOT}/usr/share/icons/hicolor/symbolic/apps"

# Copy binary
if [ -f "${PROJECT_DIR}/builddir/target/release/gst-pipeline-studio" ]; then
    test_ok cp "${PROJECT_DIR}/builddir/target/release/gst-pipeline-studio" "${BUILDROOT}/usr/bin/"
else
    echo "Error: Release binary not found."
    exit 1
fi

# Copy desktop file
DESKTOP_FILE="${PROJECT_DIR}/data/dev.mooday.GstPipelineStudio.desktop.in"
if [ -f "${DESKTOP_FILE}" ]; then
    test_ok sed 's/@icon@/dev.mooday.GstPipelineStudio/' "${DESKTOP_FILE}" > \
        "${BUILDROOT}/usr/share/applications/dev.mooday.GstPipelineStudio.desktop"
else
    echo "Error: Desktop file not found at ${DESKTOP_FILE}"
    exit 1
fi

# Copy icons
ICONS_DIR="${PROJECT_DIR}/data/icons"
if [ -f "${ICONS_DIR}/dev.mooday.GstPipelineStudio.svg" ]; then
    test_ok cp "${ICONS_DIR}/dev.mooday.GstPipelineStudio.svg" \
        "${BUILDROOT}/usr/share/icons/hicolor/scalable/apps/"
fi
if [ -f "${ICONS_DIR}/dev.mooday.GstPipelineStudio-symbolic.svg" ]; then
    test_ok cp "${ICONS_DIR}/dev.mooday.GstPipelineStudio-symbolic.svg" \
        "${BUILDROOT}/usr/share/icons/hicolor/symbolic/apps/"
fi

# Set permissions
test_ok chmod 755 "${BUILDROOT}/usr/bin/gst-pipeline-studio"

# Create spec file
cat > "${RPM_BUILD_DIR}/SPECS/${PACKAGE_NAME}.spec" << EOF
Name:           ${PACKAGE_NAME}
Version:        ${VERSION}
Release:        1%{?dist}
Summary:        ${DESCRIPTION}

License:        ${LICENSE}
URL:            https://gitlab.freedesktop.org/dabrain34/GstPipelineStudio

Requires:       gtk4 >= 4.0.0
Requires:       gstreamer1 >= 1.20
Requires:       gstreamer1-plugins-base >= 1.20
Requires:       gstreamer1-plugins-good
Recommends:     gstreamer1-plugins-bad-free
Recommends:     gstreamer1-plugins-ugly-free
Suggests:       gstreamer1-libav

%description
GstPipelineStudio is a graphical user interface for the GStreamer framework
that allows users to visually create, edit, and debug GStreamer pipelines.
The application provides a drag-and-drop interface for building complex
multimedia pipelines from individual GStreamer elements.

%install
cp -a %{_builddir}/../BUILDROOT/%{name}-%{version}-1.%{_arch}/* %{buildroot}/

%files
%{_bindir}/gst-pipeline-studio
%{_datadir}/applications/dev.mooday.GstPipelineStudio.desktop
%{_datadir}/icons/hicolor/scalable/apps/dev.mooday.GstPipelineStudio.svg
%{_datadir}/icons/hicolor/symbolic/apps/dev.mooday.GstPipelineStudio-symbolic.svg

%changelog
* $(date "+%a %b %d %Y") ${MAINTAINER} - ${VERSION}-1
- Package version ${VERSION}
EOF

# Build the RPM package
echo "Building RPM..."
test_ok rpmbuild --define "_topdir ${RPM_BUILD_DIR}" \
    --define "_builddir ${RPM_BUILD_DIR}/BUILD" \
    --define "_buildrootdir ${RPM_BUILD_DIR}/BUILDROOT" \
    --define "_rpmdir ${RPM_BUILD_DIR}/RPMS" \
    --define "_srcrpmdir ${RPM_BUILD_DIR}/SRPMS" \
    --define "_specdir ${RPM_BUILD_DIR}/SPECS" \
    --define "_sourcedir ${RPM_BUILD_DIR}/SOURCES" \
    -bb "${RPM_BUILD_DIR}/SPECS/${PACKAGE_NAME}.spec"

# Copy RPM to installer directory
RPM_FILE=$(find "${RPM_BUILD_DIR}/RPMS" -name "*.rpm" | head -1)
if [ -n "${RPM_FILE}" ]; then
    test_ok cp "${RPM_FILE}" "${SCRIPT_DIR}/"
    RPM_BASENAME=$(basename "${RPM_FILE}")
    echo "Package created: ${SCRIPT_DIR}/${RPM_BASENAME}"
else
    echo "Error: RPM file not found after build"
    exit 1
fi

# Cleanup
rm -rf "${RPM_BUILD_DIR}"

echo "Done!"
