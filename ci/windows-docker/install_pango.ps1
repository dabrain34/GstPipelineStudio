. C:\common.ps1

$mesonArgs = "-Dintrospection=disabled -Dbuild-examples=false -Dbuild-testsuite=false"

Clone-Repo $script:DEFAULT_PANGO_BRANCH "https://gitlab.gnome.org/gnome/pango.git" "C:\pango"
Set-Location C:\pango
Build-WithMeson "pango" $mesonArgs
Remove-BuildDir "C:\pango" "pango"
