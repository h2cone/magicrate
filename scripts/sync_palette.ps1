param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Utf8NoBom([string]$Path, [string]$Value) {
  $parent = Split-Path -Parent $Path
  if ($parent -and -not (Test-Path -LiteralPath $parent)) {
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
  }

  $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
  [System.IO.File]::WriteAllText($Path, $Value, $utf8NoBom)
}

function Convert-HexToRgb([string]$Hex) {
  $clean = $Hex.TrimStart('#')
  if ($clean.Length -eq 8) {
    $clean = $clean.Substring(0, 6)
  }
  if ($clean.Length -ne 6) {
    throw "Expected 6 or 8 hex digits, got '$Hex'."
  }

  return @{
    R = [Convert]::ToInt32($clean.Substring(0, 2), 16)
    G = [Convert]::ToInt32($clean.Substring(2, 2), 16)
    B = [Convert]::ToInt32($clean.Substring(4, 2), 16)
  }
}

function Get-RgbFloatLiteral([string]$Hex) {
  $rgb = Convert-HexToRgb $Hex
  $format = {
    param([int]$Channel)
    $value = [Math]::Round($Channel / 255.0, 6)
    return $value.ToString("0.######", [System.Globalization.CultureInfo]::InvariantCulture)
  }

  return "Color($(& $format $rgb.R), $(& $format $rgb.G), $(& $format $rgb.B), 1)"
}

function Get-AsepriteColorLiteral([string]$Hex) {
  $clean = $Hex.TrimStart('#')
  if ($clean.Length -eq 6) {
    $clean += "FF"
  }
  if ($clean.Length -ne 8) {
    throw "Expected 6 or 8 hex digits, got '$Hex'."
  }

  $r = [Convert]::ToInt32($clean.Substring(0, 2), 16)
  $g = [Convert]::ToInt32($clean.Substring(2, 2), 16)
  $b = [Convert]::ToInt32($clean.Substring(4, 2), 16)
  $a = [Convert]::ToInt32($clean.Substring(6, 2), 16)
  return "Color { r = $r, g = $g, b = $b, a = $a }"
}

function Replace-Section([string]$Content, [string]$Pattern, [string]$Replacement) {
  return [regex]::Replace($Content, $Pattern, $Replacement, 1)
}

function Replace-All([string]$Content, [string]$Pattern, [string]$Replacement) {
  return [regex]::Replace($Content, $Pattern, $Replacement)
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$palettePath = Join-Path $repoRoot "godot\pipeline\palette\magicrate_palette.json"
$playerScriptPath = Join-Path $repoRoot "godot\pipeline\aseprite\scripts\player.lua"
$ldtkPath = Join-Path $repoRoot "godot\pipeline\ldtk\levels.ldtk"

$entityScenePaths = @{
  PushableCrate = (Join-Path $repoRoot "godot\entity\pushable_crate.tscn")
  GoalPetal = (Join-Path $repoRoot "godot\entity\goal_petal.tscn")
  BridgeSwitch = (Join-Path $repoRoot "godot\entity\bridge_switch.tscn")
  BridgeTile = (Join-Path $repoRoot "godot\entity\bridge_tile.tscn")
}

$playerRolePaletteMap = [ordered]@{
  transparent = "transparent"
  outline = "midnight"
  shadow = "stone"
  body = "sky"
  highlight = "cream"
  blush = "pink"
  sparkle = "sun"
  foot = "peach"
  eye = "midnight"
  eye_shine = "cream"
}

$tagPaletteMap = [ordered]@{
  wake = "sun"
  idle = "sky"
  move = "lime"
  jump = "pink"
}

$entityPaletteMap = @{
  PushableCrate = "amber"
  GoalPetal = "crimson"
  BridgeSwitch = "lime"
  BridgeTile = "stone"
}

$intGridPaletteMap = @{
  SOLID_ALL = "stone"
  SOLID_BOX_ONLY = "lavender"
  BRIDGE_RESERVE = "sky"
  HAZARD = "crimson"
  FALL_BLOCK = "amber"
}

$entityDefPaletteMap = @{
  PlayerSpawn = "pink"
  PushableCrate = "amber"
  GoalPetal = "crimson"
  BridgeSwitch = "lime"
  BridgeTile = "stone"
}

$paletteJson = Get-Content -Raw -LiteralPath $palettePath | ConvertFrom-Json
$entriesByName = @{}
foreach ($entry in $paletteJson.entries) {
  $entriesByName[$entry.name] = @{
    Slot = [int]$entry.slot
    Hex = [string]$entry.hex
  }
}

function Get-PaletteEntry([string]$Name) {
  if (-not $entriesByName.ContainsKey($Name)) {
    throw "Palette entry '$Name' was not found in $palettePath."
  }

  return $entriesByName[$Name]
}

$playerScript = Get-Content -Raw -LiteralPath $playerScriptPath

$playerRoleLines = @("local P <const> = {")
foreach ($key in $playerRolePaletteMap.Keys) {
  $entry = Get-PaletteEntry $playerRolePaletteMap[$key]
  $playerRoleLines += "  $key = $($entry.Slot),"
}
$playerRoleLines += "}"

$paletteLines = @("local PALETTE_COLORS <const> = {")
foreach ($entry in @($paletteJson.entries | Sort-Object slot)) {
  $paletteLines += "  [$($entry.slot)] = $(Get-AsepriteColorLiteral $entry.hex),"
}
$paletteLines += "}"

$tagLines = @("local TAG_COLORS <const> = {")
foreach ($key in $tagPaletteMap.Keys) {
  $entry = Get-PaletteEntry $tagPaletteMap[$key]
  $tagLines += "  $key = $(Get-AsepriteColorLiteral $entry.Hex),"
}
$tagLines += "}"

$playerScript = Replace-Section $playerScript `
  '(?s)local P <const> = \{.*?\r?\n\}\r?\n\r?\n(?=local PALETTE_COLORS <const> = \{)' `
  (($playerRoleLines -join "`r`n") + "`r`n`r`n")

$playerScript = Replace-Section $playerScript `
  '(?s)local PALETTE_COLORS <const> = \{.*?\r?\n\}\r?\n\r?\n(?=local TAG_COLORS <const> = \{)' `
  (($paletteLines -join "`r`n") + "`r`n`r`n")

$playerScript = Replace-Section $playerScript `
  '(?s)local TAG_COLORS <const> = \{.*?\r?\n\}\r?\n\r?\n(?=local BODY_CHAR_MAP <const> = \{)' `
  (($tagLines -join "`r`n") + "`r`n`r`n")

Write-Utf8NoBom -Path $playerScriptPath -Value $playerScript

$ldtkContent = Get-Content -Raw -LiteralPath $ldtkPath
$ldtkContent = Replace-All $ldtkContent '("bgColor":\s*")#[0-9A-Fa-f]{6}(")' ('$1' + (Get-PaletteEntry "slate").Hex + '$2')
$ldtkContent = Replace-All $ldtkContent '("defaultLevelBgColor":\s*")#[0-9A-Fa-f]{6}(")' ('$1' + (Get-PaletteEntry "fog").Hex + '$2')
$ldtkContent = Replace-All $ldtkContent '("__bgColor":\s*")#[0-9A-Fa-f]{6}(")' ('$1' + (Get-PaletteEntry "fog").Hex + '$2')
$ldtkContent = Replace-All $ldtkContent '(?s)("bgPivotY":\s*0\.5,.*?"__smartColor":\s*")#[0-9A-Fa-f]{6}(")' ('$1' + (Get-PaletteEntry "silver").Hex + '$2')

foreach ($identifier in $intGridPaletteMap.Keys) {
  $paletteName = $intGridPaletteMap[$identifier]
  $pattern = '(?s)("identifier":\s*"' + [regex]::Escape($identifier) + '".*?"color":\s*")#[0-9A-Fa-f]{6}(")'
  $ldtkContent = Replace-All $ldtkContent $pattern ('$1' + (Get-PaletteEntry $paletteName).Hex + '$2')
}

foreach ($identifier in $entityDefPaletteMap.Keys) {
  $paletteName = $entityDefPaletteMap[$identifier]
  $pattern = '(?s)("identifier":\s*"' + [regex]::Escape($identifier) + '".*?"color":\s*")#[0-9A-Fa-f]{6}(")'
  $ldtkContent = Replace-All $ldtkContent $pattern ('$1' + (Get-PaletteEntry $paletteName).Hex + '$2')

  $instancePattern = '(?s)("__identifier":\s*"' + [regex]::Escape($identifier) + '".*?"__smartColor":\s*")#[0-9A-Fa-f]{6}(")'
  $ldtkContent = Replace-All $ldtkContent $instancePattern ('$1' + (Get-PaletteEntry $paletteName).Hex + '$2')
}

Write-Utf8NoBom -Path $ldtkPath -Value $ldtkContent

foreach ($entityName in $entityScenePaths.Keys) {
  $path = $entityScenePaths[$entityName]
  $scene = Get-Content -Raw -LiteralPath $path
  $colorLiteral = Get-RgbFloatLiteral (Get-PaletteEntry $entityPaletteMap[$entityName]).Hex
  $scene = [regex]::Replace($scene, 'color = Color\([^\r\n]+\)', "color = $colorLiteral", 1)
  Write-Utf8NoBom -Path $path -Value $scene
}

Write-Host "Palette synchronized from $palettePath"
