param(
  [ValidateSet("Debug", "Release", "Both", "None")]
  [string]$Build = "Debug",

  [string]$GodotExe = "godot",
  [switch]$Editor,
  [switch]$Headless,

  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$GodotArgs
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$xtaskArgs = @(
  "xtask",
  "run",
  "--build", $Build.ToLowerInvariant(),
  "--godot-exe", $GodotExe
)

if ($Editor) {
  $xtaskArgs += "--editor"
}
if ($Headless) {
  $xtaskArgs += "--headless"
}
if ($GodotArgs -and $GodotArgs.Count -gt 0) {
  if ($GodotArgs[0] -eq "--") {
    if ($GodotArgs.Count -gt 1) {
      $GodotArgs = $GodotArgs[1..($GodotArgs.Count - 1)]
    } else {
      $GodotArgs = @()
    }
  }

  if ($GodotArgs.Count -gt 0) {
    $xtaskArgs += "--"
    $xtaskArgs += $GodotArgs
  }
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
