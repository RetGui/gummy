mod chrome;
pub mod cli;
pub mod paint;
pub mod parse;
pub mod report;
pub mod skip;
mod svg;

use anyhow::{Context, Result, anyhow, bail};
use data_url::DataUrl;

use std::{
    borrow::Cow,
    collections::HashMap,
    env, fmt, fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

use lightningcss::{
    declaration::DeclarationBlock,
    properties::Property,
    stylesheet::{ParserOptions as CssParserOptions, PrinterOptions, StyleAttribute},
    traits::{IntoOwned, ToCss},
    values::length::LengthPercentage as CssLengthPercentage,
};
use parley::{
    FontContext, Layout, LayoutContext,
    fontique::Blob,
    style::{FontFamily, FontStack, StyleProperty},
};
use scraper::{
    ElementRef, Html, Selector,
    node::{Element, Node},
};
use svgtypes::{Length as SvgLength, LengthUnit as SvgLengthUnit, ViewBox as SvgViewBox};
use vello_cpu::peniko::Color;
use vello_cpu::{Pixmap, RenderContext};

use crate::chrome::ChromeReferenceRenderer;
use crate::cli::{Args, RunMode};
use crate::paint::{read_html_document, render_reftest_document};
use crate::parse::finalize_border_widths;
use crate::parse::is_css_wide_keyword;
use crate::parse::parse_writing_mode;
use crate::parse::typed_initial_value;
use crate::parse::{apply_declaration, declaration_direction, declaration_font_size};
use crate::report::{ArtifactWriter, ReferenceReport, ReftestReport, SourceReport, TestStatus};
use gummy::prelude::{GummyAuto, GummyZero};
use gummy::{Dimension, Display, GummyTree, LengthPercentageAuto, NodeId, Rect, Style};

const VIEWPORT_WIDTH: usize = 800;
const VIEWPORT_HEIGHT: usize = 600;
const DEFAULT_WPT_DIR_NAME: &str = "web-platform-tests";
const WPT_REPOSITORY: &str = "https://github.com/web-platform-tests/wpt.git";

#[derive(Clone, Debug)]
pub struct RenderStyle {
    background: Option<Color>,
    border_color: Rect<Color>,
    border_visible: Rect<bool>,
    color: Color,
    direction: gummy::Direction,
    font_size: f32,
    is_inline: bool,
    is_table: bool,
    image: Option<Arc<Pixmap>>,
    overflow_wrap: parley::style::OverflowWrap,
    text_alignment: TextAlignment,
    white_space_nowrap: bool,
    word_break: parley::style::WordBreakStrength,
    writing_mode: WritingMode,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            background: None,
            border_color: Rect { left: Color::BLACK, right: Color::BLACK, top: Color::BLACK, bottom: Color::BLACK },
            border_visible: Rect { left: false, right: false, top: false, bottom: false },
            color: Color::BLACK,
            direction: gummy::Direction::Ltr,
            font_size: 16.0,
            is_inline: false,
            is_table: false,
            image: None,
            overflow_wrap: parley::style::OverflowWrap::Normal,
            text_alignment: TextAlignment::Start,
            white_space_nowrap: false,
            word_break: parley::style::WordBreakStrength::Normal,
            writing_mode: WritingMode::HorizontalTb,
        }
    }
}

impl RenderStyle {
    pub fn inherit_from(parent: &Self) -> Self {
        Self {
            color: parent.color,
            direction: parent.direction,
            font_size: parent.font_size,
            overflow_wrap: parent.overflow_wrap,
            text_alignment: parent.text_alignment,
            white_space_nowrap: parent.white_space_nowrap,
            word_break: parent.word_break,
            writing_mode: parent.writing_mode,
            ..Self::default()
        }
    }
}

#[derive(Clone)]
pub struct NodeContext {
    text: Option<AhemTextLayout>,
    image: Option<ImageMeasureData>,
}

#[derive(Clone)]
pub struct AhemTextLayout {
    layout: Layout<()>,
    text_alignment: TextAlignment,
    writing_mode: WritingMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextAlignment {
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

impl TextAlignment {
    pub(crate) fn parley(self) -> parley::layout::Alignment {
        match self {
            Self::Start => parley::layout::Alignment::Start,
            Self::End => parley::layout::Alignment::End,
            Self::Left => parley::layout::Alignment::Left,
            Self::Right => parley::layout::Alignment::Right,
            Self::Center => parley::layout::Alignment::Center,
            Self::Justify => parley::layout::Alignment::Justify,
        }
    }

    fn justify_content(self, direction: gummy::Direction) -> Option<gummy::JustifyContent> {
        match self {
            Self::Start => Some(gummy::JustifyContent::START),
            Self::End => Some(gummy::JustifyContent::END),
            Self::Left => Some(if direction == gummy::Direction::Ltr {
                gummy::JustifyContent::START
            } else {
                gummy::JustifyContent::END
            }),
            Self::Right => Some(if direction == gummy::Direction::Ltr {
                gummy::JustifyContent::END
            } else {
                gummy::JustifyContent::START
            }),
            Self::Center => Some(gummy::JustifyContent::CENTER),
            Self::Justify => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AhemFont {
    path: PathBuf,
    data: Arc<Vec<u8>>,
}

impl AhemFont {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn blob(&self) -> Blob<u8> {
        Blob::new(self.data.clone())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ImageMeasureData {
    size: gummy::Size<Option<f32>>,
    aspect_ratio: Option<f32>,
}

impl ImageMeasureData {
    const NONE: Self = Self { size: gummy::Size::NONE, aspect_ratio: None };
}

impl NodeContext {
    pub fn element() -> Self {
        Self { text: None, image: None }
    }

    pub fn image(image: ImageMeasureData) -> Self {
        Self { text: None, image: Some(image) }
    }

    pub(crate) fn text(
        text: String,
        font_size: f32,
        overflow_wrap: parley::style::OverflowWrap,
        text_alignment: TextAlignment,
        white_space_nowrap: bool,
        word_break: parley::style::WordBreakStrength,
        writing_mode: WritingMode,
        font_context: &mut FontContext,
        layout_context: &mut LayoutContext<()>,
    ) -> Self {
        let mut builder = layout_context.ranged_builder(font_context, &text, 1.0, true);
        builder.push_default(StyleProperty::FontStack(FontStack::Single(FontFamily::Named(Cow::Borrowed("Ahem")))));
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::OverflowWrap(overflow_wrap));
        builder.push_default(StyleProperty::TextWrapMode(if white_space_nowrap {
            parley::style::TextWrapMode::NoWrap
        } else {
            parley::style::TextWrapMode::Wrap
        }));
        builder.push_default(StyleProperty::WordBreak(word_break));
        let layout = builder.build(&text);
        Self { text: Some(AhemTextLayout { layout, text_alignment, writing_mode }), image: None }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WritingMode {
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

impl WritingMode {
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::VerticalRl | Self::VerticalLr)
    }
}

#[derive(Debug)]
pub struct Rule {
    selectors: Vec<RuleSelector>,
    declarations: Vec<Declaration>,
    order: usize,
}

#[derive(Debug)]
pub struct RuleSelector {
    matcher: Selector,
    pseudo_after: bool,
    specificity: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    property: String,
    value: String,
    parsed: Option<Property<'static>>,
    important: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CascadePriority {
    important: bool,
    inline: bool,
    specificity: u32,
    rule_order: usize,
    declaration_order: usize,
}

pub struct Document {
    tree: GummyTree<NodeContext>,
    root: NodeId,
    paint: HashMap<NodeId, RenderStyle>,
    renderer: RenderContext,
    source_path: Option<PathBuf>,
    font_context: FontContext,
    layout_context: LayoutContext<()>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct FuzzyLimits {
    max_difference: [usize; 2],
    total_pixels: [usize; 2],
}

impl FuzzyLimits {
    fn allows(self, difference: PixelDifference) -> bool {
        (self.max_difference[0]..=self.max_difference[1]).contains(&difference.max_difference)
            && (self.total_pixels[0]..=self.total_pixels[1]).contains(&difference.total_pixels)
    }
}

impl fmt::Display for FuzzyLimits {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn range(value: [usize; 2]) -> String {
            if value[0] == value[1] { value[0].to_string() } else { format!("{}-{}", value[0], value[1]) }
        }
        write!(formatter, "maxDifference={}; totalPixels={}", range(self.max_difference), range(self.total_pixels))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelDifference {
    pub max_difference: usize,
    pub total_pixels: usize,
}

impl PixelDifference {
    fn between(actual: &[u8], expected: &[u8]) -> Self {
        let (actual, _) = actual.as_chunks::<4>();
        let (expected, _) = expected.as_chunks::<4>();
        let mut result = Self { max_difference: 0, total_pixels: actual.len().abs_diff(expected.len()) };
        for (actual, expected) in actual.iter().zip(expected) {
            let difference = actual[..3]
                .iter()
                .zip(&expected[..3])
                .map(|(actual, expected)| actual.abs_diff(*expected) as usize)
                .max()
                .unwrap_or(0);
            result.max_difference = result.max_difference.max(difference);
            result.total_pixels += usize::from(difference > 0);
        }
        if actual.len() != expected.len() {
            result.max_difference = 255;
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReferenceRelation {
    Match,
    Mismatch,
}

impl ReferenceRelation {
    fn is_satisfied_by(self, equivalent: bool) -> bool {
        match self {
            Self::Match => equivalent,
            Self::Mismatch => !equivalent,
        }
    }

    pub fn operator(self) -> &'static str {
        match self {
            Self::Match => "==",
            Self::Mismatch => "!=",
        }
    }

    pub fn expectation(self) -> &'static str {
        match self {
            Self::Match => "must match",
            Self::Mismatch => "must differ",
        }
    }
}

impl fmt::Display for ReferenceRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Match => "match",
            Self::Mismatch => "mismatch",
        })
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReftestReference {
    reference: PathBuf,
    relation: ReferenceRelation,
    fuzzy: Result<FuzzyLimits, String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Reftest {
    test: PathBuf,
    references: Vec<ReftestReference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReftestLink {
    pub href: String,
    pub relation: ReferenceRelation,
}

pub fn main() -> Result<()> {
    let args = Args::parse()?;
    if !args.skip_download {
        ensure_wpt_checkout(&args.wpt_dir)?;
    }
    let ahem_font = load_ahem_font(&args.ahem_font, &args.wpt_dir)?;

    match args.mode {
        RunMode::AllCss { filter } => run_css_reftests(&args.wpt_dir, &ahem_font, filter.as_deref()),
        RunMode::Pair { test, reference } => {
            let artifacts = ArtifactWriter::prepare()?;
            let reference_renderer = ChromeReferenceRenderer::start(&args.wpt_dir, ahem_font.path())?;
            let html = read_html_document(&test).map_err(|error| error.to_string());
            let relation = html
                .as_ref()
                .ok()
                .and_then(|html| reftest_relation_for_reference(html, &args.wpt_dir, &test, &reference))
                .unwrap_or(ReferenceRelation::Match);
            let fuzzy = html.and_then(|html| fuzzy_limits(&html, &args.wpt_dir, &test, &reference));
            let reftest = Reftest {
                test: test.clone(),
                references: vec![ReftestReference { reference: reference.clone(), relation, fuzzy }],
            };
            let result = run_reftest_and_save(&reftest, &args.wpt_dir, &artifacts, 0, &ahem_font, &reference_renderer)?;
            let report_path = artifacts.write_report(std::slice::from_ref(&result))?;
            let status_path = artifacts.write_status_manifest(std::slice::from_ref(&result))?;
            println!("\nWPT report written to {}", cli_clickable_link(&report_path));
            println!("WPT status manifest written to {}", cli_clickable_link(&status_path));

            match result.status {
                TestStatus::Pass => {
                    println!(
                        "PASS {} {} {} ({}x{}): {}",
                        test.display(),
                        relation.operator(),
                        reference.display(),
                        VIEWPORT_WIDTH,
                        VIEWPORT_HEIGHT,
                        result.reason()
                    );
                    Ok(())
                }
                TestStatus::Skip => {
                    println!("SKIP {}: {}", test.display(), result.reason());
                    Ok(())
                }
                TestStatus::Fail | TestStatus::Error => {
                    bail!("{} {}: {}", result.status, test.display(), result.reason())
                }
            }
        }
    }
}

pub fn cli_clickable_link(path: &PathBuf) -> String {
    let report_path = path.to_string_lossy().replace('\\', "/");

    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", report_path, report_path)
}

pub fn default_wpt_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_WPT_DIR_NAME)
}

pub fn ensure_wpt_checkout(wpt_dir: &Path) -> Result<()> {
    if wpt_dir.join("css").is_dir() {
        return Ok(());
    }

    if let Some(parent) = wpt_dir.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    }

    println!("WPT checkout not found at {}; cloning {}", wpt_dir.display(), WPT_REPOSITORY);
    let status = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(WPT_REPOSITORY)
        .arg(wpt_dir)
        .status()
        .context("failed to start git; install git or pass --skip-download with an existing --wpt-dir")?;

    if !status.success() {
        bail!("git clone failed with status {status}");
    }

    Ok(())
}

pub fn load_ahem_font(explicit_path: &Option<PathBuf>, wpt_dir: &Path) -> Result<AhemFont> {
    let path = if let Some(path) = explicit_path {
        fs::metadata(path).with_context(|| format!("Ahem font not found at {}", path.display()))?;
        path.clone()
    } else {
        let candidates = [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fonts/Ahem.ttf"),
            wpt_dir.join("fonts/Ahem.ttf"),
            wpt_dir.join("css/fonts/ahem/Ahem.ttf"),
        ];
        candidates.into_iter().find(|path| path.exists()).ok_or_else(|| {
            anyhow!("Ahem.ttf is required but was not found in the WPT checkout. Pass --ahem-font PATH")
        })?
    };
    let data = fs::read(&path).with_context(|| format!("failed to read Ahem font at {}", path.display()))?;
    Ok(AhemFont { path, data: Arc::new(data) })
}

pub fn run_css_reftests(wpt_dir: &Path, ahem_font: &AhemFont, filter: Option<&str>) -> Result<()> {
    let css_dir = wpt_dir.join("css");
    if !css_dir.is_dir() {
        bail!("CSS test directory not found at {}", css_dir.display());
    }

    let mut tests = load_reftests(wpt_dir, &css_dir, filter)?;
    tests.sort();

    if tests.is_empty() {
        bail!("No CSS reftests discovered under {}", css_dir.display());
    }

    let artifacts = ArtifactWriter::prepare()?;
    let reference_renderer = ChromeReferenceRenderer::start(wpt_dir, ahem_font.path())?;
    let results = run_reftests_in_parallel(&tests, wpt_dir, &artifacts, ahem_font, &reference_renderer)?;
    let passed = results.iter().filter(|result| result.status == TestStatus::Pass).count();
    let failed = results.iter().filter(|result| result.status == TestStatus::Fail).count();
    let errors = results.iter().filter(|result| result.status == TestStatus::Error).count();
    let skipped = results.iter().filter(|result| result.status == TestStatus::Skip).count();
    let problems = tests
        .iter()
        .zip(&results)
        .filter(|(_, result)| matches!(result.status, TestStatus::Fail | TestStatus::Error))
        .map(|(reftest, result)| (reftest, result.reason()))
        .collect::<Vec<_>>();

    let report_path = artifacts.write_report(&results)?;
    let status_path = artifacts.write_status_manifest(&results)?;
    if !problems.is_empty() {
        println!("\nFailed or errored reftests (showing up to 20):");
        for (reftest, reason) in problems.iter().take(20) {
            println!("  {}: {}", reftest.test.display(), reason);
        }
        if problems.len() > 20 {
            println!("  ... and {} more", problems.len() - 20);
        }
    }
    println!("\nWPT report written to {}", cli_clickable_link(&report_path));
    println!("WPT status manifest written to {}", cli_clickable_link(&status_path));
    println!("CSS reftests complete: {passed} passed, {failed} failed, {errors} errors, {skipped} skipped");

    if problems.is_empty() { Ok(()) } else { bail!("{} CSS reftests failed or errored", problems.len()) }
}

fn run_reftests_in_parallel(
    tests: &[Reftest],
    wpt_dir: &Path,
    artifacts: &ArtifactWriter,
    ahem_font: &AhemFont,
    reference_renderer: &ChromeReferenceRenderer,
) -> Result<Vec<ReftestReport>> {
    let worker_count = thread::available_parallelism().map(|count| count.get()).unwrap_or(1).min(tests.len());
    println!(
        "Running {} CSS reftests at {}x{} with {worker_count} worker threads...",
        tests.len(),
        VIEWPORT_WIDTH,
        VIEWPORT_HEIGHT
    );

    let next_test = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    let mut ordered_results = (0..tests.len()).map(|_| None).collect::<Vec<_>>();
    let mut first_error = None;

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let sender = sender.clone();
            let next_test = &next_test;
            scope.spawn(move || {
                loop {
                    let index = next_test.fetch_add(1, Ordering::Relaxed);
                    let Some(reftest) = tests.get(index) else {
                        break;
                    };
                    let result =
                        run_reftest_and_save(reftest, wpt_dir, artifacts, index, ahem_font, reference_renderer);
                    if sender.send((index, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);

        for (completed, (index, result)) in receiver.into_iter().enumerate() {
            let reftest = &tests[index];
            match result {
                Ok(result) => {
                    match result.status {
                        TestStatus::Pass => {
                            println!("PASS {:>5}/{} {}", completed + 1, tests.len(), reftest.test.display());
                        }
                        TestStatus::Fail => {
                            println!("FAIL {:>5}/{} {}", completed + 1, tests.len(), reftest.test.display());
                        }
                        TestStatus::Error => println!(
                            "ERROR {:>4}/{} {}: {}",
                            completed + 1,
                            tests.len(),
                            reftest.test.display(),
                            result.reason()
                        ),
                        TestStatus::Skip => println!(
                            "SKIP {:>5}/{} {}: {}",
                            completed + 1,
                            tests.len(),
                            reftest.test.display(),
                            result.reason()
                        ),
                    }
                    ordered_results[index] = Some(result);
                }
                Err(error) => {
                    println!("ERROR {:>4}/{} {}: {error}", completed + 1, tests.len(), reftest.test.display());
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
    });

    if let Some(error) = first_error {
        return Err(error);
    }
    ordered_results
        .into_iter()
        .enumerate()
        .map(|(index, result)| result.ok_or_else(|| anyhow!("worker did not return result {}", index + 1)))
        .collect()
}

fn run_reftest_and_save(
    reftest: &Reftest,
    wpt_dir: &Path,
    artifacts: &ArtifactWriter,
    index: usize,
    ahem_font: &AhemFont,
    reference_renderer: &ChromeReferenceRenderer,
) -> Result<ReftestReport> {
    let name = report_path(&reftest.test, wpt_dir);
    let test_source = source_report(&reftest.test, wpt_dir);
    let reference_sources =
        reftest.references.iter().map(|reference| source_report(&reference.reference, wpt_dir)).collect::<Vec<_>>();
    if let Some(reason) = skip::reason_for_test(&reftest.test, wpt_dir) {
        return Ok(ReftestReport {
            name,
            test_source,
            reference_sources,
            status: TestStatus::Skip,
            reason,
            actual_image: None,
            references: Vec::new(),
        });
    }

    let actual = match render_reftest_document(&reftest.test, ahem_font) {
        Ok(actual) => actual,
        Err(error) => {
            return Ok(ReftestReport {
                name,
                test_source,
                reference_sources,
                status: TestStatus::Error,
                reason: format!("Test render: {error}"),
                actual_image: None,
                references: Vec::new(),
            });
        }
    };
    let actual_image = Some(artifacts.save_image(index, "actual", &actual, VIEWPORT_WIDTH, VIEWPORT_HEIGHT)?);
    let mut references = Vec::with_capacity(reftest.references.len());

    for (reference_index, reference) in reftest.references.iter().enumerate() {
        let fuzzy = match &reference.fuzzy {
            Ok(fuzzy) => *fuzzy,
            Err(error) => {
                references.push(ReferenceReport {
                    relation: reference.relation,
                    status: TestStatus::Error,
                    reason: format!("Invalid WPT fuzzy metadata: {error}"),
                    difference: None,
                    fuzzy: None,
                    reference_image: None,
                    difference_image: None,
                });
                continue;
            }
        };
        let reference_buffer = match reference_renderer.screenshot(&reference.reference) {
            Ok(buffer) => buffer,
            Err(error) => {
                references.push(ReferenceReport {
                    relation: reference.relation,
                    status: TestStatus::Error,
                    reason: format!("Chrome reference screenshot: {error}"),
                    difference: None,
                    fuzzy: Some(fuzzy),
                    reference_image: None,
                    difference_image: None,
                });
                continue;
            }
        };
        let image_kind = format!("reference-{:03}", reference_index + 1);
        let reference_image =
            Some(artifacts.save_image(index, &image_kind, &reference_buffer, VIEWPORT_WIDTH, VIEWPORT_HEIGHT)?);
        let difference = PixelDifference::between(&actual, &reference_buffer);
        let difference_kind = format!("difference-{:03}", reference_index + 1);
        let difference_image = artifacts.save_difference_image(
            index,
            &difference_kind,
            &actual,
            &reference_buffer,
            VIEWPORT_WIDTH,
            VIEWPORT_HEIGHT,
        )?;
        let equivalent = fuzzy.allows(difference);
        let status = if reference.relation.is_satisfied_by(equivalent) { TestStatus::Pass } else { TestStatus::Fail };
        let reason = format!(
            "Renders are considered {}; this reference {}. Detected maxDifference={} and totalPixels={}; fuzzy match range: {fuzzy}.",
            if equivalent { "equivalent" } else { "different" },
            reference.relation.expectation(),
            difference.max_difference,
            difference.total_pixels
        );
        references.push(ReferenceReport {
            relation: reference.relation,
            status,
            reason,
            difference: Some(difference),
            fuzzy: Some(fuzzy),
            reference_image,
            difference_image,
        });
    }

    let status = aggregate_reftest_status(&references);
    let reason = match status {
        TestStatus::Pass => {
            "Reference conditions satisfied: at least one match reference matched (when present), and every mismatch reference differed."
        }
        TestStatus::Fail => "One or more required reference conditions were not satisfied.",
        TestStatus::Error => "A required reference comparison could not be completed.",
        TestStatus::Skip => "A required reference comparison is outside this runner's supported scope.",
    }
    .to_string();

    Ok(ReftestReport { name, test_source, reference_sources, status, reason, actual_image, references })
}

fn aggregate_reftest_status(references: &[ReferenceReport]) -> TestStatus {
    fn match_status(references: &[&ReferenceReport]) -> TestStatus {
        if references.is_empty() || references.iter().any(|reference| reference.status == TestStatus::Pass) {
            TestStatus::Pass
        } else if references.iter().any(|reference| reference.status == TestStatus::Error) {
            TestStatus::Error
        } else if references.iter().any(|reference| reference.status == TestStatus::Skip) {
            TestStatus::Skip
        } else {
            TestStatus::Fail
        }
    }

    fn mismatch_status(references: &[&ReferenceReport]) -> TestStatus {
        if references.iter().any(|reference| reference.status == TestStatus::Fail) {
            TestStatus::Fail
        } else if references.iter().any(|reference| reference.status == TestStatus::Error) {
            TestStatus::Error
        } else if references.iter().any(|reference| reference.status == TestStatus::Skip) {
            TestStatus::Skip
        } else {
            TestStatus::Pass
        }
    }

    let matches =
        references.iter().filter(|reference| reference.relation == ReferenceRelation::Match).collect::<Vec<_>>();
    let mismatches =
        references.iter().filter(|reference| reference.relation == ReferenceRelation::Mismatch).collect::<Vec<_>>();
    let statuses = [match_status(&matches), mismatch_status(&mismatches)];
    if statuses.contains(&TestStatus::Fail) {
        TestStatus::Fail
    } else if statuses.contains(&TestStatus::Error) {
        TestStatus::Error
    } else if statuses.contains(&TestStatus::Skip) {
        TestStatus::Skip
    } else {
        TestStatus::Pass
    }
}

fn report_path(path: &Path, wpt_dir: &Path) -> String {
    path.strip_prefix(wpt_dir).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn source_report(path: &Path, wpt_dir: &Path) -> SourceReport {
    SourceReport { display_path: report_path(path, wpt_dir), file_path: path.to_path_buf() }
}

pub fn load_reftests(wpt_dir: &Path, css_dir: &Path, filter: Option<&str>) -> Result<Vec<Reftest>> {
    let mut html_files = Vec::new();
    load_html_files(css_dir, &mut html_files)?;
    let mut tests = Vec::new();

    for test in html_files {
        if let Some(filter) = filter
            && !test.to_string_lossy().contains(filter)
        {
            continue;
        }

        let html = read_html_document(&test)?;
        let mut references = Vec::new();
        for link in reftest_links(&html) {
            let reference = resolve_wpt_href(wpt_dir, test.parent().unwrap_or(css_dir), &link.href);
            if reference.is_file() {
                let fuzzy = fuzzy_limits(&html, wpt_dir, &test, &reference);
                references.push(ReftestReference { reference, relation: link.relation, fuzzy });
            }
        }
        if !references.is_empty() {
            tests.push(Reftest { test, references });
        }
    }

    Ok(tests)
}

pub fn load_html_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            load_html_files(&path, out)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("html" | "htm" | "xhtml" | "xht")
        ) {
            out.push(path);
        }
    }
    Ok(())
}

pub fn reftest_links(html: &str) -> Vec<ReftestLink> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("link").unwrap();
    document
        .select(&selector)
        .filter_map(|link| {
            let rel = link.value().attr("rel")?;
            let has_match = rel.split_ascii_whitespace().any(|item| item.eq_ignore_ascii_case("match"));
            let has_mismatch = rel.split_ascii_whitespace().any(|item| item.eq_ignore_ascii_case("mismatch"));
            let relation = match (has_match, has_mismatch) {
                (true, false) => ReferenceRelation::Match,
                (false, true) => ReferenceRelation::Mismatch,
                _ => return None,
            };
            let href = clean_href(link.value().attr("href")?.trim());
            Some(ReftestLink { href, relation })
        })
        .collect()
}

pub fn reftest_match_hrefs(html: &str) -> Vec<String> {
    reftest_links(html)
        .into_iter()
        .filter(|link| link.relation == ReferenceRelation::Match)
        .map(|link| link.href)
        .collect()
}

pub fn clean_href(href: &str) -> String {
    href.split(['#', '?']).next().unwrap_or(href).to_string()
}

pub fn resolve_wpt_href(wpt_dir: &Path, base_dir: &Path, href: &str) -> PathBuf {
    let href = Path::new(href);
    if href.is_absolute() || href.has_root() {
        let relative = href.strip_prefix(Path::new("/")).unwrap_or(href);
        wpt_dir.join(relative)
    } else {
        base_dir.join(href)
    }
}

fn reftest_relation_for_reference(
    html: &str,
    wpt_dir: &Path,
    test: &Path,
    reference: &Path,
) -> Option<ReferenceRelation> {
    let base = test.parent().unwrap_or(wpt_dir);
    let reference = fs::canonicalize(reference).unwrap_or_else(|_| reference.to_path_buf());
    reftest_links(html).into_iter().find_map(|link| {
        let target = resolve_wpt_href(wpt_dir, base, &link.href);
        let target = fs::canonicalize(&target).unwrap_or(target);
        (target == reference).then_some(link.relation)
    })
}

fn fuzzy_limits(html: &str, wpt_dir: &Path, test: &Path, reference: &Path) -> Result<FuzzyLimits, String> {
    if !html.contains("fuzzy") {
        return Ok(FuzzyLimits::default());
    }
    let document = Html::parse_document(html);
    let selector = Selector::parse("meta[name=fuzzy]").unwrap();
    let base = test.parent().unwrap_or(wpt_dir);
    let reference = fs::canonicalize(reference).unwrap_or_else(|_| reference.to_path_buf());
    let mut fallback = None;
    let mut specific = None;

    for meta in document.select(&selector) {
        let content = meta.value().attr("content").unwrap_or_default().trim();
        let (target, value) = content.rsplit_once(':').map_or((None, content), |(target, value)| (Some(target), value));
        let limits = parse_fuzzy_limits(value)?;
        let slot = if let Some(target) = target {
            let target = resolve_wpt_href(wpt_dir, base, &clean_href(target.trim()));
            let target = fs::canonicalize(&target).unwrap_or(target);
            if target != reference {
                continue;
            }
            &mut specific
        } else {
            &mut fallback
        };
        if slot.replace(limits).is_some() {
            return Err("multiple fuzzy values apply to the same reference".into());
        }
    }
    Ok(specific.or(fallback).unwrap_or_default())
}

fn parse_fuzzy_limits(value: &str) -> Result<FuzzyLimits, String> {
    let values = value.split(';').collect::<Vec<_>>();
    if values.len() != 2 {
        return Err(format!("malformed fuzzy value {value:?}"));
    }
    let mut max_difference = None;
    let mut total_pixels = None;
    let mut positional = Vec::new();
    for value in values {
        let (name, value) = value.split_once('=').map_or((None, value), |(name, value)| (Some(name.trim()), value));
        let range = parse_fuzzy_range(value.trim())?;
        let slot = match name {
            Some("maxDifference") => &mut max_difference,
            Some("totalPixels") => &mut total_pixels,
            Some(name) => return Err(format!("unknown fuzzy property {name:?}")),
            None => {
                positional.push(range);
                continue;
            }
        };
        if slot.replace(range).is_some() {
            return Err("duplicate fuzzy property".into());
        }
    }
    let mut positional = positional.into_iter();
    let max_difference = max_difference.or_else(|| positional.next()).ok_or("missing maxDifference")?;
    let total_pixels = total_pixels.or_else(|| positional.next()).ok_or("missing totalPixels")?;
    if positional.next().is_some() {
        return Err("too many fuzzy values".into());
    }
    Ok(FuzzyLimits { max_difference, total_pixels })
}

fn parse_fuzzy_range(value: &str) -> Result<[usize; 2], String> {
    let (min, max) = value.split_once('-').unwrap_or((value, value));
    let min = min.trim().parse().map_err(|_| format!("invalid fuzzy range {value:?}"))?;
    let max = max.trim().parse().map_err(|_| format!("invalid fuzzy range {value:?}"))?;
    if min > max {
        return Err(format!("invalid fuzzy range {value:?}"));
    }
    Ok([min, max])
}

pub fn run_reftest_pair(test_path: &Path, reference_path: &Path) -> Result<usize> {
    let wpt_dir = default_wpt_dir();
    let ahem_font = load_ahem_font(&None, &wpt_dir)?;
    let test = render_reftest_document(test_path, &ahem_font)?;
    let reference = ChromeReferenceRenderer::start(&wpt_dir, ahem_font.path())?.screenshot(reference_path)?;
    let html = read_html_document(test_path)?;
    let relation =
        reftest_relation_for_reference(&html, &wpt_dir, test_path, reference_path).unwrap_or(ReferenceRelation::Match);
    let fuzzy = fuzzy_limits(&html, &wpt_dir, test_path, reference_path).map_err(anyhow::Error::msg)?;
    let difference = PixelDifference::between(&test, &reference);
    if relation.is_satisfied_by(fuzzy.allows(difference)) { Ok(0) } else { Ok(difference.total_pixels.max(1)) }
}

pub fn build_node(
    node_ref: ego_tree::NodeRef<Node>,
    rules: &[Rule],
    inherited: &RenderStyle,
    document: &mut Document,
) -> Result<Option<NodeId>> {
    match node_ref.value() {
        Node::Element(element) => build_element(node_ref, element, rules, inherited, document),
        Node::Text(text) => {
            let text = collapse_text(text);
            if text.is_empty() {
                Ok(None)
            } else {
                let style = Style::default();
                let node_context = NodeContext::text(
                    text,
                    inherited.font_size,
                    inherited.overflow_wrap,
                    inherited.text_alignment,
                    inherited.white_space_nowrap,
                    inherited.word_break,
                    inherited.writing_mode,
                    &mut document.font_context,
                    &mut document.layout_context,
                );
                let node = document.tree.new_leaf_with_context(style, node_context)?;
                let mut render_style = RenderStyle::inherit_from(inherited);
                render_style.is_inline = true;
                document.paint.insert(node, render_style);
                Ok(Some(node))
            }
        }
        _ => Ok(None),
    }
}

pub fn build_element(
    node_ref: ego_tree::NodeRef<Node>,
    element: &Element,
    rules: &[Rule],
    inherited: &RenderStyle,
    document: &mut Document,
) -> Result<Option<NodeId>> {
    if matches!(element.name(), "head" | "meta" | "link" | "script" | "style" | "title") {
        return Ok(None);
    }

    let element_ref = ElementRef::wrap(node_ref).expect("element nodes must produce an ElementRef");
    let mut declarations = matching_declarations(&element_ref, rules);
    if let Some(inline_style) = element.attr("style") {
        declarations.extend(parse_declarations(inline_style)?.into_iter().enumerate().map(
            |(declaration_order, declaration)| {
                (
                    CascadePriority {
                        important: declaration.important,
                        inline: true,
                        specificity: 0,
                        rule_order: 0,
                        declaration_order,
                    },
                    declaration,
                )
            },
        ));
    }
    declarations.sort_by_key(|(priority, _)| *priority);

    let mut render_style = RenderStyle::inherit_from(inherited);
    let mut style = Style {
        display: Display::Block,
        box_sizing: gummy::BoxSizing::ContentBox,
        direction: inherited.direction,
        ..Style::default()
    };

    let mut image_measure = None;
    if element.name() == "img" {
        render_style.is_inline = true;
        style.item_is_replaced = true;
        style.replaced = true;
        image_measure = Some(ImageMeasureData::NONE);

        if let Some(src) = element.attr("src") {
            let image = load_image(document.source_path.as_deref(), src)?;
            style.aspect_ratio = image.measure.aspect_ratio;
            image_measure = Some(image.measure);
            render_style.image = Some(image.pixmap);
        }

        if let Some(width) = element.attr("width").and_then(|value| value.parse::<f32>().ok()) {
            style.size.width = Dimension::length(width.max(0.0));
        }
        if let Some(height) = element.attr("height").and_then(|value| value.parse::<f32>().ok()) {
            style.size.height = Dimension::length(height.max(0.0));
        }
    }

    let mut computed_font_size = inherited.font_size;
    for (_, declaration) in &declarations {
        if declaration.property == "font-size"
            && let Some(font_size) = declaration_font_size(declaration, inherited.font_size)
        {
            computed_font_size = font_size;
        }
    }
    render_style.font_size = computed_font_size;
    apply_user_agent_defaults(element.name(), &mut style, &mut render_style, computed_font_size);

    for (_, declaration) in &declarations {
        match declaration.property.as_str() {
            "direction" => {
                if let Some(direction) = declaration_direction(declaration) {
                    style.direction = direction;
                    render_style.direction = direction;
                }
            }
            "writing-mode" => {
                if let Some(writing_mode) = parse_writing_mode(&declaration.value) {
                    render_style.writing_mode = writing_mode;
                }
            }
            _ => {}
        }
    }

    for (_, declaration) in declarations {
        let font_size = if declaration.property == "font-size" { inherited.font_size } else { computed_font_size };
        apply_declaration(&mut style, &mut render_style, &declaration, font_size, Some(inherited));
    }
    finalize_border_widths(&mut style, &render_style);

    let mut children = Vec::new();
    let next_inherited = render_style.clone();
    for child in node_ref.children() {
        if let Some(child_id) = build_node(child, rules, &next_inherited, document)? {
            children.push(child_id);
        }
    }
    if let Some(after) = build_after_pseudo(&element_ref, rules, &next_inherited, document)? {
        children.push(after);
    }

    let inline_formatting_context = style.display == Display::Block
        && !children.is_empty()
        && children.iter().all(|child| {
            document.tree.style(*child).is_ok_and(|style| style.position != gummy::Position::Absolute)
                && document.paint.get(child).is_some_and(|paint| paint.is_inline)
        });
    if inline_formatting_context {
        style.display = Display::Flex;
        style.flex_wrap = if render_style.white_space_nowrap { gummy::FlexWrap::NoWrap } else { gummy::FlexWrap::Wrap };
        if let Some(justify_content) = render_style.text_alignment.justify_content(render_style.direction) {
            style.justify_content = Some(justify_content);
        }
    }

    if style.display == Display::Flex {
        for child in &children {
            let mut child_style = document.tree.style(*child)?.clone();
            if inline_formatting_context || render_style.is_table {
                child_style.flex_grow = 0.0;
                child_style.flex_shrink =
                    if inline_formatting_context && !render_style.white_space_nowrap { 1.0 } else { 0.0 };
                child_style.flex_basis = Dimension::AUTO;
            }
            let zero_basis = matches!(
                child_style.flex_basis.tag(),
                gummy::CompactLength::LENGTH_TAG | gummy::CompactLength::PERCENT_TAG
            ) && child_style.flex_basis.value() == 0.0;
            if !zero_basis {
                if matches!(style.flex_direction, gummy::FlexDirection::Row | gummy::FlexDirection::RowReverse)
                    && child_style.min_size.width.is_auto()
                {
                    child_style.min_size.width = Dimension::ZERO;
                } else if matches!(
                    style.flex_direction,
                    gummy::FlexDirection::Column | gummy::FlexDirection::ColumnReverse
                ) && child_style.min_size.height.is_auto()
                {
                    child_style.min_size.height = Dimension::ZERO;
                }
                document.tree.set_style(*child, child_style)?;
            }
        }
    }

    let node = if children.is_empty() {
        let context = image_measure.map_or_else(NodeContext::element, NodeContext::image);
        document.tree.new_leaf_with_context(style, context)?
    } else {
        document.tree.new_with_children(style, &children)?
    };
    document.paint.insert(node, render_style);
    Ok(Some(node))
}

fn apply_user_agent_defaults(name: &str, style: &mut Style, render_style: &mut RenderStyle, font_size: f32) {
    match name {
        "body" => {
            let margin = LengthPercentageAuto::length(8.0);
            style.margin = Rect { left: margin, right: margin, top: margin, bottom: margin };
        }
        "p" => {
            style.margin.top = LengthPercentageAuto::length(font_size);
            style.margin.bottom = LengthPercentageAuto::length(font_size);
        }
        "a" | "abbr" | "b" | "bdi" | "bdo" | "cite" | "code" | "data" | "del" | "dfn" | "em" | "i" | "ins" | "kbd"
        | "mark" | "q" | "ruby" | "s" | "samp" | "small" | "span" | "strike" | "strong" | "sub" | "sup" | "time"
        | "u" | "var" => render_style.is_inline = true,
        _ => {}
    }
}

#[cfg(test)]
mod user_agent_style_tests {
    use super::*;

    #[test]
    fn applies_browser_body_paragraph_and_inline_defaults() {
        let mut body = Style::default();
        apply_user_agent_defaults("body", &mut body, &mut RenderStyle::default(), 16.0);
        let eight_px = LengthPercentageAuto::length(8.0);
        assert_eq!(body.margin, Rect { left: eight_px, right: eight_px, top: eight_px, bottom: eight_px });

        let mut paragraph = Style::default();
        apply_user_agent_defaults("p", &mut paragraph, &mut RenderStyle::default(), 20.0);
        assert_eq!(paragraph.margin.top, LengthPercentageAuto::length(20.0));
        assert_eq!(paragraph.margin.bottom, LengthPercentageAuto::length(20.0));

        let mut strong = RenderStyle::default();
        apply_user_agent_defaults("strong", &mut Style::default(), &mut strong, 16.0);
        assert!(strong.is_inline);
    }
}

#[derive(Debug)]
struct LoadedImage {
    pixmap: Arc<Pixmap>,
    measure: ImageMeasureData,
}

fn load_image(source_path: Option<&Path>, src: &str) -> Result<LoadedImage> {
    let src = src.trim();
    if src.starts_with("data:") {
        let data_url = DataUrl::process(src).with_context(|| "failed to parse image data URL")?;
        let is_svg = data_url.mime_type().matches("image", "svg+xml");
        if !is_svg && !data_url.mime_type().matches("image", "png") {
            bail!("unsupported image data URL type {}", data_url.mime_type());
        }
        let (bytes, _) = data_url.decode_to_vec().with_context(|| "failed to decode image data URL")?;
        return if is_svg {
            load_svg(&bytes, source_path.and_then(Path::parent), "image data URL")
        } else {
            load_png(&bytes, "image data URL")
        };
    }

    let source_path = source_path.ok_or_else(|| anyhow!("cannot resolve image {src:?} without a document path"))?;
    let src = clean_href(src);
    let path = if Path::new(&src).has_root() {
        let root = source_path
            .ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == DEFAULT_WPT_DIR_NAME))
            .ok_or_else(|| anyhow!("cannot resolve root-relative image {src:?}"))?;
        root.join(src.trim_start_matches(['/', '\\']))
    } else {
        source_path.parent().unwrap_or(source_path).join(src)
    };
    let bytes = fs::read(&path).with_context(|| format!("failed to read image {}", path.display()))?;
    let label = path.display().to_string();
    if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("svg")) {
        load_svg(&bytes, path.parent(), &label)
    } else {
        load_png(&bytes, &label)
    }
}

fn load_png(bytes: &[u8], label: &str) -> Result<LoadedImage> {
    let pixmap = Pixmap::from_png(Cursor::new(bytes)).with_context(|| format!("failed to decode PNG {label}"))?;
    let width = pixmap.width() as f32;
    let height = pixmap.height() as f32;
    let measure = ImageMeasureData {
        size: gummy::Size::new(width, height),
        aspect_ratio: (height > 0.0).then_some(width / height),
    };
    Ok(LoadedImage { pixmap: Arc::new(pixmap), measure })
}

fn load_svg(bytes: &[u8], resources_dir: Option<&Path>, label: &str) -> Result<LoadedImage> {
    let source = std::str::from_utf8(bytes).with_context(|| format!("SVG {label} is not UTF-8"))?;
    let xml = usvg::roxmltree::Document::parse(source).with_context(|| format!("failed to parse SVG XML {label}"))?;
    let root = xml.root_element();
    if root.tag_name().name() != "svg" {
        bail!("image {label} does not have an SVG root element");
    }

    let options = usvg::Options { resources_dir: resources_dir.map(Path::to_path_buf), ..usvg::Options::default() };
    let tree = usvg::Tree::from_xmltree(&xml, &options).with_context(|| format!("failed to decode SVG {label}"))?;
    let rendered_size = tree.size();
    let width = svg_dimension_is_intrinsic(root.attribute("width")).then_some(rendered_size.width());
    let height = svg_dimension_is_intrinsic(root.attribute("height")).then_some(rendered_size.height());
    let aspect_ratio = match (width, height) {
        (Some(width), Some(height)) if height > 0.0 => Some(width / height),
        _ => root
            .attribute("viewBox")
            .and_then(|value| value.parse::<SvgViewBox>().ok())
            .map(|view_box| (view_box.w / view_box.h) as f32),
    };
    let measure = ImageMeasureData { size: gummy::Size { width, height }, aspect_ratio };
    let pixmap = svg::rasterize(&tree, VIEWPORT_WIDTH as u16, VIEWPORT_HEIGHT as u16);
    Ok(LoadedImage { pixmap: Arc::new(pixmap), measure })
}

fn svg_dimension_is_intrinsic(value: Option<&str>) -> bool {
    value.and_then(|value| value.parse::<SvgLength>().ok()).is_some_and(|length| {
        length.unit != SvgLengthUnit::Percent && length.number.is_finite() && length.number >= 0.0
    })
}

pub fn matching_declarations(element: &ElementRef<'_>, rules: &[Rule]) -> Vec<(CascadePriority, Declaration)> {
    matching_declarations_for(element, rules, false)
}

pub fn matching_declarations_for(
    element: &ElementRef<'_>,
    rules: &[Rule],
    pseudo_after: bool,
) -> Vec<(CascadePriority, Declaration)> {
    let mut matched = Vec::new();
    for rule in rules {
        let Some(specificity) = rule
            .selectors
            .iter()
            .filter(|selector| selector.pseudo_after == pseudo_after)
            .filter(|selector| selector.matcher.matches(element))
            .map(|selector| selector.specificity)
            .max()
        else {
            continue;
        };
        for (declaration_order, declaration) in rule.declarations.iter().enumerate() {
            matched.push((
                CascadePriority {
                    important: declaration.important,
                    inline: false,
                    specificity,
                    rule_order: rule.order,
                    declaration_order,
                },
                declaration.clone(),
            ));
        }
    }
    matched
}

pub fn build_after_pseudo(
    element: &ElementRef<'_>,
    rules: &[Rule],
    inherited: &RenderStyle,
    document: &mut Document,
) -> Result<Option<NodeId>> {
    let mut declarations = matching_declarations_for(element, rules, true);
    if declarations.is_empty()
        || declarations.iter().any(|(_, declaration)| {
            declaration.property == "content" && matches!(declaration.value.as_str(), "none" | "normal")
        })
    {
        return Ok(None);
    }
    declarations.sort_by_key(|(priority, _)| *priority);

    let mut render_style = RenderStyle::inherit_from(inherited);
    let mut style = Style {
        display: Display::Block,
        box_sizing: gummy::BoxSizing::ContentBox,
        direction: inherited.direction,
        ..Style::default()
    };
    let mut computed_font_size = inherited.font_size;
    for (_, declaration) in &declarations {
        if declaration.property == "font-size"
            && let Some(font_size) = declaration_font_size(declaration, inherited.font_size)
        {
            computed_font_size = font_size;
        }
    }
    render_style.font_size = computed_font_size;
    for (_, declaration) in declarations {
        let font_size = if declaration.property == "font-size" { inherited.font_size } else { computed_font_size };
        apply_declaration(&mut style, &mut render_style, &declaration, font_size, Some(inherited));
    }
    finalize_border_widths(&mut style, &render_style);

    let node = document.tree.new_leaf_with_context(style, NodeContext::element())?;
    document.paint.insert(node, render_style);
    Ok(Some(node))
}

pub fn parse_declarations(input: &str) -> Result<Vec<Declaration>> {
    let attribute =
        StyleAttribute::parse(input, CssParserOptions { error_recovery: true, ..CssParserOptions::default() })
            .map_err(|error| anyhow!("failed to parse style attribute: {error}"))?;
    declarations_from_block(&attribute.declarations)
}

pub fn declarations_from_block(block: &DeclarationBlock<'_>) -> Result<Vec<Declaration>> {
    let mut declarations = Vec::with_capacity(block.len());
    append_declarations(&mut declarations, &block.declarations, false)?;
    append_declarations(&mut declarations, &block.important_declarations, true)?;
    Ok(declarations)
}

pub fn append_declarations(
    declarations: &mut Vec<Declaration>,
    properties: &[Property<'_>],
    important: bool,
) -> Result<()> {
    for property in properties {
        append_declaration(declarations, property, important)?;
    }
    Ok(())
}

pub fn append_declaration(declarations: &mut Vec<Declaration>, property: &Property<'_>, important: bool) -> Result<()> {
    let property_id = property.property_id();
    if let Some(longhand_ids) = property_id.longhands() {
        let mut expanded = Vec::with_capacity(longhand_ids.len());
        for longhand_id in &longhand_ids {
            if let Some(longhand) = property.longhand(longhand_id) {
                expanded.push(longhand);
            }
        }

        if expanded.len() == longhand_ids.len() {
            for longhand in &expanded {
                append_declaration(declarations, longhand, important)?;
            }
            return Ok(());
        }

        let value = property
            .value_to_css_string(PrinterOptions { minify: true, ..PrinterOptions::default() })
            .map_err(|error| anyhow!("failed to serialize CSS property value: {error}"))?;
        if is_css_wide_keyword(&value) {
            for longhand_id in longhand_ids {
                let property_name = longhand_id
                    .to_css_string(PrinterOptions { minify: true, ..PrinterOptions::default() })
                    .map_err(|error| anyhow!("failed to serialize CSS property name: {error}"))?;
                let parsed = typed_initial_value(&longhand_id, &property_name, &value);
                declarations.push(Declaration { property: property_name, value: value.clone(), parsed, important });
            }
            return Ok(());
        }
    }

    let property_name = property
        .property_id()
        .to_css_string(PrinterOptions { minify: true, ..PrinterOptions::default() })
        .map_err(|error| anyhow!("failed to serialize CSS property name: {error}"))?;
    let value = property
        .value_to_css_string(PrinterOptions { minify: true, ..PrinterOptions::default() })
        .map_err(|error| anyhow!("failed to serialize CSS property value: {error}"))?;
    let parsed = if is_css_wide_keyword(&value) {
        typed_initial_value(&property.property_id(), &property_name, &value)
    } else {
        Some(property.clone().into_owned())
    };
    declarations.push(Declaration { property: property_name, value, parsed, important });
    Ok(())
}

pub fn collapse_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn compare_buffers(buf_a: &[u8], buf_b: &[u8]) -> bool {
    PixelDifference::between(buf_a, buf_b) == PixelDifference { max_difference: 0, total_pixels: 0 }
}

pub fn count_differing_pixels(buf_a: &[u8], buf_b: &[u8]) -> usize {
    PixelDifference::between(buf_a, buf_b).total_pixels
}
