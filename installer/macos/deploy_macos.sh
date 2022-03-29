#!/bin/bash


test_ok() {
  $*
  if [ $? != 0 ]; then
    exit 1
  fi

}

# depenency library:
# Make a .app file: https://gist.github.com/oubiwann/453744744da1141ccc542ff75b47e0cf
# Make a .dmg file: https://github.com/LinusU/node-appdmg
# Can't find library: https://www.jianshu.com/p/441a7553700f
BUILD_DIR=builddir
PROJECTDIR="$( cd "$(dirname "$0")/../" ; pwd -P )"
TARGETDIR="${PROJECTDIR}/${BUILD_DIR}/INSTALL_GPS"
VERSION="$(date +%y%m%d)"
export VERSION
echo "VERSION=$VERSION"


GSTREAMER_OPTS="
        -Dforce_fallback_for=gstreamer-1.0,libffi,pcre2 \
        -Dgstreamer-1.0:libav=disabled \
        -Dgstreamer-1.0:examples=disabled \
				-Dgstreamer-1.0:introspection=disabled \
        -Dgstreamer-1.0:rtsp_server=disabled \
        -Dgstreamer-1.0:devtools=disabled \
				-Dgst-plugins-base:tests=disabled \
				-Dgstreamer-1.0:tests=disabled \
				-Dgst-plugins-bad:openexr=disabled -Dgstreamer-1.0:gst-examples=disabled \
				-Dorc:gtk_doc=disabled \
				-Dgstreamer-1.0:python=disabled"

# rebuild app release version
rm -rf "${TARGETDIR}"
test_ok meson --prefix=$TARGETDIR --buildtype=release ${BUILD_DIR} ${GSTREAMER_OPTS}
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
    if [[ '/usr/lib/' != ${lib:0:9} && '/System/Library/' != ${lib:0:16} ]]; then
      if [[ '@' == ${lib:0:1} ]]; then
        if [[ '@loader_path' == ${lib:0:12} ]]; then
          cp -n "${lib/@loader_path/$lib_dir}" $folder
        else
          echo "Unsupport path: $lib"
        fi
      else
        cp -n $lib $folder
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


lib_dependency_copy ${TARGETDIR}/bin/gst_pipeline_studio "${TARGETDIR}/bin"
lib_dependency_copy ${TARGETDIR}/lib/libgobject-2.0.0.dylib "${TARGETDIR}/bin"
lib_dependency_copy ${TARGETDIR}/lib/libsoup-2.4.1.dylib "${TARGETDIR}/bin"

# lib_dependency_copy ${TARGETDIR}/bin/libunistring.2.dylib "${TARGETDIR}/bin"
# lib_dependency_copy /usr/local/lib/libcairo-script-interpreter.2.dylib "${TARGETDIR}/bin"
# lib_dependency_copy /usr/local/lib/libgettextsrc-0.20.1.dylib "${TARGETDIR}/bin"
# lib_dependency_copy /usr/local/lib/libharfbuzz-icu.0.dylib "${TARGETDIR}/bin"


for file in ${TARGETDIR}/lib/gstreamer-1.0/*.dylib
do
    echo "${file}"
    lib_dependency_copy ${file} "${TARGETDIR}/lib/gstreamer-1.0"
done

test_ok cp -f "${PROJECTDIR}/macos/mac_launcher.sh" "${TARGETDIR}/bin/launcher.sh"
# cp -f /usr/local/lib/libgtk-gtk4.1.dylib "${TARGETDIR}/bin"
# cp -f /usr/local/lib/libgirepository-1.0.1.dylib "${TARGETDIR}/bin"
# cp -f /usr/local/lib/librsvg-2.2.dylib "${TARGETDIR}/bin"
# cp -f /usr/local/lib/libgthread-2.0.0.dylib "${TARGETDIR}/bin"
# echo "[done]"

# copy GStreamer dependencies
# cp -f /usr/local/lib/gstreamer-1.0/libgtk-gtk4.1.dylib "${TARGETDIR}/lib/gstreamer-1.0"

# copy GDBus/Helper and dependencies files
# echo -n "Copy GDBus/Helper and dependencies......"
# cp -f /usr/local/bin/gdbus "${TARGETDIR}/bin"
# cp -f /usr/local/bin/gdk-pixbuf-query-loaders "${TARGETDIR}/bin"
# lib_dependency_copy ${TARGETDIR}/bin/gdbus "${TARGETDIR}/bin"
# lib_dependency_copy ${TARGETDIR}/bin/gdk-pixbuf-query-loaders "${TARGETDIR}/bin"
# echo "[done]"



# copy GTK runtime dependencies resource
# echo -n "Copy GTK runtime resource......"
# cp -rf /usr/local/lib/gio "${TARGETDIR}/lib/"
# cp -rf /usr/local/lib/gtk-3.0 "${TARGETDIR}/lib/"
# cp -rf /usr/local/lib/gdk-pixbuf-2.0 "${TARGETDIR}/lib/"
# cp -rf /usr/local/lib/girepository-1.0 "${TARGETDIR}/lib/"
# cp -rf /usr/local/lib/libgda-5.0 "${TARGETDIR}/lib/"
# # Avoid override the latest locale file
# cp -r /usr/local/share/locale "${TARGETDIR}/share/"
# cp -rf /usr/local/share/icons "${TARGETDIR}/share/"
# cp -rf /usr/local/share/fontconfig "${TARGETDIR}/share/"
# cp -rf /usr/local/share/themes/Mac "${TARGETDIR}/share/themes/"
# cp -rf /usr/local/share/themes/Default "${TARGETDIR}/share/themes/"
# cp -rf /usr/local/share/gtksourceview-4 "${TARGETDIR}/share/"
# glib-compile-schemas /usr/local/share/glib-2.0/schemas
# cp -f /usr/local/share/glib-2.0/schemas/gschema* "${TARGETDIR}/share/glib-2.0/schemas"
# # find "${TARGETDIR}/bin" -type f -path '*.dll.a' -exec rm '{}' \;
# lib_dependency_analyze ${TARGETDIR}/lib ${TARGETDIR}/bin
# lib_dependency_analyze ${TARGETDIR}/bin ${TARGETDIR}/bin
# echo "[done]"

# copy app icons and license files to target dir
echo -n "Copy app icon(svg) files......"
cp -f "${PROJECTDIR}/../data/icons/org.freedesktop.dabrain34.GstPipelineStudio.ico" "${TARGETDIR}/bin"
cp -f "${PROJECTDIR}/../data/icons/org.freedesktop.dabrain34.GstPipelineStudio.svg" "${TARGETDIR}/share/icons/hicolor/scalable/apps"
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

# make installer package
echo "make macos installer(.dmg)......"
cp "${PROJECTDIR}/macos/installers/dmg.json" gps_dmg.json
cp "${PROJECTDIR}/macos/installers/background.png" "${PROJECTDIR}/${BUILD_DIR}/gps_dmg_background.png"
rm -f ${PROJECTDIR}/GstPipelineStudio-${VERSION}-macos.dmg
appdmg gps_dmg.json "${PROJECTDIR}/GstPipelineStudio-${VERSION}-macos.dmg"
if [ $? -eq 0 ]; then
  echo "[done]"
  else
  echo "[failed]"
fi

# make portable package
echo -n "make macos portable......"
tar czf "${PROJECTDIR}/GstPipelineStudio-${VERSION}-macos.tar.gz" -C "${PROJECTDIR}" ${BUILD_DIR}/GstPipelineStudio.app
if [ $? -eq 0 ]; then
  echo "[done]"
  else
  echo "[failed]"
fi
