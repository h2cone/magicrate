param(
  [string]$RepoUrl = "https://github.com/godot-rust/gdext.git",
  [string]$Branch = "master",
  [switch]$DryRun,
  [switch]$SkipLockfile
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$xtaskArgs = @(
  "xtask",
  "update-gdext",
  "--repo-url", $RepoUrl,
  "--branch", $Branch
)

if ($DryRun) {
  $xtaskArgs += "--dry-run"
}
if ($SkipLockfile) {
  $xtaskArgs += "--skip-lockfile"
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
