#!/bin/bash


test_ok() {
  $*
  if [ $? != 0 ]; then
    exit 1
  fi

}

# To have m4 in the path
source ~/.zshrc
eval "$(/opt/homebrew/bin/brew shellenv)"
# dependency library:
# Make a .app file: https://gist.github.com/oubiwann/453744744da1141ccc542ff75b47e0cf
# Make a .dmg file: https://github.com/LinusU/node-appdmg
# Can't find library: https://www.jianshu.com/p/441a7553700f
BUILD_DIR=builddir
PROJECTDIR="$( cd "$(dirname "$0")/../" ; pwd -P )"
TARGETDIR="${PROJECTDIR}/${BUILD_DIR}/INSTALL_GPS"
VERSION="$(cat VERSION)"
export VERSION
echo "VERSION=$VERSION"

pip3 install docutils

# rust toolchain
curl https://sh.rustup.rs -sSf | sh -s -- -y
source $HOME/.cargo/env

# OpenSSL is keg-only on Homebrew, set paths for openssl-sys crate
export OPENSSL_DIR="$(brew --prefix openssl@3)"
# libffi is keg-only on Homebrew, needed because macOS Tahoe 26 SDK
# removed /usr/include/ffi and the vendored libffi 3.2.9999 has
# CFI assembly bugs with Apple Clang 17+
export LIBFFI_DIR="$(brew --prefix libffi)"
export PKG_CONFIG_PATH="${OPENSSL_DIR}/lib/pkgconfig:${LIBFFI_DIR}/lib/pkgconfig:${PKG_CONFIG_PATH}"

cargo install cargo-c

GSTREAMER_OPTS="
        -Dforce_fallback_for=gstreamer-1.0,gtk,glib \
        -Dglib:introspection=disabled \
        -Dglib:tests=false \
        -Dgstreamer-1.0:libav=enabled \
        -Dgstreamer-1.0:examples=disabled \
        -Dgstreamer-1.0:introspection=disabled \
        -Dgstreamer-1.0:rtsp_server=disabled \
        -Dgstreamer-1.0:devtools=disabled \
        -Dgstreamer-1.0:ges=disabled \
        -Dgstreamer-1.0:rs=disabled \
        -Dgstreamer-1.0:gpl=enabled \
        -Dgstreamer-1.0:python=disabled \
        -Dgstreamer-1.0:tests=disabled \
        -Dgstreamer-1.0:gtk=enabled \
        -Dgst-plugins-base:tests=disabled \
        -Dgst-plugins-good:tests=disabled \
        -Dgst-plugins-bad:openexr=disabled -Dgstreamer-1.0:gst-examples=disabled \
        -Dgst-plugins-bad:vulkan=disabled \
        -Dgst-plugins-bad:webrtc=disabled \
        -Dgst-plugins-bad:webrtcdsp=disabled \
        -Dgst-plugins-bad:tests=disabled \
        -Dorc:gtk_doc=disabled \
        -Dgtk:introspection=disabled \
        -Dgtk:build-examples=false \
        -Dgtk:build-tests=false \
        -Dgtk:media-gstreamer=disabled \
        -Dgtk:x11-backend=false \
        -Dgtk:macos-backend=true \
        -Dgtk:wayland-backend=false \
        -Dgtk:print-cups=disabled \
        -Dgtk:vulkan=disabled \
        -Dgtk:build-demos=false \
        -Djson-glib:introspection=disabled \
        "


# rebuild app release version
rm -rf "${TARGETDIR}"
test_ok meson subprojects update --reset
test_ok meson setup --prefix=$TARGETDIR --buildtype=release ${BUILD_DIR} ${GSTREAMER_OPTS}
test_ok ninja -C ${BUILD_DIR} install

# copy app data files to target dir
echo -n "Copy app data files......"
test_ok mkdir -p "${TARGETDIR}/etc/"
mkdir -p "${TARGETDIR}/lib/gstreamer-1.0"
mkdir -p "${TARGETDIR}/share/doc"
mkdir -p "${TARGETDIR}/share/themes"
mkdir -p "${TARGETDIR}/share/glib-2.0/schemas"
mkdir -p "${TARGETDIR}/share/licenses/GstPipelineStudio"
mkdir -p "${TARGETDIR}/share/icons/hicolor/scalable/apps"
echo "[done]"

function lib_dependency_copy
{
  local target=$1
  local folder=$2

  lib_dir="$( cd "$( dirname "$1" )" >/dev/null 2>&1 && pwd )"
  libraries="$(otool -L $target | grep "/*.*dylib" -o | xargs)"
  for lib in $libraries; do
    # Skip Homebrew's gettext/libintl - we use proxy-libintl from the build
    if [[ $lib == *"/gettext/"* || $lib == *"libintl"* ]]; then
      continue
    fi

    if [[ '/usr/lib/' != ${lib:0:9} && '/System/Library/' != ${lib:0:16} ]]; then
      if [[ '@' == ${lib:0:1} ]]; then
        if [[ '@loader_path' == ${lib:0:12} ]]; then
          cp -n "${lib/@loader_path/$lib_dir}" $folder
        else
          echo "Unsupported path: $lib"
        fi
      else
        if [[ $lib != $target ]]; then
          cp -n $lib $folder
        fi
      fi
    fi
  done
}

function lib_dependency_analyze
{
  # This function use otool to analyze library dependency.
  # then copy the dependency libraries to destination path

  local library_dir=$1
  local targets_dir=$2

  libraries="$(find $library_dir -name \*.dylib -o -name \*.so -type f)"
  for lib in $libraries; do
      lib_dependency_copy $lib $targets_dir
      # otool -L $lib | grep "/usr/local/*.*dylib" -o | xargs -I{} cp -n "{}" "$targets_dir"
  done
}

# copy app dependency library to target dir
echo -n "Copy app dependency library......"

lib_dependency_copy ${TARGETDIR}/bin/gst-pipeline-studio "${TARGETDIR}/bin"
lib_dependency_copy ${TARGETDIR}/lib/libgobject-2.0.0.dylib "${TARGETDIR}/bin"
lib_dependency_copy ${TARGETDIR}/lib/libsoup-2.4.1.dylib "${TARGETDIR}/bin"
lib_dependency_copy "${TARGETDIR}/bin/libgtk-4.1.dylib" "${TARGETDIR}/bin"


for file in ${TARGETDIR}/lib/gstreamer-1.0/*.dylib
do
    echo "${file}"
    lib_dependency_copy ${file} "${TARGETDIR}/lib/"
done

test_ok cp -f "${PROJECTDIR}/macos/mac_launcher.sh" "${TARGETDIR}/bin/launcher.sh"


# # find "${TARGETDIR}/bin" -type f -path '*.dll.a' -exec rm '{}' \;
lib_dependency_analyze ${TARGETDIR}/lib ${TARGETDIR}/bin
lib_dependency_analyze ${TARGETDIR}/bin ${TARGETDIR}/bin
echo "[done]"

# Verify proxy-libintl from the build (installed by meson)
echo -n "Verifying proxy-libintl......"

# Meson should have already installed proxy-libintl to lib/
PROXY_LIBINTL="${TARGETDIR}/lib/libintl.8.dylib"

if [ ! -f "$PROXY_LIBINTL" ]; then
    echo "[ERROR: proxy-libintl not installed by meson]"
    exit 1
fi

# Verify it has g_libintl symbols (confirms it's proxy-libintl, not Homebrew gettext)
if ! nm -g "$PROXY_LIBINTL" 2>/dev/null | grep -q "_g_libintl_"; then
    echo "[ERROR: libintl missing g_libintl symbols - wrong libintl installed]"
    exit 1
fi

echo "[verified]"

# copy app icons and license files to target dir
echo -n "Copy app icon(svg) files......"
cp -f "${PROJECTDIR}/../data/icons/dev.mooday.GstPipelineStudio.ico" "${TARGETDIR}/bin"
cp -f "${PROJECTDIR}/../data/icons/dev.mooday.GstPipelineStudio.svg" "${TARGETDIR}/share/icons/hicolor/scalable/apps"
echo "[done]"


# download license file: LGPL-3.0
echo -n "Downloading the remote license file......"
cp -f "${PROJECTDIR}/../LICENSE" "${TARGETDIR}/share/licenses/GstPipelineStudio"
if [ ! -f "${TARGETDIR}/share/licenses/GstPipelineStudio/gpl-3.0.txt" ]; then
  curl "https://www.gnu.org/licenses/gpl-3.0.txt" -o "${TARGETDIR}/share/licenses/GstPipelineStudio/gpl-3.0.txt"
  if [ $? -eq 0 ]; then
    echo "[done]"
  else
    echo "[failed]"
  fi
else
  echo "[done]"
fi

# remove useless files
echo -n "Cleaning unnecessary files........."

# Clean unnecessary folders in share/ to reduce installer size
shareFoldersToClean=("doc" "gtk-4.0" "man" "gdb" "aclocal" "bash-completion" "cmake" "gettext" "glib-2.0" "gst-plugins-base" "gstreamer-1.0" "installed-tests" "themes" "thumbnailers")
for folder in "${shareFoldersToClean[@]}"; do
  folderPath="${TARGETDIR}/share/${folder}"
  if [ -d "$folderPath" ]; then
    echo "Cleaning $folderPath"
    rm -rf "$folderPath"
  fi
done

# Clean unnecessary folders in lib/
libFoldersToClean=("pkgconfig" "cmake" "gio" "glib-2.0" "graphene-1.0" "gstreamer-1.0/include" "gtk-4.0" "cairo" "gdk-pixbuf-2.0")
for folder in "${libFoldersToClean[@]}"; do
  folderPath="${TARGETDIR}/lib/${folder}"
  if [ -d "$folderPath" ]; then
    echo "Cleaning $folderPath"
    rm -rf "$folderPath"
  fi
done

# Clean other unnecessary files and directories
rm -f ${TARGETDIR}/lib/*.a
rm -rf ${TARGETDIR}/libexec
rm -rf ${TARGETDIR}/etc

echo "[done]"

echo "make macos executable file(.app)......"
cd "${PROJECTDIR}/${BUILD_DIR}"
cp "${PROJECTDIR}/macos/installers/Info.plist" "${PROJECTDIR}/${BUILD_DIR}"
cp "${PROJECTDIR}/macos/installers/mac.icns" "${PROJECTDIR}/${BUILD_DIR}/GstPipelineStudio.icns"
../macos/mac_app_pack.sh --path "${TARGETDIR}" --name "GstPipelineStudio" --info "Info.plist" --icons "GstPipelineStudio.icns"
if [ $? -eq 0 ]; then
  echo "[done]"
  else
  echo "[failed]"
fi

# sign all libraries and the application
echo -n "Signing application and libraries......"
# Sign all dylibs first
find "${PROJECTDIR}/${BUILD_DIR}/GstPipelineStudio.app" -name "*.dylib" -exec codesign --force --sign - {} \; 2>/dev/null
find "${PROJECTDIR}/${BUILD_DIR}/GstPipelineStudio.app" -name "*.so" -exec codesign --force --sign - {} \; 2>/dev/null
# Sign the main executable
codesign --force --sign - "${PROJECTDIR}/${BUILD_DIR}/GstPipelineStudio.app/Contents/MacOS/gst-pipeline-studio-real" 2>/dev/null
# Sign the launcher script wrapper if it exists
if [ -f "${PROJECTDIR}/${BUILD_DIR}/GstPipelineStudio.app/Contents/MacOS/gst-pipeline-studio" ]; then
  codesign --force --sign - "${PROJECTDIR}/${BUILD_DIR}/GstPipelineStudio.app/Contents/MacOS/gst-pipeline-studio" 2>/dev/null
fi
# Sign the entire app bundle with deep signature
codesign --force --deep --sign - "${PROJECTDIR}/${BUILD_DIR}/GstPipelineStudio.app" 2>/dev/null
# Remove quarantine attributes
xattr -cr "${PROJECTDIR}/${BUILD_DIR}/GstPipelineStudio.app" 2>/dev/null
echo "[done]"

# make installer package
echo "make macos installer(.dmg)......"
cp "${PROJECTDIR}/macos/installers/dmg.json" gps_dmg.json
cp "${PROJECTDIR}/macos/installers/background.png" "${PROJECTDIR}/${BUILD_DIR}/gps_dmg_background.png"
rm -f ${PROJECTDIR}/GstPipelineStudio-${VERSION}.dmg
appdmg gps_dmg.json "${PROJECTDIR}/GstPipelineStudio-${VERSION}.dmg"
if [ $? -eq 0 ]; then
  echo "[done]"
  else
  echo "[failed]"
fi

# make portable package
echo -n "make macos portable......"
tar czf "${PROJECTDIR}/GstPipelineStudio-${VERSION}.tar.gz" -C "${PROJECTDIR}" ${BUILD_DIR}/GstPipelineStudio.app
if [ $? -eq 0 ]; then
  echo "[done]"
  else
  echo "[failed]"
fi
