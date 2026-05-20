//! Expand CLI `--wars` arguments (directories, globs, single files) into concrete CSV paths.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSetBuilder};

/// Accept any mix of:
///   - direct paths to CSV files
///   - paths to directories (every `.csv` directly inside is picked up, sub-directories ignored)
///   - glob patterns (e.g. `wars/*.csv` or `**/*.csv`)
pub fn expand_war_paths(inputs: &[String]) -> anyhow::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for raw in inputs {
        let p = Path::new(raw);
        if is_glob(raw) {
            // Use globset against the working directory, walking the deepest non-glob prefix.
            let (root, _) = split_glob_root(raw);
            let mut builder = GlobSetBuilder::new();
            builder.add(Glob::new(raw).map_err(|e| anyhow::anyhow!("bad glob `{raw}`: {e}"))?);
            let set = builder.build()?;
            walk_dir(&root, &mut |entry| {
                if set.is_match(entry) && is_csv(entry) {
                    let canon = entry.to_path_buf();
                    if seen.insert(canon.clone()) {
                        out.push(canon);
                    }
                }
            })?;
        } else if p.is_dir() {
            for entry in std::fs::read_dir(p)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && is_csv(&path) && seen.insert(path.clone()) {
                    out.push(path);
                }
            }
        } else if p.is_file() {
            if seen.insert(p.to_path_buf()) {
                out.push(p.to_path_buf());
            }
        } else {
            return Err(anyhow::anyhow!(
                "war input `{raw}` is neither a file, directory, nor matching glob"
            ));
        }
    }
    out.sort();
    Ok(out)
}

fn is_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

fn is_csv(p: &Path) -> bool {
    p.extension()
        .map(|e| e.eq_ignore_ascii_case("csv"))
        .unwrap_or(false)
}

fn split_glob_root(pattern: &str) -> (PathBuf, String) {
    let mut root = PathBuf::new();
    let parts: Vec<&str> = pattern.split(['/', '\\']).collect();
    let mut i = 0;
    while i < parts.len() && !is_glob(parts[i]) {
        root.push(parts[i]);
        i += 1;
    }
    if root.as_os_str().is_empty() {
        root.push(".");
    }
    let rest = parts[i..].join("/");
    (root, rest)
}

fn walk_dir(root: &Path, f: &mut impl FnMut(&Path)) -> std::io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    if root.is_file() {
        f(root);
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, f)?;
        } else {
            f(&path);
        }
    }
    Ok(())
}
