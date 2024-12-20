#FIXME: The latest gstreamer windows image contains wix.
$source = "https://github.com/wixtoolset/wix3/releases/download/wix3112rtm/wix311-binaries.zip"
$destination = Join-Path $PSScriptRoot -ChildPath 'wix.zip'
$wixFolder = Join-Path $PSScriptRoot -ChildPath 'wix/'

try
{
    Push-Location $PSScriptRoot

    if(Test-Path $destination)
    {
        Write-Host "WiX already download at: $destination"
    }
    else
    {
        Write-Host "Downloading $source ..."
        Invoke-WebRequest -Uri $source -OutFile $destination
        Write-Host "Download finished" -ForegroundColor Green
    }
    if(Test-Path $wixFolder)
    {
        Write-Host "WiX already extracted at: $wixFolder"
    }
    else
    {
        Write-Host "Extracting $destination ..."
        Expand-Archive -LiteralPath $destination -DestinationPath $wixFolder
        Write-Host "Extraction finished" -ForegroundColor Green
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