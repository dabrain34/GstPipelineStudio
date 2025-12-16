. C:\common.ps1

# force fallback for glib is due to a bug when building libsoup. See https://gitlab.freedesktop.org/gstreamer/gstreamer/-/merge_requests/10136
$mesonArgs = "-Dforce_fallback_for=glib" +
    " -Dglib:tests=false" +
    " -Dglib:introspection=disabled" +
    " -Dlibnice:tests=disabled" +
    " -Dlibnice:examples=disabled" +
    " -Dffmpeg:tests=disabled" +
    " -Dopenh264:tests=disabled" +
    " -Dpygobject:tests=false" +
    " -Dges=disabled" +
    " -Drtsp_server=disabled" +
    " -Ddevtools=disabled" +
    " -Dsharp=disabled" +
    " -Dpython=disabled" +
    " -Dlibxml2:python=false" +
    " -Dgpl=enabled"

cmd /c rmdir /s /q C:\gstreamer
Clone-Repo $script:DEFAULT_GST_BRANCH "https://gitlab.freedesktop.org/gstreamer/gstreamer.git" "C:\gstreamer"
Set-Location C:\gstreamer
Build-WithMeson "gst" $mesonArgs
Remove-BuildDir "C:\gstreamer" "gst"
