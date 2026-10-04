use crate::browser_tree::read_directory;

#[test]
fn sample_tree_filters_hidden_build_outputs_and_symlinks_and_sorts_folders_first() {
    let root = std::env::temp_dir().join(format!("oxitone-tree-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Kicks")).unwrap();
    for folder in [".hidden", "node_modules", "target", "dist"] {
        std::fs::create_dir(root.join(folder)).unwrap();
    }
    for file in [
        "a.WAV",
        "b.flac",
        "text.ts",
        ".private.wav",
        "unsupported.ogg",
    ] {
        std::fs::write(root.join(file), []).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("a.WAV"), root.join("link.wav")).unwrap();
    let entries = read_directory(&root).unwrap();
    let names: Vec<_> = entries
        .iter()
        .map(|e| e.path.file_name().unwrap().to_str().unwrap())
        .collect();
    assert_eq!(names, ["Kicks", "a.WAV", "b.flac"]);
    assert!(entries[0].directory && !entries[1].directory);
    assert!(read_directory(&root.join("missing")).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
