param([string]$MakeNsis = '', [string]$OutputDirectory = '', [switch]$SkipPortableBuild, [switch]$Offline)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
$payload = Join-Path $project 'dist'
if (!$OutputDirectory) { $OutputDirectory = Join-Path $project 'releases' }
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (!$MakeNsis) {
    $command = Get-Command makensis.exe -ErrorAction SilentlyContinue
    if ($command) { $MakeNsis = $command.Source }
}
if (!$MakeNsis -or !(Test-Path -LiteralPath $MakeNsis -PathType Leaf)) {
    throw 'Specify -MakeNsis with the path to an existing NSIS makensis.exe.'
}
if (!$SkipPortableBuild) {
    & (Join-Path $PSScriptRoot 'Build-Rust.ps1') -Offline:$Offline
    if ($LASTEXITCODE -ne 0) { throw 'Portable build failed' }
}
$manifest = Get-Content -LiteralPath (Join-Path $payload 'SHA256.json') -Raw | ConvertFrom-Json
foreach ($entry in $manifest) {
    if ((Get-FileHash -LiteralPath (Join-Path $payload $entry.path) -Algorithm SHA256).Hash -ne $entry.Hash) {
        throw "Payload hash mismatch: $($entry.path)"
    }
}
$version = [regex]::Match([IO.File]::ReadAllText((Join-Path $project 'Cargo.toml')), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if (!$version) { throw 'Cargo version missing' }
New-Item -ItemType Directory -Path $output -Force | Out-Null
$buildDirectory = Join-Path $project '.qa/installer-build'
New-Item -ItemType Directory -Path $buildDirectory -Force | Out-Null
$uninstall = Join-Path $buildDirectory 'uninstall-files.nsh'
$files = @(Get-ChildItem -LiteralPath $payload -Recurse -File)
$lines = foreach ($file in $files) {
    $relative = $file.FullName.Substring($payload.Length + 1)
    if ($relative -match '[\r\n"$]') { throw 'Unsupported payload filename' }
    'Delete "$INSTDIR\' + $relative + '"'
}
$directories = Get-ChildItem -LiteralPath $payload -Recurse -Directory | Sort-Object { $_.FullName.Length } -Descending
foreach ($directory in $directories) {
    $relative = $directory.FullName.Substring($payload.Length + 1)
    if ($relative -match '[\r\n"$]') { throw 'Unsupported payload directory' }
    $lines += 'RMDir "$INSTDIR\' + $relative + '"'
}
[IO.File]::WriteAllLines($uninstall, $lines, [Text.UTF8Encoding]::new($true))
$installer = Join-Path $output "QuickTool-Rust-Setup-$version-x64.exe"
& $MakeNsis '/V2' "/DPAYLOAD=$payload" "/DOUTPUT=$installer" "/DVERSION=$version" "/DUNINSTALL_FILES=$uninstall" (Join-Path $PSScriptRoot 'installer.nsi')
if ($LASTEXITCODE -ne 0) { throw 'NSIS build failed' }
$hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
"$hash  $([IO.Path]::GetFileName($installer))" | Set-Content -LiteralPath ($installer + '.sha256') -Encoding ascii
Write-Output "Installer: $installer"
Write-Output "SHA256: $hash"
