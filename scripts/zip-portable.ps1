param([Parameter(Mandatory = $true)][string]$Directory, [Parameter(Mandatory = $true)][string]$Destination)
$ErrorActionPreference = 'Stop'
$releaseRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../release'))
$sourcePath = (Resolve-Path -LiteralPath $Directory).Path
$archivePath = [System.IO.Path]::GetFullPath($Destination)
if (-not $sourcePath.StartsWith($releaseRoot + [System.IO.Path]::DirectorySeparatorChar) -or -not $archivePath.StartsWith($releaseRoot + [System.IO.Path]::DirectorySeparatorChar)) { throw 'Portable paths must stay inside this project release directory.' }
if (Test-Path -LiteralPath $archivePath) { throw 'Archive already exists; keep the previous artifact.' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($sourcePath, $archivePath, [System.IO.Compression.CompressionLevel]::Optimal, $false)
