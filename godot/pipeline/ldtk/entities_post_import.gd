@tool

# LDtk entity post-import hook.
# Assign this script in the LDtk importer "Entities post-import" field.

const ENTITY_SCENES := {
	"PushableCrate": "res://entity/pushable_crate.tscn",
	"GoalPetal": "res://entity/goal_petal.tscn",
	"BridgeSwitch": "res://entity/bridge_switch.tscn",
	"BridgeTile": "res://entity/bridge_tile.tscn",
}

const ENTITY_ANCHORS := {
	"PlayerSpawn": Vector2.ZERO,
	"PushableCrate": Vector2.ZERO,
}


func post_import(entity_layer: LDTKEntityLayer) -> LDTKEntityLayer:
	var owner := _resolve_owner(entity_layer)

	for entity in entity_layer.entities:
		var identifier := _get_entity_identifier(entity)
		var iid := _get_entity_iid(entity)
		var placeholder := _find_placeholder(entity_layer, identifier, iid)

		if identifier == "PlayerSpawn":
			_ensure_player_spawn_node(entity_layer, entity, placeholder, owner)
			continue

		var scene_path: String = ENTITY_SCENES.get(identifier, "")
		if scene_path.is_empty():
			continue

		var packed: PackedScene = load(scene_path)
		if packed == null:
			push_error("Missing entity scene: %s" % scene_path)
			continue

		var instance := packed.instantiate()
		instance.name = _build_entity_name(identifier, entity)
		var anchor: Vector2 = ENTITY_ANCHORS.get(identifier, Vector2(0.5, 0.5))
		instance.position = _get_entity_anchor_position(entity, anchor)

		instance.set_meta("ldtk_identifier", identifier)
		instance.set_meta("ldtk_iid", iid)

		entity_layer.add_child(instance)
		if owner and instance != owner:
			instance.owner = owner

		if placeholder:
			placeholder.free()

	return entity_layer


func _get_entity_identifier(entity_data: Variant) -> String:
	if entity_data is Dictionary:
		if entity_data.has("identifier"):
			return str(entity_data["identifier"])
		if entity_data.has("definition") and entity_data["definition"] is Dictionary:
			return str(entity_data["definition"].get("identifier", ""))
	if entity_data is LDTKEntity:
		return entity_data.identifier
	return ""


func _get_entity_iid(entity_data: Variant) -> String:
	if entity_data is Dictionary and entity_data.has("iid"):
		return str(entity_data["iid"])
	if entity_data is LDTKEntity:
		return entity_data.iid
	return ""


func _get_entity_position(entity_data: Variant) -> Vector2:
	if entity_data is Dictionary and entity_data.has("position"):
		var pos = entity_data["position"]
		if pos is Vector2 or pos is Vector2i:
			return Vector2(pos.x, pos.y)
	if entity_data is LDTKEntity:
		return Vector2(entity_data.position)
	return Vector2.ZERO


func _get_entity_size(entity_data: Variant) -> Vector2:
	if entity_data is Dictionary and entity_data.has("size"):
		var size = entity_data["size"]
		if size is Vector2 or size is Vector2i:
			return Vector2(size.x, size.y)
	if entity_data is LDTKEntity:
		return Vector2(entity_data.size)
	return Vector2.ZERO


func _get_entity_anchor_position(entity_data: Variant, anchor: Vector2) -> Vector2:
	var pos := _get_entity_position(entity_data)
	var size := _get_entity_size(entity_data)
	if size == Vector2.ZERO:
		return pos
	return pos + (size * anchor)


func _build_entity_name(identifier: String, entity_data: Variant) -> String:
	var iid := _get_entity_iid(entity_data)
	if iid.is_empty():
		return identifier
	return "%s_%s" % [identifier, iid.left(8)]


func _resolve_owner(node: Node) -> Node:
	if node.owner:
		return node.owner

	var current: Node = node
	while current:
		if current is LDTKLevel:
			return current
		current = current.get_parent()

	return node


func _find_placeholder(entity_layer: LDTKEntityLayer, identifier: String, iid: String) -> Node2D:
	for child in entity_layer.get_children():
		if child is not Node2D:
			continue

		var node: Node2D = child
		var node_iid := str(node.get("iid"))
		if not iid.is_empty() and node_iid == iid:
			return node

		if iid.is_empty() and String(node.name) == identifier:
			return node

	return null


func _ensure_player_spawn_node(
		entity_layer: LDTKEntityLayer,
		entity_data: Variant,
		placeholder: Node2D,
		owner: Node
) -> void:
	if placeholder:
		placeholder.visible = false
		return

	var marker := Marker2D.new()
	marker.name = "PlayerSpawn"
	marker.position = _get_entity_anchor_position(entity_data, ENTITY_ANCHORS.get("PlayerSpawn", Vector2.ZERO))
	marker.set_meta("ldtk_identifier", "PlayerSpawn")
	marker.set_meta("ldtk_iid", _get_entity_iid(entity_data))
	entity_layer.add_child(marker)
	if owner and marker != owner:
		marker.owner = owner
