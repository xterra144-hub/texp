use std::path::{Path, PathBuf};
use smallvec::SmallVec;

type Prefix = SmallVec<[u8; 8]>;

fn normalize_path(path: &Path) -> Vec<u8> {
    let s = path.to_string_lossy();
    let s = s.replace('\\', "/");
    let s = s.to_lowercase();
    let mut result = Vec::with_capacity(s.len());
    let mut prev_slash = false;
    for c in s.chars() {
        if c == '/' {
            if prev_slash { continue; }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        result.push(c as u8);
    }
    if result.len() > 1 && result.last() == Some(&b'/') {
        result.pop();
    }
    result
}

fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

struct Node {
    prefix: Prefix,
    terminal: bool,
    full_path: Option<String>,
    n_children: u16,
    inner: NodeInner,
}

enum NodeInner {
    N4 { keys: [u8; 4], children: [Option<Box<Node>>; 4] },
    N16 { keys: [u8; 16], children: [Option<Box<Node>>; 16] },
    N48 { child_index: Vec<u8>, children: Vec<Option<Box<Node>>> },
    N256 { children: Vec<Option<Box<Node>>> },
}

impl Node {
    fn new() -> Self {
        Self {
            prefix: Prefix::new(),
            terminal: false,
            full_path: None,
            n_children: 0,
            inner: NodeInner::N4 { keys: [0; 4], children: Default::default() },
        }
    }

    fn find_child(&self, byte: u8) -> Option<&Node> {
        match &self.inner {
            NodeInner::N4 { keys, children } => {
                let i = keys.iter().position(|&k| k == byte)?;
                children[i].as_ref().map(|c| c.as_ref())
            }
            NodeInner::N16 { keys, children } => {
                let i = keys.iter().position(|&k| k == byte)?;
                children[i].as_ref().map(|c| c.as_ref())
            }
            NodeInner::N48 { child_index, children } => {
                let idx = child_index[byte as usize];
                if idx == 0 { None } else { children[(idx - 1) as usize].as_ref().map(|c| c.as_ref()) }
            }
            NodeInner::N256 { children } => {
                children[byte as usize].as_ref().map(|c| c.as_ref())
            }
        }
    }

    fn find_child_mut(&mut self, byte: u8) -> Option<&mut Node> {
        match &mut self.inner {
            NodeInner::N4 { keys, children } => {
                let i = keys.iter().position(|&k| k == byte)?;
                children[i].as_mut().map(|c| c.as_mut())
            }
            NodeInner::N16 { keys, children } => {
                let i = keys.iter().position(|&k| k == byte)?;
                children[i].as_mut().map(|c| c.as_mut())
            }
            NodeInner::N48 { child_index, children } => {
                let idx = child_index[byte as usize];
                if idx == 0 { None } else { children[(idx - 1) as usize].as_mut().map(|c| c.as_mut()) }
            }
            NodeInner::N256 { children } => {
                children[byte as usize].as_mut().map(|c| c.as_mut())
            }
        }
    }

    fn add_child(&mut self, byte: u8, child: Node) {
        if self.n_children == self.max_children() {
            self.grow();
        }
        self.add_child_inner(byte, child);
    }

    fn max_children(&self) -> u16 {
        match &self.inner {
            NodeInner::N4 { .. } => 4,
            NodeInner::N16 { .. } => 16,
            NodeInner::N48 { .. } => 48,
            NodeInner::N256 { .. } => 256,
        }
    }

    fn add_child_inner(&mut self, byte: u8, child: Node) {
        let n = self.n_children as usize;
        match &mut self.inner {
            NodeInner::N4 { keys, children } => {
                keys[n] = byte;
                children[n] = Some(Box::new(child));
            }
            NodeInner::N16 { keys, children } => {
                keys[n] = byte;
                children[n] = Some(Box::new(child));
            }
            NodeInner::N48 { child_index, children } => {
                child_index[byte as usize] = (n + 1) as u8;
                children[n] = Some(Box::new(child));
            }
            NodeInner::N256 { children } => {
                children[byte as usize] = Some(Box::new(child));
            }
        }
        self.n_children += 1;
    }

    fn drain_children(&mut self) -> Vec<(u8, Box<Node>)> {
        let mut result = Vec::with_capacity(self.n_children as usize);
        match &mut self.inner {
            NodeInner::N4 { keys, children } => {
                for i in 0..self.n_children as usize {
                    if let Some(c) = children[i].take() {
                        result.push((keys[i], c));
                        keys[i] = 0;
                    }
                }
            }
            NodeInner::N16 { keys, children } => {
                for i in 0..self.n_children as usize {
                    if let Some(c) = children[i].take() {
                        result.push((keys[i], c));
                        keys[i] = 0;
                    }
                }
            }
            NodeInner::N48 { child_index, children } => {
                for byte in 0..=255u16 {
                    let b = byte as u8;
                    let idx = child_index[b as usize];
                    if idx > 0 {
                        if let Some(c) = children[(idx - 1) as usize].take() {
                            result.push((b, c));
                        }
                        child_index[b as usize] = 0;
                    }
                }
            }
            NodeInner::N256 { children } => {
                for byte in 0..=255u16 {
                    let b = byte as u8;
                    if let Some(c) = children[b as usize].take() {
                        result.push((b, c));
                    }
                }
            }
        }
        self.n_children = 0;
        result
    }

    fn grow(&mut self) {
        let children = self.drain_children();
        let n = children.len();
        let new_inner = match &self.inner {
            NodeInner::N4 { .. } => {
                let mut keys = [0u8; 16];
                let mut new_children: [Option<Box<Node>>; 16] = Default::default();
                for (i, (k, c)) in children.into_iter().enumerate() {
                    keys[i] = k;
                    new_children[i] = Some(c);
                }
                NodeInner::N16 { keys, children: new_children }
            }
            NodeInner::N16 { .. } => {
                let mut child_index = vec![0u8; 256];
                let mut new_children: Vec<Option<Box<Node>>> = (0..48).map(|_| None).collect();
                for (i, (k, c)) in children.into_iter().enumerate() {
                    child_index[k as usize] = (i + 1) as u8;
                    new_children[i] = Some(c);
                }
                NodeInner::N48 { child_index, children: new_children }
            }
            NodeInner::N48 { .. } => {
                let mut new_children: Vec<Option<Box<Node>>> = (0..256).map(|_| None).collect();
                for (k, c) in children {
                    new_children[k as usize] = Some(c);
                }
                NodeInner::N256 { children: new_children }
            }
            NodeInner::N256 { .. } => unreachable!(),
        };
        self.inner = new_inner;
        self.n_children = n as u16;
    }

    fn swap_state(&mut self) -> State {
        State {
            prefix: std::mem::take(&mut self.prefix),
            terminal: self.terminal,
            full_path: self.full_path.take(),
            n_children: self.n_children,
            inner: std::mem::replace(&mut self.inner,
                NodeInner::N4 { keys: [0; 4], children: Default::default() }),
        }
    }

    fn insert(&mut self, key: &[u8], full_path: String) {
        self.insert_recursive(key, 0, full_path);
    }

    fn insert_recursive(&mut self, key: &[u8], depth: usize, full_path: String) {
        let existing = &self.prefix[..];
        let remaining = &key[depth..];
        let split = common_prefix_len(existing, remaining);

        if split < existing.len() {
            let State { prefix: old_prefix, terminal: old_terminal, full_path: old_full_path, n_children: old_n, inner: old_inner } = self.swap_state();

            self.prefix = Prefix::from(&old_prefix[..split]);
            self.terminal = false;

            let existing_byte = old_prefix[split];
            let existing_child = Node {
                prefix: Prefix::from(&old_prefix[split + 1..]),
                terminal: old_terminal,
                full_path: old_full_path,
                n_children: old_n,
                inner: old_inner,
            };
            match &mut self.inner {
                NodeInner::N4 { keys, children } => {
                    keys[0] = existing_byte;
                    children[0] = Some(Box::new(existing_child));
                }
                _ => unreachable!(),
            }
            self.n_children = 1;

            if remaining.len() == split {
                self.terminal = true;
                self.full_path = Some(full_path);
            } else {
                let new_byte = remaining[split];
                let mut new_child = Node::new();
                new_child.prefix = Prefix::from(&remaining[split + 1..]);
                new_child.terminal = true;
                new_child.full_path = Some(full_path);
                self.add_child(new_byte, new_child);
            }
            return;
        }

        if depth + split == key.len() {
            self.terminal = true;
            self.full_path = Some(full_path);
            return;
        }

        let next_byte = key[depth + split];
        if let Some(child) = self.find_child_mut(next_byte) {
            child.insert_recursive(key, depth + split + 1, full_path);
        } else {
            let mut new_child = Node::new();
            new_child.prefix = Prefix::from(&remaining[split + 1..]);
            new_child.terminal = true;
            new_child.full_path = Some(full_path);
            self.add_child(next_byte, new_child);
        }
    }

    fn collect_children(&self) -> Vec<(u8, &Node)> {
        let mut children: Vec<(u8, &Node)> = Vec::new();
        match &self.inner {
            NodeInner::N4 { keys, children: ch } => {
                for i in 0..self.n_children as usize {
                    if let Some(c) = &ch[i] {
                        children.push((keys[i], c.as_ref()));
                    }
                }
            }
            NodeInner::N16 { keys, children: ch } => {
                for i in 0..self.n_children as usize {
                    if let Some(c) = &ch[i] {
                        children.push((keys[i], c.as_ref()));
                    }
                }
            }
            NodeInner::N48 { child_index, children: ch } => {
                for byte in 0..=255u16 {
                    let b = byte as u8;
                    let idx = child_index[b as usize];
                    if idx > 0 {
                        if let Some(c) = &ch[(idx - 1) as usize] {
                            children.push((b, c.as_ref()));
                        }
                    }
                }
            }
            NodeInner::N256 { children: ch } => {
                for byte in 0..=255u16 {
                    let b = byte as u8;
                    if let Some(c) = &ch[b as usize] {
                        children.push((b, c.as_ref()));
                    }
                }
            }
        }
        children.sort_by_key(|&(k, _)| k);
        children
    }

    fn collect_with_limit(&self, mut path_so_far: Vec<u8>, results: &mut Vec<String>, max: usize) {
        if results.len() >= max { return; }
        path_so_far.extend_from_slice(&self.prefix);
        if self.terminal {
            if let Some(fp) = &self.full_path {
                results.push(fp.clone());
                if results.len() >= max { return; }
            }
        }
        let children = self.collect_children();
        for (byte, child) in children {
            if results.len() >= max { return; }
            let mut child_path = path_so_far.clone();
            child_path.push(byte);
            child.collect_with_limit(child_path, results, max);
        }
    }

    fn find_completions_recursive(&self, prefix: &[u8], depth: usize, mut path_so_far: Vec<u8>, results: &mut Vec<String>, max: usize) {
        if results.len() >= max { return; }

        let ep = &self.prefix;
        let rp = &prefix[depth..];
        let matched = common_prefix_len(ep, rp);

        if matched < ep.len() {
            if matched == rp.len() && !rp.is_empty() {
                let mut full = path_so_far;
                full.extend_from_slice(&ep[..matched]);
                self.collect_with_limit(full, results, max);
            }
            return;
        }

        path_so_far.extend_from_slice(ep);

        if depth + ep.len() >= prefix.len() {
            self.collect_with_limit(path_so_far, results, max);
            return;
        }

        let next_byte = prefix[depth + ep.len()];
        if let Some(child) = self.find_child(next_byte) {
            let mut child_path = path_so_far;
            child_path.push(next_byte);
            child.find_completions_recursive(prefix, depth + ep.len() + 1, child_path, results, max);
        }
    }

    fn all_paths(&self, mut path_so_far: Vec<u8>, results: &mut Vec<String>, max: usize) {
        if results.len() >= max { return; }
        path_so_far.extend_from_slice(&self.prefix);
        if self.terminal {
            if let Some(fp) = &self.full_path {
                results.push(fp.clone());
                if results.len() >= max { return; }
            }
        }
        let children = self.collect_children();
        for (byte, child) in children {
            if results.len() >= max { return; }
            let mut child_path = path_so_far.clone();
            child_path.push(byte);
            child.all_paths(child_path, results, max);
        }
    }

    fn component_matches(path_bytes: &[u8], query: &str) -> bool {
        let s = String::from_utf8_lossy(path_bytes);
        let query_lower = query.to_lowercase();
        for comp in s.split('/') {
            let comp = comp.to_lowercase();
            if comp.starts_with(&query_lower) || comp.contains(&query_lower) {
                return true;
            }
        }
        false
    }
}

struct State {
    prefix: Prefix,
    terminal: bool,
    full_path: Option<String>,
    n_children: u16,
    inner: NodeInner,
}

#[allow(dead_code)]
pub struct ArtIndex {
    root: Option<Box<Node>>,
    pub size: usize,
}

#[allow(dead_code)]
impl ArtIndex {
    pub fn new() -> Self {
        Self { root: None, size: 0 }
    }

    pub fn insert(&mut self, path: &Path) {
        let key = normalize_path(path);
        if key.is_empty() { return; }
        let full_path = path.to_string_lossy().to_string();
        if let Some(root) = &mut self.root {
            root.insert(&key, full_path);
        } else {
            let mut root = Node::new();
            root.prefix = Prefix::from(key.as_slice());
            root.terminal = true;
            root.full_path = Some(full_path);
            self.root = Some(Box::new(root));
        }
        self.size += 1;
    }

    pub fn find_completions(&self, prefix: &[u8], max: usize) -> Vec<String> {
        let mut results = Vec::new();
        if prefix.is_empty() || max == 0 { return results; }
        if let Some(root) = &self.root {
            root.find_completions_recursive(prefix, 0, Vec::new(), &mut results, max);
        }
        results
    }

    pub fn search(&self, query: &str, _current_dir: &Path, max: usize) -> Vec<PathBuf> {
        let mut results: Vec<String> = Vec::new();
        if let Some(root) = &self.root {
            if !query.is_empty() {
                let query_lower = query.to_lowercase();

                {
                    let mut all = Vec::new();
                    root.all_paths(Vec::new(), &mut all, max * 100);
                    for path in all {
                        if results.len() >= max { break; }
                        if results.contains(&path) { continue; }
                        if Node::component_matches(path.as_bytes(), &query_lower) {
                            results.push(path);
                        }
                    }
                }

                results.sort_by(|a, b| {
                    let a_name = Path::new(a).file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                    let b_name = Path::new(b).file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                    let a_exact = a_name == query_lower;
                    let b_exact = b_name == query_lower;
                    let a_prefix = a_name.starts_with(&query_lower);
                    let b_prefix = b_name.starts_with(&query_lower);
                    match (a_exact, b_exact) {
                        (true, false) => return std::cmp::Ordering::Less,
                        (false, true) => return std::cmp::Ordering::Greater,
                        _ => {}
                    }
                    match (a_prefix, b_prefix) {
                        (true, false) => return std::cmp::Ordering::Less,
                        (false, true) => return std::cmp::Ordering::Greater,
                        _ => {}
                    }
                    let a_is_dir = Path::new(a).is_dir();
                    let b_is_dir = Path::new(b).is_dir();
                    match (a_is_dir, b_is_dir) {
                        (true, false) => return std::cmp::Ordering::Less,
                        (false, true) => return std::cmp::Ordering::Greater,
                        _ => {}
                    }
                    a.len().cmp(&b.len()).then(a.cmp(b))
                });
                results.truncate(max);
            }
        }
        results.into_iter().map(PathBuf::from).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none() || self.size == 0
    }

    pub fn clear(&mut self) {
        self.root = None;
        self.size = 0;
    }
}
