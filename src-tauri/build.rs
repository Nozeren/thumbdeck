use std::path::PathBuf;

fn main() {
    bundle_packs();
    tauri_build::build()
}

/// Compile the toolkit packs into the app: `toolkits/packs/*.toml` from the thumbdeck-toolkits
/// submodule, or from the checkout in $THUMBDECK_TOOLKITS (for trying out pack changes).
fn bundle_packs() {
    println!("cargo:rerun-if-env-changed=THUMBDECK_TOOLKITS");
    let repo = std::env::var_os("THUMBDECK_TOOLKITS").map(PathBuf::from).unwrap_or_else(|| "toolkits".into());
    let dir = std::fs::canonicalize(repo.join("packs")).unwrap_or_else(|_| {
        panic!("no toolkit packs in {}: run `git submodule update --init`", repo.display())
    });
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut packs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("can't read the packs folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    packs.sort();
    let mut code = String::from("pub const BUNDLED: &[(&str, &str)] = &[\n");
    for path in packs {
        println!("cargo:rerun-if-changed={}", path.display());
        let id = path.file_stem().unwrap().to_string_lossy();
        code += &format!("    ({id:?}, include_str!({:?})),\n", path.display().to_string());
    }
    code += "];\n";
    // When the bundled packs were committed: a clone of the repo older than that is ignored
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git").arg("-C").arg(&dir).args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let time = git(&["log", "-1", "--format=%ct"]).and_then(|t| t.parse::<u64>().ok()).unwrap_or(0);
    // Checking out another commit of the submodule changes its HEAD
    if let Some(git_dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
    }
    code += &format!("pub const BUNDLED_TIME: u64 = {time};\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bundled_packs.rs");
    std::fs::write(out, code).unwrap();
}
