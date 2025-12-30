@echo off
set MYDIR=%~dp0
setlocal
set PATH=%MYDIR%bin;%PATH%
set GST_PLUGIN_PATH=%MYDIR%\lib\gstreamer-1.0
start "" "%MYDIR%bin\gst-pipeline-studio.exe"