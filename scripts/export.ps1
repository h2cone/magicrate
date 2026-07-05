param(
  [ValidateSet("Release", "Both")]
  [string]$Build = "Release",

  [string]$GodotExe = "godot",
  [string]$PresetName = "Windows Desktop",

  [string]$OutDir = "export",
  [string]$ExeName = "game.exe",

  [switch]$ForceCreateExportPreset,
  [switch]$IncludePdb,
  [switch]$NoRecoveryMode
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$xtaskArgs = @(
  "xtask",
  "export",
  "--build", $Build.ToLowerInvariant(),
  "--godot-exe", $GodotExe,
  "--preset-name", $PresetName,
  "--out-dir", $OutDir,
  "--exe-name", $ExeName
)

if ($ForceCreateExportPreset) {
  $xtaskArgs += "--force-create-export-preset"
}
if ($IncludePdb) {
  $xtaskArgs += "--include-pdb"
}
if ($NoRecoveryMode) {
  $xtaskArgs += "--no-recovery-mode"
}

Push-Location $repoRoot
try {
  & cargo @xtaskArgs
  if ((Test-Path variable:LASTEXITCODE) -and $LASTEXITCODE -ne 0) {
    throw "cargo $($xtaskArgs -join ' ') failed with exit code $LASTEXITCODE."
  }
} finally {
  Pop-Location
}
