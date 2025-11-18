pip3 install meson==1.8.2

# $env:MESON_ARGS = "--prefix=C:\gst-install\ -Dbuildtype=release " +
#         "-Dforce_fallback_for=gstreamer-1.0 " +
#         "-Dgstreamer-1.0:libav=disabled " +
#         "-Dgstreamer-1.0:examples=disabled " +
#         "-Dgstreamer-1.0:introspection=disabled " +
#         "-Dgstreamer-1.0:rtsp_server=disabled " +
#         "-Dgstreamer-1.0:devtools=disabled " +
#         "-Dgstreamer-1.0:ges=disabled " +
#         "-Dgstreamer-1.0:tests=disabled " +
#         "-Dgstreamer-1.0:gst-examples=disabled " +
#         "-Dgstreamer-1.0:python=disabled " +
#         "-Dgstreamer-1.0:gtk=enabled " +
#         "-Dgst-plugins-base:tests=disabled " +
#         "-Dgst-plugins-bad:openexr=disabled " +
#         "-Dgst-plugins-bad:vulkan=disabled " +
#         "-Dgst-plugins-bad:webrtc=disabled " +
#         "-Dgst-plugins-bad:webrtcdsp=disabled " +
#         "-Dorc:gtk_doc=disabled " +
#         "-Dgtk4:introspection=disabled " +
#         "-Dgtk4:build-examples=false " +
#         "-Dgtk4:build-tests=false " +
#         "-Dgtk4:build-demos=false " +
#         "-Dgtk4:media-gstreamer=disabled " +
#         "-Dgtk4:x11-backend=false " +
#         "-Dgtk4:macos-backend=true " +
#         "-Dgtk4:print-cups=disabled " +
#         "-Dgtk4:vulkan=disabled " +
#         "-Dlibxml2:python=false " +
#         "-Djson-glib:introspection=disabled "
$env:MESON_ARGS = "--prefix=C:\gst-install\ -Dbuildtype=release "

cmd.exe /C "C:\BuildTools\Common7\Tools\VsDevCmd.bat -host_arch=amd64 -arch=amd64 && meson _build $env:MESON_ARGS && meson compile -C _build && ninja -C _build install"
if (!$?) {
    Write-Host "Failed to build and install GstPipelineStudio"
    Exit 1
}