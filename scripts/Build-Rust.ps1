param([string]$OutputDirectory = '', [switch]$Offline, [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
if (!$OutputDirectory) { $OutputDirectory = Join-Path $project 'dist' }
$output = [IO.Path]::GetFullPath($OutputDirectory)
Push-Location $project
try {
    if (!$SkipBuild) {
        $buildArguments = @('build', '--release', '--bin', 'quicktool-rust')
        if ($Offline) { $buildArguments += '--offline' }
        & cargo @buildArguments
        if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed' }
    }
    $metadata = & cargo metadata --offline --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
    $executable = Join-Path $metadata.target_directory 'release/quicktool-rust.exe'
    if (!(Test-Path -LiteralPath $executable)) { throw "Missing executable: $executable" }
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    Copy-Item -LiteralPath $executable -Destination (Join-Path $output 'QuickTool.exe') -Force
    $assetOutput = Join-Path $output 'assets/assets'
    New-Item -ItemType Directory -Path $assetOutput -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $project 'assets/assets/flags') -Destination $assetOutput -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $project 'assets/licenses') -Destination (Join-Path $output 'assets') -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $project 'assets/assets/quicktool.ico') -Destination $assetOutput -Force
    Copy-Item -LiteralPath (Join-Path $project 'docs') -Destination $output -Recurse -Force
    @'
QuickTool Rust portable build
Run QuickTool.exe. Keep the assets directory beside the executable.
Settings and history: %APPDATA%/QuickTool-Rust
Default shortcut: Alt+Space
This rewrite uses native Rust UI and does not require Electron or Node.js.
Validation and known limitations are listed in the source docs/migration.md.
'@ | Set-Content -LiteralPath (Join-Path $output 'README.txt') -Encoding UTF8
    Get-ChildItem -LiteralPath $output -File -Recurse | Where-Object Name -ne 'SHA256.json' | Get-FileHash -Algorithm SHA256 |
        Select-Object @{Name='path';Expression={$_.Path.Substring($output.Length+1)}},Hash |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'SHA256.json') -Encoding UTF8
    Write-Output "Portable build: $output"
} finally { Pop-Location }
