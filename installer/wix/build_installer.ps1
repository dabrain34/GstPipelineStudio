# Check wix installation
$wixInstalledFolder = "C:\Program Files (x86)\WiX Toolset v3.11\bin"
if(Test-Path $wixInstalledFolder)
{
    $wixFolder = $wixInstalledFolder
}
else
{
    $prepareWix = Join-Path $PSScriptRoot -ChildPath prepare_wix.ps1
    & "$prepareWix"
    $wixFolder = Join-Path $PSScriptRoot -ChildPath 'wix/'
}
$candleToolPath = Join-Path $wixFolder -ChildPath candle.exe
$lightToolPath = Join-Path $wixFolder -ChildPath light.exe
$heatToolPath = Join-Path $wixFolder -ChildPath heat.exe

$GPSUpgradeCode = "9B87C8FF-599C-4F20-914E-AF5E68CB3DC0"

$GPSVersion = Get-Content $PSScriptRoot\..\..\VERSION -Raw
Write-Output $GPSVersion
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
    $gstreamerBinInstallDir= Join-Path $gstreamerInstallDir -ChildPath "bin"
    $gstreamerPluginInstallDir= Join-Path $gstreamerInstallDir -ChildPath "lib"
    $gstreamerShareInstallDir= Join-Path $gstreamerInstallDir -ChildPath "share"

    & "$heatToolPath" dir "$gstreamerBinInstallDir" -gg -sfrag -template:fragment -out gstreamer-1.0.wxs -cg "_gstreamer" -var var.gstreamerBinInstallDir -dr INSTALLFOLDER
    & "$heatToolPath" dir "$gstreamerPluginInstallDir" -gg -sfrag -template:fragment -out gstreamer-plugins-1.0.wxs -cg "_gstreamer_plugins" -var var.gstreamerPluginInstallDir -dr INSTALLFOLDER
    & "$heatToolPath" dir "$gstreamerShareInstallDir" -v -ke -gg -sfrag -template:fragment -out gstreamer-share-1.0.wxs -cg "_gstreamer_share" -var var.gstreamerShareInstallDir -dr INSTALLFOLDER

    $files = "gps gstreamer-1.0 gstreamer-plugins-1.0 gstreamer-share-1.0"
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
        & "$candleToolPath" "$f" -dPlatform=x64 -dGPSUpgradeCode="$GPSUpgradeCode" -dGPSVersion="$GPSVersion" -dgstreamerBinInstallDir="$gstreamerBinInstallDir" -dgstreamerPluginInstallDir="$gstreamerPluginInstallDir" -dgstreamerShareInstallDir="$gstreamerShareInstallDir"
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