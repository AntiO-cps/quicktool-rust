param([int]$Count=520,[string]$Output='tests/index-oracle.json',[string]$FilesFrom='',[switch]$Categories,[switch]$CaseMapping)
$ErrorActionPreference='Stop'
$taskProject=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$taskRuntime=(Resolve-Path (Join-Path $taskProject '../node_modules/electron/dist/electron.exe')).Path
$taskQa=Join-Path $taskProject '.qa'
[IO.Directory]::CreateDirectory($taskQa)|Out-Null
$taskPrior=[Environment]::GetEnvironmentVariable('ELECTRON_RUN_AS_NODE','Process')
try {
 $env:ELECTRON_RUN_AS_NODE='1'
 $taskArguments=@('scripts/capture-index.cjs','--count',"$Count",'--output',$Output)
 if($FilesFrom){$taskArguments+=@('--files-from',$FilesFrom)}
 if($Categories){$taskArguments=@('scripts/capture-index-categories.cjs')}
 if($CaseMapping){$taskArguments=@('scripts/capture-js-case.cjs')}
 $taskProcess=Start-Process -FilePath $taskRuntime -ArgumentList $taskArguments -WorkingDirectory $taskProject -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskQa 'index-capture.out') -RedirectStandardError (Join-Path $taskQa 'index-capture.err')
 Get-Content (Join-Path $taskQa 'index-capture.out')
 if($taskProcess.ExitCode -ne 0){Get-Content (Join-Path $taskQa 'index-capture.err');throw "original index capture failed: $($taskProcess.ExitCode)"}
 if($Categories){ & rustfmt --edition 2024 (Join-Path $taskProject 'src/index_categories.rs') }
 if($CaseMapping){ & rustfmt --edition 2024 (Join-Path $taskProject 'src/js_case_data.rs') }
} finally {
 [Environment]::SetEnvironmentVariable('ELECTRON_RUN_AS_NODE',$taskPrior,'Process')
}
