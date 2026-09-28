//! Plugin frames: `plugin://<id>/<path>` serves the files of a working plugin, from inside its
//! folder only. HTML pages get thumbdeck's colors, the kit and the page API (`window.thumbdeck`)
//! put in first, so they're there before the page's own scripts run.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use tauri::http::{Request, Response, StatusCode};

const API: &str = include_str!("frame/api.js");
const KIT: &str = include_str!("frame/kit.css");
const THEME: &str = include_str!("../../../src/lib/theme.css");

fn mime(path: &Path) -> &'static str {
    match path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// What goes first in every page: the colors, the kit (unless the page turns it off), the API
pub fn inject(html: &str) -> String {
    let kit_off = {
        let lower = html.to_ascii_lowercase();
        lower.contains(r#"name="thumbdeck-kit""#) && lower.contains(r#"content="off""#)
    };
    let head = format!(
        "<style>{THEME}</style>{}<script>{API}</script>",
        if kit_off { String::new() } else { format!("<style>{KIT}</style>") }
    );
    // After <head> when there is one, else after <html> or the doctype, else at the very start
    let lower = html.to_ascii_lowercase();
    let at = ["<head", "<html", "<!doctype"]
        .iter()
        .find_map(|tag| {
            let start = lower.find(tag)?;
            Some(start + lower[start..].find('>')? + 1)
        })
        .unwrap_or(0);
    format!("{}{head}{}", &html[..at], &html[at..])
}

/// The file a request asks for, inside the plugin's folder
pub fn resolve(folder: &Path, url_path: &str) -> Result<PathBuf, StatusCode> {
    let rel = percent_decode(url_path.trim_start_matches('/'));
    let rel = if rel.is_empty() { "index.html".to_string() } else { rel };
    let root = folder.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;
    let file = root.join(&rel).canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;
    if !file.starts_with(&root) {
        return Err(StatusCode::FORBIDDEN);
    }
    if !file.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(file)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        if let (b'%', Some(b)) = (bytes[i], hex) {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn respond(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Cache-Control", "no-store") // a linked plugin changes while you write it
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Owned(body))
        .unwrap()
}

/// Answer a `plugin://` request. `folder_of` finds a working plugin's folder by id.
pub fn serve(request: &Request<Vec<u8>>, folder_of: impl Fn(&str) -> Option<PathBuf>) -> Response<Cow<'static, [u8]>> {
    let uri = request.uri();
    let id = uri.host().unwrap_or_default();
    let Some(folder) = folder_of(id) else {
        return respond(StatusCode::NOT_FOUND, "text/plain", format!("no working plugin called {id}").into_bytes());
    };
    let file = match resolve(&folder, uri.path()) {
        Ok(f) => f,
        Err(status) => return respond(status, "text/plain", format!("{} isn't in the plugin", uri.path()).into_bytes()),
    };
    let Ok(bytes) = std::fs::read(&file) else {
        return respond(StatusCode::NOT_FOUND, "text/plain", b"can't read it".to_vec());
    };
    let kind = mime(&file);
    let body = if kind.starts_with("text/html") { inject(&String::from_utf8_lossy(&bytes)).into_bytes() } else { bytes };
    respond(StatusCode::OK, kind, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn the_api_goes_first() {
        let page = inject("<!doctype html><html><head><title>x</title></head><body><script>use()</script></body></html>");
        let api = page.find("window.thumbdeck").unwrap();
        assert!(page.starts_with("<!doctype html><html><head><style>"), "{}", &page[..60]);
        assert!(api < page.find("use()").unwrap());
        assert!(page.contains("--bg0") && page.contains(".td-list"));
        let bare = inject("<ul class=\"td-list\"></ul>");
        assert!(bare.starts_with("<style>") && bare.ends_with("<ul class=\"td-list\"></ul>"));
        let off = inject("<head><meta name=\"thumbdeck-kit\" content=\"off\"></head>");
        assert!(off.contains("--bg0") && !off.contains(".td-list"), "the colors stay, the kit goes");
    }

    #[test]
    fn only_files_inside_the_plugin() {
        let d = TempDir::new("frame-files");
        d.file("p/tab.html", "x").file("p/sub/a b.js", "y").file("secret.txt", "z");
        let p = d.0.join("p");
        assert!(resolve(&p, "/tab.html").is_ok());
        assert!(resolve(&p, "/sub/a%20b.js").is_ok());
        assert_eq!(resolve(&p, "/../secret.txt").unwrap_err(), StatusCode::FORBIDDEN);
        assert_eq!(resolve(&p, "/sub/%2e%2e/%2e%2e/secret.txt").unwrap_err(), StatusCode::FORBIDDEN);
        assert_eq!(resolve(&p, "/nope.html").unwrap_err(), StatusCode::NOT_FOUND);
        assert_eq!(resolve(&p, "/sub").unwrap_err(), StatusCode::NOT_FOUND, "a folder isn't a file");
    }

    #[test]
    fn serving() {
        let d = TempDir::new("frame-serve");
        d.file("p/tab.html", "<head></head>hi").file("p/app.js", "go()");
        let folder = d.0.join("p");
        let get = |url: &str| {
            let req = Request::builder().uri(url).body(Vec::new()).unwrap();
            serve(&req, |id| (id == "mine").then(|| folder.clone()))
        };
        let page = get("plugin://mine/tab.html?td=abc");
        assert_eq!(page.status(), StatusCode::OK);
        assert!(String::from_utf8_lossy(page.body()).contains("window.thumbdeck"));
        let js = get("plugin://mine/app.js");
        assert_eq!((js.headers()["Content-Type"].to_str().unwrap(), js.body().as_ref()), ("text/javascript; charset=utf-8", b"go()".as_ref()));
        assert_eq!(get("plugin://other/tab.html").status(), StatusCode::NOT_FOUND);
    }
}
