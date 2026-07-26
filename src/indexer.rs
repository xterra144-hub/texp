use crate::art::ArtIndex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;

pub struct InMemoryIndex {
    art: Arc<Mutex<ArtIndex>>,
    building: Arc<AtomicBool>,
    built: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl InMemoryIndex {
    pub fn new() -> Self {
        Self {
            art: Arc::new(Mutex::new(ArtIndex::new())),
            building: Arc::new(AtomicBool::new(false)),
            built: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn search(&self, query: &str, current_dir: &Path, max: usize) -> Vec<PathBuf> {
        let art = self.art.lock().unwrap();
        art.search(query, current_dir, max)
    }

    pub fn find_completions(&self, prefix: &[u8], max: usize) -> Vec<String> {
        let art = self.art.lock().unwrap();
        art.find_completions(prefix, max)
    }
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let art = self.art.lock().unwrap();
        art.is_empty()
    }

    pub fn is_built(&self) -> bool {
        self.built.load(Ordering::Relaxed)
    }

    pub fn is_building(&self) -> bool {
        self.building.load(Ordering::Relaxed)
    }

    pub fn ensure_built(
        self: &Arc<Self>,
        root_path: PathBuf,
        skip_dirs: Vec<String>,
        status_tx: mpsc::Sender<String>,
    ) {
        if self.built.load(Ordering::Relaxed) || self.building.load(Ordering::Relaxed) {
            return;
        }
        self.building.store(true, Ordering::Relaxed);

        let art = self.art.clone();
        let built = self.built.clone();
        let building = self.building.clone();

        std::thread::spawn(move || {
            let walker = ignore::WalkBuilder::new(&root_path)
                .follow_links(false)
                .same_file_system(true)
                .hidden(false)
                .git_ignore(true)
                .build();

            let mut count = 0;
            for entry in walker.flatten() {
                let path = entry.path();
                let path_str = path.to_string_lossy();
                if skip_dirs.iter().any(|s| path_str.contains(s)) {
                    continue;
                }
                if path.is_file() || path.is_dir() {
                    let mut a = art.lock().unwrap();
                    a.insert(path);
                    count += 1;
                }
                if count > 0 && count % 500 == 0 {
                    let _ = status_tx.send(format!("Indexing... {} files", count));
                }
            }
            let _ = status_tx.send(format!("Indexing complete ({})", count));
            built.store(true, Ordering::Relaxed);
            building.store(false, Ordering::Relaxed);
        });
    }

    pub fn clear(&mut self) {
        let mut art = self.art.lock().unwrap();
        art.clear();
        self.built.store(false, Ordering::Relaxed);
        self.building.store(false, Ordering::Relaxed);
    }
}
