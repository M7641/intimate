//! Write the report bundle: `report.json` (machine), `report.html` and
//! `report.md` (human), and `crawl.json` (the route map).

use anyhow::Result;
use html_escape::encode_text;
use serde::Serialize;

use crate::bundle::Bundle;
use crate::findings::{Finding, Findings, Severity};
use crate::models::{RunMeta, VisitRecord};

#[derive(Serialize)]
struct Report<'a> {
    meta: &'a RunMeta,
    findings: &'a [Finding],
}

pub fn write(
    bundle: &Bundle,
    meta: &RunMeta,
    findings: &Findings,
    visits: &[VisitRecord],
) -> Result<()> {
    let report = Report {
        meta,
        findings: findings.items(),
    };
    bundle.write_json("report.json", &report)?;
    bundle.write_json("crawl.json", &visits)?;
    bundle.write_text("report.md", &markdown(meta, findings))?;
    bundle.write_text("report.html", &html(meta, findings))?;
    Ok(())
}

fn sev_badge(s: Severity) -> &'static str {
    match s {
        Severity::Error => "ERROR",
        Severity::Warning => "WARN",
        Severity::Info => "INFO",
    }
}

fn markdown(meta: &RunMeta, findings: &Findings) -> String {
    let mut s = String::new();
    s.push_str(&format!("# forage report — {}\n\n", meta.base_url));
    s.push_str(&format!(
        "Ran {:.0}s of a {}s budget · {} pages · {} routes · {} controls exercised · {} requests\n\n",
        meta.duration_s,
        meta.budget_s,
        meta.pages_visited,
        meta.routes_covered,
        meta.controls_exercised,
        meta.requests_observed,
    ));
    s.push_str(&format!(
        "**{} findings — {} errors, {} warnings**\n\n",
        meta.findings_total, meta.errors, meta.warnings
    ));

    let mut items: Vec<&Finding> = findings.items().iter().collect();
    items.sort_by(|a, b| b.severity.cmp(&a.severity));
    for f in items {
        s.push_str(&format!(
            "- `{}` **{}** — {} ({})",
            sev_badge(f.severity),
            f.category.label(),
            f.detail,
            f.url
        ));
        if !f.seen_on.is_empty() {
            s.push_str(&format!(" · +{} more pages", f.seen_on.len()));
        }
        s.push('\n');
    }
    s
}

fn html(meta: &RunMeta, findings: &Findings) -> String {
    let mut rows = String::new();
    let mut items: Vec<&Finding> = findings.items().iter().collect();
    items.sort_by(|a, b| b.severity.cmp(&a.severity));
    for f in items {
        let sev = sev_badge(f.severity).to_lowercase();
        let status = f.http_status.map(|s| s.to_string()).unwrap_or_default();
        let req = f.request_url.as_deref().unwrap_or("");
        let shot = f
            .screenshot
            .as_ref()
            .map(|s| format!("<a href=\"{0}\">shot</a>", encode_text(s)))
            .unwrap_or_default();
        let extra = if f.seen_on.is_empty() {
            String::new()
        } else {
            format!(" <span class=mut>+{} pages</span>", f.seen_on.len())
        };
        rows.push_str(&format!(
            "<tr class={sev}><td><span class=\"badge {sev}\">{badge}</span></td>\
             <td>{cat}</td><td>{detail}{extra}</td>\
             <td class=mut>{url}</td><td class=mut>{status}</td>\
             <td class=mut title=\"{reqfull}\">{reqshort}</td><td>{shot}</td></tr>",
            badge = sev_badge(f.severity),
            cat = encode_text(f.category.label()),
            detail = encode_text(&f.detail),
            url = encode_text(&f.url),
            reqfull = encode_text(req),
            reqshort = encode_text(&truncate(req, 48)),
        ));
    }

    format!(
        r#"<!doctype html><html><head><meta charset=utf-8>
<title>forage — {base}</title>
<style>
  body {{ font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; margin: 2rem; color: #1a1a1a; background: #fafafa; }}
  h1 {{ font-size: 1.3rem; }}
  .meta {{ color: #555; margin-bottom: 1rem; }}
  .counts span {{ margin-right: 1rem; font-weight: 600; }}
  table {{ border-collapse: collapse; width: 100%; background: #fff; }}
  th, td {{ text-align: left; padding: .4rem .6rem; border-bottom: 1px solid #eee; vertical-align: top; }}
  th {{ background: #f0f0f0; position: sticky; top: 0; }}
  .mut {{ color: #888; font-size: .85em; word-break: break-all; }}
  .badge {{ padding: .1rem .4rem; border-radius: 3px; font-size: .75em; font-weight: 700; color: #fff; }}
  .badge.error {{ background: #c0392b; }}
  .badge.warn {{ background: #d68910; }}
  .badge.info {{ background: #7f8c8d; }}
  tr.error td:first-child {{ border-left: 3px solid #c0392b; }}
</style></head><body>
<h1>forage report</h1>
<div class=meta>{base}</div>
<div class=meta>ran {dur:.0}s / {budget}s budget · {pages} pages · {routes} routes · {ctrl} controls exercised · {reqs} requests</div>
<div class=counts><span style="color:#c0392b">{errors} errors</span>
<span style="color:#d68910">{warnings} warnings</span>
<span>{total} findings</span></div>
<table>
<thead><tr><th>sev</th><th>category</th><th>detail</th><th>page</th><th>status</th><th>request</th><th></th></tr></thead>
<tbody>{rows}</tbody>
</table>
</body></html>"#,
        base = encode_text(&meta.base_url),
        dur = meta.duration_s,
        budget = meta.budget_s,
        pages = meta.pages_visited,
        routes = meta.routes_covered,
        ctrl = meta.controls_exercised,
        reqs = meta.requests_observed,
        errors = meta.errors,
        warnings = meta.warnings,
        total = meta.findings_total,
        rows = rows,
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
