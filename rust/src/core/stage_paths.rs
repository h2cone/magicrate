pub fn is_room_scene_file(file_name: &str) -> bool {
    (file_name.starts_with("Room_") && file_name.ends_with(".scn"))
        || (file_name.starts_with("Room_") && file_name.ends_with(".tscn"))
}

pub fn room_coords_from_file_name(file_name: &str) -> Option<(i32, i32)> {
    let trimmed = file_name.strip_prefix("Room_")?;
    let trimmed = trimmed
        .strip_suffix(".scn")
        .or_else(|| trimmed.strip_suffix(".tscn"))?;

    let mut parts = trimmed.split('_');
    let x = parts.next()?.parse::<i32>().ok()?;
    let y = parts.next()?.parse::<i32>().ok()?;

    if parts.next().is_some() {
        return None;
    }

    Some((x, y))
}

pub fn room_sort_key(file_name: &str) -> (i32, i32) {
    room_coords_from_file_name(file_name)
        .map(|(x, y)| (y, x))
        .unwrap_or((0, 0))
}

pub fn collect_sorted_room_files(entries: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut room_files: Vec<String> = entries
        .into_iter()
        .filter(|file_name| is_room_scene_file(file_name))
        .collect();

    room_files.sort_by(|a, b| {
        let a_key = room_sort_key(a);
        let b_key = room_sort_key(b);
        a_key.cmp(&b_key).then_with(|| a.cmp(b))
    });

    room_files
}

#[cfg(test)]
mod tests {
    use super::{
        collect_sorted_room_files, is_room_scene_file, room_coords_from_file_name, room_sort_key,
    };

    #[test]
    fn detects_supported_room_extensions() {
        assert!(is_room_scene_file("Room_0_0.scn"));
        assert!(is_room_scene_file("Room_0_0.tscn"));
        assert!(!is_room_scene_file("Room_0_0.png"));
        assert!(!is_room_scene_file("something_else.tscn"));
    }

    #[test]
    fn parses_room_coordinates() {
        assert_eq!(room_coords_from_file_name("Room_1_2.scn"), Some((1, 2)));
        assert_eq!(room_coords_from_file_name("Room_-3_4.tscn"), Some((-3, 4)));
    }

    #[test]
    fn rejects_invalid_room_file_names() {
        assert_eq!(room_coords_from_file_name("Room_1.scn"), None);
        assert_eq!(room_coords_from_file_name("Room_1_2_3.scn"), None);
        assert_eq!(room_coords_from_file_name("Room_a_2.scn"), None);
        assert_eq!(room_coords_from_file_name("1_2.scn"), None);
    }

    #[test]
    fn room_sort_key_orders_by_y_then_x() {
        assert_eq!(room_sort_key("Room_2_7.scn"), (7, 2));
    }

    #[test]
    fn collect_sorted_room_files_filters_and_sorts() {
        let files = vec![
            "Room_2_0.scn".to_string(),
            "ignore.txt".to_string(),
            "Room_0_1.tscn".to_string(),
            "Room_1_0.scn".to_string(),
        ];

        let sorted = collect_sorted_room_files(files);

        assert_eq!(
            sorted,
            vec![
                "Room_1_0.scn".to_string(),
                "Room_2_0.scn".to_string(),
                "Room_0_1.tscn".to_string(),
            ]
        );
    }
}
