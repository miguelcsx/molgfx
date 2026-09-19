//! Guards the Python surface against silently losing touch with the facade.
//!
//! `mypy.stubtest` proves a stub matches the compiled module, but it says
//! nothing about the names the facade publishes and the binding never carried.
//! Those are the ones that disappear without a sound: a Rust caller keeps the
//! name, a Python caller never learns it existed, and nothing fails.
//!
//! So this reads the public surface of each crate the facade re-exports — the
//! `pub use` lists in the crate roots, which is the same list the facade globs —
//! and requires every name to be either carried by a stub or written down in
//! `facade-coverage-allowlist.txt` with the reason it is not.
//!
//! The allowlist is checked for staleness in both directions: an entry naming a
//! name that no longer exists, or one that is now carried by a stub, fails. The
//! list can only shrink, and only on purpose.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Where the reason for each deliberately uncarried name is written down.
const ALLOWLIST: &str = "facade-coverage-allowlist.txt";

/// The crates the facade re-exports, each with the stub module that carries it.
///
/// A crate whose stub module is absent is one the binding deliberately omits
/// wholesale; every one of its names has to be allowlisted.
const CRATES: &[&str] = &[
    "molgfx-core",
    "molgfx-math",
    "molgfx-gpu",
    "molgfx-render",
    "molgfx-semantic",
];

/// Facade-root names the curated Python root deliberately does not carry.
///
/// The binding keeps the difference composition unbound today, so the name
/// stays reachable in Rust only; the Python surface can add it, never silently
/// drop what it does carry.
const PY_ROOT_ABSENT: &[&str] = &["DifferenceScene"];

#[test]
fn every_facade_name_is_carried_or_explained() {
    let repository = repository_root();
    let stub_dir = python_package_root().join("_engine");
    let allowlist = read_allowlist(&repository);

    let mut carried: BTreeSet<String> = BTreeSet::new();
    for stub in stub_modules(&stub_dir) {
        carried.extend(stub_exports(&stub));
    }

    let mut unexplained = Vec::new();
    let mut seen = BTreeSet::new();
    for crate_name in CRATES {
        for (module, name) in public_names(&repository, crate_name) {
            let key = format!("{crate_name}::{module}::{name}");
            seen.insert(key.clone());
            if carried.contains(&name) || allowlist.covers(crate_name, &module, &name) {
                continue;
            }
            unexplained.push(key);
        }
    }

    let stale: Vec<&String> = allowlist
        .entries
        .iter()
        .filter(|entry| !seen.contains(*entry) || is_carried(entry, &carried))
        .collect();

    assert!(
        stale.is_empty(),
        "the allowlist names {} the facade no longer publishes unbound:\n  {}",
        stale.len(),
        stale
            .iter()
            .map(|entry| entry.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    assert!(
        unexplained.is_empty(),
        "{} names are neither carried by a stub nor have a written reason:\n  {}",
        unexplained.len(),
        unexplained.join("\n  ")
    );
}

/// The curated facade root is the Python package root.
///
/// Both doors carry the same set: a Rust program writes `molgfx::Scene`, a
/// Python program writes `molgfx.Scene`. Every name the facade root curates
/// has to be re-exported by `python/molgfx/__init__.py` or excused in
/// [`PY_ROOT_ABSENT`] with the reason written down, so the two doors cannot
/// drift apart quietly.
#[test]
fn the_python_root_carries_the_curated_facade_root() {
    let repository = repository_root();
    let root = std::fs::read_to_string(repository.join("crates/molgfx/src/lib.rs"))
        .expect("the facade root must exist");
    let python = std::fs::read_to_string(repository.join("python/molgfx/__init__.py"))
        .expect("the package root must exist");

    // The gate is about names, not features: a cfg-gated subsystem the binding
    // carries wholesale still counts.
    let visible: String = root
        .lines()
        .filter(|line| !line.trim_start().starts_with("#["))
        .collect::<Vec<_>>()
        .join("\n");
    let curated = statements(&visible)
        .iter()
        .filter(|statement| statement.starts_with("pub use crate::"))
        .flat_map(|statement| {
            statement
                .trim_start_matches("pub use")
                .trim_end_matches(';')
                .split_once('{')
                .map_or(Vec::new(), |(_, names)| {
                    let names = names.split_once('}').map_or(names, |(inside, _)| inside);
                    names.split(',').filter_map(rust_name).collect()
                })
        })
        .collect::<BTreeSet<_>>();

    assert!(
        !curated.is_empty(),
        "the facade root must curate at least one name"
    );
    let missing: Vec<String> = curated
        .iter()
        .filter(|name| !PY_ROOT_ABSENT.contains(&name.as_str()))
        .filter(|name| !python_contains(&python, name))
        .cloned()
        .collect();
    assert!(
        missing.is_empty(),
        "{} curated facade-root names are absent from the Python root:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

/// Whether the package root's `__all__` names a curated name.
fn python_contains(source: &str, name: &str) -> bool {
    source
        .lines()
        .any(|line| line.trim().starts_with(&format!("\"{name}\",")))
}

/// The names a crate publishes from its root, paired with the module they come
/// from, so a whole module can be excused in one line.
///
/// The parse is deliberately literal: `pub use <module>::{A, B as C};` and
/// `pub use <module>::A;` are the only two shapes a crate root uses, and
/// `pub(crate) use` is not a `pub use`, so an internal re-export cannot be
/// mistaken for a published one.
fn public_names(repository: &Path, crate_name: &str) -> Vec<(String, String)> {
    let root = repository
        .join("crates")
        .join(crate_name)
        .join("src")
        .join("lib.rs");
    let Ok(source) = std::fs::read_to_string(&root) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for statement in statements(&source) {
        let body = statement
            .trim_start_matches("pub use")
            .trim_end_matches(';');
        let head = head_of(body);
        let module = head_segment(head).to_owned();
        match body.split_once('{') {
            Some((_, tail)) => {
                let inner = match tail.split_once('}') {
                    Some((inner, _)) => inner,
                    None => tail,
                };
                for item in inner.split(',') {
                    if let Some(name) = rust_name(item) {
                        names.push((module.clone(), name));
                    }
                }
            }
            None => {
                if let Some(name) = rust_name(tail_segment(body)) {
                    names.push((module.clone(), name));
                }
            }
        }
    }
    names
}

/// The part of a `pub use` body before the brace list, if there is one.
fn head_of(body: &str) -> &str {
    match body.split_once('{') {
        Some((head, _)) => head,
        None => body,
    }
}

/// The first `::`-separated segment, which for a crate root is the module.
fn head_segment(path: &str) -> &str {
    match path.split_once("::") {
        Some((segment, _)) => segment.trim(),
        None => path.trim(),
    }
}

/// The last `::`-separated segment.
fn tail_segment(path: &str) -> &str {
    match path.rsplit_once("::") {
        Some((_, segment)) => segment.trim(),
        None => path.trim(),
    }
}

/// Every `pub use` statement in a source file, comments removed.
fn statements(source: &str) -> Vec<String> {
    let uncommented: String = source
        .lines()
        .map(|line| match line.split_once("//") {
            Some((code, _)) => code,
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");
    uncommented
        .split(';')
        .map(str::trim)
        .filter(|statement| statement.starts_with("pub use"))
        .map(|statement| format!("{statement};"))
        .collect()
}

/// The name a `use` item binds: the alias when there is one, the path's tail
/// otherwise. `None` for an empty item.
fn rust_name(item: &str) -> Option<String> {
    let item = item.trim();
    if item.is_empty() {
        return None;
    }
    let tail = match item.split_once(" as ") {
        Some((_, alias)) => alias.trim(),
        None => tail_segment(item),
    };
    (!tail.is_empty()).then(|| tail.to_owned())
}

/// The `__all__` literal of a stub, read as text rather than imported.
fn stub_exports(stub: &Path) -> BTreeSet<String> {
    let Ok(source) = std::fs::read_to_string(stub) else {
        return BTreeSet::new();
    };
    let Some(start) = source.find("__all__") else {
        return BTreeSet::new();
    };
    let Some(open) = source[start..].find('[') else {
        return BTreeSet::new();
    };
    let rest = &source[start + open + 1..];
    let Some(close) = rest.find(']') else {
        return BTreeSet::new();
    };
    rest[..close]
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim().trim_matches('"');
            (!entry.is_empty()).then(|| entry.to_owned())
        })
        .collect()
}

fn stub_modules(stub_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = stub_dir.read_dir() else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "pyi"))
        .collect()
}

/// The written reasons, split into whole-module and per-name entries.
struct Allowlist {
    modules: Vec<(String, String)>,
    entries: BTreeSet<String>,
}

impl Allowlist {
    fn covers(&self, crate_name: &str, module: &str, name: &str) -> bool {
        let key = format!("{crate_name}::{module}::{name}");
        if self.entries.contains(&key) {
            return true;
        }
        // A whole-module excuse matches on segment boundaries, so excusing
        // `residency` cannot silently excuse `residency_machine`.
        let path = format!("{crate_name}::{module}");
        self.modules
            .iter()
            .any(|(prefix, _)| path == prefix.as_str() || path.starts_with(&format!("{prefix}::")))
    }
}

/// Whether a per-name entry now names something a stub carries, which makes the
/// excuse obsolete and the list longer than it needs to be.
fn is_carried(entry: &str, carried: &BTreeSet<String>) -> bool {
    match entry.rsplit_once("::") {
        Some((_, name)) => carried.contains(name),
        None => false,
    }
}

/// Reads `reason<TAB>crate::module::name`, where the name may be `*` to excuse
/// the module whole. Blank lines and `#` comments are skipped.
fn read_allowlist(repository: &Path) -> Allowlist {
    let path = repository.join("crates").join("molgfx-py").join(ALLOWLIST);
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => panic!("{} must list every unbound name: {error}", path.display()),
    };
    let mut modules = Vec::new();
    let mut entries = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((reason, key)) = line.split_once(char::is_whitespace) else {
            panic!("{ALLOWLIST}:{} has no reason: {line}", index + 1);
        };
        let key = key.trim();
        assert!(
            !reason.trim().is_empty() && !key.is_empty(),
            "{ALLOWLIST}:{} malformed: {line}",
            index + 1
        );
        match key.strip_suffix("::*") {
            Some(prefix) => modules.push((prefix.to_owned(), reason.trim().to_owned())),
            None => {
                entries.insert(key.to_owned());
            }
        }
    }
    Allowlist { modules, entries }
}

/// The repository root, derived from this crate's manifest.
fn repository_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    match manifest.parent().and_then(Path::parent) {
        Some(repository) => repository.to_path_buf(),
        None => manifest.to_path_buf(),
    }
}

/// The Python package, beside the crates rather than inside this one.
fn python_package_root() -> PathBuf {
    repository_root().join("python").join("molgfx")
}
