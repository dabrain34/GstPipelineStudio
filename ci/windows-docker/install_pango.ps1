# Source common configuration and functions
. C:\common.ps1

# Pango-specific meson args
$pangoMesonArgs = "-Dintrospection=disabled" +
    " -Dbuild-examples=false" +
    " -Dbuild-testsuite=false"

# Clone pango
Clone-Repo $DEFAULT_PANGO_BRANCH "https://gitlab.gnome.org/gnome/pango.git" "C:\pango"

# Build pango
Set-Location C:\pango
Build-WithMeson "pango" $pangoMesonArgs

# Cleanup
Remove-BuildDir "C:\pango" "pango"

Exit 0
