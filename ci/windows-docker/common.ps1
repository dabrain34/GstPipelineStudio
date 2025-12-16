# Common configuration and functions for Windows build scripts

# TLS configuration
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

# Version configuration (centralized)
$script:DEFAULT_PANGO_BRANCH = "1.56.4"
$script:DEFAULT_LIBXML2_BRANCH = "v2.15.1"
$script:DEFAULT_GTK_BRANCH = "4.20.3"
$script:DEFAULT_GST_BRANCH = "1.28.0"

# Build tools paths
$env:VS_BUILD_TOOLS = "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\Tools\VsDevCmd.bat"
$env:VS_BUILD_TOOLS = "C:\BuildTools\Common7\Tools\VsDevCmd.bat"
$env:INSTALL_PREFIX = "C:\gst-install\"
$env:MESON_PREFIX = "--prefix=$env:INSTALL_PREFIX -Dbuildtype=release"

# Helper function: Clone repository
function Clone-Repo {
    param($branch, $url, $dest)
    git clone -b $branch --depth 1 $url $dest
    if (!$?) {
        Write-Host "Failed to clone $dest"
        Exit 1
    }
}

# Helper function: Build with meson
function Build-WithMeson {
    param($name, $mesonArgs)
    Write-Output "Building $name"
    cmd.exe /C "`"$env:VS_BUILD_TOOLS`" -host_arch=amd64 -arch=amd64 && meson setup _build $env:MESON_PREFIX $mesonArgs && meson compile -C _build && ninja -C _build install"
    if (!$?) {
        Write-Host "Failed to build and install $name"
        Exit 1
    }
}

# Helper function: Cleanup build directory
function Remove-BuildDir {
    param($path, $name)
    Set-Location C:\
    cmd /c rmdir /s /q `"$path`"
    if (!$?) {
        Write-Host "Failed to remove $name checkout"
        Exit 1
    }
}
