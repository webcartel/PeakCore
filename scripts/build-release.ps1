$ErrorActionPreference = "Stop"

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$uiRoot = Join-Path $repositoryRoot "ui"
$uiManifest = Join-Path $uiRoot "src-tauri\Cargo.toml"
$daemonOutput = Join-Path $repositoryRoot "target\release"
$uiExecutable = Join-Path $uiRoot "src-tauri\target\release\peakcore-ui.exe"
$pairedUiExecutable = Join-Path $daemonOutput "peakcore-ui.exe"

Push-Location $repositoryRoot
try {
    cargo build --workspace --release
    if ($LASTEXITCODE -ne 0) {
        throw "Rust workspace release build failed."
    }

    npm ci --prefix $uiRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Frontend dependency installation failed."
    }

    npm run build --prefix $uiRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Frontend production build failed."
    }

    cargo build --release --manifest-path $uiManifest
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri UI release build failed."
    }

    if (-not (Test-Path -LiteralPath $uiExecutable -PathType Leaf)) {
        throw "The Tauri UI executable was not created: $uiExecutable"
    }
    New-Item -ItemType Directory -Force -Path $daemonOutput | Out-Null
    Copy-Item -LiteralPath $uiExecutable -Destination $pairedUiExecutable -Force

    Get-Item (Join-Path $daemonOutput "peakcore.exe"), $pairedUiExecutable |
        Select-Object Name, Length, FullName
}
finally {
    Pop-Location
}
