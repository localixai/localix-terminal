//! Tests of `file_icons`: a file's picture is told by its name, and every picture named is one
//! the app holds.

use super::*;

#[test]
fn a_file_is_told_by_its_whole_name_then_how_it_begins_then_its_extension() {
    for (name, icon) in [
        ("main.rs", "rust"),
        ("App.TSX", "tsx"),
        ("Dockerfile", "docker"),
        ("Dockerfile.dev", "docker"),
        ("package.json", "npm"),
        ("data.json", "json"),
        ("tsconfig.build.json", "tsconfig"),
        (".env.local", "dotenv"),
        (".gitignore", "git"),
        ("archive.tar.gz", "zip"),
        ("README.md", "markdown"),
        ("Cargo.lock", "config"),
        ("notes", "file"),
        (".zshrc", "file"),
        ("weird.xyz", "file"),
    ] {
        assert_eq!(of(name), icon, "{name}");
    }
}

#[test]
fn every_picture_named_is_held_and_every_one_held_is_a_drawing_with_nothing_to_run() {
    let held = |icon: &str| ICONS.iter().any(|(name, _)| *name == icon);
    let named = (NAMES.iter().map(|(_, icon)| *icon))
        .chain(STARTS.iter().map(|(_, icon)| *icon))
        .chain(KINDS.iter().map(|(_, icon)| *icon))
        .chain([PLAIN, "folder", "folder-open"]);
    for icon in named {
        assert!(held(icon), "no picture called {icon}");
    }
    for (name, svg) in ICONS {
        assert!(svg.trim_start().starts_with("<svg"), "{name} is no drawing");
        for run in ["<script", "javascript:", "http://", "https://", " on"] {
            let found = svg.replace("http://www.w3.org/", "").contains(run);
            // ` on` is only an event handler before a `=`.
            let handler = run == " on"
                && !svg.split(" on").skip(1).any(|rest| {
                    rest.split_once('=')
                        .is_some_and(|(word, _)| word.chars().all(char::is_alphabetic))
                });
            assert!(!found || handler, "{name} holds {run}");
        }
    }
}

#[test]
fn every_drawing_in_the_folder_is_in_the_table() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/browser/file_icons");
    let mut on_disk: Vec<String> = (std::fs::read_dir(folder).unwrap())
        .filter_map(|entry| {
            entry
                .ok()?
                .file_name()
                .to_str()?
                .strip_suffix(".svg")
                .map(str::to_string)
        })
        .collect();
    on_disk.sort();
    let table: Vec<&str> = ICONS.iter().map(|(name, _)| *name).collect();
    assert_eq!(on_disk, table);
}

#[test]
fn the_page_is_given_every_picture_whole_as_data() {
    let script = script();
    let given = script
        .strip_prefix("FI(")
        .and_then(|rest| rest.strip_suffix(')'));
    let given: serde_json::Value = serde_json::from_str(given.unwrap()).unwrap();
    assert_eq!(given.as_object().unwrap().len(), ICONS.len());
    for (name, _) in ICONS {
        let drawing = given[name].as_str().unwrap();
        assert!(
            drawing.starts_with("<svg") && drawing.ends_with("</svg>"),
            "{name}"
        );
    }
}
