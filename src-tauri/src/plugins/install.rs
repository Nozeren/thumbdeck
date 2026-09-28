//! Getting plugins: cloned from a git URL at their latest release tag (a folder of a repository
//! with several plugins: `url#plugins/django`), or linked from a folder on disk.

use super::manifest::{self, version_key};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// An installed plugin, as saved in the settings
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Installed {
    pub id: String,
    /// Where it came from: a git URL (maybe with #subfolder) or, when linked, a folder
    pub source: String,
    /// The release tag it's at; None: the default branch (a repository without tags) or linked
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// A folder used where it is (for developing a plugin), not a clone
    #[serde(default)]
    pub linked: bool,
}

fn yes() -> bool {
    true
}

/// Where thumbdeck keeps its own data: $XDG_DATA_HOME/thumbdeck or ~/.local/share/thumbdeck
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| Some(crate::projects::dirs_home()?.join(".local/share")))?;
    Some(base.join("thumbdeck"))
}

/// Where installed plugins are cloned
pub fn plugins_dir() -> Option<PathBuf> {
    Some(data_dir()?.join("plugins"))
}

/// A source split into the repository and the plugin's folder in it
fn split(source: &str) -> (&str, Option<&str>) {
    match source.split_once('#') {
        Some((url, sub)) if !sub.trim_matches('/').is_empty() => (url, Some(sub.trim_matches('/'))),
        Some((url, _)) => (url, None),
        None => (source, None),
    }
}

impl Installed {
    /// The plugin's folder (with plugin.toml)
    pub fn folder(&self, plugins: &Path) -> PathBuf {
        if self.linked {
            return PathBuf::from(&self.source);
        }
        let clone = plugins.join(&self.id);
        match split(&self.source).1 {
            Some(sub) => clone.join(sub),
            None => clone,
        }
    }
}

fn git(dir: Option<&Path>, args: &[&str]) -> Result<Output, String> {
    let mut cmd = Command::new("git");
    if let Some(dir) = dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.args(args)
        .env_clear()
        .envs(crate::runner::shell_env().iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0") // fail instead of asking for a password
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes") // or for a passphrase
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("couldn't run git: {e}"))
}

/// Run git; its last line of stderr on failure
fn git_ok(dir: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let out = git(dir, args)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("git failed").trim().to_string())
    }
}

/// How a plugin's release tags start: `v` (`v1.2.0`), or `<id>-v` for a plugin in a folder of a
/// repository with several
fn tag_prefix(id: &str, in_folder: bool) -> String {
    if in_folder { format!("{id}-v") } else { "v".to_string() }
}

/// The newest release tag for a plugin, and its version
fn latest_tag(clone: &Path, prefix: &str) -> Result<Option<(String, String)>, String> {
    let tags = git_ok(Some(clone), &["tag", "--list"])?;
    Ok(tags
        .lines()
        .filter_map(|t| Some((t.to_string(), t.strip_prefix(prefix)?.to_string())))
        .filter(|(_, v)| v.split('-').next().is_some_and(|c| c.split('.').count() == 3 && c.split('.').all(|p| p.parse::<u64>().is_ok())))
        .max_by(|a, b| version_key(&a.1).cmp(&version_key(&b.1))))
}

/// Check out a tag (or stay on the branch) and make sure the manifest there is the plugin's
fn settle(clone: &Path, folder: &Path, id: &str, tag: Option<&str>, prefix: &str) -> Result<(), String> {
    if let Some(tag) = tag {
        git_ok(Some(clone), &["checkout", "-q", "--detach", tag]).map_err(|e| format!("couldn't check out {tag}: {e}"))?;
    }
    let m = manifest::read(folder).map_err(|p| p.join("; "))?;
    if m.id != id {
        return Err(format!("the plugin's id changed from {id} to {}", m.id));
    }
    if let Some(tag) = tag {
        if Some(m.version.as_str()) != tag.strip_prefix(prefix) {
            return Err(format!("the tag {tag} holds version {} of the plugin; the tag and the version must match", m.version));
        }
    }
    Ok(())
}

/// Clone a plugin and check out its latest release. `plugins` is where clones go.
pub fn install(source: &str, plugins: &Path) -> Result<Installed, String> {
    let source = source.trim();
    let (url, sub) = split(source);
    if url.is_empty() {
        return Err("give a git URL (or a path to a git repository)".into());
    }
    std::fs::create_dir_all(plugins).map_err(|e| e.to_string())?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = plugins.join(format!(".installing-{nanos:x}"));
    let result = (|| {
        git_ok(None, &["clone", "-q", url, &tmp.to_string_lossy()]).map_err(|e| format!("couldn't clone {url}: {e}"))?;
        let folder = match sub {
            Some(s) => tmp.join(s),
            None => tmp.clone(),
        };
        if !folder.join("plugin.toml").is_file() {
            return Err(match sub {
                Some(s) => format!("there's no plugin.toml in {s} of {url}"),
                None => format!("there's no plugin.toml at the top of {url} (for a plugin in a folder: {url}#folder)"),
            });
        }
        let id = manifest::read(&folder).map_err(|p| p.join("; "))?.id;
        if plugins.join(&id).exists() {
            return Err(format!("a plugin called {id} is installed already"));
        }
        let prefix = tag_prefix(&id, sub.is_some());
        let tag = latest_tag(&tmp, &prefix)?.map(|(t, _)| t);
        settle(&tmp, &folder, &id, tag.as_deref(), &prefix)?;
        std::fs::rename(&tmp, plugins.join(&id)).map_err(|e| e.to_string())?;
        Ok(Installed { id, source: source.to_string(), tag, enabled: true, linked: false })
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Use a plugin from a folder, where it is
pub fn link(folder: &Path) -> Result<Installed, String> {
    let folder = folder.canonicalize().map_err(|_| format!("there's no folder {}", folder.display()))?;
    let id = manifest::read(&folder).map_err(|p| p.join("; "))?.id;
    Ok(Installed { id, source: folder.to_string_lossy().to_string(), tag: None, enabled: true, linked: true })
}

/// A newer release, if there is one: its tag, or "the latest commit" for a plugin without tags
pub fn newer(p: &Installed, plugins: &Path) -> Result<Option<String>, String> {
    if p.linked {
        return Ok(None);
    }
    let clone = plugins.join(&p.id);
    git_ok(Some(&clone), &["fetch", "-q", "--tags", "--force", "origin"])?;
    match &p.tag {
        Some(current) => {
            let prefix = tag_prefix(&p.id, split(&p.source).1.is_some());
            let current = current.strip_prefix(&prefix).unwrap_or(current);
            let latest = latest_tag(&clone, &prefix)?;
            Ok(latest.filter(|(_, v)| version_key(v) > version_key(current)).map(|(t, _)| t))
        }
        None => {
            let head = git_ok(Some(&clone), &["rev-parse", "HEAD"])?;
            let upstream = git_ok(Some(&clone), &["rev-parse", "@{upstream}"])?;
            let behind = git_ok(Some(&clone), &["merge-base", "--is-ancestor", &head, &upstream]).is_ok();
            Ok((head != upstream && behind).then(|| "the latest commit".to_string()))
        }
    }
}

/// Move a plugin to its newest release; returns the new record
pub fn update(p: &Installed, plugins: &Path) -> Result<Installed, String> {
    let Some(next) = newer(p, plugins)? else { return Ok(p.clone()) };
    let clone = plugins.join(&p.id);
    let folder = p.folder(plugins);
    let prefix = tag_prefix(&p.id, split(&p.source).1.is_some());
    if p.tag.is_none() {
        git_ok(Some(&clone), &["merge", "-q", "--ff-only", "@{upstream}"])?;
        settle(&clone, &folder, &p.id, None, &prefix)?;
        return Ok(p.clone());
    }
    if let Err(e) = settle(&clone, &folder, &p.id, Some(&next), &prefix) {
        // Back to the release that worked
        if let Some(old) = &p.tag {
            let _ = git_ok(Some(&clone), &["checkout", "-q", "--detach", old]);
        }
        return Err(format!("couldn't update to {next}: {e}"));
    }
    Ok(Installed { tag: Some(next), ..p.clone() })
}

/// Delete an installed plugin's clone (a linked folder is left alone)
pub fn remove(p: &Installed, plugins: &Path) -> Result<(), String> {
    if p.linked {
        return Ok(());
    }
    let clone = plugins.join(&p.id);
    if clone.exists() {
        std::fs::remove_dir_all(&clone).map_err(|e| format!("couldn't delete {}: {e}", clone.display()))?;
    }
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// A git repository to install plugins from
    pub struct Upstream(pub PathBuf);

    impl Upstream {
        pub fn new(dir: PathBuf) -> Self {
            std::fs::create_dir_all(&dir).unwrap();
            git_ok(Some(&dir), &["init", "-q", "-b", "main"]).unwrap();
            Upstream(dir)
        }
        pub fn manifest(&self, folder: &str, id: &str, version: &str) -> &Self {
            let dir = self.0.join(folder);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("plugin.toml"), format!("id = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\napi = 1\n")).unwrap();
            self
        }
        pub fn commit(&self, msg: &str) -> &Self {
            git_ok(Some(&self.0), &["add", "-A"]).unwrap();
            git_ok(Some(&self.0), &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", msg]).unwrap();
            self
        }
        pub fn tag(&self, tag: &str) -> &Self {
            git_ok(Some(&self.0), &["tag", tag]).unwrap();
            self
        }
        pub fn url(&self) -> String {
            self.0.to_string_lossy().to_string()
        }
    }

    fn version(p: &Installed, plugins: &Path) -> String {
        manifest::read(&p.folder(plugins)).unwrap().version
    }

    #[test]
    fn install_takes_the_latest_release_then_updates() {
        let d = TempDir::new("install-tags");
        let plugins = d.0.join("plugins");
        let up = Upstream::new(d.0.join("pomodoro"));
        up.manifest("", "pomodoro", "1.0.0").commit("one").tag("v1.0.0");
        up.manifest("", "pomodoro", "1.2.0").commit("two").tag("v1.2.0");
        up.manifest("", "pomodoro", "1.10.0").commit("three").tag("v1.10.0");
        up.manifest("", "pomodoro", "2.0.0-dev").commit("work in progress");

        let p = install(&up.url(), &plugins).unwrap();
        assert_eq!((p.id.as_str(), p.tag.as_deref()), ("pomodoro", Some("v1.10.0")), "1.10 is newer than 1.2");
        assert_eq!(version(&p, &plugins), "1.10.0", "not the unreleased commit");
        assert_eq!(newer(&p, &plugins).unwrap(), None);
        assert!(install(&up.url(), &plugins).unwrap_err().contains("installed already"));

        up.manifest("", "pomodoro", "1.11.0").commit("four").tag("v1.11.0");
        assert_eq!(newer(&p, &plugins).unwrap().as_deref(), Some("v1.11.0"));
        let p = update(&p, &plugins).unwrap();
        assert_eq!((p.tag.as_deref(), version(&p, &plugins).as_str()), (Some("v1.11.0"), "1.11.0"));

        up.manifest("", "pomodoro", "1.12.1").commit("wrong").tag("v1.12.0");
        assert!(update(&p, &plugins).unwrap_err().contains("must match"));
        assert_eq!(version(&p, &plugins), "1.11.0", "back to the release that worked");

        remove(&p, &plugins).unwrap();
        assert!(!plugins.join("pomodoro").exists());
    }

    #[test]
    fn a_plugin_in_a_folder_of_a_bigger_repository() {
        let d = TempDir::new("install-mono");
        let plugins = d.0.join("plugins");
        let up = Upstream::new(d.0.join("thumbdeck-plugins"));
        up.manifest("plugins/django", "django", "1.0.0").manifest("plugins/git", "git", "3.0.0").commit("both");
        up.tag("django-v1.0.0").tag("git-v3.0.0").tag("v9.0.0");

        let p = install(&format!("{}#plugins/django/", up.url()), &plugins).unwrap();
        assert_eq!(p.tag.as_deref(), Some("django-v1.0.0"), "its own tags only");
        assert!(p.folder(&plugins).ends_with("plugins/django/plugins/django"));
        assert!(install(&format!("{}#plugins/nope", up.url()), &plugins).unwrap_err().contains("no plugin.toml in plugins/nope"));
        assert!(install(&up.url(), &plugins).unwrap_err().contains("#folder"), "says how to name the folder");
    }

    #[test]
    fn without_tags_the_default_branch() {
        let d = TempDir::new("install-branch");
        let plugins = d.0.join("plugins");
        let up = Upstream::new(d.0.join("todos"));
        up.manifest("", "todos", "0.1.0").commit("one");
        let p = install(&up.url(), &plugins).unwrap();
        assert_eq!(p.tag, None);
        assert_eq!(newer(&p, &plugins).unwrap(), None);
        up.manifest("", "todos", "0.2.0").commit("two");
        assert_eq!(newer(&p, &plugins).unwrap().as_deref(), Some("the latest commit"));
        update(&p, &plugins).unwrap();
        assert_eq!(version(&p, &plugins), "0.2.0");
    }

    #[test]
    fn bad_sources_say_why() {
        let d = TempDir::new("install-bad");
        let plugins = d.0.join("plugins");
        assert!(install("/nowhere/at/all", &plugins).unwrap_err().starts_with("couldn't clone"));
        assert!(install("  ", &plugins).is_err());
        let broken = Upstream::new(d.0.join("broken"));
        std::fs::write(broken.0.join("plugin.toml"), "id = \"Broken\"\nname = \"x\"\nversion = \"1\"\napi = 1\n").unwrap();
        broken.commit("x");
        assert!(install(&broken.url(), &plugins).unwrap_err().contains("lowercase"));
        assert_eq!(std::fs::read_dir(&plugins).unwrap().count(), 0, "nothing left behind");
    }

    #[test]
    fn linking_a_folder() {
        let d = TempDir::new("install-link");
        d.file("mine/plugin.toml", "id = \"mine\"\nname = \"Mine\"\nversion = \"0.1.0\"\napi = 1\n");
        let p = link(&d.0.join("mine")).unwrap();
        assert!(p.linked && p.id == "mine");
        assert_eq!(p.folder(Path::new("/unused")), d.0.join("mine").canonicalize().unwrap());
        assert_eq!(newer(&p, Path::new("/unused")).unwrap(), None);
        remove(&p, Path::new("/unused")).unwrap();
        assert!(d.0.join("mine/plugin.toml").exists(), "a linked folder is never deleted");
        assert!(link(&d.0.join("nope")).is_err());
    }
}
