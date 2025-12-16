. C:\common.ps1

$mesonArgs = "-Dpython=disabled -Diconv=disabled"

Clone-Repo $script:DEFAULT_LIBXML2_BRANCH "https://gitlab.gnome.org/gnome/libxml2.git" "C:\libxml2"
Set-Location C:\libxml2
Build-WithMeson "libxml2" $mesonArgs
Remove-BuildDir "C:\libxml2" "libxml2"
