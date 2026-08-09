use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{FuzzyLimits, PixelDifference, ReferenceRelation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestStatus {
    Pass,
    Fail,
    Error,
    Skip,
}

impl TestStatus {
    fn css_class(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Error => "error",
            Self::Skip => "skip",
        }
    }
}

impl fmt::Display for TestStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Error => "ERROR",
            Self::Skip => "SKIP",
        })
    }
}

#[derive(Debug)]
pub struct ReftestReport {
    pub name: String,
    pub test_source: SourceReport,
    pub reference_sources: Vec<SourceReport>,
    pub status: TestStatus,
    pub reason: String,
    pub actual_image: Option<String>,
    pub references: Vec<ReferenceReport>,
}

#[derive(Debug)]
pub struct SourceReport {
    pub display_path: String,
    pub file_path: PathBuf,
}

#[derive(Debug)]
pub struct ReferenceReport {
    pub relation: ReferenceRelation,
    pub status: TestStatus,
    pub reason: String,
    pub difference: Option<PixelDifference>,
    pub fuzzy: Option<FuzzyLimits>,
    pub reference_image: Option<String>,
    pub difference_image: Option<String>,
}

impl ReftestReport {
    pub fn reason(&self) -> String {
        self.reason.clone()
    }
}

pub struct ArtifactWriter {
    output_dir: PathBuf,
    images_dir: PathBuf,
}

#[derive(Default)]
struct ReportNode<'a> {
    children: BTreeMap<String, ReportNode<'a>>,
    results: Vec<&'a ReftestReport>,
}

impl<'a> ReportNode<'a> {
    fn insert(&mut self, result: &'a ReftestReport) {
        let mut node = self;
        for component in result.name.split('/').filter(|component| !component.is_empty()) {
            node = node.children.entry(component.to_string()).or_default();
        }
        node.results.push(result);
    }

    fn counts(&self) -> ResultCounts {
        let mut counts = ResultCounts::default();
        for result in &self.results {
            counts.add(result.status);
        }
        for child in self.children.values() {
            counts += child.counts();
        }
        counts
    }
}

#[derive(Clone, Copy, Default)]
struct ResultCounts {
    total: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    skipped: usize,
}

impl ResultCounts {
    fn add(&mut self, status: TestStatus) {
        self.total += 1;
        match status {
            TestStatus::Pass => self.passed += 1,
            TestStatus::Fail => self.failed += 1,
            TestStatus::Error => self.errors += 1,
            TestStatus::Skip => self.skipped += 1,
        }
    }

    fn failures(self) -> usize {
        self.failed + self.errors
    }
}

impl std::ops::AddAssign for ResultCounts {
    fn add_assign(&mut self, other: Self) {
        self.total += other.total;
        self.passed += other.passed;
        self.failed += other.failed;
        self.errors += other.errors;
        self.skipped += other.skipped;
    }
}

fn category_ids(report_tree: &ReportNode<'_>) -> BTreeMap<String, usize> {
    let mut ids = BTreeMap::new();
    let mut next_id = 0;
    if let Some(css) = report_tree.children.get("css") {
        for (name, node) in ordered_css_children(css) {
            collect_category_ids(node, &format!("css/{name}"), &mut ids, &mut next_id);
        }
    }
    for (name, node) in &report_tree.children {
        if name != "css" {
            collect_category_ids(node, name, &mut ids, &mut next_id);
        }
    }
    ids
}

fn ordered_css_children<'a>(node: &'a ReportNode<'a>) -> Vec<(&'a String, &'a ReportNode<'a>)> {
    let mut children = node.children.iter().collect::<Vec<_>>();
    children.sort_by(|(left, _), (right, _)| {
        let rank = |name: &str| match name {
            "css-flexbox" => 0,
            "css-grid" => 1,
            "CSS2" => 2,
            _ => 3,
        };
        rank(left).cmp(&rank(right)).then_with(|| left.cmp(right))
    });
    children
}

fn collect_category_ids(node: &ReportNode<'_>, path: &str, ids: &mut BTreeMap<String, usize>, next_id: &mut usize) {
    if node.children.is_empty() {
        return;
    }
    ids.insert(path.to_string(), *next_id);
    *next_id += 1;
    for (name, child) in &node.children {
        collect_category_ids(child, &format!("{path}/{name}"), ids, next_id);
    }
}

impl ArtifactWriter {
    pub fn prepare() -> Result<Self> {
        let output_dir = target_dir().join("wpt");
        let images_dir = output_dir.join("artifacts");

        if images_dir.exists() {
            fs::remove_dir_all(&images_dir)
                .with_context(|| format!("failed to clear old WPT artifacts at {}", images_dir.display()))?;
        }
        if output_dir.exists() {
            for entry in fs::read_dir(&output_dir)
                .with_context(|| format!("failed to inspect old WPT reports at {}", output_dir.display()))?
            {
                let path = entry?.path();
                let is_report = path.is_file()
                    && path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
                        name == "report.html" || name.starts_with("report-") && name.ends_with(".html")
                    });
                if is_report {
                    fs::remove_file(&path)
                        .with_context(|| format!("failed to clear old WPT report at {}", path.display()))?;
                }
            }
        }
        fs::create_dir_all(&images_dir)
            .with_context(|| format!("failed to create WPT artifact directory at {}", images_dir.display()))?;

        Ok(Self { output_dir, images_dir })
    }

    pub fn save_image(&self, index: usize, kind: &str, rgba: &[u8], width: usize, height: usize) -> Result<String> {
        let test_dir_name = format!("{:05}", index + 1);
        let test_dir = self.images_dir.join(&test_dir_name);
        fs::create_dir_all(&test_dir)
            .with_context(|| format!("failed to create WPT artifact directory at {}", test_dir.display()))?;
        let path = test_dir.join(format!("{kind}.png"));
        write_png(&path, rgba, width, height)?;
        Ok(format!("artifacts/{test_dir_name}/{kind}.png"))
    }

    pub fn save_difference_image(
        &self,
        index: usize,
        kind: &str,
        actual: &[u8],
        expected: &[u8],
        width: usize,
        height: usize,
    ) -> Result<Option<String>> {
        let expected_len = width * height * 4;
        anyhow::ensure!(
            actual.len() == expected_len && expected.len() == expected_len,
            "diff buffers must both contain {expected_len} bytes for a {width}x{height} RGBA image"
        );
        let Some(image) = difference_image(actual, expected) else {
            return Ok(None);
        };
        self.save_image(index, kind, &image, width, height).map(Some)
    }

    pub fn write_report(&self, results: &[ReftestReport]) -> Result<PathBuf> {
        let path = self.output_dir.join("report.html");
        let html = render_html(results, &self.output_dir);
        fs::write(&path, html).with_context(|| format!("failed to write WPT report at {}", path.display()))?;
        Ok(path)
    }

    pub fn write_status_manifest(&self, results: &[ReftestReport]) -> Result<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("results.txt");
        fs::write(&path, render_status_manifest(results))
            .with_context(|| format!("failed to write WPT status manifest at {}", path.display()))?;
        Ok(path)
    }
}

fn render_status_manifest(results: &[ReftestReport]) -> String {
    let mut entries = results
        .iter()
        .map(|result| {
            let status = match result.status {
                TestStatus::Pass => "PASS",
                TestStatus::Skip => "SKIP",
                TestStatus::Fail | TestStatus::Error => "FAIL",
            };
            format!("{} {status}", relative_manifest_name(&result.name))
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.to_ascii_lowercase());
    entries.push(String::new());
    entries.join("\n")
}

fn relative_manifest_name(name: &str) -> String {
    let normalized = name.replace('\\', "/");
    if let Some((_, relative)) = normalized.rsplit_once("/web-platform-tests/") {
        return relative.to_string();
    }
    if Path::new(name).is_absolute() {
        return Path::new(name).file_name().and_then(|name| name.to_str()).unwrap_or("unknown-test").to_string();
    }
    normalized.trim_start_matches("./").to_string()
}

fn target_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("WPT crate must be inside the workspace")
        .join("target")
}

fn write_png(path: &Path, rgba: &[u8], width: usize, height: usize) -> Result<()> {
    let expected_len = width * height * 4;
    anyhow::ensure!(
        rgba.len() == expected_len,
        "render buffer has {} bytes; expected {expected_len} for a {width}x{height} RGBA image",
        rgba.len()
    );

    let file = fs::File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    let mut encoder = png::Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().with_context(|| format!("failed to encode {}", path.display()))?;
    writer.write_image_data(rgba).with_context(|| format!("failed to encode {}", path.display()))?;
    Ok(())
}

fn render_html(results: &[ReftestReport], report_dir: &Path) -> String {
    let passed = results.iter().filter(|result| result.status == TestStatus::Pass).count();
    let failed = results.iter().filter(|result| result.status == TestStatus::Fail).count();
    let errors = results.iter().filter(|result| result.status == TestStatus::Error).count();
    let skipped = results.iter().filter(|result| result.status == TestStatus::Skip).count();
    let executed = results.len().saturating_sub(skipped);
    let overall = ResultCounts { total: results.len(), passed, failed, errors, skipped };
    let mut report_tree = ReportNode::default();
    for result in results {
        report_tree.insert(result);
    }
    let category_ids = category_ids(&report_tree);
    let category_controls = render_category_controls(&report_tree, &category_ids);
    let mut html = String::new();
    write!(
        html,
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>WPT Reftests</title>
<style>
body {{ margin: 2rem; background-color: #fbfbff; color: black; font-family: Arial, sans-serif; }}
h1 {{ margin-bottom: .25rem; }}
.summary {{ display: flex; flex-wrap: wrap; gap: .75rem; margin: 1.5rem 0; }}
.summary span {{ border: 1px solid #8888; padding: .55rem .8rem; }}
.tabs {{ border-bottom: 1px solid #8888; display: flex; gap: .25rem; margin-top: 1.5rem; }}
.tabs button {{ border: 1px solid transparent; cursor: pointer; font: inherit; padding: .65rem 1rem; }}
.tabs button[aria-selected="true"] {{ border-color: #8888; border-bottom-color: Canvas; font-weight: 700; margin-bottom: -1px; }}
.tab-panel {{ padding-top: 1rem; }}
.filters {{ align-items: flex-start; display: flex; flex-wrap: wrap; gap: 1rem; margin-bottom: 1.5rem; }}
.filters label {{ align-items: center; cursor: pointer; display: inline-flex; gap: .5rem; }}
.filters > button {{ font: inherit; }}
.category-filter {{ border: 1px solid #8888; max-width: 100%; }}
.category-filter > summary {{ cursor: pointer; padding: .45rem .65rem; }}
.category-filter-body {{ border-top: 1px solid #8888; padding: .65rem; }}
.category-actions {{ display: flex; gap: .5rem; margin-bottom: .65rem; }}
.category-actions button {{ font: inherit; }}
.category-options, .category-children {{ list-style: none; margin: 0; padding: 0; }}
.category-options {{ max-height: 18rem; overflow: auto; }}
.category-children {{ margin-left: 1.25rem; }}
.category-option > label {{ padding: .2rem 0; }}
.category-count {{ color: #666; font-size: .8rem; }}
.no-results {{ border: 1px dashed #8888; padding: 1rem; }}
.report-tree {{ display: grid; gap: .5rem; }}
.tree-node {{ border: 1px solid #8888;  }}
.tree-node > summary {{ align-items: baseline; cursor: pointer; display: flex; flex-wrap: wrap; gap: .5rem; padding: .65rem .8rem; }}
.node-name {{ font-weight: 650; overflow-wrap: anywhere; }}
.node-counts {{ color: #777; font-size: .875rem; }}
.node-children {{ border-top: 1px solid #8888; display: grid; gap: .5rem; margin-left: 1rem; padding: .5rem; }}
.test-node {{ border-left-width: .4rem; }}
.result {{ border: 1px solid #8888; border-left-width: .4rem; padding: 1rem; }}
.reference-result {{ border-top: 1px solid #8888; margin-top: 1rem; padding-top: .75rem; }}
.reference-result h4 {{ margin: 0 0 .5rem; }}
.pass {{ border-left-color: #238636; }}
.fail, .error {{ border-left-color: #da3633; }}
.skip {{ border-left-color: #9a6700; }}
.paths, .reason {{ overflow-wrap: anywhere; }}
.paths a, .source-path a {{ color: #0969da; }}
.source-view {{ border: 1px solid #8888; margin: 1rem 0; }}
.source-view > summary {{ cursor: pointer; font-weight: 650; padding: .65rem .8rem; }}
.source-view-body {{ border-top: 1px solid #8888; padding: 0 .8rem .8rem; }}
.source-tabs {{ border-bottom: 1px solid #8888; display: flex; flex-wrap: wrap; gap: .25rem; margin-top: .8rem; }}
.source-tabs button {{ background: transparent; border: 1px solid transparent; cursor: pointer; font: inherit; padding: .55rem .8rem; }}
.source-tabs button[aria-selected="true"] {{ background: #fff; border-color: #8888; border-bottom-color: #fff; font-weight: 700; margin-bottom: -1px; }}
.source-panel {{ padding-top: .7rem; }}
.source-path {{ margin: 0 0 .55rem; overflow-wrap: anywhere; }}
.source-code {{ background: #fff; border: 1px solid #8888; font: 13px/1.45 ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", monospace; margin: 0; max-height: 32rem; overflow: auto; padding: .8rem; tab-size: 2; white-space: pre; }}
#pass-rates-panel {{ width:70%; min-width: 200px; margin-left: auto; margin-right: auto; }}
.comparison {{ display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(min(100%, 30rem), 1fr)); }}
figure {{ margin: 0; min-width: 0; }}
figcaption {{ font-weight: 600; margin: .75rem 0 .4rem; }}
img {{ background: white; border: 1px solid #8888; display: block; height: auto; image-rendering: pixelated; max-width: 100%; }}
.missing {{ border: 1px dashed #8888; padding: 2rem; }}
.table-wrap {{ overflow-x: auto; }}
.stats-table {{ border-collapse: collapse; width: 100%; }}
.stats-table caption {{ font-size: 1.15rem; font-weight: 700; padding: .5rem 0 1rem; text-align: left; }}
.stats-table th, .stats-table td {{ border-bottom: 1px solid #8888; padding: .6rem .75rem; text-align: right; }}
.stats-table th:first-child, .stats-table td:first-child {{ text-align: left; }}
.stats-table thead th {{ position: sticky; top: 0; background-color: #cfceda; }}
.stats-table .overall {{ font-weight: 700; }}
.hierarchy-label {{ display: inline-block; padding-left: calc(var(--depth) * 1.25rem); }}
.pass-tree-toggle {{ background: none; border: 0; cursor: pointer; font: inherit; padding: 0; text-align: left; }}
.pass-tree-toggle .disclosure {{ display: inline-block; width: 1rem; }}
.pass-tree-toggle[aria-expanded="false"] .disclosure {{ transform: rotate(-90deg); }}
.pass-tree-spacer {{ display: inline-block; width: 1rem; }}
[hidden] {{ display: none !important; }}
</style>
</head>
<body>
<h1>WPT Reftests</h1>
<p>Each test must match at least one <code>rel=&quot;match&quot;</code> reference (when present) and differ from every <code>rel=&quot;mismatch&quot;</code> reference. Comparisons use reference-specific WPT <code>&lt;meta name=&quot;fuzzy&quot;&gt;</code> limits; tests without them use exact pixel equality.</p>
<div class="summary"><span><strong>{}</strong> total</span><span><strong>{executed}</strong> run</span><span><strong>{passed}</strong> passed</span><span><strong>{failed}</strong> failed</span><span><strong>{errors}</strong> errors</span><span><strong>{skipped}</strong> skipped</span></div>
<div class="tabs" role="tablist" aria-label="Report views">
<button id="results-tab" type="button" role="tab" aria-selected="true" aria-controls="results-panel">Results</button>
<button id="pass-rates-tab" type="button" role="tab" aria-selected="false" aria-controls="pass-rates-panel" tabindex="-1">Pass rates</button>
</div>
<section id="results-panel" class="tab-panel" role="tabpanel" aria-labelledby="results-tab">
<div class="filters"><label><input id="failed-only" type="checkbox"> Show only failed tests</label><label><input id="skipped-only" type="checkbox"> Show only skipped tests</label><button id="toggle-images" type="button" aria-pressed="false">Expand all images</button>{category_controls}</div>
<p id="no-results" class="no-results" hidden>No tests match the selected filters.</p>
<div class="report-tree">
"#,
        results.len()
    )
    .unwrap();

    let mut result_index = 0;
    for (name, node) in &report_tree.children {
        write_report_node(&mut html, name, node, 0, name, &category_ids, report_dir, &mut result_index);
    }
    for result in &report_tree.results {
        write_result(&mut html, result, result_index, report_dir);
        result_index += 1;
    }

    html.push_str(
        "</div>\n</section>\n<section id=\"pass-rates-panel\" class=\"tab-panel\" role=\"tabpanel\" aria-labelledby=\"pass-rates-tab\" hidden>\n",
    );
    write_pass_rate_table(&mut html, overall, &report_tree);
    html.push_str("</section>\n");

    html.push_str(
        r##"<script>
document.querySelectorAll('[role="tablist"]').forEach((tablist) => {
  const tabs = Array.from(tablist.querySelectorAll(':scope > [role="tab"]'));
  const panels = tabs.map((tab) => document.getElementById(tab.getAttribute("aria-controls"))).filter(Boolean);
  function selectTab(selectedTab) {
    tabs.forEach((tab) => {
      const selected = tab === selectedTab;
      tab.setAttribute("aria-selected", String(selected));
      tab.tabIndex = selected ? 0 : -1;
    });
    panels.forEach((panel) => {
      panel.hidden = panel.id !== selectedTab.getAttribute("aria-controls");
    });
  }
  tabs.forEach((tab) => {
    tab.addEventListener("click", () => selectTab(tab));
    tab.addEventListener("keydown", (event) => {
      let nextTab;
      if (event.key === "Home") nextTab = tabs[0];
      if (event.key === "End") nextTab = tabs[tabs.length - 1];
      if (event.key === "ArrowLeft") nextTab = tabs[(tabs.indexOf(tab) - 1 + tabs.length) % tabs.length];
      if (event.key === "ArrowRight") nextTab = tabs[(tabs.indexOf(tab) + 1) % tabs.length];
      if (!nextTab) return;
      event.preventDefault();
      selectTab(nextTab);
      nextTab.focus();
    });
  });
});

const failedOnly = document.querySelector("#failed-only");
const skippedOnly = document.querySelector("#skipped-only");
const categoryToggles = Array.from(document.querySelectorAll(".category-toggle"));
const categoryToggleById = new Map(categoryToggles.map((toggle) => [toggle.dataset.category, toggle]));
const imageToggle = document.querySelector("#toggle-images");
function applyResultFilters() {
  document.querySelectorAll(".result").forEach((result) => {
    const notFailed = result.dataset.status !== "fail" && result.dataset.status !== "error";
    result.hidden = (failedOnly.checked && notFailed) || (skippedOnly.checked && result.dataset.status !== "skip");
  });
  document.querySelectorAll(".tree-node").forEach((node) => {
    let categoryHidden = false;
    if (node.classList.contains("category-node")) {
      const categoryToggle = categoryToggleById.get(node.dataset.category);
      categoryHidden = categoryToggle && !categoryToggle.checked && !categoryToggle.indeterminate;
    } else if (node.classList.contains("test-node")) {
      const owningCategory = node.parentElement.closest(".category-node");
      const categoryToggle = owningCategory && categoryToggleById.get(owningCategory.dataset.category);
      categoryHidden = categoryToggle && !categoryToggle.checked;
    }
    const failuresHidden = failedOnly.checked && node.dataset.failures === "0";
    const skipsHidden = skippedOnly.checked && node.dataset.skipped === "0";
    node.hidden = Boolean(categoryHidden || failuresHidden || skipsHidden);
  });
  Array.from(document.querySelectorAll(".group-node:not(.category-node)")).reverse().forEach((node) => {
    const children = Array.from(node.querySelectorAll(":scope > .node-children > .tree-node, :scope > .node-children > .result"));
    if (children.length > 0 && children.every((child) => child.hidden)) node.hidden = true;
  });
  const roots = Array.from(document.querySelectorAll(".report-tree > .tree-node, .report-tree > .result"));
  document.querySelector("#no-results").hidden = roots.some((node) => !node.hidden);
}
function updateCategoryParents(toggle) {
  let option = toggle.closest(".category-option");
  let parentList = option.parentElement;
  while (parentList.classList.contains("category-children")) {
    const parentOption = parentList.closest(".category-option");
    const parentToggle = parentOption.querySelector(":scope > label > .category-toggle");
    const childToggles = Array.from(parentList.querySelectorAll(":scope > .category-option > label > .category-toggle"));
    const allChecked = childToggles.every((child) => child.checked);
    const anyChecked = childToggles.some((child) => child.checked || child.indeterminate);
    parentToggle.checked = allChecked;
    parentToggle.indeterminate = !allChecked && anyChecked;
    option = parentOption;
    parentList = option.parentElement;
  }
}
failedOnly.addEventListener("change", () => {
  if (failedOnly.checked) skippedOnly.checked = false;
  applyResultFilters();
});
skippedOnly.addEventListener("change", () => {
  if (skippedOnly.checked) failedOnly.checked = false;
  applyResultFilters();
});
categoryToggles.forEach((toggle) => toggle.addEventListener("change", () => {
  const descendants = toggle.closest(".category-option").querySelectorAll(".category-toggle");
  descendants.forEach((descendant) => {
    descendant.checked = toggle.checked;
    descendant.indeterminate = false;
  });
  updateCategoryParents(toggle);
  applyResultFilters();
}));
document.querySelector("#show-all-categories").addEventListener("click", () => {
  categoryToggles.forEach((toggle) => {
    toggle.checked = true;
    toggle.indeterminate = false;
  });
  applyResultFilters();
});
document.querySelector("#hide-all-categories").addEventListener("click", () => {
  categoryToggles.forEach((toggle) => {
    toggle.checked = false;
    toggle.indeterminate = false;
  });
  applyResultFilters();
});
imageToggle.addEventListener("click", () => {
  const expand = imageToggle.getAttribute("aria-pressed") !== "true";
  if (expand) {
    document.querySelectorAll(".tree-node").forEach((node) => { node.open = true; });
  } else {
    document.querySelectorAll(".test-node").forEach((node) => { node.open = false; });
  }
  imageToggle.setAttribute("aria-pressed", String(expand));
  imageToggle.textContent = expand ? "Hide all images" : "Expand all images";
});
const passRateRows = Array.from(document.querySelectorAll(".pass-rate-row"));
function updatePassRateTree() {
  let collapsedDepth = null;
  passRateRows.forEach((row) => {
    const depth = Number(row.dataset.depth);
    if (collapsedDepth !== null && depth > collapsedDepth) {
      row.hidden = true;
      return;
    }
    collapsedDepth = null;
    row.hidden = false;
    const toggle = row.querySelector(".pass-tree-toggle");
    if (toggle && toggle.getAttribute("aria-expanded") === "false") collapsedDepth = depth;
  });
}
document.querySelectorAll(".pass-tree-toggle").forEach((toggle) => {
  toggle.addEventListener("click", () => {
    toggle.setAttribute("aria-expanded", String(toggle.getAttribute("aria-expanded") !== "true"));
    updatePassRateTree();
  });
});
updatePassRateTree();
applyResultFilters();
</script>
</body>
</html>
"##,
    );
    html
}

fn render_category_controls(report_tree: &ReportNode<'_>, category_ids: &BTreeMap<String, usize>) -> String {
    let mut html = String::from(
        "<details class=\"category-filter\"><summary>Hide categories</summary><div class=\"category-filter-body\"><div class=\"category-actions\"><button id=\"show-all-categories\" type=\"button\">Show all</button><button id=\"hide-all-categories\" type=\"button\">Hide all</button></div><ul class=\"category-options\">",
    );
    if let Some(css) = report_tree.children.get("css") {
        for (name, node) in ordered_css_children(css) {
            write_category_control(
                &mut html,
                name,
                node,
                &format!("css/{name}"),
                category_enabled_by_default(name),
                category_ids,
            );
        }
    }
    for (name, node) in &report_tree.children {
        if name != "css" {
            write_category_control(&mut html, name, node, name, category_enabled_by_default(name), category_ids);
        }
    }
    html.push_str("</ul></div></details>");
    html
}

fn write_category_control(
    html: &mut String,
    name: &str,
    node: &ReportNode<'_>,
    path: &str,
    checked: bool,
    category_ids: &BTreeMap<String, usize>,
) {
    let Some(id) = category_ids.get(path) else {
        return;
    };
    let checked_attribute = if checked { " checked" } else { "" };
    write!(
        html,
        "<li class=\"category-option\"><label><input class=\"category-toggle\" type=\"checkbox\" data-category=\"{id}\"{checked_attribute}> {} <span class=\"category-count\">({} tests)</span></label>",
        escape_html(name),
        node.counts().total
    )
    .unwrap();
    let has_child_categories = node.children.iter().any(|(_, child)| !child.children.is_empty());
    if has_child_categories {
        html.push_str("<ul class=\"category-children\">");
        for (child_name, child) in &node.children {
            write_category_control(html, child_name, child, &format!("{path}/{child_name}"), checked, category_ids);
        }
        html.push_str("</ul>");
    }
    html.push_str("</li>");
}

fn category_enabled_by_default(category: &str) -> bool {
    matches!(category, "CSS2" | "css-flexbox" | "css-grid")
}

fn write_pass_rate_table(html: &mut String, overall: ResultCounts, report_tree: &ReportNode<'_>) {
    html.push_str(
        "<div class=\"table-wrap\"><table class=\"stats-table\"><caption>Pass rates by category</caption><thead><tr><th scope=\"col\">Category</th><th scope=\"col\">Total</th><th scope=\"col\">Run</th><th scope=\"col\">Passed</th><th scope=\"col\">Failed</th><th scope=\"col\">Errors</th><th scope=\"col\">Skipped</th><th scope=\"col\">Pass rate<br>(all tests)</th><th scope=\"col\">Pass rate<br>(excluding skipped)</th></tr></thead><tbody>\n",
    );
    write_pass_rate_row(html, "Overall", overall, None, false);
    if let Some(css) = report_tree.children.get("css") {
        for (name, node) in ordered_css_children(css) {
            write_pass_rate_node(html, name, node, 0);
        }
    }
    for (name, node) in &report_tree.children {
        if name != "css" {
            write_pass_rate_node(html, name, node, 0);
        }
    }
    html.push_str("</tbody></table></div>\n");
}

fn write_pass_rate_node(html: &mut String, name: &str, node: &ReportNode<'_>, depth: usize) {
    let has_children = node.children.values().any(|child| child.results.is_empty());
    write_pass_rate_row(html, name, node.counts(), Some(depth), has_children);
    for (child_name, child) in &node.children {
        if child.results.is_empty() {
            write_pass_rate_node(html, child_name, child, depth + 1);
        }
    }
}

fn write_pass_rate_row(
    html: &mut String,
    category: &str,
    counts: ResultCounts,
    depth: Option<usize>,
    has_children: bool,
) {
    let executed = counts.total.saturating_sub(counts.skipped);
    let pass_rate = (counts.total > 0).then(|| format!("{:.1}%", counts.passed as f64 / counts.total as f64 * 100.0));
    let executed_pass_rate = (executed > 0).then(|| format!("{:.1}%", counts.passed as f64 / executed as f64 * 100.0));
    let (row_attributes, category) = if let Some(depth) = depth {
        let disclosure = if has_children {
            if depth == 0 {
                "<button class=\"pass-tree-toggle\" type=\"button\" aria-expanded=\"false\"><span class=\"disclosure\" aria-hidden=\"true\">&#9662;</span>"
            } else {
                "<button class=\"pass-tree-toggle\" type=\"button\" aria-expanded=\"true\"><span class=\"disclosure\" aria-hidden=\"true\">&#9662;</span>"
            }
        } else {
            "<span class=\"pass-tree-spacer\"></span>"
        };
        let close = if has_children { "</button>" } else { "" };
        (
            format!(" class=\"pass-rate-row\" data-depth=\"{depth}\""),
            format!(
                "<span class=\"hierarchy-label\" style=\"--depth: {depth}\">{disclosure}{}{close}</span>",
                escape_html(category)
            ),
        )
    } else {
        (" class=\"overall\"".to_string(), escape_html(category))
    };
    writeln!(
        html,
        "<tr{row_attributes}><th scope=\"row\">{category}</th><td>{}</td><td>{executed}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        counts.total,
        counts.passed,
        counts.failed,
        counts.errors,
        counts.skipped,
        pass_rate.as_deref().unwrap_or("&mdash;"),
        executed_pass_rate.as_deref().unwrap_or("&mdash;")
    )
    .unwrap();
}

fn write_report_node(
    html: &mut String,
    name: &str,
    node: &ReportNode<'_>,
    depth: usize,
    path: &str,
    category_ids: &BTreeMap<String, usize>,
    report_dir: &Path,
    result_index: &mut usize,
) {
    let counts = node.counts();
    let is_test = !node.results.is_empty();
    let status_class = if counts.errors > 0 {
        " error"
    } else if counts.failed > 0 {
        " fail"
    } else if counts.skipped > 0 && counts.passed == 0 {
        " skip"
    } else if is_test {
        " pass"
    } else {
        ""
    };
    let node_class = if is_test { "test-node" } else { "group-node" };
    let category_id = category_ids.get(path).copied();
    let category_class = if category_id.is_some() { " category-node" } else { "" };
    let category_attribute = category_id.map(|id| format!(" data-category=\"{id}\"")).unwrap_or_default();
    let open = if counts.failures() > 0 || depth < 2 { " open" } else { "" };
    write!(
        html,
        "<details class=\"tree-node {node_class}{status_class}{category_class}\" data-failures=\"{}\" data-skipped=\"{}\"{category_attribute}{open}><summary><span class=\"node-name\">{}</span><span class=\"node-counts\">{} tests &middot; {} passed &middot; {} failed &middot; {} errors &middot; {} skipped</span></summary>\n<div class=\"node-children\">\n",
        counts.failures(),
        counts.skipped,
        escape_html(name),
        counts.total,
        counts.passed,
        counts.failed,
        counts.errors,
        counts.skipped
    )
    .unwrap();

    for result in &node.results {
        write_result(html, result, *result_index, report_dir);
        *result_index += 1;
    }
    if path == "css" {
        for (child_name, child) in ordered_css_children(node) {
            write_report_node(
                html,
                child_name,
                child,
                depth + 1,
                &format!("{path}/{child_name}"),
                category_ids,
                report_dir,
                result_index,
            );
        }
    } else {
        for (child_name, child) in &node.children {
            write_report_node(
                html,
                child_name,
                child,
                depth + 1,
                &format!("{path}/{child_name}"),
                category_ids,
                report_dir,
                result_index,
            );
        }
    }
    html.push_str("</div></details>\n");
}

fn write_result(html: &mut String, result: &ReftestReport, result_index: usize, report_dir: &Path) {
    write!(
        html,
        "<article class=\"result {}\" data-status=\"{}\"><p class=\"paths\"><strong>Test:</strong> ",
        result.status.css_class(),
        result.status.css_class()
    )
    .unwrap();
    write_source_link(html, &result.test_source, report_dir);
    writeln!(html, "<br><strong>Status:</strong> {}</p>", result.status).unwrap();
    writeln!(html, "<p class=\"reason\"><strong>Reason:</strong> {}</p>", escape_html(&result.reason)).unwrap();
    write_source_tabs(html, result, result_index, report_dir);
    for (index, reference) in result.references.iter().enumerate() {
        write!(
            html,
            "<section class=\"reference-result {}\"><h4>Reference {}: {} ({})</h4><p class=\"paths\"><strong>Expected/reference:</strong> ",
            reference.status.css_class(),
            index + 1,
            reference.relation,
            reference.relation.expectation()
        )
        .unwrap();
        if let Some(source) = result.reference_sources.get(index) {
            write_source_link(html, source, report_dir);
        } else {
            html.push_str("Unavailable");
        }
        writeln!(html, "<br><strong>Status:</strong> {}</p>", reference.status).unwrap();
        writeln!(html, "<p class=\"reason\"><strong>Reason:</strong> {}</p>", escape_html(&reference.reason)).unwrap();
        if let (Some(difference), Some(fuzzy)) = (reference.difference, reference.fuzzy) {
            writeln!(
                html,
                "<p class=\"metrics\"><strong>Detected:</strong> maxDifference={}, totalPixels={}. <strong>Fuzzy match range:</strong> {}.</p>",
                difference.max_difference, difference.total_pixels, fuzzy
            )
            .unwrap();
        }
        if reference.status != TestStatus::Skip {
            html.push_str("<div class=\"comparison\">");
            write_image(html, "Test", "Test render", result.actual_image.as_deref());
            write_image(
                html,
                &format!("Reference ({})", reference.relation.expectation()),
                "Reference render",
                reference.reference_image.as_deref(),
            );
            if reference.difference_image.is_some() {
                write_image(
                    html,
                    "Difference",
                    "Pixel difference; red marks mismatched pixels",
                    reference.difference_image.as_deref(),
                );
            }
            html.push_str("</div>");
        }
        html.push_str("</section>\n");
    }
    html.push_str("</article>\n");
}

fn write_source_tabs(html: &mut String, result: &ReftestReport, result_index: usize, report_dir: &Path) {
    let mut sources = Vec::with_capacity(result.reference_sources.len() + 1);
    sources.push(("Test".to_string(), &result.test_source));
    sources.extend(
        result.reference_sources.iter().enumerate().map(|(index, source)| (format!("Expected {}", index + 1), source)),
    );

    html.push_str("<details class=\"source-view\"><summary>Source code</summary><div class=\"source-view-body\">");
    write!(
        html,
        "<div class=\"source-tabs\" role=\"tablist\" aria-label=\"Source code for {}\">",
        escape_html(&result.name)
    )
    .unwrap();
    for (source_index, (label, _)) in sources.iter().enumerate() {
        let selected = source_index == 0;
        write!(
            html,
            "<button id=\"source-{result_index}-tab-{source_index}\" type=\"button\" role=\"tab\" aria-selected=\"{selected}\" aria-controls=\"source-{result_index}-panel-{source_index}\"{}>{}</button>",
            if selected { "" } else { " tabindex=\"-1\"" },
            escape_html(label)
        )
        .unwrap();
    }
    html.push_str("</div>");

    for (source_index, (_, source)) in sources.iter().enumerate() {
        write!(
            html,
            "<section id=\"source-{result_index}-panel-{source_index}\" class=\"source-panel\" role=\"tabpanel\" aria-labelledby=\"source-{result_index}-tab-{source_index}\"{}><p class=\"source-path\">",
            if source_index == 0 { "" } else { " hidden" }
        )
        .unwrap();
        write_source_link(html, source, report_dir);
        html.push_str("</p>");
        match fs::read(&source.file_path) {
            Ok(bytes) => {
                let code = String::from_utf8_lossy(&bytes);
                write!(html, "<pre class=\"source-code\"><code>{}</code></pre>", escape_html(&code)).unwrap();
            }
            Err(error) => {
                write!(html, "<p class=\"missing\">Source unavailable: {}</p>", escape_html(&error.to_string()))
                    .unwrap();
            }
        }
        html.push_str("</section>");
    }
    html.push_str("</div></details>\n");
}

fn write_source_link(html: &mut String, source: &SourceReport, report_dir: &Path) {
    let href = escape_html(&source_href(&source.file_path, report_dir));
    let display_path = escape_html(&source.display_path);
    write!(html, "<a href=\"{href}\" target=\"_blank\" rel=\"noopener\">{display_path}</a>").unwrap();
}

fn write_image(html: &mut String, title: &str, description: &str, image: Option<&str>) {
    write!(html, "<figure><figcaption>{title}</figcaption>").unwrap();
    if let Some(image) = image {
        write!(
            html,
            "<a href=\"{image}\"><img alt=\"{description}\" loading=\"lazy\" width=\"800\" height=\"600\" src=\"{image}\"></a>"
        )
        .unwrap();
    } else {
        write!(html, "<p class=\"missing\">{description} unavailable</p>").unwrap();
    }
    html.push_str("</figure>");
}

fn source_href(source_path: &Path, report_dir: &Path) -> String {
    let source_path = absolute_path(source_path);
    let report_dir = absolute_path(report_dir);
    if let Some(relative) = relative_path(&report_dir, &source_path) {
        return encode_href_path(&relative.to_string_lossy().replace('\\', "/"));
    }
    url::Url::from_file_path(source_path).map_or_else(|_| "#".to_string(), Into::into)
}

fn absolute_path(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |current| current.join(path))
    };
    fs::canonicalize(&path).unwrap_or(path)
}

fn relative_path(from_dir: &Path, target: &Path) -> Option<PathBuf> {
    let from = from_dir.components().collect::<Vec<_>>();
    let target = target.components().collect::<Vec<_>>();
    let common = from.iter().zip(&target).take_while(|(left, right)| left == right).count();
    if common == 0 {
        return None;
    }

    let mut relative = PathBuf::new();
    for component in &from[common..] {
        if matches!(component, std::path::Component::Normal(_) | std::path::Component::ParentDir) {
            relative.push("..");
        }
    }
    for component in &target[common..] {
        relative.push(component.as_os_str());
    }
    Some(relative)
}

fn encode_href_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            write!(encoded, "%{byte:02X}").unwrap();
        }
    }
    encoded
}

fn escape_html(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn difference_image(actual: &[u8], expected: &[u8]) -> Option<Vec<u8>> {
    let mut differs = false;
    let mut image = Vec::with_capacity(actual.len());
    for (actual, expected) in actual.chunks_exact(4).zip(expected.chunks_exact(4)) {
        if actual[..3] != expected[..3] {
            image.extend_from_slice(&[255, 0, 0, 255]);
            differs = true;
        } else {
            image.extend(actual[..3].iter().map(|channel| ((*channel as u16 + 3 * 255) / 4) as u8));
            image.push(255);
        }
    }
    differs.then_some(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difference_image_marks_mismatches_red_over_faded_context() {
        let actual = [0, 0, 0, 255, 0, 128, 0, 255];
        let expected = [0, 0, 0, 255, 0, 0, 0, 255];

        let image = difference_image(&actual, &expected).unwrap();

        assert_eq!(image, [191, 191, 191, 255, 255, 0, 0, 255]);
        assert!(difference_image(&actual, &actual).is_none());
    }

    #[test]
    fn status_manifest_is_relative_sorted_and_uses_three_states() {
        let result = |name: &str, status| ReftestReport {
            name: name.to_string(),
            test_source: SourceReport { display_path: name.to_string(), file_path: PathBuf::from(name) },
            reference_sources: Vec::new(),
            status,
            reason: String::new(),
            actual_image: None,
            references: Vec::new(),
        };
        let results = [
            result("css/z-last.html", TestStatus::Error),
            result(r"C:\Users\someone\repo\web-platform-tests\css\a-first.html", TestStatus::Pass),
            result("css/m-middle.html", TestStatus::Skip),
        ];

        assert_eq!(
            render_status_manifest(&results),
            "css/a-first.html PASS\ncss/m-middle.html SKIP\ncss/z-last.html FAIL\n"
        );
    }
}
