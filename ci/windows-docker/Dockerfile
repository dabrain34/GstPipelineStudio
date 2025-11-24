# escape=`
ARG GST_WINDOWS_IMAGE="registry.freedesktop.org/gstreamer/gstreamer/amd64/windows:2025-09-19.0-main"
FROM ${GST_WINDOWS_IMAGE}

# Make sure any failure in PowerShell is fatal
ENV ErrorActionPreference='Stop'
SHELL ["powershell","-NoLogo", "-NonInteractive", "-Command"]

ARG DEFAULT_PANGO_BRANCH="1.56.4"
ARG DEFAULT_GTK_BRANCH="4.16.7"
ARG DEFAULT_GST_BRANCH="1.26.8"

# The rust version is set by the images_template.yml via GST_RS_STABLE
ARG RUST_VERSION="invalid"

RUN choco install -y pkgconfiglite nasm llvm

RUN pip3 install meson==1.8.2

RUN Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile C:\rustup-init.exe
RUN C:\rustup-init.exe -y --profile minimal --default-toolchain $env:RUST_VERSION

# https://stackoverflow.com/a/50716450
RUN setx PATH '%PATH%;C:\Program Files\NASM;C:\gst-install\bin;c:\Program Files\gettext-iconv\bin;C:\Users\ContainerAdministrator\.cargo\bin'
ENV PKG_CONFIG_PATH="C:\gst-install\lib\pkgconfig"

RUN cargo install cargo-c

# This is working with wrap files for GTK and GStreamer but need to build pango first
# Prebuilt to save some compile time on Windows
COPY install_pango.ps1 install_gst.ps1 install_gtk.ps1 C:\
RUN C:\install_pango.ps1
RUN C:\install_gtk.ps1
RUN C:\install_gst.ps1


RUN Invoke-WebRequest -Uri https://github.com/mlocati/gettext-iconv-windows/releases/download/v0.21-v1.16/gettext0.21-iconv1.16-shared-64.exe -OutFile C:\gettext0.21.exe
RUN C:\gettext0.21.exe /SILENT

# needed by gtk
RUN pip3 install packaging
