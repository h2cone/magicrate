@tool

# Convert IG_Rules IntGrid cells into collision + logic markers.
#
# Use this script in importer option:
#   level_post_import = res://pipeline/ldtk/level_post_import.gd
#
# IG_Rules values:
#   1 = solid (player + crate)
#   2 = box-only solid (crate only)
#   3 = bridge reserve marker
#   4 = hazard marker
#   5 = fall-block marker

const IG_RULES_LAYER_NAME := "ig_rules-values"
const GENERATED_NODE_NAME := "__IGRulesGenerated"
const COLLISION_ALL_NODE := "CollisionAll"
const COLLISION_BOX_ONLY_NODE := "CollisionBoxOnly"
const MARKERS_NODE := "Markers"

const RULE_SOLID_ALL := 1
const RULE_SOLID_BOX_ONLY := 2
const RULE_BRIDGE_RESERVE := 3
const RULE_HAZARD := 4
const RULE_FALL_BLOCK := 5

const CELL_SIZE_DEFAULT := Vector2(8, 8)


func post_import(level: LDTKLevel) -> LDTKLevel:
	var rules_tilemap := _find_ig_rules_layer(level)
	if rules_tilemap == null:
		return level

	_rebuild_generated_nodes(level, rules_tilemap)

	return level


func _find_ig_rules_layer(level: LDTKLevel) -> TileMapLayer:
	for child in level.get_children():
		if child is not TileMapLayer:
			continue
		var tilemap: TileMapLayer = child
		if String(tilemap.name).to_lower() == IG_RULES_LAYER_NAME:
			return tilemap
	return null


func _rebuild_generated_nodes(level: LDTKLevel, tilemap: TileMapLayer) -> void:
	var old := level.get_node_or_null(GENERATED_NODE_NAME)
	if old:
		old.free()

	var scene_owner := _resolve_owner(level)

	var root := Node2D.new()
	root.name = GENERATED_NODE_NAME
	level.add_child(root)
	_set_owner_if_valid(root, scene_owner)

	var collision_all := StaticBody2D.new()
	collision_all.name = COLLISION_ALL_NODE
	collision_all.collision_layer = 1
	collision_all.collision_mask = 0
	root.add_child(collision_all)
	_set_owner_if_valid(collision_all, scene_owner)

	var collision_box_only := StaticBody2D.new()
	collision_box_only.name = COLLISION_BOX_ONLY_NODE
	collision_box_only.collision_layer = 4
	collision_box_only.collision_mask = 0
	root.add_child(collision_box_only)
	_set_owner_if_valid(collision_box_only, scene_owner)

	var markers := Node2D.new()
	markers.name = MARKERS_NODE
	root.add_child(markers)
	_set_owner_if_valid(markers, scene_owner)

	var cells := tilemap.get_used_cells()
	if cells.is_empty():
		return

	var tile_size := CELL_SIZE_DEFAULT
	if tilemap.tile_set:
		var ts := tilemap.tile_set.tile_size
		if ts.x > 0 and ts.y > 0:
			tile_size = Vector2(ts.x, ts.y)

	for cell in cells:
		var cell_value := _cell_value_from_intgrid_tile(tilemap, cell)
		var local_pos := tilemap.position + tilemap.map_to_local(cell)

		match cell_value:
			RULE_SOLID_ALL:
				_add_collision_shape(collision_all, local_pos, tile_size, scene_owner)
			RULE_SOLID_BOX_ONLY:
				_add_collision_shape(collision_box_only, local_pos, tile_size, scene_owner)
			RULE_BRIDGE_RESERVE:
				_add_marker(markers, "ig_bridge_reserve", local_pos, tile_size, scene_owner)
			RULE_HAZARD:
				_add_marker(markers, "ig_hazard_marker", local_pos, tile_size, scene_owner)
			RULE_FALL_BLOCK:
				_add_marker(markers, "ig_fall_block_marker", local_pos, tile_size, scene_owner)
			_:
				pass


func _cell_value_from_intgrid_tile(tilemap: TileMapLayer, cell: Vector2i) -> int:
	var atlas_coords := tilemap.get_cell_atlas_coords(cell)
	if atlas_coords.x < 0:
		return 0

	# IntGrid values are mapped to atlas x-index in import order.
	# Enforce IG_Rules values as contiguous 1..N in LDtk.
	return atlas_coords.x + 1


func _add_collision_shape(body: StaticBody2D, local_pos: Vector2, cell_size: Vector2, scene_owner: Node) -> void:
	var shape := RectangleShape2D.new()
	shape.size = cell_size

	var collider := CollisionShape2D.new()
	collider.shape = shape
	collider.position = local_pos
	body.add_child(collider)
	_set_owner_if_valid(collider, scene_owner)


func _add_marker(parent: Node2D, group_name: String, local_pos: Vector2, cell_size: Vector2, scene_owner: Node) -> void:
	var marker := Marker2D.new()
	marker.position = local_pos
	marker.add_to_group(group_name)
	marker.set_meta("cell_size", cell_size)
	parent.add_child(marker)
	_set_owner_if_valid(marker, scene_owner)


func _resolve_owner(level: Node) -> Node:
	if level.owner:
		return level.owner
	return level


func _set_owner_if_valid(node: Node, scene_owner: Node) -> void:
	if scene_owner and node != scene_owner:
		node.owner = scene_owner
