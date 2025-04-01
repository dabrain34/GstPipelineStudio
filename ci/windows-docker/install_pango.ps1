[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12;

$env:MESON_ARGS = "--prefix=C:\gst-install\ -Dbuildtype=release" +
" -Dintrospection=disabled" +
" -Dbuild-examples=false" +
" -Dbuild-testsuite=false"

# Download pango all its subprojects
git clone -b $env:DEFAULT_PANGO_BRANCH --depth 1 https://gitlab.gnome.org/gnome/pango.git C:\pango
if (!$?) {
  Write-Host "Failed to clone pango"
  Exit 1
}

Set-Location C:\pango
$env:VS_BUILD_TOOLS = "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat"
$env:VS_BUILD_TOOLS = "C:\BuildTools\Common7\Tools\VsDevCmd.bat"
Write-Output "Building pango"
cmd.exe /C "`"$env:VS_BUILD_TOOLS`" -host_arch=amd64 -arch=amd64 && meson _build $env:MESON_ARGS && meson compile -C _build && ninja -C _build install"

if (!$?) {
  Write-Host "Failed to build and install pango"
  Exit 1
}


# cmd /c rmdir /s /q  C:\pango
# if (!$?) {
#   Write-Host "Failed to remove pango checkout"
#   Exit 1
# }
