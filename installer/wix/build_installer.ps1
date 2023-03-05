$wixFolder = Join-Path $PSScriptRoot -ChildPath 'wix/'
$candleToolPath = Join-Path $wixFolder -ChildPath candle.exe
$lightToolPath = Join-Path $wixFolder -ChildPath light.exe
$heatToolPath = Join-Path $wixFolder -ChildPath heat.exe
$GPSUpgradeCode = "9B87C8FF-599C-4F20-914E-AF5E68CB3DC0"
$GPSVersion = $(git describe --always --abbrev=0)
Write-Output $GPSVersion
$GPSVersion = "0.2.3"

try
{
    Push-Location $PSScriptRoot

    if(-not (Test-Path $wixFolder))
    {
        throw "Folder $wixFolder does not exist. Start DownloadAndExtractWix.ps1 script to create it."
    }
    if((-not (Test-Path $candleToolPath)) -or (-not (Test-Path $lightToolPath)))
    {
        throw "Tools required to build installer (candle.exe and light.exe) do not exist in wix folder."
    }
    # GST and GTK are installed in this folder by prepare_gstreamer.ps1.
    # GST and GTK are built by the docker image.
    $gstreamerInstallDir="c:\gst-install-clean"
    $gstreamerBinInstallDir= Join-Path $gstreamerInstallDir -ChildPath "bin/"
    $gstreamerPluginInstallDir= Join-Path $gstreamerInstallDir -ChildPath "lib\gstreamer-1.0"

    & "$heatToolPath" dir "$gstreamerBinInstallDir" -gg -sfrag -template:fragment -out gstreamer-1.0.wxs -cg "_gstreamer" -var var.gstreamerBinInstallDir -dr INSTALLFOLDER
    & "$heatToolPath" dir "$gstreamerPluginInstallDir" -gg -sfrag -template:fragment -out gstreamer-plugins-1.0.wxs -cg "_gstreamer_plugins" -var var.gstreamerPluginInstallDir -dr INSTALLFOLDER

    $files = "gps gstreamer-1.0 gstreamer-plugins-1.0"
    $wxs_files = @()
    $obj_files = @()
    foreach ($f in $files.split(" ")){
        $wxs_files += "$f.wxs"
        $obj_files += "$f.wixobj"
    }
    Write-Output $wxs_files
    Write-Output $obj_files
    # compiling wxs file into wixobj
    $msiFileName = "GstPipelineStudio-$GPSVersion.msi"
    foreach ($f in $wxs_files){
        & "$candleToolPath" "$f" -dPlatform=x64 -dGPSUpgradeCode="$GPSUpgradeCode" -dGPSVersion="$GPSVersion" -dgstreamerBinInstallDir="$gstreamerBinInstallDir" -dgstreamerPluginInstallDir="$gstreamerPluginInstallDir"
        if($LASTEXITCODE -ne 0)
        {
            throw "Compilation of $wxsFileName failed with exit code $LASTEXITCODE"
        }
    }
    
    $AllArgs = $obj_files + @('-out', $msiFileName)

    & $lightToolPath $AllArgs -ext WixUIExtension
    if($LASTEXITCODE -ne 0)
    {
        throw "Linking of $wixobjFileName failed with exit code $LASTEXITCODE"
    }
}
catch
{
    Write-Error $_
    exit 1
}
finally
{
    Pop-Location
}