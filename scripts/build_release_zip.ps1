param(
    [Parameter(Mandatory = $true)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $distDir = Join-Path (Get-Location) 'target\dist'
    $stagingDir = Join-Path $distDir "inzone-buds-battery-tray-$Version-windows-x64"
    $zipPath = Join-Path $distDir "inzone-buds-battery-tray-$Version-windows-x64.zip"

    $requiredFiles = @(
        'target\release\inzone-buds-battery-tray.exe',
        'config\settings.json',
        'docs\protocol.md',
        'README.md',
        'LICENSE'
    )
    foreach ($file in $requiredFiles) {
        if (-not (Test-Path -LiteralPath $file)) {
            throw "required file not found: $file"
        }
    }

    Remove-Item -LiteralPath $stagingDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $zipPath -Force -ErrorAction SilentlyContinue

    New-Item -ItemType Directory -Path $stagingDir | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $stagingDir 'config') | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $stagingDir 'docs') | Out-Null

    Copy-Item -LiteralPath 'target\release\inzone-buds-battery-tray.exe' -Destination $stagingDir
    Copy-Item -LiteralPath 'config\settings.json' -Destination (Join-Path $stagingDir 'config')
    Copy-Item -LiteralPath 'docs\protocol.md' -Destination (Join-Path $stagingDir 'docs')
    Copy-Item -LiteralPath 'README.md', 'LICENSE' -Destination $stagingDir

    Compress-Archive -Path (Join-Path $stagingDir '*') -DestinationPath $zipPath -Force

    $hash = Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath
    Write-Host ''
    Write-Host 'Release zip created successfully.'
    Write-Host "zip=$zipPath"
    Write-Host "sha256=$($hash.Hash)"
}
finally {
    Pop-Location
}
