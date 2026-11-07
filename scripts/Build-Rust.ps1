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
    # Regenerate the portable tree from scratch so removed payload (e.g. the
    # former standalone flags) cannot leak into the installer.
    if (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output -Recurse -Force }
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    Copy-Item -LiteralPath $executable -Destination (Join-Path $output 'QuickTool.exe') -Force
    $assetOutput = Join-Path $output 'assets/assets'
    New-Item -ItemType Directory -Path $assetOutput -Force | Out-Null
    # Flags ship inside the country module package; no standalone copy.
    Copy-Item -LiteralPath (Join-Path $project 'assets/licenses') -Destination (Join-Path $output 'assets') -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $project 'assets/assets/quicktool.ico') -Destination $assetOutput -Force
    # Stage the optional-module package store. The installer core section always
    # extracts it; component sections copy the selected packages into
    # assets/modules, and the app can install/uninstall them later in settings.
    $packageOutput = Join-Path $output 'assets/packages'
    New-Item -ItemType Directory -Path $packageOutput -Force | Out-Null
    foreach ($module in (Get-ChildItem -LiteralPath (Join-Path $project 'assets/modules') -Directory)) {
        Copy-Item -LiteralPath $module.FullName -Destination (Join-Path $packageOutput $module.Name) -Recurse -Force
    }
    Copy-Item -LiteralPath (Join-Path $project 'docs') -Destination $output -Recurse -Force
    # Keep the notices of the actual Windows dependency graph beside the binary.
    $licenseMetadata = & cargo metadata --offline --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cargo license metadata failed' }
    $dependencyIds = [Collections.Generic.HashSet[string]]::new()
    foreach ($node in $licenseMetadata.resolve.nodes) { [void]$dependencyIds.Add($node.id) }
    $rustLicenseRoot = Join-Path $output 'licenses/rust'
    New-Item -ItemType Directory -Path $rustLicenseRoot -Force | Out-Null
    $licenseIndex = foreach ($package in ($licenseMetadata.packages | Sort-Object name, version)) {
        if (!$dependencyIds.Contains($package.id)) { continue }
        $packageRoot = Split-Path -Parent $package.manifest_path
        $licenseFiles = @(Get-ChildItem -LiteralPath $packageRoot -File | Where-Object Name -Match '^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)([._-].*)?$')
        if ($package.license_file) {
            $declaredLicense = if ([IO.Path]::IsPathRooted($package.license_file)) { $package.license_file } else { Join-Path $packageRoot $package.license_file }
            if (Test-Path -LiteralPath $declaredLicense -PathType Leaf) { $licenseFiles += Get-Item -LiteralPath $declaredLicense }
        }
        $packageLicenseOutput = Join-Path $rustLicenseRoot ($package.name + '-' + $package.version)
        New-Item -ItemType Directory -Path $packageLicenseOutput -Force | Out-Null
        foreach ($licenseFile in ($licenseFiles | Sort-Object FullName -Unique)) {
            Copy-Item -LiteralPath $licenseFile.FullName -Destination $packageLicenseOutput -Force
        }
        [PSCustomObject]@{ name=$package.name; version=$package.version; license=$package.license; files=@($licenseFiles.Name | Sort-Object -Unique) }
    }
    $licenseIndex | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $rustLicenseRoot 'INDEX.json') -Encoding UTF8
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
