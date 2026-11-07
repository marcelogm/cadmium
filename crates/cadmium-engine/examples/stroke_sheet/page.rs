use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::compare::Comparison;
use crate::painting::{SHEET_HEIGHT_PX, SHEET_WIDTH_PX};
use crate::scenarios::{Cell, SPEEDS_PX_PER_SECOND};

const STYLE: &str = "\
:root{color-scheme:light dark;--bg:#f4f2ee;--fg:#1d1b18;--muted:#6b665e;--card:#fff;--line:#d9d4cb}\
@media (prefers-color-scheme:dark){:root{--bg:#1a1917;--fg:#ece8e1;--muted:#a39d93;--card:#24221f;--line:#3a3732}}\
body{margin:0;padding:16px;background:var(--bg);color:var(--fg);font:14px/1.4 system-ui,sans-serif}\
h1{font-size:20px;margin:0 0 4px}p{margin:0 0 12px;color:var(--muted)}\
.modes{position:sticky;top:0;background:var(--bg);padding:8px 0;z-index:1}\
.modes button{font:inherit;padding:6px 12px;margin-right:6px;border:1px solid var(--line);background:var(--card);color:var(--fg);border-radius:6px;cursor:pointer}\
.modes button.on{border-color:#c0392b;color:#c0392b}\
.grid{display:grid;grid-template-columns:160px repeat(4,minmax(0,1fr));gap:8px;align-items:start;overflow-x:auto}\
.head{font-weight:600}.row{font-weight:600;padding-top:8px}\
figure{margin:0;background:var(--card);border:1px solid var(--line);border-radius:6px;padding:4px}\
figure img{width:100%;display:block;border-radius:3px}\
figcaption{font-size:12px;color:var(--muted);padding:2px}\
figcaption b{color:#c0392b}.same b{color:#2e8b57}";

const SCRIPT: &str = "\
const imgs=[...document.querySelectorAll('figure img')];\
function show(mode){imgs.forEach(i=>i.src=i.dataset[mode]);\
document.querySelectorAll('.modes button').forEach(b=>b.classList.toggle('on',b.dataset.mode===mode));}\
document.querySelectorAll('.modes button').forEach(b=>b.onclick=()=>show(b.dataset.mode));\
document.addEventListener('keydown',e=>{if(e.key===' '){e.preventDefault();\
const on=document.querySelector('.modes button.on').dataset.mode;show(on==='reference'?'candidate':'reference');}});";

fn write_png(path: &Path, rgba: &[u8]) {
    let file = std::fs::File::create(path).expect("the sheet image can be written");
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        SHEET_WIDTH_PX,
        SHEET_HEIGHT_PX,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgba))
        .expect("the sheet image can be encoded");
}

pub(crate) fn write_cell_images(
    out_dir: &Path,
    cell: &Cell,
    reference: &[u8],
    candidate: &[u8],
    comparison: &Comparison,
) {
    let stem = cell.file_stem();
    write_png(&out_dir.join(format!("{stem}_reference.png")), reference);
    write_png(&out_dir.join(format!("{stem}_candidate.png")), candidate);
    write_png(
        &out_dir.join(format!("{stem}_heat.png")),
        &comparison.heat_rgba,
    );
}

fn figure(result: &Result) -> String {
    let stem = result.cell.file_stem();
    let comparison = &result.comparison;
    let class = if comparison.changed_share == 0.0 {
        " class=\"same\""
    } else {
        ""
    };
    format!(
        "<figure><img src=\"{stem}_heat.png\" data-heat=\"{stem}_heat.png\" \
         data-reference=\"{stem}_reference.png\" data-candidate=\"{stem}_candidate.png\" \
         loading=\"lazy\" alt=\"{name}\"><figcaption{class}>muda <b>{changed:.2}%</b> dos pixels \
         · média {mean:.3} · máx {max}</figcaption></figure>",
        name = result.cell.row_name(),
        changed = comparison.changed_share * 100.0,
        mean = comparison.mean_difference,
        max = comparison.max_difference,
    )
}

fn grid(results: &[Result]) -> String {
    let mut html = String::from("<div class=\"grid\"><div></div>");
    for speed in SPEEDS_PX_PER_SECOND {
        let _ = write!(html, "<div class=\"head\">{speed:.0} px/s</div>");
    }
    let mut row = String::new();
    for result in results {
        let name = result.cell.row_name();
        if name != row {
            let _ = write!(html, "<div class=\"row\">{name}</div>");
            row = name;
        }
        html.push_str(&figure(result));
    }
    html.push_str("</div>");
    html
}

pub(crate) fn write_index(out_dir: &Path, candidate: &str, results: &[Result]) -> PathBuf {
    let html = format!(
        "<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>Stroke sheet</title><style>{STYLE}</style></head><body>\
         <h1>Folha de traços: {candidate}</h1>\
         <p>Vermelho marca onde o candidato difere da referência (hoje). \
         Espaço alterna entre referência e candidato. Diferença em níveis de 0 a 255; \
         \"muda\" conta pixels com diferença de 3 ou mais.</p>\
         <div class=\"modes\"><button data-mode=\"heat\" class=\"on\">mapa de diferença</button>\
         <button data-mode=\"reference\">referência</button>\
         <button data-mode=\"candidate\">candidato</button></div>{grid}\
         <script>{SCRIPT}</script></body></html>",
        grid = grid(results),
    );
    let path = out_dir.join("index.html");
    std::fs::write(&path, html).expect("the sheet page can be written");
    path
}
