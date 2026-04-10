-- Magicrate player generator for Aseprite.
-- Usage:
--   aseprite --batch --script godot/pipeline/aseprite/scripts/player.lua
-- Optional params:
--   --script-param output=C:\path\to\player.aseprite
--   --script-param save=false
--   --script-param close=true

local CANVAS_SIZE <const> = 8
local TRANSPARENT <const> = 0

local P <const> = {
  transparent = 0,
  outline = 1,
  shadow = 2,
  body = 3,
  highlight = 4,
  blush = 5,
  sparkle = 6,
  foot = 7,
  eye = 1,
  eye_shine = 4,
}

local PALETTE_COLORS <const> = {
  [0] = Color { r = 0, g = 0, b = 0, a = 0 },
  [1] = Color { r = 29, g = 43, b = 83, a = 255 },
  [2] = Color { r = 95, g = 87, b = 79, a = 255 },
  [3] = Color { r = 41, g = 173, b = 255, a = 255 },
  [4] = Color { r = 255, g = 241, b = 232, a = 255 },
  [5] = Color { r = 255, g = 119, b = 168, a = 255 },
  [6] = Color { r = 255, g = 236, b = 39, a = 255 },
  [7] = Color { r = 255, g = 204, b = 170, a = 255 },
  [8] = Color { r = 0, g = 228, b = 54, a = 255 },
  [9] = Color { r = 255, g = 0, b = 77, a = 255 },
  [10] = Color { r = 255, g = 163, b = 0, a = 255 },
  [11] = Color { r = 64, g = 70, b = 91, a = 255 },
  [12] = Color { r = 105, g = 106, b = 121, a = 255 },
  [13] = Color { r = 131, g = 118, b = 156, a = 255 },
  [14] = Color { r = 173, g = 173, b = 181, a = 255 },
  [15] = Color { r = 0, g = 0, b = 0, a = 255 },
}

local TAG_COLORS <const> = {
  wake = Color { r = 255, g = 236, b = 39, a = 255 },
  idle = Color { r = 41, g = 173, b = 255, a = 255 },
  move = Color { r = 0, g = 228, b = 54, a = 255 },
  jump = Color { r = 255, g = 119, b = 168, a = 255 },
}

local BODY_CHAR_MAP <const> = {
  o = P.outline,
  b = P.body,
  h = P.highlight,
  e = P.eye,
  i = P.eye_shine,
  c = P.blush,
  f = P.foot,
}

local SHADOW_CHAR_MAP <const> = {
  s = P.shadow,
}

local FX_CHAR_MAP <const> = {
  k = P.sparkle,
}

local BODY_PATTERNS <const> = {
  dead = {
    "........",
    "..oooo..",
    ".ohhhho.",
    ".obbbbo.",
    ".obbbbo.",
    ".obbbbo.",
    "..oooo..",
    "........",
  },
  stir = {
    "........",
    "..oooo..",
    ".ohhhho.",
    ".obeebo.",
    ".obbbbo.",
    ".obbbbo.",
    "..f..f..",
    "........",
  },
  open = {
    "........",
    "..oooo..",
    ".ohihio.",
    ".obebeo.",
    ".obebeo.",
    ".oc..co.",
    "..f..f..",
    "........",
  },
  open_up = {
    "..oooo..",
    ".ohihio.",
    ".obebeo.",
    ".obebeo.",
    ".obbbbo.",
    ".oc..co.",
    "..f..f..",
    "........",
  },
  blink = {
    "........",
    "..oooo..",
    ".ohhhho.",
    ".oee.ee.",
    ".obbbbo.",
    ".oc..co.",
    "..f..f..",
    "........",
  },
  step_left = {
    "........",
    ".oooo...",
    "ohihio..",
    "obebeo..",
    "obbbbo..",
    "oc..co..",
    "ff..f...",
    "........",
  },
  step_right = {
    "........",
    "...oooo.",
    "..ohihio",
    "..obebeo",
    "..obbbbo",
    "..oc..co",
    "...f..ff",
    "........",
  },
  jump_squash = {
    "........",
    "........",
    ".oooooo.",
    "ohihihho",
    "obebebbo",
    ".oc..co.",
    ".ff..ff.",
    "........",
  },
  jump_peak = {
    "..oooo..",
    ".ohhhho.",
    ".obibio.",
    ".obebeo.",
    ".obebeo.",
    ".oc..co.",
    "..f..f..",
    "........",
  },
}

local SHADOW_PATTERNS <const> = {
  none = {
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
  },
  base = {
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    "..ssss..",
    "...ss...",
  },
  wide = {
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    ".ssssss.",
    "..ssss..",
  },
  small = {
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    "...ss...",
    "........",
  },
  left = {
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    ".ssss...",
    "..ss....",
  },
  right = {
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    "...ssss.",
    "....ss..",
  },
}

local FX_PATTERNS <const> = {
  none = {
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
    "........",
  },
  sparkle_right = {
    "......k.",
    ".....kk.",
    "......k.",
    "........",
    "........",
    "........",
    "........",
    "........",
  },
  sparkle_left = {
    ".k......",
    ".kk.....",
    ".k......",
    "........",
    "........",
    "........",
    "........",
    "........",
  },
}

local FRAME_SPECS <const> = {
  { tag = "wake", duration = 0.08, body = "dead", shadow = "wide", fx = "none" },
  { tag = "wake", duration = 0.08, body = "stir", shadow = "wide", fx = "none" },
  { tag = "wake", duration = 0.10, body = "open", shadow = "base", fx = "sparkle_right" },
  { tag = "wake", duration = 0.12, body = "open_up", shadow = "small", fx = "sparkle_right" },
  { tag = "idle", duration = 0.16, body = "open", shadow = "base", fx = "none" },
  { tag = "idle", duration = 0.12, body = "open_up", shadow = "small", fx = "none" },
  { tag = "idle", duration = 0.10, body = "blink", shadow = "base", fx = "none" },
  { tag = "idle", duration = 0.12, body = "open", shadow = "base", fx = "sparkle_left" },
  { tag = "move", duration = 0.10, body = "step_left", shadow = "left", fx = "none" },
  { tag = "move", duration = 0.08, body = "open_up", shadow = "small", fx = "none" },
  { tag = "move", duration = 0.10, body = "step_right", shadow = "right", fx = "none" },
  { tag = "move", duration = 0.08, body = "open_up", shadow = "small", fx = "none" },
  { tag = "move", duration = 0.10, body = "step_left", shadow = "left", fx = "none" },
  { tag = "move", duration = 0.08, body = "open_up", shadow = "small", fx = "none" },
  { tag = "jump", duration = 0.10, body = "jump_squash", shadow = "wide", fx = "none" },
  { tag = "jump", duration = 0.14, body = "jump_peak", shadow = "small", fx = "sparkle_right" },
}

local function bool_param(name, default_value)
  local raw = app.params[name]
  if raw == nil then
    return default_value
  end

  raw = string.lower(tostring(raw))
  return raw == "1" or raw == "true" or raw == "yes" or raw == "on"
end

local function get_script_directory()
  local source = debug.getinfo(1, "S").source
  if string.sub(source, 1, 1) == "@" then
    source = string.sub(source, 2)
  end
  return app.fs.filePath(source)
end

local function default_output_path()
  return app.fs.normalizePath(
    app.fs.joinPath(get_script_directory(), "..", "src", "player.aseprite")
  )
end

local function make_palette()
  local palette = Palette(16)
  for index, color in pairs(PALETTE_COLORS) do
    palette:setColor(index, color)
  end
  return palette
end

local function make_spec()
  return ImageSpec {
    width = CANVAS_SIZE,
    height = CANVAS_SIZE,
    colorMode = ColorMode.INDEXED,
    transparentColor = TRANSPARENT,
  }
end

local function draw_pattern(image, pattern, char_map)
  for y = 1, #pattern do
    local row = pattern[y]
    for x = 1, #row do
      local glyph = string.sub(row, x, x)
      local color = char_map[glyph]
      if color ~= nil then
        image:drawPixel(x - 1, y - 1, color)
      end
    end
  end
end

local function add_tag(sprite, name, first_frame, last_frame)
  local tag = sprite:newTag(first_frame, last_frame)
  tag.name = name
  tag.color = TAG_COLORS[name]

  if name == "idle" or name == "jump" then
    tag.aniDir = AniDir.PING_PONG
  else
    tag.aniDir = AniDir.FORWARD
  end

  return tag
end

local function build_sprite()
  local spec = make_spec()
  local sprite = Sprite(spec)
  sprite.transparentColor = TRANSPARENT
  sprite:setPalette(make_palette())
  sprite.filename = ""
  sprite.data = "Generated by godot/pipeline/aseprite/scripts/player.lua (native 8x8 PICO-8 style)"

  for frame_number = 2, #FRAME_SPECS do
    sprite:newEmptyFrame(frame_number)
  end

  local default_layer = sprite.layers[1]

  local shadow_layer = sprite:newLayer()
  shadow_layer.name = "Shadow"
  shadow_layer.opacity = 160

  local body_layer = sprite:newLayer()
  body_layer.name = "Body"
  body_layer.opacity = 255

  local fx_layer = sprite:newLayer()
  fx_layer.name = "FX"
  fx_layer.opacity = 255

  sprite:deleteLayer(default_layer)

  shadow_layer.stackIndex = 1
  body_layer.stackIndex = 2
  fx_layer.stackIndex = 3

  local tag_ranges = {}

  for frame_number, frame_spec in ipairs(FRAME_SPECS) do
    local shadow_image = Image(spec)
    local body_image = Image(spec)
    local fx_image = Image(spec)

    shadow_image:clear()
    body_image:clear()
    fx_image:clear()

    draw_pattern(shadow_image, SHADOW_PATTERNS[frame_spec.shadow], SHADOW_CHAR_MAP)
    draw_pattern(body_image, BODY_PATTERNS[frame_spec.body], BODY_CHAR_MAP)
    draw_pattern(fx_image, FX_PATTERNS[frame_spec.fx], FX_CHAR_MAP)

    sprite:newCel(shadow_layer, frame_number, shadow_image)
    sprite:newCel(body_layer, frame_number, body_image)
    sprite:newCel(fx_layer, frame_number, fx_image)

    sprite.frames[frame_number].duration = frame_spec.duration

    local range = tag_ranges[frame_spec.tag]
    if range == nil then
      tag_ranges[frame_spec.tag] = { first = frame_number, last = frame_number }
    else
      range.last = frame_number
    end
  end

  add_tag(sprite, "wake", tag_ranges.wake.first, tag_ranges.wake.last)
  add_tag(sprite, "idle", tag_ranges.idle.first, tag_ranges.idle.last)
  add_tag(sprite, "move", tag_ranges.move.first, tag_ranges.move.last)
  add_tag(sprite, "jump", tag_ranges.jump.first, tag_ranges.jump.last)

  return sprite
end

local save_output = bool_param("save", true)
local close_after_save = bool_param("close", false)
local output_path = app.params.output or default_output_path()

local ok, err = pcall(function()
  local sprite = build_sprite()

  if save_output then
    local output_dir = app.fs.filePath(output_path)
    if output_dir ~= "" then
      app.fs.makeAllDirectories(output_dir)
    end
    sprite:saveAs(output_path)
    print("Generated player sprite at " .. output_path)
  else
    print("Generated player sprite in memory")
  end

  if close_after_save then
    sprite:close()
  end
end)

if not ok then
  print("player.lua error: " .. tostring(err))
  error(err)
end
