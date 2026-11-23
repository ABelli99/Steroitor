# Build release della versione nativa e installer NSIS in native\target\release\steroitor_native_v<versione>.
# NSIS: quello scaricato da Tauri, oppure makensis nel PATH.
$ErrorActionPreference = 'Stop'
$native = Split-Path $PSScriptRoot
$version = (Select-String -Path "$native\Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$makensis = @("$env:LOCALAPPDATA\tauri\NSIS\makensis.exe", (Get-Command makensis -ErrorAction SilentlyContinue).Source) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $makensis) { throw 'makensis non trovato: installa NSIS o fai una build Tauri che lo scarica' }

# Cartella di build separata: target\release\steroitor.exe è spesso bloccato da un'istanza aperta.
$env:CARGO_TARGET_DIR = "$native\target\installer-build"
cargo build --release --manifest-path "$native\Cargo.toml"
if ($LASTEXITCODE) { throw 'cargo build fallita' }

$out = "$native\target\release\steroitor_native_v$version"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "$env:CARGO_TARGET_DIR\release\steroitor.exe" $out
& $makensis /V2 "/DVERSION=$version" "/DEXE=$out\steroitor.exe" "/DOUT=$out\Steroitor-Native_${version}_x64-setup.exe" "$PSScriptRoot\steroitor-native.nsi"
if ($LASTEXITCODE) { throw 'makensis fallito' }
Get-ChildItem $out
