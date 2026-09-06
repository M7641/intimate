//! Generic control discovery, plus small page-state probes.
//!
//! calque's a11y tree tells us *what* interactive roles exist; forage needs to
//! *drive* them, so this injects JS that returns an addressable inventory — a
//! robust CSS selector, the control kind, options, and the owning form — for
//! every visible control on the page.

use anyhow::Result;
use chromiumoxide::Page;

use crate::models::ControlSpec;

/// Snapshot of the page used to detect breakage after an interaction.
#[derive(Debug, Clone, Default)]
pub struct PageState {
    pub body_len: usize,
    pub has_error: bool,
}

/// Discover every visible, drivable control on the current page.
pub async fn discover_controls(page: &Page) -> Vec<ControlSpec> {
    match page.evaluate_function(DISCOVER_JS).await {
        Ok(result) => result.into_value::<Vec<ControlSpec>>().unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Same-origin `<a href>` links the app actually rendered.
pub async fn discover_links(page: &Page) -> Vec<String> {
    let js = "() => Array.from(document.querySelectorAll('a[href]')).map(a => a.href)";
    match page.evaluate_function(js).await {
        Ok(result) => result.into_value::<Vec<String>>().unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// A short alphabetic token from the visible text, used to feed search inputs so
/// results are plausibly non-empty.
pub async fn page_token(page: &Page) -> Option<String> {
    let js = "() => { const t = (document.body ? document.body.innerText : '') \
        .split(/\\s+/).find(w => w.length >= 4 && /^[A-Za-z]+$/.test(w)); return t || null; }";
    page.evaluate_function(js)
        .await
        .ok()
        .and_then(|r| r.into_value::<Option<String>>().ok())
        .flatten()
}

pub async fn page_state(page: &Page) -> PageState {
    let js = "() => ({ \
        title: document.title || '', \
        bodyLen: (document.body ? document.body.innerText.length : 0), \
        hasError: !!document.querySelector('[role=alert], .error, .error-page, .error-message') \
    })";
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        body_len: usize,
        has_error: bool,
    }
    match page.evaluate_function(js).await {
        Ok(result) => match result.into_value::<Raw>() {
            Ok(r) => PageState {
                body_len: r.body_len,
                has_error: r.has_error,
            },
            Err(_) => PageState::default(),
        },
        Err(_) => PageState::default(),
    }
}

pub async fn current_title(page: &Page) -> Option<String> {
    page.evaluate("document.title")
        .await
        .ok()
        .and_then(|r| r.into_value::<String>().ok())
        .filter(|s| !s.is_empty())
}

/// Set a form control's value and fire input/change so the app's reactive layer
/// (React-controlled inputs, live filters, …) sees a real user edit. Borrowed
/// from calque's recipe engine.
pub async fn set_value(page: &Page, selector: &str, value: &str) -> Result<()> {
    let js = format!(
        "() => {{ const el = document.querySelector({sel}); \
         if (el) {{ el.value = {val}; \
         el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
         el.dispatchEvent(new Event('change', {{ bubbles: true }})); }} }}",
        sel = serde_json::to_string(selector)?,
        val = serde_json::to_string(value)?,
    );
    page.evaluate_function(js).await?;
    Ok(())
}

/// Tick a checkbox/radio and fire input/change.
pub async fn set_checked(page: &Page, selector: &str) -> Result<()> {
    let js = format!(
        "() => {{ const el = document.querySelector({sel}); \
         if (el) {{ el.checked = true; \
         el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
         el.dispatchEvent(new Event('change', {{ bubbles: true }})); }} }}",
        sel = serde_json::to_string(selector)?,
    );
    page.evaluate_function(js).await?;
    Ok(())
}

/// The discovery script. Returns an array of objects matching [`ControlSpec`].
const DISCOVER_JS: &str = r#"
() => {
  const esc = (s) => (window.CSS && CSS.escape) ? CSS.escape(s) : String(s).replace(/[^a-zA-Z0-9_-]/g, '\\$&');
  const selectorFor = (el) => {
    if (el.id) return '#' + esc(el.id);
    const nm = el.getAttribute('name');
    if (nm) return el.tagName.toLowerCase() + '[name="' + nm.replace(/"/g, '\\"') + '"]';
    const parts = [];
    let node = el;
    while (node && node.nodeType === 1 && parts.length < 6) {
      if (node.id) { parts.unshift('#' + esc(node.id)); break; }
      const tag = node.tagName.toLowerCase();
      const parent = node.parentElement;
      if (!parent) { parts.unshift(tag); break; }
      const sibs = Array.from(parent.children).filter(c => c.tagName === node.tagName);
      parts.unshift(tag + ':nth-of-type(' + (sibs.indexOf(node) + 1) + ')');
      node = parent;
    }
    return parts.join(' > ');
  };
  const labelFor = (el) => {
    const aria = el.getAttribute('aria-label');
    if (aria) return aria.trim();
    if (el.id) { const l = document.querySelector('label[for="' + el.id.replace(/"/g, '\\"') + '"]'); if (l) return l.innerText.trim(); }
    const wrap = el.closest('label'); if (wrap) return wrap.innerText.trim();
    if (el.placeholder) return el.placeholder.trim();
    const t = (el.innerText || el.value || '').trim();
    return t || null;
  };
  const formFor = (el) => {
    const f = el.form || el.closest('form');
    if (!f) return null;
    const sensitive = !!f.querySelector('input[type=password], input[type=file], [name*=card i], [name*=cvv i], [name*=iban i]');
    return { selector: selectorFor(f), method: (f.getAttribute('method') || 'get').toLowerCase(), has_sensitive: sensitive };
  };
  const kindOf = (el) => {
    const tag = el.tagName.toLowerCase();
    if (tag === 'select') return 'select';
    if (tag === 'textarea') return 'text';
    if (tag === 'button') return 'button';
    const role = el.getAttribute('role');
    if (role === 'tab') return 'tab';
    if (role === 'button') return 'button';
    if (tag === 'input') {
      const t = (el.getAttribute('type') || 'text').toLowerCase();
      if (t === 'checkbox') return 'checkbox';
      if (t === 'radio') return 'radio';
      if (t === 'number' || t === 'range') return 'number';
      if (t === 'date' || t === 'datetime-local' || t === 'month' || t === 'week') return 'date';
      if (t === 'email') return 'email';
      if (t === 'search') return 'search';
      if (t === 'submit' || t === 'button' || t === 'image' || t === 'reset') return 'button';
      if (t === 'password' || t === 'file' || t === 'hidden') return 'other';
      return 'text';
    }
    return 'other';
  };
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const s = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none';
  };
  const out = [];
  const seen = new Set();
  document.querySelectorAll('select, input, textarea, button, [role=button], [role=tab]').forEach((el) => {
    if (!visible(el)) return;
    const kind = kindOf(el);
    if (kind === 'other') return;
    const selector = selectorFor(el);
    if (seen.has(selector)) return;
    seen.add(selector);
    const options = el.tagName.toLowerCase() === 'select'
      ? Array.from(el.options).map(o => o.value).filter(v => v !== '')
      : [];
    out.push({
      selector,
      kind,
      label: labelFor(el),
      name: el.getAttribute('name') || el.id || null,
      options,
      form: formFor(el),
      disabled: !!el.disabled,
    });
  });
  return out;
}
"#;
