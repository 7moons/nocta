param([Parameter(Mandatory = $true)][string]$Source, [string]$Version)
$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path -LiteralPath $Source).Path
$executable = Join-Path $sourceRoot 'chrome.exe'
if (-not (Test-Path -LiteralPath $executable)) { throw 'Choose a fresh Chrome for Testing Windows distribution directory containing chrome.exe.' }
if (-not $Version) { $Version = (Get-Item -LiteralPath $executable).VersionInfo.ProductVersion }
& node (Join-Path $PSScriptRoot 'stage-browser.mjs') $sourceRoot $Version
if ($LASTEXITCODE -ne 0) { throw 'Managed browser staging failed.' }
