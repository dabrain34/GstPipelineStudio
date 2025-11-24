[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12;

# Download gstreamer and all its subprojects
git clone -b $env:DEFAULT_GST_BRANCH --depth 1 https://gitlab.freedesktop.org/gstreamer/gstreamer.git C:\gstreamer
if (!$?) {
  Write-Host "Failed to clone gstreamer"
  Exit 1
}

Set-Location C:\gstreamer
# force fallback for glib is due to a bug when building libsoup. See https://gitlab.freedesktop.org/gstreamer/gstreamer/-/merge_requests/10136
$env:MESON_ARGS = "--prefix=C:\gst-install\ -Dbuildtype=release " +
    "-Dforce_fallback_for=glib " +
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
    "-Dgpl=enabled "

Write-Output "Building gst"
cmd.exe /C "C:\BuildTools\Common7\Tools\VsDevCmd.bat -host_arch=amd64 -arch=amd64 && meson _build $env:MESON_ARGS && meson compile -C _build && ninja -C _build install"

if (!$?) {
  Write-Host "Failed to build and install gst"
  Exit 1
}

Set-Location C:\
cmd /c rmdir /s /q  C:\gstreamer
if (!$?) {
  Write-Host "Failed to remove gst checkout"
  Exit 1
}
