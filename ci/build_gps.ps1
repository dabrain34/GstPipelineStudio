$env:MESON_ARGS = "--prefix=C:\gst-install\ -Dbuildtype=release"
cmd.exe /C "C:\BuildTools\Common7\Tools\VsDevCmd.bat -host_arch=amd64 -arch=amd64 && meson _build $env:MESON_ARGS && meson compile -C _build && ninja -C _build install"
if (!$?) {
    Write-Host "Failed to build and install GstPipelineStudio"
    Exit 1
}