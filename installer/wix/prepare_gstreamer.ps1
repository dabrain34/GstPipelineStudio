# install gtk, gstreamer built within the docker image to a clean folder used by wix to generate the package.
Remove-Item -Recurse -Force c:\gst-install-clean
New-Item c:\gst-install-clean -ItemType Directory
New-Item c:\gst-install-clean\bin -ItemType Directory

Copy-Item -Path C:\gst-install\bin\*.dll -Destination c:\gst-install-clean\bin\
Copy-Item -Path C:\gst-install\bin\*.exe -Destination c:\gst-install-clean\bin\

New-Item c:\gst-install-clean\lib\gstreamer-1.0 -ItemType Directory
Copy-Item -Path C:\gst-install\lib\gstreamer-1.0\*.dll -Destination c:\gst-install-clean\lib\gstreamer-1.0

