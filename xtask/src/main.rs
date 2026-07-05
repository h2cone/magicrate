use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use gdxtask::{
    addons, cli, godot, paths,
    process::{self, Program},
    run, update,
};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

const DEFAULT_USER_AGENT: &str = "magicrate-xtask";
const STAGE_MANIFEST_FILTER: &str = "pipeline/ldtk/stage_manifest.txt";

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Cross-platform project workflow tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run(run::RunArgs),
    Export(ExportArgs),
    UpdateGdext(update::gdext::UpdateGdextArgs),
    UpdateGodotAddons(UpdateGodotAddonsCommand),
}

#[derive(Debug, Args)]
struct ExportArgs {
    #[arg(long, value_enum, default_value_t = cli::ExportTarget::Windows)]
    target: cli::ExportTarget,
    #[arg(long, default_value = "godot")]
    godot_exe: String,
    #[arg(long, default_value = "Windows Desktop")]
    preset_name: String,
    #[arg(long, default_value = "export")]
    out_dir: PathBuf,
    #[arg(long, default_value = "game")]
    product_name: String,
    #[arg(long)]
    exe_name: Option<String>,
    #[arg(long, value_enum, default_value_t = ExportBuild::Release)]
    build: ExportBuild,
    #[arg(long)]
    force_create_export_preset: bool,
    #[arg(long)]
    include_pdb: bool,
    #[arg(long)]
    no_recovery_mode: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ExportBuild {
    Release,
    Both,
}

#[derive(Debug, Args)]
struct UpdateGodotAddonsCommand {
    #[arg(long, value_enum, default_value_t = GodotAddonSelection::All)]
    addon: GodotAddonSelection,
    #[command(flatten)]
    args: addons::UpdateGodotAddonsArgs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GodotAddonSelection {
    All,
    Ldtk,
    Aseprite,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = paths::ProjectPaths::discover(&paths::Layout::default())?;

    match cli.command {
        Command::Run(args) => run::execute(&paths, args),
        Command::Export(args) => export(&paths, args),
        Command::UpdateGdext(mut args) => {
            use_default_user_agent(&mut args.user_agent);
            update::gdext::execute(&paths, args)
        }
        Command::UpdateGodotAddons(mut command) => {
            use_default_user_agent(&mut command.args.user_agent);
            addons::execute(&paths, ADDONS, command.addon.into(), command.args)
        }
    }
}

fn export(paths: &paths::ProjectPaths, args: ExportArgs) -> Result<()> {
    for cargo_args in export_build_args(args.build) {
        process::run("cargo", &paths.rust_dir, &cargo_args)?;
    }

    let out_dir = resolve_out_dir(&paths.root, &args.out_dir);
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;

    ensure_export_preset(
        &paths.godot_dir,
        &args.preset_name,
        args.target,
        args.force_create_export_preset,
    )?;
    godot::normalize_extension_list(&paths.godot_dir)?;
    write_stage_manifest(&paths.godot_dir)?;

    let godot_exe = godot::resolve_godot_executable(&args.godot_exe)?;
    let output = export_output_path(&out_dir, args.target, &args.product_name, args.exe_name);
    let mut godot_args = Vec::new();
    godot_args.push("--headless".to_owned());
    if !args.no_recovery_mode {
        godot_args.push("--recovery-mode".to_owned());
    }
    godot_args.push("--path".to_owned());
    godot_args.push(paths.godot_dir.to_string_lossy().into_owned());
    godot_args.push("--export-release".to_owned());
    godot_args.push(args.preset_name);
    godot_args.push(output.to_string_lossy().into_owned());

    process::run(Program::new(godot_exe), &paths.root, &godot_args)?;
    wait_for_path(&output, Duration::from_secs(30))
        .with_context(|| format!("export output not found at {}", output.display()))?;

    if args.target == cli::ExportTarget::Windows {
        copy_release_dll(paths, &out_dir, args.include_pdb)?;
    }

    println!("Output: {}", output.display());
    println!("Distribute the folder: {}", out_dir.display());
    Ok(())
}

fn export_build_args(build: ExportBuild) -> Vec<Vec<String>> {
    let mut args = vec![vec![
        "build".to_owned(),
        "--release".to_owned(),
        "--locked".to_owned(),
    ]];
    if build == ExportBuild::Both {
        args.push(vec!["build".to_owned(), "--locked".to_owned()]);
    }
    args
}

fn resolve_out_dir(root: &Path, out_dir: &Path) -> PathBuf {
    if out_dir.is_absolute() {
        out_dir.to_path_buf()
    } else {
        root.join(out_dir)
    }
}

fn export_output_path(
    out_dir: &Path,
    target: cli::ExportTarget,
    product_name: &str,
    exe_name: Option<String>,
) -> PathBuf {
    match target {
        cli::ExportTarget::Windows => out_dir.join(exe_name.unwrap_or_else(|| {
            if product_name.ends_with(".exe") {
                product_name.to_owned()
            } else {
                format!("{product_name}.exe")
            }
        })),
        cli::ExportTarget::Macos => out_dir.join(format!("{product_name}.zip")),
    }
}

fn ensure_export_preset(
    godot_dir: &Path,
    preset_name: &str,
    target: cli::ExportTarget,
    force: bool,
) -> Result<()> {
    let path = godot_dir.join("export_presets.cfg");
    if !path.is_file() {
        let content = default_export_preset(0, preset_name, target);
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
        return Ok(());
    }

    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut lines = existing.lines().map(str::to_owned).collect::<Vec<_>>();
    let (found_index, max_index) = find_preset_index(&lines, preset_name);

    let preset_index = if let Some(index) = found_index {
        index
    } else {
        if !force {
            anyhow::bail!(
                "export preset '{preset_name}' not found in {}; pass --force-create-export-preset",
                path.display()
            );
        }
        let next = max_index.map_or(0, |index| index + 1);
        if lines.last().is_some_and(|line| !line.is_empty()) {
            lines.push(String::new());
        }
        lines.extend(
            default_export_preset(next, preset_name, target)
                .lines()
                .map(str::to_owned),
        );
        next
    };

    ensure_preset_key(
        &mut lines,
        preset_index,
        "include_filter",
        STAGE_MANIFEST_FILTER,
    );
    ensure_options(&mut lines, preset_index, target);

    fs::write(&path, lines.join("\r\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn default_export_preset(index: usize, preset_name: &str, target: cli::ExportTarget) -> String {
    let mut lines = vec![
        format!("[preset.{index}]"),
        format!("name=\"{preset_name}\""),
        format!("platform=\"{}\"", export_platform(target)),
        "runnable=true".to_owned(),
        "dedicated_server=false".to_owned(),
        "custom_features=\"\"".to_owned(),
        "export_filter=\"all_resources\"".to_owned(),
        format!("include_filter=\"{STAGE_MANIFEST_FILTER}\""),
        "exclude_filter=\"\"".to_owned(),
        "export_path=\"\"".to_owned(),
        "encryption_include_filters=\"\"".to_owned(),
        "encryption_exclude_filters=\"\"".to_owned(),
        "encrypt_pck=false".to_owned(),
        "encrypt_directory=false".to_owned(),
        String::new(),
        format!("[preset.{index}.options]"),
    ];
    lines.extend(
        default_export_options(target)
            .iter()
            .map(|line| (*line).to_owned()),
    );
    lines.join("\r\n")
}

fn export_platform(target: cli::ExportTarget) -> &'static str {
    match target {
        cli::ExportTarget::Windows => "Windows Desktop",
        cli::ExportTarget::Macos => "macOS",
    }
}

fn default_export_options(target: cli::ExportTarget) -> &'static [&'static str] {
    match target {
        cli::ExportTarget::Windows => &[
            "binary_format/architecture=\"x86_64\"",
            "binary_format/embed_pck=false",
        ],
        cli::ExportTarget::Macos => &[],
    }
}

fn find_preset_index(lines: &[String], preset_name: &str) -> (Option<usize>, Option<usize>) {
    let mut current = None;
    let mut found = None;
    let mut max_index = None;

    for line in lines {
        if let Some(index) = parse_preset_header(line, "preset.") {
            current = Some(index);
            max_index = Some(max_index.map_or(index, |max: usize| max.max(index)));
            continue;
        }
        if current.is_some() && line == &format!("name=\"{preset_name}\"") {
            found = current;
        }
    }

    (found, max_index)
}

fn parse_preset_header(line: &str, prefix: &str) -> Option<usize> {
    let body = line.strip_prefix('[')?.strip_suffix(']')?;
    let rest = body.strip_prefix(prefix)?;
    if rest.contains('.') {
        return None;
    }
    rest.parse().ok()
}

fn ensure_preset_key(lines: &mut Vec<String>, index: usize, key: &str, value: &str) {
    let Some((start, end)) = section_bounds(lines, &format!("[preset.{index}]")) else {
        return;
    };
    let replacement = format!("{key}=\"{value}\"");

    for line in &mut lines[start + 1..end] {
        if line.starts_with(&format!("{key}=")) {
            *line = replacement;
            return;
        }
    }

    lines.insert(end, replacement);
}

fn ensure_options(lines: &mut Vec<String>, index: usize, target: cli::ExportTarget) {
    let defaults = default_export_options(target);
    if defaults.is_empty() {
        return;
    }

    let header = format!("[preset.{index}.options]");
    let (start, mut end) = match section_bounds(lines, &header) {
        Some(bounds) => bounds,
        None => {
            if lines.last().is_some_and(|line| !line.is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            let start = lines.len() - 1;
            (start, lines.len())
        }
    };

    for default in defaults {
        let key = default.split_once('=').map_or(*default, |(key, _)| key);
        if lines[start + 1..end]
            .iter()
            .any(|line| line.starts_with(&format!("{key}=")))
        {
            continue;
        }
        lines.insert(end, (*default).to_owned());
        end += 1;
    }
}

fn section_bounds(lines: &[String], header: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| line == header)?;
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with('['))
        .map_or(lines.len(), |offset| start + 1 + offset);
    Some((start, end))
}

fn write_stage_manifest(godot_dir: &Path) -> Result<()> {
    let stage_dir = godot_dir.join("pipeline/ldtk/levels");
    let manifest_path = godot_dir.join(STAGE_MANIFEST_FILTER);
    let mut entries = Vec::new();

    if stage_dir.is_dir() {
        for entry in fs::read_dir(&stage_dir)
            .with_context(|| format!("failed to read {}", stage_dir.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some((x, y)) = room_scene_key(&name) {
                entries.push((y, x, name));
            }
        }
    }

    entries.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
    let lines = entries
        .into_iter()
        .map(|(_, _, name)| format!("res://pipeline/ldtk/levels/{name}"))
        .collect::<Vec<_>>();

    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&manifest_path, lines.join("\n"))
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;
    Ok(())
}

fn room_scene_key(name: &str) -> Option<(i32, i32)> {
    let stem = name
        .strip_suffix(".scn")
        .or_else(|| name.strip_suffix(".tscn"))?;
    let rest = stem.strip_prefix("Room_")?;
    let (x, y) = rest.split_once('_')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

fn wait_for_path(path: &Path, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    if path.exists() {
        Ok(())
    } else {
        anyhow::bail!("timed out")
    }
}

fn copy_release_dll(paths: &paths::ProjectPaths, out_dir: &Path, include_pdb: bool) -> Result<()> {
    let dll_path = out_dir.join("rust.dll");
    if !dll_path.exists() {
        let source = paths.rust_dir.join("target/release/rust.dll");
        fs::copy(&source, &dll_path).with_context(|| {
            format!(
                "failed to copy release GDExtension DLL from {} to {}",
                source.display(),
                dll_path.display()
            )
        })?;
    }

    if include_pdb {
        let source = paths.rust_dir.join("target/release/rust.pdb");
        if source.exists() {
            fs::copy(&source, out_dir.join("rust.pdb"))
                .with_context(|| format!("failed to copy {}", source.display()))?;
        }
    }

    println!("GDExtension DLL (release): {}", dll_path.display());
    Ok(())
}

impl From<GodotAddonSelection> for addons::AddonSelection<'static> {
    fn from(selection: GodotAddonSelection) -> Self {
        match selection {
            GodotAddonSelection::All => Self::All,
            GodotAddonSelection::Ldtk => Self::One("ldtk"),
            GodotAddonSelection::Aseprite => Self::One("aseprite"),
        }
    }
}

fn use_default_user_agent(user_agent: &mut String) {
    if user_agent == "gdxtask" {
        *user_agent = DEFAULT_USER_AGENT.to_owned();
    }
}

const ADDONS: &[addons::AddonSpec] = &[
    addons::AddonSpec {
        label: "ldtk",
        repo: "heygleeson/godot-ldtk-importer",
        package_dir: "ldtk-importer",
        target_dir: "addons/ldtk-importer",
        download: addons::DownloadKind::ReleaseAssetZip,
    },
    addons::AddonSpec {
        label: "aseprite",
        repo: "viniciusgerevini/godot-aseprite-wizard",
        package_dir: "AsepriteWizard",
        target_dir: "addons/AsepriteWizard",
        download: addons::DownloadKind::Zipball,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_run_release_editor() {
        let cli = Cli::try_parse_from(["xtask", "run", "--build", "release", "--editor"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Run(run::RunArgs {
                build: cli::BuildMode::Release,
                editor: true,
                headless: false,
                ..
            })
        ));
    }

    #[test]
    fn parses_export_legacy_defaults() {
        let cli = Cli::try_parse_from(["xtask", "export"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Export(ExportArgs {
                target: cli::ExportTarget::Windows,
                build: ExportBuild::Release,
                product_name,
                ..
            }) if product_name == "game"
        ));
    }

    #[test]
    fn export_output_path_keeps_explicit_exe_name() {
        assert_eq!(
            export_output_path(
                Path::new("export"),
                cli::ExportTarget::Windows,
                "magicrate",
                Some("game.exe".to_owned())
            ),
            Path::new("export").join("game.exe")
        );
    }

    #[test]
    fn parses_update_godot_addons_single_ref() {
        let cli = Cli::try_parse_from([
            "xtask",
            "update-godot-addons",
            "--addon",
            "aseprite",
            "--ref",
            "v9.8.0",
            "--dry-run",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Command::UpdateGodotAddons(UpdateGodotAddonsCommand {
                addon: GodotAddonSelection::Aseprite,
                args: addons::UpdateGodotAddonsArgs {
                    ref_name: Some(_),
                    dry_run: true,
                    ..
                },
            })
        ));
    }

    #[test]
    fn room_scene_key_accepts_scn_and_tscn() {
        assert_eq!(room_scene_key("Room_2_-1.scn"), Some((2, -1)));
        assert_eq!(room_scene_key("Room_-3_4.tscn"), Some((-3, 4)));
        assert_eq!(room_scene_key("Room_x_4.scn"), None);
        assert_eq!(room_scene_key("Level_0_0.scn"), None);
    }

    #[test]
    fn export_build_release_skips_debug_by_default() {
        assert_eq!(
            export_build_args(ExportBuild::Release),
            vec![vec!["build", "--release", "--locked"]]
        );
        assert_eq!(
            export_build_args(ExportBuild::Both),
            vec![
                vec!["build", "--release", "--locked"],
                vec!["build", "--locked"]
            ]
        );
    }
}
