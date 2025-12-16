. C:\common.ps1

$mesonArgs = "-Dintrospection=disabled" +
    " -Dbuild-examples=false" +
    " -Dbuild-tests=false" +
    " -Dbuild-demos=false" +
    " -Dmedia-gstreamer=disabled" +
    " -Dx11-backend=false" +
    " -Dmacos-backend=false" +
    " -Dvulkan=disabled" +
    " -Dprint-cups=disabled"

cmd /c rmdir /s /q C:\gtk
Clone-Repo $script:DEFAULT_GTK_BRANCH "https://gitlab.gnome.org/gnome/gtk.git" "C:\gtk"
Set-Location C:\gtk
Build-WithMeson "gtk" $mesonArgs
Remove-BuildDir "C:\gtk" "gtk"
