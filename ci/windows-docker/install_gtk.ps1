# Source common configuration and functions
. C:\common.ps1

# GTK-specific meson args
$gtkMesonArgs = "-Dintrospection=disabled" +
    " -Dbuild-examples=false" +
    " -Dbuild-tests=false" +
    " -Dbuild-demos=false" +
    " -Dmedia-gstreamer=disabled" +
    " -Dx11-backend=false" +
    " -Dmacos-backend=false" +
    " -Dvulkan=disabled" +
    " -Dprint-cups=disabled"

# Clone gtk
Clone-Repo $DEFAULT_GTK_BRANCH "https://gitlab.gnome.org/gnome/gtk.git" "C:\gtk"

# Build gtk
Set-Location C:\gtk
Build-WithMeson "gtk" $gtkMesonArgs

# Cleanup
Remove-BuildDir "C:\gtk" "gtk"

Exit 0
