# Source common configuration and functions
. C:\common.ps1

# GStreamer-specific meson args
# force fallback for glib is due to a bug when building libsoup. See https://gitlab.freedesktop.org/gstreamer/gstreamer/-/merge_requests/10136
$gstMesonArgs = "-Dforce_fallback_for=glib " +
    "-Dglib:tests=false " +
    "-Dglib:introspection=disabled " +
    "-Dlibnice:tests=disabled " +
    "-Dlibnice:examples=disabled " +
    "-Dffmpeg:tests=disabled " +
    "-Dopenh264:tests=disabled " +
    "-Dpygobject:tests=false " +
    "-Dges=disabled " +
    "-Drtsp_server=disabled " +
    "-Ddevtools=disabled " +
    "-Dsharp=disabled " +
    "-Dpython=disabled " +
    "-Dvaapi=disabled " +
    "-Dlibxml2:python=false " +
    "-Dgpl=enabled"

# Clone gstreamer
Clone-Repo $DEFAULT_GST_BRANCH "https://gitlab.freedesktop.org/gstreamer/gstreamer.git" "C:\gstreamer"

# Build gstreamer
Set-Location C:\gstreamer
Build-WithMeson "gst" $gstMesonArgs

# Cleanup
Remove-BuildDir "C:\gstreamer" "gst"

Exit 0
