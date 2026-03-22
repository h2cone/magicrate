use gameplay_core::stage_paths;
use godot::{
    classes::{DirAccess, ResourceLoader, file_access::ModeFlags},
    obj::Singleton,
    prelude::*,
    tools::GFile,
};

use crate::config::SceneContract;

pub fn discover_stage_paths(contract: &SceneContract) -> Vec<String> {
    if let Some(stage_paths) = load_stage_manifest(contract.stage_manifest, contract.stage_dir) {
        return stage_paths;
    }

    let mut resource_loader = ResourceLoader::singleton();
    let mut file_names: Vec<String> = resource_loader
        .list_directory(contract.stage_dir)
        .to_vec()
        .into_iter()
        .map(|entry: GString| entry.to_string())
        .collect();

    if file_names.is_empty() {
        file_names = DirAccess::get_files_at(contract.stage_dir)
            .to_vec()
            .into_iter()
            .map(|entry: GString| entry.to_string())
            .collect();
    }

    let room_files = stage_paths::collect_sorted_room_files(file_names);

    room_files
        .into_iter()
        .map(|file_name| format!("{}/{}", contract.stage_dir, file_name))
        .collect()
}

fn load_stage_manifest(manifest_path: &str, stage_dir: &str) -> Option<Vec<String>> {
    let mut manifest = GFile::open(manifest_path, ModeFlags::READ).ok()?;
    let raw = manifest.read_as_gstring_entire().ok()?.to_string();

    let stage_paths: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            if line.starts_with("res://") {
                line.to_string()
            } else {
                format!("{}/{}", stage_dir, line)
            }
        })
        .collect();

    if stage_paths.is_empty() {
        None
    } else {
        Some(stage_paths)
    }
}
