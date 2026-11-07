param([ValidatePattern('^[a-z0-9-]+$')][string]$Label='before',[switch]$UseSnapshot)
$ErrorActionPreference='Stop'
$taskProject=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
# Benchmark artifact directory; override with QUICKTOOL_QA_BASE for isolated runs.
$taskDeps=if($env:QUICKTOOL_QA_BASE){Join-Path $env:QUICKTOOL_QA_BASE 'quicktool-rust-target/release/deps'}else{(Join-Path $taskProject 'target/release/deps')}
$taskQa=Join-Path $taskProject '.qa'
$taskSource=Get-Content (Join-Path $taskProject 'tests/index_parity.rs') -Raw
$taskSource=$taskSource.Replace('//!','//').Replace('include_str!("index-oracle.json")','include_str!("../tests/index-oracle.json")').Replace('use quicktool_rust::index;','')
$taskSource=[regex]::Replace($taskSource,'(?m)^#\[test\]\r?\n','')
$taskSource=[regex]::Replace($taskSource,'(?m)^#\[ignore[^\r\n]*\]\r?\n','')
$taskCountries=Get-Content (Join-Path $taskProject 'src/countries.rs') -Raw
$taskCompare=$taskCountries.Substring($taskCountries.IndexOf('pub fn locale_compare('))
$taskPrelude=@'
#![allow(dead_code)]
// Actual index module; provider and UI output branches are unused in this isolated benchmark.
mod core { pub struct ResultItem; impl ResultItem { pub fn new(_: &str,_: &str)->Self {Self} } }
mod file_providers { pub struct Entry{pub path:String,pub name:String,pub is_dir:bool,pub size:Option<u64>} pub fn search(_: &str,_: &str,_:usize)->Result<Option<Vec<Entry>>,String>{Ok(None)} }
'@
$taskPrelude+="`n#[path=`"../src/js_case.rs`"] mod js_case;`nmod countries { use std::{cmp::Ordering,sync::OnceLock};`n$taskCompare`n}`n#[path=`"index-$Label.rs`"] mod index;`n"
$taskSource=$taskPrelude+$taskSource+"`nfn main(){synthetic_10000_scan_and_query_benchmark();}`n"
$taskDriver=Join-Path $taskQa "index-$Label-driver.rs"
$taskSnapshot=Join-Path $taskQa "index-$Label.rs"
[IO.File]::WriteAllText($taskDriver,$taskSource)
if(-not $UseSnapshot){Copy-Item -LiteralPath (Join-Path $taskProject 'src/index.rs') -Destination $taskSnapshot}
if(Test-Path (Join-Path $taskProject 'src/index_categories.rs')) {Copy-Item -LiteralPath (Join-Path $taskProject 'src/index_categories.rs') -Destination (Join-Path $taskQa 'index_categories.rs')}
$env:CARGO_MANIFEST_DIR=$taskProject
$taskOutput=Join-Path $taskQa "index-$Label-release.exe"
$taskRustArgs=@('--edition=2024','--crate-name','index_release_audit',$taskDriver,'-C','opt-level=3','-C','panic=abort','-C','debug-assertions=off','-C','lto=thin','-C','codegen-units=1','-C','embed-bitcode=yes','-L',('dependency='+$taskDeps),'-o',$taskOutput)
foreach($taskNative in Get-ChildItem (Join-Path :USERPROFILE '.cargo/registry/src') -Directory -Filter 'index.crates.io-*' | ForEach-Object { Get-ChildItem D:DocumentsChatGPTquicktoolust-rewrite.FullName -Directory -Filter 'windows_x86_64_msvc-*' }) {$taskRustArgs+=@('-L',('native='+$taskNative.FullName+'/lib'))}
foreach($taskCrate in @('rusqlite','serde','serde_json','unicode_normalization','getrandom','notify','walkdir','icu_properties','icu_collator','icu_locale')) { $taskLibrary=Get-ChildItem $taskDeps -Filter ('lib'+$taskCrate+'-*.rlib') | Sort-Object LastWriteTime -Descending | Select-Object -First 1; if(-not $taskLibrary){throw "missing cached release dependency $taskCrate"}; $taskRustArgs+=@('--extern',($taskCrate+'='+$taskLibrary.FullName)) }
& rustc @taskRustArgs
if($LASTEXITCODE -ne 0){throw 'isolated release benchmark compile failed'}
& $taskOutput
if($LASTEXITCODE -ne 0){throw 'isolated release benchmark failed'}
Copy-Item -LiteralPath (Join-Path $taskQa 'index-10000-rust.json') -Destination (Join-Path $taskQa "index-10000-rust-release-$Label.json")
