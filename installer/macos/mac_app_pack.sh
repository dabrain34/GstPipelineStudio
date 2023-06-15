#!/usr/bin/env bash

VERSION=4.0.1
SCRIPT=`basename "$0"`
APP_NAME="GstPipelineStudio"
APP_ICONS="/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/GenericApplicationIcon.icns"
OSX_VERSION=`sw_vers -productVersion`
PWD=`pwd`

function usage {
	cat <<EOF
$SCRIPT v${VERSION} for for Mac OS X
Usage:
  $SCRIPT [options]
Options:
  -h, --help 		Prints this help message, then exits
  -p, --path    Name of the path to 'appify' (required)
  -n, --name 		Name of the application (default "$APP_NAME")
  -o, --info    Name of the Info.plist
  -i, --icons		Name of the icons file to use when creating the app
                        (defaults to $APP_ICONS)
  -v, --version		Prints the version of this script, then exits
Description:
  Creates the GTK Mac app from a GTK install path.
  Appify has one required parameter, the path to appify:
    $SCRIPT --path gtk-app-install-path
  Note that you cannot rename appified apps. If you want to give your app
  a custom name, use the '--name' option
    $SCRIPT --path gtk-app-install-path --name "Sweet"
Copyright:
  Copyright (c) Thomas Aylott <http://subtlegradient.com/>
  Modified by Mathias Bynens <http://mathiasbynens.be/>
  Modified by Andrew Dvorak <http://OhReally.net/>
  Rewritten by Duncan McGreggor <http://github.com/oubiwann/>
  Modified by Zuhong Tao <https://github.com/taozuhong>
EOF
	exit 1
}

function version {
	echo "v${VERSION}"
	exit 1
}

function error {
	echo
	echo "ERROR: $1"
	echo
	usage
}

while :; do
  case $1 in
    -h | --help )    usage;;
    -p | --path )    APP_BUILD="$2"; shift ;;
    -n | --name )    APP_NAME="$2"; shift ;;
    -o | --info )    APP_INFO="$2"; shift ;;
    -i | --icons )   APP_ICONS="$2"; shift ;;
    -v | --version ) version;;
    -- )             shift; break ;;
    * )              break ;;
  esac
  shift
done

if [ -z ${APP_BUILD+nil} ]; then
	error "The GTK app path to appify must be provided!"
fi

if [ ! -d "$APP_BUILD" ]; then
	error "Can't find the target path '$APP_BUILD'"
fi

if [ -a "$APP_NAME.app" ]; then
	rm -rf "$APP_NAME.app"
fi

SCRIPT_PATH="$( cd "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"
APP_OUT_DIR="$( cd "$(dirname "$APP_BUILD")" >/dev/null 2>&1 ; pwd -P )"

APP_TOP_DIR=$APP_OUT_DIR/$APP_NAME.app
APP_CON_DIR=$APP_TOP_DIR/Contents
APP_RES_DIR=$APP_CON_DIR/Resources
APP_EXE_DIR=$APP_CON_DIR/MacOS
APP_ETC_DIR=$APP_RES_DIR/etc
APP_LIB_DIR=$APP_RES_DIR/lib

# Copy GstPipelineStudio.app resource
mkdir -vp "$APP_CON_DIR"/{MacOS,Resources}
cp -f "$APP_INFO" "$APP_CON_DIR/Info.plist"
cp -f "$APP_ICONS" "$APP_RES_DIR/$APP_NAME.icns"
cp -rf "$APP_BUILD/etc" "$APP_RES_DIR"
cp -rf "$APP_BUILD/lib" "$APP_RES_DIR"
cp -rf "$APP_BUILD/share" "$APP_RES_DIR"
cp -rf "$APP_BUILD/libexec" "$APP_RES_DIR"
cp $APP_BUILD/bin/gst_pipeline_studio $APP_EXE_DIR/gst_pipeline_studio-real
cp $APP_BUILD/bin/launcher.sh $APP_EXE_DIR/gst_pipeline_studio
chmod 766 "$APP_EXE_DIR/gst_pipeline_studio"
chmod 766 "$APP_EXE_DIR/gst_pipeline_studio-real"
chmod -R 766 "$APP_RES_DIR"/libexec/gstreamer-1.0


# Copy dependency libraries and update their search path
source $SCRIPT_PATH/mac_app_path.sh
if ls $APP_BUILD/bin/*.so 1> /dev/null 2>&1; then
  for sofile in $APP_BUILD/bin/*.so; do
    cp $sofile $APP_LIB_DIR
  done
fi
cp $APP_BUILD/bin/*.dylib $APP_LIB_DIR
chmod -R 766 $APP_LIB_DIR

echo -n "relocate the gstreamer plugins......"
for file in $APP_LIB_DIR/gstreamer-1.0/*.dylib
do
  echo "relocating ${file}"
  lib_change_paths \
    @executable_path/../Resources/lib \
    $APP_LIB_DIR \
    ${file}
done

lib_change_paths \
  @executable_path/../Resources/lib \
  $APP_LIB_DIR \
  $APP_EXE_DIR/gst_pipeline_studio-real

# lib_change_paths \
#   @executable_path/../Resources/lib \
#   $APP_LIB_DIR \
#   $APP_EXE_DIR/gdbus

# lib_change_paths \
#   @executable_path/../Resources/lib \
#   $APP_LIB_DIR \
#   $APP_EXE_DIR/gdk-pixbuf-query-loaders

lib_change_siblings $APP_LIB_DIR @loader_path

# Gio modules
# gio_modules="$(find $APP_LIB_DIR/gio/modules/ -name \*.dylib -o -name \*.so -type f)"
# for gio_module in $gio_modules; do
#   lib_change_paths \
#     @executable_path/../Resources/lib \
#     $APP_LIB_DIR \
#     $gio_module
# done

# # Gdk-pixbuf plugins
# pixbuf_plugins="$(find $APP_LIB_DIR/gdk-pixbuf-2.0/2.10.0/loaders/ -name \*.dylib -o -name \*.so -type f)"
# for pixbuf_plugin in $pixbuf_plugins; do
#   lib_change_paths \
#     @executable_path/../Resources/lib \
#     $APP_LIB_DIR \
#     $pixbuf_plugin
# done


echo "Mac app bundled at '$APP_TOP_DIR'"
