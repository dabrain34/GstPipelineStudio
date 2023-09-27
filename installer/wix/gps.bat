@echo off
set MYDIR=%~dp0
setlocal
set PATH=%MYDIR%bin;%PATH%
echo %PATH%
set GST_PLUGIN_PATH=%MYDIR%\lib\gstreamer-1.0
echo %GST_PLUGIN_PATH%
gst_pipeline_studio.exe