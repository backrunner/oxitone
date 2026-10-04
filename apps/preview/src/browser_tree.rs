//! Lazy filesystem reads run on GPUI's background executor, never in render/audio.
use crate::ui::Preview;
use gpui::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub directory: bool,
}
#[derive(Default)]
pub struct BrowserTree {
    pub focused: bool,
    pub row_bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<PathBuf, Bounds<Pixels>>>>,
    pub resource_bounds: std::rc::Rc<std::cell::RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    pub roots: Vec<PathBuf>,
    pub children: BTreeMap<PathBuf, Result<Vec<Entry>, String>>,
    pub expanded: BTreeSet<PathBuf>,
    pub loading: BTreeSet<PathBuf>,
    pub selected: Option<PathBuf>,
    pub drag: Option<crate::sample_drop::SampleDrag>,
    pub scroll: ScrollHandle,
}
pub fn read_directory(path: &Path) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for (count, entry) in std::fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        if count >= 8192 {
            return Err("Folder exceeds 8192 entries; choose a smaller folder".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || matches!(name.as_ref(), "node_modules" | "target" | "dist") {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        let audio = path.extension().is_some_and(|e| {
            matches!(
                e.to_string_lossy().to_lowercase().as_str(),
                "wav" | "wave" | "aif" | "aiff" | "flac" | "mp3" | "mp4" | "m4a"
            )
        });
        if kind.is_dir() || (kind.is_file() && audio) {
            entries.push(Entry {
                path,
                directory: kind.is_dir(),
            });
        }
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.path.file_name().cmp(&b.path.file_name()))
    });
    Ok(entries)
}
impl Preview {
    pub fn expand_sample_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let tree = &mut self.document.browser;
        if tree.expanded.remove(&path) {
            return;
        }
        tree.expanded.insert(path.clone());
        if tree.children.len() >= 256 {
            tree.children.clear();
        }
        if tree.children.contains_key(&path) || !tree.loading.insert(path.clone()) {
            return;
        }
        let work = path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { read_directory(&work) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.document.browser.loading.remove(&path);
                this.document.browser.children.insert(path, result);
                cx.notify();
            });
        })
        .detach();
    }
    pub fn add_sample_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Add sample folder".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |this, cx| {
                    for path in paths {
                        if !this.document.browser.roots.contains(&path) {
                            this.document.browser.roots.push(path.clone());
                            this.expand_sample_folder(path, cx);
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
}
