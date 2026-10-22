param([string]$Executable = '', [string]$Report = '')
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
if (!$Executable) { $Executable = Join-Path $project 'dist/QuickTool.exe' }
if (!$Report) { $Report = Join-Path $project '.qa/expanded-verification.json' }
$Executable = [IO.Path]::GetFullPath($Executable)
$Report = [IO.Path]::GetFullPath($Report)
if (!(Test-Path -LiteralPath $Executable -PathType Leaf)) { throw 'Application executable missing' }
New-Item -ItemType Directory -Path (Split-Path -Parent $Report) -Force | Out-Null
$started = [DateTime]::UtcNow
$process = Start-Process -FilePath $Executable -ArgumentList @('--verify-expanded', ('"' + $Report + '"')) -WindowStyle Hidden -PassThru
try {
    if (!$process.WaitForExit(45000)) {
        Stop-Process -Id $process.Id
        throw 'Isolated expanded regression timed out'
    }
    $exitCode = $process.ExitCode
} finally {
    # Release the process handle before an installation test uninstalls the EXE.
    $process.Dispose()
}
if ($exitCode -ne 0) { throw "Expanded regression exited $exitCode; inspect $Report" }
if (!(Test-Path -LiteralPath $Report) -or (Get-Item -LiteralPath $Report).LastWriteTimeUtc -lt $started) {
    throw 'Application did not produce a fresh expanded report'
}
$result = Get-Content -LiteralPath $Report -Raw | ConvertFrom-Json
if (!$result.passed) { throw "Expanded regression failed; inspect $Report" }
Write-Output "PASS: $Report"
