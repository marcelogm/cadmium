//! The side-by-side page of two renders.

use std::fmt::Write as _;
use std::path::Path;

use crate::BRUSHES_FILE;

const BEFORE: &str = "before";
const AFTER: &str = "after";
const STYLE: &str = "body{margin:0;padding:16px;background:#ecebe6;color:#1f1d1a;\
font:14px system-ui,sans-serif}h1{font-size:20px}h2{font-size:15px;margin:20px 0 6px}\
.pair{display:grid;grid-template-columns:1fr 1fr;gap:8px}figure{margin:0}\
img{width:100%;height:auto;display:block;border:1px solid #d6d2c8}\
figcaption{font-size:12px;color:#6b665d}";

/// `NN` and brush name, one per rendered sheet, from a render's brush list.
fn brushes(dir: &Path) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(dir.join(BRUSHES_FILE)).expect("a render's brush list");
    text.lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(index, name)| (index.to_string(), name.to_string()))
        .collect()
}

fn copy_render(from: &Path, to: &Path, sheets: &[(String, String)]) {
    std::fs::create_dir_all(to).expect("the copy's directory is creatable");
    for (index, _) in sheets {
        let file = format!("{index}.png");
        std::fs::copy(from.join(&file), to.join(&file)).expect("the sheet copies");
    }
}

fn row(html: &mut String, index: &str, name: &str) {
    let _ = write!(
        html,
        "<section><h2>{name}</h2><div class=\"pair\">\
         <figure><img src=\"{BEFORE}/{index}.png\" alt=\"{name}, before\"><figcaption>before</figcaption></figure>\
         <figure><img src=\"{AFTER}/{index}.png\" alt=\"{name}, after\"><figcaption>after</figcaption></figure>\
         </div></section>"
    );
}

/// Copies the `before` and `after` renders into `out` and writes `out/index.html`.
pub(crate) fn compare(before: &Path, after: &Path, out: &Path) {
    let sheets = brushes(before);
    copy_render(before, &out.join(BEFORE), &sheets);
    copy_render(after, &out.join(AFTER), &sheets);
    let mut html = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Brush sheets</title>\
         <style>{STYLE}</style><h1>Brush sheets: before and after</h1>"
    );
    for (index, name) in &sheets {
        row(&mut html, index, name);
    }
    std::fs::write(out.join("index.html"), html).expect("the page is writable");
    println!("{}", out.join("index.html").display());
}
