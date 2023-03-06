# escape=`

FROM "registry.freedesktop.org/gstreamer/gstreamer/amd64/windows:2022-09-23.0-main"

# Make sure any failure in PowerShell is fatal
ENV ErrorActionPreference='Stop'
SHELL ["powershell","-NoLogo", "-NonInteractive", "-Command"]

ARG DEFAULT_GST_BRANCH="1.22"
ARG DEFAULT_GTK_BRANCH="4.8.2"
ARG DEFAULT_PANGO_BRANCH="1.50.14"
ARG RUST_VERSION="invalid"

RUN choco install -y pkgconfiglite nasm llvm

# https://stackoverflow.com/a/50716450
RUN setx PATH '%PATH%;C:\Program Files\NASM;C:\gst-install\bin;c:\Program Files\gettext-iconv\bin'
ENV PKG_CONFIG_PATH="C:\gst-install\lib\pkgconfig"

COPY install_pango.ps1 install_gst.ps1 install_gtk.ps1 C:\
RUN C:\install_pango.ps1
RUN C:\install_gst.ps1
RUN C:\install_gtk.ps1

RUN Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile C:\rustup-init.exe
RUN C:\rustup-init.exe -y --profile minimal --default-toolchain $env:RUST_VERSION

RUN Invoke-WebRequest -Uri https://github.com/mlocati/gettext-iconv-windows/releases/download/v0.21-v1.16/gettext0.21-iconv1.16-shared-64.exe -OutFile C:\gettext0.21.exe
RUN C:\gettext0.21.exe /SILENT
