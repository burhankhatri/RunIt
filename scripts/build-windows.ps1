# Build RunIt for Windows into dist\RunIt-windows-x64.zip
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Set-Location (Join-Path $PSScriptRoot '..')
$tag = 'b12651'
$sum = 'e58fa9a137d8046c69334005ad6fb3e904ddc7e56f2a61ad1b844335826ef3f4'
$eng = 'engines\windows-x64'

if (-not (Test-Path "$eng\.tag") -or (Get-Content "$eng\.tag") -ne $tag) {
    $zip = Join-Path ([IO.Path]::GetTempPath()) 'llama.zip'
    Invoke-WebRequest "https://github.com/ggml-org/llama.cpp/releases/download/$tag/llama-$tag-bin-win-vulkan-x64.zip" -OutFile $zip
    if ((Get-FileHash $zip -Algorithm SHA256).Hash -ne $sum) { throw 'the engine download does not match its checksum' }
    $raw = Join-Path ([IO.Path]::GetTempPath()) 'llama-raw'
    Remove-Item $raw -Recurse -Force -ErrorAction SilentlyContinue
    Expand-Archive $zip $raw
    Remove-Item $eng -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory $eng -Force | Out-Null
    $src = Split-Path (Get-ChildItem $raw -Recurse -Filter llama-bench.exe | Select-Object -First 1).FullName
    Copy-Item "$src\llama-bench.exe", "$src\llama-server.exe" $eng
    Get-ChildItem $src -Filter *.dll | Where-Object { $_.Name -match '^(ggml|llama\.dll|llama-common|llama-bench-impl|llama-server-impl|mtmd|libomp)' } | Copy-Item -Destination $eng
    Get-ChildItem $src -Filter 'LICENSE*' | Copy-Item -Destination $eng
    # The engine needs the Visual C++ runtime, which a fresh Windows may not have.
    $vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -property installationPath
    $crt = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory | Where-Object { $_.FullName -notmatch 'debug' } | Sort-Object FullName | Select-Object -Last 1
    if (-not $crt) { throw 'the Visual C++ runtime was not found' }
    Copy-Item "$($crt.FullName)\msvcp140*.dll", "$($crt.FullName)\vcruntime140*.dll" $eng
    Set-Content "$eng\.tag" $tag
}

$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release --target x86_64-pc-windows-msvc -p runit-ffi -p runit-cli
if ($LASTEXITCODE) { throw 'the Rust build failed' }
$rust = Join-Path $(if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }) 'x86_64-pc-windows-msvc\release'

$version = (Select-String '^version = "(.*)"' Cargo.toml | Select-Object -First 1).Matches.Groups[1].Value
$out = 'dist\RunIt'
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
dotnet publish apps\windows\RunIt.csproj -c Release -o $out -p:Version=$version -p:DebugType=none --nologo
if ($LASTEXITCODE) { throw 'the app build failed' }
Copy-Item "$rust\runit.dll" $out
Copy-Item "$rust\runit.exe" "$out\runit-cli.exe"
Copy-Item $eng "$out\engine" -Recurse
New-Item -ItemType Directory "$out\licenses" | Out-Null
Copy-Item LICENSE, apps\shared\fonts\OFL-*.txt "$out\licenses"
Remove-Item dist\RunIt-windows-x64.zip -ErrorAction SilentlyContinue
Compress-Archive "$out\*" dist\RunIt-windows-x64.zip
Get-Item dist\RunIt-windows-x64.zip | Select-Object Name, Length
