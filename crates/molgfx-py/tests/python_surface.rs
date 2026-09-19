//! Guards the Python package against becoming a second implementation.
//!
//! Everything the package exposes is computed in Rust; the Python side is a
//! re-export layer. This test reads `python/molgfx/**` and rejects any
//! statement that is not a docstring, an import, an `__all__` literal, or the
//! one `__getattr__` hook that explains a moved name, so a helper that starts
//! doing real work — or a module that starts importing `numpy` and computing
//! with it — fails here rather than shipping.
//!
//! One subpackage is deliberately shaped differently: `molgfx/viewer`, the
//! Jupyter widget, is *glue* rather than a re-export layer — it defines the
//! widget class and its forwarding methods. Glue is held to a stricter line
//! than the re-export surface, recursively through every definition: no loop
//! statements, no comprehensions, and no numpy anywhere, so event forwarding
//! and display are all it can do. Every measurable computation stays in
//! Rust.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The Python checker: parses each module and reports every statement
/// the surface rules reject. Kept beside the gate it enforces.
const CHECKER: &str = r#"import ast, pathlib, sys

# The only top-level statements the package may carry beyond imports: a
# docstring, an `__all__` literal, a bare `pass`, and the single hook that
# turns a name looked up in the wrong namespace into a moved-name report.
def is_docstring(node):
    return (
        isinstance(node, ast.Expr)
        and isinstance(node.value, ast.Constant)
        and isinstance(node.value.value, str)
    )


def is_dunder_all(node):
    return (
        isinstance(node, ast.Assign)
        and len(node.targets) == 1
        and isinstance(node.targets[0], ast.Name)
        and node.targets[0].id == "__all__"
        and isinstance(node.value, (ast.List, ast.Tuple))
    )


def is_moved_name_hook(node):
    return (
        isinstance(node, ast.FunctionDef)
        and node.name == "__getattr__"
        and not node.decorator_list
        and len(node.body) == 1
        and isinstance(node.body[0], ast.Raise)
    )


def imports_numpy(node):
    if isinstance(node, ast.Import):
        return any(alias.name.split(".")[0] == "numpy" for alias in node.names)
    if isinstance(node, ast.ImportFrom):
        return (node.module or "").split(".")[0] == "numpy"
    return False


# The viewer glue may forward and display, never compute. These constructs
# are how per-entity or per-pixel work would start in Python, so the gate
# rejects them anywhere in the subpackage, recursively. `Try` stays allowed:
# optional-dependency plumbing is glue; the constructs above are compute.
FORBIDDEN_GLUE_STATEMENTS = (
    ast.For,
    ast.AsyncFor,
    ast.While,
    ast.Match,
    ast.Delete,
    ast.Global,
    ast.Nonlocal,
)

FORBIDDEN_GLUE_EXPRESSIONS = (
    ast.ListComp,
    ast.SetComp,
    ast.DictComp,
    ast.GeneratorExp,
)


def check_glue(path, tree):
    offenders = []
    for node in ast.walk(tree):
        if imports_numpy(node):
            offenders.append(f"{path}:{node.lineno}: numpy is imported in Python")
        elif isinstance(node, FORBIDDEN_GLUE_STATEMENTS):
            offenders.append(
                f"{path}:{node.lineno}: {type(node).__name__} in glue (no loops in Python)"
            )
        elif isinstance(node, FORBIDDEN_GLUE_EXPRESSIONS):
            offenders.append(
                f"{path}:{node.lineno}: {type(node).__name__} in glue (no comprehensions in Python)"
            )
    return offenders


root = pathlib.Path(sys.argv[1])
offenders = []
for path in sorted(root.rglob("*.py")):
    tree = ast.parse(path.read_text(), filename=str(path))
    if "viewer" in path.relative_to(root).parts:
        offenders.extend(check_glue(path, tree))
        continue
    for node in tree.body:
        if imports_numpy(node):
            offenders.append(f"{path}:{node.lineno}: numpy is imported in Python")
            continue
        if isinstance(node, (ast.Import, ast.ImportFrom, ast.Module, ast.Pass)):
            continue
        if is_docstring(node) or is_dunder_all(node) or is_moved_name_hook(node):
            continue
        offenders.append(f"{path}:{getattr(node, 'lineno', 0)}: {type(node).__name__}")
for offender in offenders:
    print(offender)
sys.exit(1 if offenders else 0)
"#;

#[test]
fn the_python_package_only_re_exports() {
    let root = python_package_root();
    let modules = python_modules(&root);
    assert!(
        !modules.is_empty(),
        "{} must contain the Python package",
        root.display()
    );

    let output = Command::new("python3")
        .arg("-c")
        .arg(CHECKER)
        .arg(&root)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(error) => panic!("python3 must run to inspect the package: {error}"),
    };
    assert!(
        output.status.success(),
        "the Python package must only declare:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// The package's directory, one level below the repository root.
///
/// The package is built by `maturin`, which reads it from `python/` at the
/// repository root rather than from this crate, so the path is derived from the
/// crate's own manifest instead of repeated as a literal.
fn python_package_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = match manifest.parent().and_then(Path::parent) {
        Some(repository) => repository,
        None => manifest,
    };
    repository.join("python").join("molgfx")
}

fn python_modules(root: &Path) -> Vec<PathBuf> {
    let mut modules = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "py") {
                modules.push(path);
            }
        }
    }
    modules
}
