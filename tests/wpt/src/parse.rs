use crate::CssParserOptions;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use lightningcss::traits::{IntoOwned, Parse};
use lightningcss::values::color::CssColor;
use lightningcss::values::length::LengthValue;
use lightningcss::values::percentage::DimensionPercentage;
use lightningcss::{
    media_query::{
        MediaCondition, MediaFeature, MediaFeatureComparison, MediaFeatureId, MediaFeatureName, MediaFeatureValue,
        MediaList, MediaType, Operator, Qualifier, QueryFeature,
    },
    properties::{Property, PropertyId},
    rules::{CssRule, CssRuleList, supports::SupportsCondition},
    stylesheet::{ParserOptions, PrinterOptions, StyleSheet},
    traits::ToCss,
    values::{length::Length, resolution::Resolution},
};
use scraper::{Html, Selector};

use vello_cpu::RenderContext;

use crate::CssLengthPercentage;
use crate::paint::measure_content;
use crate::{
    Color, Declaration, Document, NodeContext, RenderStyle, Rule, RuleSelector, VIEWPORT_HEIGHT, VIEWPORT_WIDTH,
    WritingMode, build_node, declarations_from_block, matching_declarations_for, parse_declarations,
};
use gummy::prelude::{FromFr, GummyAuto, GummyFitContent, GummyGridLine, GummyMaxContent, GummyMinContent, GummyZero};
use gummy::{
    AvailableSpace, CheapCloneStr, Dimension, Display, GridPlacement, GridTemplateArea, GridTemplateComponent,
    GridTemplateRepetition, GummyTree, LengthPercentage, LengthPercentageAuto, MaxTrackSizingFunction,
    MinTrackSizingFunction, NodeId, Point, Rect, RepetitionCount, Size, Style, TrackSizingFunction,
};

pub fn parse_and_layout_with_path(html: &str, source_path: Option<&Path>) -> anyhow::Result<Document> {
    let document = Html::parse_document(html);
    let rules = parse_document_rules_with_path(&document, source_path)?;
    let root_element = document.root_element();

    let mut render_document = Document {
        tree: GummyTree::new(),
        root: NodeId::from(0usize),
        paint: HashMap::new(),
        renderer: RenderContext::new(VIEWPORT_WIDTH as u16, VIEWPORT_HEIGHT as u16),
        source_path: source_path.map(Path::to_path_buf),
    };

    let root_style = Style {
        display: Display::Block,
        size: Size {
            width: Dimension::length(VIEWPORT_WIDTH as f32),
            height: Dimension::length(VIEWPORT_HEIGHT as f32),
        },
        ..Style::default()
    };
    let root = render_document.tree.new_leaf_with_context(root_style, NodeContext::element())?;
    render_document.root = root;
    render_document.paint.insert(root, RenderStyle { background: Some(Color::WHITE), ..RenderStyle::default() });

    let inherited = RenderStyle::default();
    if let Some(html_node) = build_node(*root_element, &rules, &inherited, &mut render_document)? {
        render_document.tree.add_child(root, html_node)?;
    }

    render_document.tree.compute_layout_with_measure(
        render_document.root,
        Size {
            width: AvailableSpace::Definite(VIEWPORT_WIDTH as f32),
            height: AvailableSpace::Definite(VIEWPORT_HEIGHT as f32),
        },
        measure_content,
    )?;

    Ok(render_document)
}

fn parse_document_rules_with_path(document: &Html, source_path: Option<&Path>) -> anyhow::Result<Vec<Rule>> {
    let stylesheet_selector = Selector::parse("style, link").unwrap();
    let mut rules = Vec::new();
    for element in document.select(&stylesheet_selector) {
        if let Some(media) = element.value().attr("media").filter(|media| !media.trim().is_empty())
            && !media_attribute_matches(media)
        {
            continue;
        }

        let (css, stylesheet_path) = match element.value().name() {
            "style" => (clean_embedded_css(&element.text().collect::<String>()), source_path.map(Path::to_path_buf)),
            "link" if is_stylesheet_link(&element) => {
                let Some(source_path) = source_path else {
                    continue;
                };
                let Some(href) = element.value().attr("href") else {
                    continue;
                };
                let Some(path) = resolve_stylesheet_path(source_path, href) else {
                    continue;
                };
                let Ok(css) = fs::read_to_string(&path) else {
                    continue;
                };
                (css, Some(path))
            }
            _ => continue,
        };

        let mut import_stack = HashSet::new();
        if let Some(path) = &stylesheet_path {
            import_stack.insert(fs::canonicalize(path).unwrap_or_else(|_| path.clone()));
        }
        for mut rule in parse_css_rules_with_path(&css, stylesheet_path.as_deref(), &mut import_stack)? {
            rule.order = rules.len();
            rules.push(rule);
        }
    }
    Ok(rules)
}

pub(crate) fn active_declarations_with_path(
    html: &str,
    source_path: Option<&Path>,
) -> anyhow::Result<Vec<Declaration>> {
    let document = Html::parse_document(html);
    let rules = parse_document_rules_with_path(&document, source_path)?;
    let selector = Selector::parse("*").unwrap();
    let mut active = Vec::new();

    for element in document.select(&selector) {
        if matches!(element.value().name(), "head" | "meta" | "link" | "script" | "style" | "title") {
            continue;
        }

        active.extend(winning_declarations(&element, &rules, false)?);
        let generated = winning_declarations(&element, &rules, true)?;
        let generates_box = !generated.is_empty()
            && !generated.iter().any(|declaration| {
                declaration.property == "content" && matches!(declaration.value.as_str(), "none" | "normal")
            });
        if generates_box {
            active.extend(generated);
        }
    }

    Ok(active)
}

fn winning_declarations(
    element: &scraper::ElementRef<'_>,
    rules: &[Rule],
    pseudo_after: bool,
) -> anyhow::Result<Vec<Declaration>> {
    let mut declarations = matching_declarations_for(element, rules, pseudo_after);
    if !pseudo_after && let Some(inline_style) = element.value().attr("style") {
        declarations.extend(parse_declarations(inline_style)?.into_iter().enumerate().map(
            |(declaration_order, declaration)| {
                (
                    crate::CascadePriority {
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

    let mut winners = HashMap::new();
    for (_, declaration) in declarations {
        winners.insert(declaration.property.clone(), declaration);
    }
    Ok(winners.into_values().collect())
}

fn parse_css_rules_with_path(
    css: &str,
    source_path: Option<&Path>,
    import_stack: &mut HashSet<PathBuf>,
) -> anyhow::Result<Vec<Rule>> {
    let stylesheet = StyleSheet::parse(css, ParserOptions { error_recovery: true, ..ParserOptions::default() })
        .map_err(|error| anyhow!("failed to parse stylesheet: {error}"))?;
    let mut rules = Vec::new();
    append_css_rules(&stylesheet.rules, source_path, import_stack, &mut rules)?;
    Ok(rules)
}

fn append_css_rules(
    css_rules: &CssRuleList<'_>,
    source_path: Option<&Path>,
    import_stack: &mut HashSet<PathBuf>,
    rules: &mut Vec<Rule>,
) -> anyhow::Result<()> {
    for css_rule in &css_rules.0 {
        match css_rule {
            CssRule::Style(style_rule) => append_style_rule(style_rule, rules)?,
            CssRule::Media(media_rule) if media_list_matches(&media_rule.query) => {
                append_css_rules(&media_rule.rules, source_path, import_stack, rules)?;
            }
            CssRule::Supports(supports_rule) if supports_condition_matches(&supports_rule.condition) => {
                append_css_rules(&supports_rule.rules, source_path, import_stack, rules)?;
            }
            CssRule::Import(import_rule)
                if (import_rule.media.media_queries.is_empty() || media_list_matches(&import_rule.media))
                    && import_rule.supports.as_ref().is_none_or(supports_condition_matches) =>
            {
                append_imported_stylesheet(import_rule.url.as_ref(), source_path, import_stack, rules)?;
            }
            CssRule::LayerBlock(layer_rule) => {
                append_css_rules(&layer_rule.rules, source_path, import_stack, rules)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn append_imported_stylesheet(
    href: &str,
    source_path: Option<&Path>,
    import_stack: &mut HashSet<PathBuf>,
    rules: &mut Vec<Rule>,
) -> anyhow::Result<()> {
    let Some(source_path) = source_path else {
        return Ok(());
    };
    let Some(path) = resolve_stylesheet_path(source_path, href) else {
        return Ok(());
    };
    let canonical_path = fs::canonicalize(&path).unwrap_or(path);
    if !import_stack.insert(canonical_path.clone()) {
        return Ok(());
    }

    let result = (|| {
        let Ok(css) = fs::read_to_string(&canonical_path) else {
            return Ok(());
        };
        let imported = parse_css_rules_with_path(&css, Some(&canonical_path), import_stack)?;
        for mut rule in imported {
            rule.order = rules.len();
            rules.push(rule);
        }
        Ok(())
    })();
    import_stack.remove(&canonical_path);
    result
}

fn clean_embedded_css(css: &str) -> String {
    let trimmed = css.trim();
    let trimmed = trimmed.strip_prefix("<![CDATA[").unwrap_or(trimmed);
    trimmed.strip_suffix("]]>").unwrap_or(trimmed).to_string()
}

fn is_stylesheet_link(element: &scraper::ElementRef<'_>) -> bool {
    let rel = element.value().attr("rel").unwrap_or_default();
    let has_relation = |expected: &str| rel.split_ascii_whitespace().any(|item| item.eq_ignore_ascii_case(expected));
    has_relation("stylesheet") && !has_relation("alternate") && element.value().attr("disabled").is_none()
}

pub(crate) fn media_attribute_matches(media: &str) -> bool {
    let css = format!("@media {media} {{}}");
    let Ok(stylesheet) = StyleSheet::parse(&css, ParserOptions::default()) else {
        return false;
    };
    matches!(stylesheet.rules.0.first(), Some(CssRule::Media(rule)) if media_list_matches(&rule.query))
}

fn resolve_stylesheet_path(source_path: &Path, href: &str) -> Option<PathBuf> {
    let href = href.split(['#', '?']).next()?.trim();
    if href.is_empty()
        || href.starts_with("data:")
        || href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("//")
    {
        return None;
    }

    if href.starts_with('/') {
        let root = source_path.ancestors().find(|ancestor| ancestor.join("css").is_dir())?;
        Some(root.join(href.trim_start_matches('/')))
    } else {
        Some(source_path.parent()?.join(href))
    }
}

fn append_style_rule(
    style_rule: &lightningcss::rules::style::StyleRule<'_>,
    rules: &mut Vec<Rule>,
) -> anyhow::Result<()> {
    let mut selectors = Vec::new();
    for selector in &style_rule.selectors.0 {
        let specificity = selector.specificity();
        let selector_text = selector
            .to_css_string(PrinterOptions { minify: true, ..PrinterOptions::default() })
            .map_err(|error| anyhow!("failed to serialize CSS selector: {error}"))?;
        let (selector_text, pseudo_after) = selector_text
            .strip_suffix("::after")
            .or_else(|| selector_text.strip_suffix(":after"))
            .map_or((selector_text.as_str(), false), |selector| (selector, true));
        if let Ok(matcher) = Selector::parse(selector_text) {
            selectors.push(RuleSelector { matcher, pseudo_after, specificity });
        }
    }
    if selectors.is_empty() {
        return Ok(());
    }
    rules.push(Rule {
        selectors,
        declarations: declarations_from_block(&style_rule.declarations)?,
        order: rules.len(),
    });
    Ok(())
}

fn media_list_matches(media: &MediaList<'_>) -> bool {
    if media.media_queries.is_empty() {
        return true;
    }
    media.media_queries.iter().any(|query| {
        let media_type_matches = matches!(query.media_type, MediaType::All | MediaType::Screen);
        let condition_matches = query.condition.as_ref().is_none_or(media_condition_matches);
        let matches = media_type_matches && condition_matches;
        if query.qualifier == Some(Qualifier::Not) { !matches } else { matches }
    })
}

fn media_condition_matches(condition: &MediaCondition<'_>) -> bool {
    match condition {
        MediaCondition::Feature(feature) => media_feature_matches(feature),
        MediaCondition::Not(condition) => !media_condition_matches(condition),
        MediaCondition::Operation { operator: Operator::And, conditions } => {
            conditions.iter().all(media_condition_matches)
        }
        MediaCondition::Operation { operator: Operator::Or, conditions } => {
            conditions.iter().any(media_condition_matches)
        }
        MediaCondition::Unknown(_) => false,
    }
}

#[derive(Debug)]
enum EvaluatedMediaValue {
    Number(f32),
    Ident(String),
}

fn media_feature_matches(feature: &MediaFeature<'_>) -> bool {
    match feature {
        QueryFeature::Plain { name, value } => media_feature_value(name)
            .zip(evaluate_media_value(value))
            .is_some_and(|(actual, expected)| compare_media_values(&actual, &expected, MediaFeatureComparison::Equal)),
        QueryFeature::Boolean { name } => media_feature_value(name).is_some_and(|value| match value {
            EvaluatedMediaValue::Number(value) => value != 0.0,
            EvaluatedMediaValue::Ident(value) => value != "none",
        }),
        QueryFeature::Range { name, operator, value } => media_feature_value(name)
            .zip(evaluate_media_value(value))
            .is_some_and(|(actual, expected)| compare_media_values(&actual, &expected, *operator)),
        QueryFeature::Interval { name, start, start_operator, end, end_operator } => {
            let Some(actual) = media_feature_value(name) else {
                return false;
            };
            let Some(start) = evaluate_media_value(start) else {
                return false;
            };
            let Some(end) = evaluate_media_value(end) else {
                return false;
            };
            compare_media_values(&start, &actual, *start_operator) && compare_media_values(&actual, &end, *end_operator)
        }
    }
}

fn media_feature_value(name: &MediaFeatureName<'_, MediaFeatureId>) -> Option<EvaluatedMediaValue> {
    let MediaFeatureName::Standard(name) = name else {
        return None;
    };
    let number = |value| Some(EvaluatedMediaValue::Number(value));
    let ident = |value: &str| Some(EvaluatedMediaValue::Ident(value.to_string()));

    match name {
        MediaFeatureId::Width | MediaFeatureId::DeviceWidth => number(VIEWPORT_WIDTH as f32),
        MediaFeatureId::Height | MediaFeatureId::DeviceHeight => number(VIEWPORT_HEIGHT as f32),
        MediaFeatureId::AspectRatio | MediaFeatureId::DeviceAspectRatio => {
            number(VIEWPORT_WIDTH as f32 / VIEWPORT_HEIGHT as f32)
        }
        MediaFeatureId::Orientation => ident("landscape"),
        MediaFeatureId::Resolution | MediaFeatureId::WebKitDevicePixelRatio | MediaFeatureId::MozDevicePixelRatio => {
            number(1.0)
        }
        MediaFeatureId::Grid => number(0.0),
        MediaFeatureId::Color => number(8.0),
        MediaFeatureId::ColorIndex | MediaFeatureId::Monochrome => number(0.0),
        MediaFeatureId::HorizontalViewportSegments | MediaFeatureId::VerticalViewportSegments => number(1.0),
        MediaFeatureId::OverflowBlock | MediaFeatureId::OverflowInline => ident("scroll"),
        MediaFeatureId::DisplayMode => ident("browser"),
        MediaFeatureId::Update => ident("fast"),
        MediaFeatureId::ColorGamut | MediaFeatureId::VideoColorGamut => ident("srgb"),
        MediaFeatureId::DynamicRange | MediaFeatureId::VideoDynamicRange => ident("standard"),
        MediaFeatureId::Pointer | MediaFeatureId::AnyPointer => ident("fine"),
        MediaFeatureId::Hover | MediaFeatureId::AnyHover => ident("hover"),
        MediaFeatureId::Scripting => ident("none"),
        MediaFeatureId::PrefersColorScheme => ident("light"),
        MediaFeatureId::PrefersReducedMotion
        | MediaFeatureId::PrefersReducedTransparency
        | MediaFeatureId::PrefersContrast
        | MediaFeatureId::PrefersReducedData => ident("no-preference"),
        MediaFeatureId::ForcedColors | MediaFeatureId::InvertedColors | MediaFeatureId::NavControls => ident("none"),
        MediaFeatureId::EnvironmentBlending => ident("opaque"),
        MediaFeatureId::Scan => ident("progressive"),
    }
}

fn evaluate_media_value(value: &MediaFeatureValue<'_>) -> Option<EvaluatedMediaValue> {
    match value {
        MediaFeatureValue::Length(length) => evaluate_media_length(length).map(EvaluatedMediaValue::Number),
        MediaFeatureValue::Number(value) => Some(EvaluatedMediaValue::Number(*value)),
        MediaFeatureValue::Integer(value) => Some(EvaluatedMediaValue::Number(*value as f32)),
        MediaFeatureValue::Boolean(value) => Some(EvaluatedMediaValue::Number(u8::from(*value) as f32)),
        MediaFeatureValue::Resolution(value) => Some(EvaluatedMediaValue::Number(match value {
            Resolution::Dpi(value) => *value / 96.0,
            Resolution::Dpcm(value) => *value * 2.54 / 96.0,
            Resolution::Dppx(value) => *value,
        })),
        MediaFeatureValue::Ratio(value) => (value.1 != 0.0).then_some(EvaluatedMediaValue::Number(value.0 / value.1)),
        MediaFeatureValue::Ident(value) => Some(EvaluatedMediaValue::Ident(value.0.to_ascii_lowercase())),
        MediaFeatureValue::Env(_) => None,
    }
}

fn evaluate_media_length(length: &Length) -> Option<f32> {
    let Length::Value(value) = length else {
        return None;
    };
    if let Some(px) = value.to_px() {
        return Some(px);
    }

    let (value, unit) = value.to_unit_value();
    let factor = match unit {
        "em" | "rem" | "ch" => 16.0,
        "ex" | "cap" => 8.0,
        "vw" | "svw" | "lvw" | "dvw" => VIEWPORT_WIDTH as f32 / 100.0,
        "vh" | "svh" | "lvh" | "dvh" => VIEWPORT_HEIGHT as f32 / 100.0,
        "vmin" | "svmin" | "lvmin" | "dvmin" => VIEWPORT_HEIGHT.min(VIEWPORT_WIDTH) as f32 / 100.0,
        "vmax" | "svmax" | "lvmax" | "dvmax" => VIEWPORT_HEIGHT.max(VIEWPORT_WIDTH) as f32 / 100.0,
        _ => return None,
    };
    Some(value * factor)
}

fn compare_media_values(
    actual: &EvaluatedMediaValue,
    expected: &EvaluatedMediaValue,
    comparison: MediaFeatureComparison,
) -> bool {
    match (actual, expected) {
        (EvaluatedMediaValue::Number(actual), EvaluatedMediaValue::Number(expected)) => match comparison {
            MediaFeatureComparison::Equal => actual == expected,
            MediaFeatureComparison::GreaterThan => actual > expected,
            MediaFeatureComparison::GreaterThanEqual => actual >= expected,
            MediaFeatureComparison::LessThan => actual < expected,
            MediaFeatureComparison::LessThanEqual => actual <= expected,
        },
        (EvaluatedMediaValue::Ident(actual), EvaluatedMediaValue::Ident(expected)) => {
            comparison == MediaFeatureComparison::Equal && actual.eq_ignore_ascii_case(expected)
        }
        _ => false,
    }
}

fn supports_condition_matches(condition: &SupportsCondition<'_>) -> bool {
    match condition {
        SupportsCondition::Not(condition) => !supports_condition_matches(condition),
        SupportsCondition::And(conditions) => conditions.iter().all(supports_condition_matches),
        SupportsCondition::Or(conditions) => conditions.iter().any(supports_condition_matches),
        SupportsCondition::Declaration { property_id, value } => supports_declaration(property_id, value),
        SupportsCondition::Selector(selector) => Selector::parse(selector).is_ok(),
        SupportsCondition::Unknown(_) => false,
    }
}

fn supports_declaration(property_id: &PropertyId<'_>, value: &str) -> bool {
    if matches!(property_id, PropertyId::Custom(_)) {
        return false;
    }

    match Property::parse_string(
        property_id.clone(),
        value,
        ParserOptions { error_recovery: false, ..ParserOptions::default() },
    ) {
        Ok(Property::Unparsed(_)) => {
            matches!(value.trim(), "initial" | "inherit" | "unset" | "revert" | "revert-layer")
        }
        Ok(Property::Custom(_)) | Err(_) => false,
        Ok(_) => true,
    }
}

pub fn apply_declaration(
    style: &mut Style,
    render_style: &mut RenderStyle,
    declaration: &Declaration,
    font_size: f32,
    inherited: Option<&RenderStyle>,
) {
    if let Some(property) = &declaration.parsed {
        if matches!(property, Property::Unparsed(_) | Property::Custom(_))
            && matches!(declaration.property.as_str(), "float" | "clear" | "writing-mode")
        {
            apply_unparsed_declaration(style, render_style, &declaration.property, &declaration.value, inherited);
            return;
        }
        apply_typed_property(style, render_style, property, font_size);
        return;
    }

    apply_unparsed_declaration(style, render_style, &declaration.property, &declaration.value, inherited);
}

fn apply_typed_property(
    style: &mut Style,
    render_style: &mut RenderStyle,
    property: &Property<'_>,
    font_size: f32,
) -> bool {
    match property {
        Property::Display(value) => {
            style.display = css_display(value);
            render_style.is_inline = css_display_is_inline(value);
            render_style.is_table = css_display_is_table(value);
        }
        Property::Position(value) => style.position = css_position(value),
        Property::BoxSizing(value, _) => {
            style.box_sizing = match value {
                lightningcss::properties::size::BoxSizing::ContentBox => gummy::BoxSizing::ContentBox,
                lightningcss::properties::size::BoxSizing::BorderBox => gummy::BoxSizing::BorderBox,
            }
        }
        Property::FontSize(value) => {
            if let Some(value) = css_font_size(value, font_size) {
                render_style.font_size = value;
            }
        }
        Property::Direction(lightningcss::properties::text::Direction::Ltr) => {
            style.direction = gummy::Direction::Ltr;
            render_style.direction = gummy::Direction::Ltr;
        }
        Property::Direction(lightningcss::properties::text::Direction::Rtl) => {
            style.direction = gummy::Direction::Rtl;
            render_style.direction = gummy::Direction::Rtl;
        }
        Property::WhiteSpace(value) => {
            use lightningcss::properties::text::WhiteSpace;
            render_style.white_space_nowrap = matches!(value, WhiteSpace::NoWrap | WhiteSpace::Pre);
        }
        Property::Width(value) => set_if_some(css_dimension(value, font_size), |value| style.size.width = value),
        Property::Height(value) => set_if_some(css_dimension(value, font_size), |value| style.size.height = value),
        Property::MinWidth(value) => set_if_some(css_dimension(value, font_size), |value| style.min_size.width = value),
        Property::MinHeight(value) => {
            set_if_some(css_dimension(value, font_size), |value| style.min_size.height = value)
        }
        Property::MaxWidth(value) => {
            set_if_some(css_max_dimension(value, font_size), |value| style.max_size.width = value)
        }
        Property::MaxHeight(value) => {
            set_if_some(css_max_dimension(value, font_size), |value| style.max_size.height = value)
        }
        Property::InlineSize(value) => set_if_some(css_dimension(value, font_size), |value| {
            set_logical_size_value(value, render_style.writing_mode, true, &mut style.size)
        }),
        Property::BlockSize(value) => set_if_some(css_dimension(value, font_size), |value| {
            set_logical_size_value(value, render_style.writing_mode, false, &mut style.size)
        }),
        Property::MinInlineSize(value) => set_if_some(css_dimension(value, font_size), |value| {
            set_logical_size_value(value, render_style.writing_mode, true, &mut style.min_size)
        }),
        Property::MinBlockSize(value) => set_if_some(css_dimension(value, font_size), |value| {
            set_logical_size_value(value, render_style.writing_mode, false, &mut style.min_size)
        }),
        Property::MaxInlineSize(value) => set_if_some(css_max_dimension(value, font_size), |value| {
            set_logical_size_value(value, render_style.writing_mode, true, &mut style.max_size)
        }),
        Property::MaxBlockSize(value) => set_if_some(css_max_dimension(value, font_size), |value| {
            set_logical_size_value(value, render_style.writing_mode, false, &mut style.max_size)
        }),
        Property::AspectRatio(value) => {
            style.aspect_ratio = value.ratio.as_ref().and_then(|ratio| (ratio.1 != 0.0).then_some(ratio.0 / ratio.1));
        }
        Property::Top(value) => {
            set_if_some(css_length_percentage_auto(value, font_size), |value| style.inset.top = value)
        }
        Property::Right(value) => {
            set_if_some(css_length_percentage_auto(value, font_size), |value| style.inset.right = value)
        }
        Property::Bottom(value) => {
            set_if_some(css_length_percentage_auto(value, font_size), |value| style.inset.bottom = value)
        }
        Property::Left(value) => {
            set_if_some(css_length_percentage_auto(value, font_size), |value| style.inset.left = value)
        }
        Property::InsetBlockStart(value) => set_logical_length_auto(
            value,
            font_size,
            &mut style.inset,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::InsetBlockEnd(value) => set_logical_length_auto(
            value,
            font_size,
            &mut style.inset,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::InsetInlineStart(value) => set_logical_length_auto(
            value,
            font_size,
            &mut style.inset,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::InsetInlineEnd(value) => set_logical_length_auto(
            value,
            font_size,
            &mut style.inset,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::FlexDirection(value, _) => style.flex_direction = css_flex_direction(value),
        Property::FlexWrap(value, _) => style.flex_wrap = css_flex_wrap(value),
        Property::FlexGrow(value, _) => style.flex_grow = *value,
        Property::FlexShrink(value, _) => style.flex_shrink = *value,
        Property::FlexBasis(value, _) => {
            set_if_some(css_length_percentage_auto(value, font_size).map(Into::into), |value| style.flex_basis = value)
        }
        Property::AlignItems(value, _) => style.align_items = Some(css_align_items(value)),
        Property::AlignSelf(value, _) => style.align_self = css_align_self(value),
        Property::AlignContent(value, _) => style.align_content = Some(css_align_content(value)),
        Property::JustifyContent(value, _) => style.justify_content = Some(css_justify_content(value)),
        Property::JustifyItems(value) => style.justify_items = Some(css_justify_items(value)),
        Property::JustifySelf(value) => style.justify_self = css_justify_self(value),
        Property::OverflowX(value) => style.overflow.x = css_overflow(value),
        Property::OverflowY(value) => style.overflow.y = css_overflow(value),
        Property::MarginLeft(value) => {
            set_if_some(css_length_percentage_auto(value, font_size), |value| style.margin.left = value)
        }
        Property::MarginRight(value) => {
            set_if_some(css_length_percentage_auto(value, font_size), |value| style.margin.right = value)
        }
        Property::MarginTop(value) => {
            set_if_some(css_length_percentage_auto(value, font_size), |value| style.margin.top = value)
        }
        Property::MarginBottom(value) => {
            set_if_some(css_length_percentage_auto(value, font_size), |value| style.margin.bottom = value)
        }
        Property::MarginBlockStart(value) => set_logical_length_auto(
            value,
            font_size,
            &mut style.margin,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::MarginBlockEnd(value) => set_logical_length_auto(
            value,
            font_size,
            &mut style.margin,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::MarginInlineStart(value) => set_logical_length_auto(
            value,
            font_size,
            &mut style.margin,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::MarginInlineEnd(value) => set_logical_length_auto(
            value,
            font_size,
            &mut style.margin,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::PaddingLeft(value) => {
            set_if_some(css_non_auto_length(value, font_size), |value| style.padding.left = value)
        }
        Property::PaddingRight(value) => {
            set_if_some(css_non_auto_length(value, font_size), |value| style.padding.right = value)
        }
        Property::PaddingTop(value) => {
            set_if_some(css_non_auto_length(value, font_size), |value| style.padding.top = value)
        }
        Property::PaddingBottom(value) => {
            set_if_some(css_non_auto_length(value, font_size), |value| style.padding.bottom = value)
        }
        Property::PaddingBlockStart(value) => set_logical_non_auto_length(
            value,
            font_size,
            &mut style.padding,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::PaddingBlockEnd(value) => set_logical_non_auto_length(
            value,
            font_size,
            &mut style.padding,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::PaddingInlineStart(value) => set_logical_non_auto_length(
            value,
            font_size,
            &mut style.padding,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::PaddingInlineEnd(value) => set_logical_non_auto_length(
            value,
            font_size,
            &mut style.padding,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderLeftWidth(value) => {
            set_if_some(css_border_width(value, font_size), |value| style.border.left = value)
        }
        Property::BorderRightWidth(value) => {
            set_if_some(css_border_width(value, font_size), |value| style.border.right = value)
        }
        Property::BorderTopWidth(value) => {
            set_if_some(css_border_width(value, font_size), |value| style.border.top = value)
        }
        Property::BorderBottomWidth(value) => {
            set_if_some(css_border_width(value, font_size), |value| style.border.bottom = value)
        }
        Property::BorderBlockStartWidth(value) => set_logical_border_width_value(
            value,
            font_size,
            &mut style.border,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderBlockEndWidth(value) => set_logical_border_width_value(
            value,
            font_size,
            &mut style.border,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderInlineStartWidth(value) => set_logical_border_width_value(
            value,
            font_size,
            &mut style.border,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderInlineEndWidth(value) => set_logical_border_width_value(
            value,
            font_size,
            &mut style.border,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderLeftStyle(value) => render_style.border_visible.left = css_border_style_visible(value),
        Property::BorderRightStyle(value) => render_style.border_visible.right = css_border_style_visible(value),
        Property::BorderTopStyle(value) => render_style.border_visible.top = css_border_style_visible(value),
        Property::BorderBottomStyle(value) => render_style.border_visible.bottom = css_border_style_visible(value),
        Property::BorderBlockStartStyle(value) => set_logical_border_visibility_value(
            value,
            &mut render_style.border_visible,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderBlockEndStyle(value) => set_logical_border_visibility_value(
            value,
            &mut render_style.border_visible,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderInlineStartStyle(value) => set_logical_border_visibility_value(
            value,
            &mut render_style.border_visible,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderInlineEndStyle(value) => set_logical_border_visibility_value(
            value,
            &mut render_style.border_visible,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderLeftColor(value) => {
            set_if_some(resolve_css_color(value, render_style.color), |value| render_style.border_color.left = value)
        }
        Property::BorderRightColor(value) => {
            set_if_some(resolve_css_color(value, render_style.color), |value| render_style.border_color.right = value)
        }
        Property::BorderTopColor(value) => {
            set_if_some(resolve_css_color(value, render_style.color), |value| render_style.border_color.top = value)
        }
        Property::BorderBottomColor(value) => {
            set_if_some(resolve_css_color(value, render_style.color), |value| render_style.border_color.bottom = value)
        }
        Property::BorderBlockStartColor(value) => set_logical_color_value(
            value,
            render_style.color,
            &mut render_style.border_color,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderBlockEndColor(value) => set_logical_color_value(
            value,
            render_style.color,
            &mut render_style.border_color,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderInlineStartColor(value) => set_logical_color_value(
            value,
            render_style.color,
            &mut render_style.border_color,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BorderInlineEndColor(value) => set_logical_color_value(
            value,
            render_style.color,
            &mut render_style.border_color,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        Property::BackgroundColor(value) => render_style.background = resolve_css_color(value, render_style.color),
        Property::Color(value) => {
            if let Some(value) = resolve_css_color(value, render_style.color) {
                render_style.color = value;
            }
        }
        Property::RowGap(value) => set_if_some(css_gap(value, font_size), |value| style.gap.height = value),
        Property::ColumnGap(value) => set_if_some(css_gap(value, font_size), |value| style.gap.width = value),
        Property::GridTemplateRows(value) => {
            set_css_grid_template(value, font_size, &mut style.grid_template_rows, &mut style.grid_template_row_names)
        }
        Property::GridTemplateColumns(value) => set_css_grid_template(
            value,
            font_size,
            &mut style.grid_template_columns,
            &mut style.grid_template_column_names,
        ),
        Property::GridTemplateAreas(value) => set_css_grid_template_areas(value, &mut style.grid_template_areas),
        Property::GridAutoRows(value) => {
            set_if_some(css_auto_tracks(value, font_size), |value| style.grid_auto_rows = value)
        }
        Property::GridAutoColumns(value) => {
            set_if_some(css_auto_tracks(value, font_size), |value| style.grid_auto_columns = value)
        }
        Property::GridAutoFlow(value) => style.grid_auto_flow = css_grid_auto_flow(*value),
        Property::GridRowStart(value) => style.grid_row.start = css_grid_line(value),
        Property::GridRowEnd(value) => style.grid_row.end = css_grid_line(value),
        Property::GridColumnStart(value) => style.grid_column.start = css_grid_line(value),
        Property::GridColumnEnd(value) => style.grid_column.end = css_grid_line(value),
        _ => return false,
    }
    true
}

fn apply_unparsed_declaration(
    style: &mut Style,
    render_style: &mut RenderStyle,
    property: &str,
    value: &str,
    inherited: Option<&RenderStyle>,
) {
    if matches!(value, "inherit" | "unset")
        && is_inherited_property(property)
        && let Some(inherited) = inherited
    {
        apply_inherited_value(style, render_style, property, inherited);
        return;
    }
    let value = if value == "initial" || value == "unset" && !is_inherited_property(property) {
        initial_property_value(property).unwrap_or(value)
    } else {
        value
    };
    match property {
        "display" => set_if_some(parse_display(value), |parsed| style.display = parsed),
        "position" => set_if_some(parse_position(value), |parsed| style.position = parsed),
        "box-sizing" => set_if_parse(value, |parsed| style.box_sizing = parsed),
        "direction" => {
            if let Ok(parsed) = value.parse() {
                style.direction = parsed;
                render_style.direction = parsed;
            }
        }
        "float" => set_if_parse(value, |parsed| style.float = parsed),
        "clear" => set_if_parse(value, |parsed| style.clear = parsed),
        "width" => set_if_parse(value, |parsed| style.size.width = parsed),
        "height" => set_if_parse(value, |parsed| style.size.height = parsed),
        "inline-size" => set_logical_size(value, render_style.writing_mode, true, &mut style.size),
        "block-size" => set_logical_size(value, render_style.writing_mode, false, &mut style.size),
        "min-width" => set_if_parse(value, |parsed| style.min_size.width = parsed),
        "min-height" => set_if_parse(value, |parsed| style.min_size.height = parsed),
        "min-inline-size" => set_logical_size(value, render_style.writing_mode, true, &mut style.min_size),
        "min-block-size" => set_logical_size(value, render_style.writing_mode, false, &mut style.min_size),
        "max-width" => set_if_parse(value, |parsed| style.max_size.width = parsed),
        "max-height" => set_if_parse(value, |parsed| style.max_size.height = parsed),
        "max-inline-size" => set_logical_size(value, render_style.writing_mode, true, &mut style.max_size),
        "max-block-size" => set_logical_size(value, render_style.writing_mode, false, &mut style.max_size),
        "aspect-ratio" => set_if_some(parse_aspect_ratio(value), |parsed| style.aspect_ratio = parsed),
        "top" => set_if_parse(value, |parsed| style.inset.top = parsed),
        "right" => set_if_parse(value, |parsed| style.inset.right = parsed),
        "bottom" => set_if_parse(value, |parsed| style.inset.bottom = parsed),
        "left" => set_if_parse(value, |parsed| style.inset.left = parsed),
        "inset-block-start" => set_logical_rect(
            value,
            &mut style.inset,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "inset-block-end" => set_logical_rect(
            value,
            &mut style.inset,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "inset-inline-start" => set_logical_rect(
            value,
            &mut style.inset,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "inset-inline-end" => set_logical_rect(
            value,
            &mut style.inset,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "flex-direction" => set_if_parse(value, |parsed| style.flex_direction = parsed),
        "flex-wrap" => set_if_parse(value, |parsed| style.flex_wrap = parsed),
        "flex-grow" => set_if_parse(value, |parsed| style.flex_grow = parsed),
        "flex-shrink" => set_if_parse(value, |parsed| style.flex_shrink = parsed),
        "flex-basis" => set_if_parse(value, |parsed| style.flex_basis = parsed),
        "align-items" => set_if_parse(value, |parsed| style.align_items = Some(parsed)),
        "align-self" => set_if_parse(value, |parsed| style.align_self = Some(parsed)),
        "align-content" => set_if_parse(value, |parsed| style.align_content = Some(parsed)),
        "justify-content" => set_if_parse(value, |parsed| style.justify_content = Some(parsed)),
        "justify-items" => set_if_parse(value, |parsed| style.justify_items = Some(parsed)),
        "justify-self" => set_if_parse(value, |parsed| style.justify_self = Some(parsed)),
        "overflow" => set_if_parse(value, |parsed| style.overflow = Point { x: parsed, y: parsed }),
        "overflow-x" => set_if_some(parse_overflow(value), |parsed| style.overflow.x = parsed),
        "overflow-y" => set_if_some(parse_overflow(value), |parsed| style.overflow.y = parsed),
        "margin" => set_rect_auto(value, &mut style.margin),
        "margin-left" => set_if_parse(value, |parsed| style.margin.left = parsed),
        "margin-right" => set_if_parse(value, |parsed| style.margin.right = parsed),
        "margin-top" => set_if_parse(value, |parsed| style.margin.top = parsed),
        "margin-bottom" => set_if_parse(value, |parsed| style.margin.bottom = parsed),
        "margin-block-start" => set_logical_rect(
            value,
            &mut style.margin,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "margin-block-end" => set_logical_rect(
            value,
            &mut style.margin,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "margin-inline-start" => set_logical_rect(
            value,
            &mut style.margin,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "margin-inline-end" => set_logical_rect(
            value,
            &mut style.margin,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "padding" => set_rect(value, &mut style.padding),
        "padding-left" => set_if_parse(value, |parsed| style.padding.left = parsed),
        "padding-right" => set_if_parse(value, |parsed| style.padding.right = parsed),
        "padding-top" => set_if_parse(value, |parsed| style.padding.top = parsed),
        "padding-bottom" => set_if_parse(value, |parsed| style.padding.bottom = parsed),
        "padding-block-start" => set_logical_rect(
            value,
            &mut style.padding,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "padding-block-end" => set_logical_rect(
            value,
            &mut style.padding,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "padding-inline-start" => set_logical_rect(
            value,
            &mut style.padding,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "padding-inline-end" => set_logical_rect(
            value,
            &mut style.padding,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-width" | "border" => {
            if let Some(width) = parse_border_width(value) {
                style.border = Rect { left: width, right: width, top: width, bottom: width };
            }
            if let Some(color) = parse_color(value) {
                render_style.border_color = Rect { left: color, right: color, top: color, bottom: color };
            }
        }
        "border-left-width" => set_if_some(parse_border_width(value), |parsed| style.border.left = parsed),
        "border-right-width" => set_if_some(parse_border_width(value), |parsed| style.border.right = parsed),
        "border-top-width" => set_if_some(parse_border_width(value), |parsed| style.border.top = parsed),
        "border-bottom-width" => set_if_some(parse_border_width(value), |parsed| style.border.bottom = parsed),
        "border-block-start-width" => set_logical_border_width(
            value,
            &mut style.border,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-block-end-width" => set_logical_border_width(
            value,
            &mut style.border,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-inline-start-width" => set_logical_border_width(
            value,
            &mut style.border,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-inline-end-width" => set_logical_border_width(
            value,
            &mut style.border,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-left-style" => render_style.border_visible.left = border_style_is_visible(value),
        "border-right-style" => render_style.border_visible.right = border_style_is_visible(value),
        "border-top-style" => render_style.border_visible.top = border_style_is_visible(value),
        "border-bottom-style" => render_style.border_visible.bottom = border_style_is_visible(value),
        "border-block-start-style" => set_logical_border_visibility(
            value,
            &mut render_style.border_visible,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-block-end-style" => set_logical_border_visibility(
            value,
            &mut render_style.border_visible,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-inline-start-style" => set_logical_border_visibility(
            value,
            &mut render_style.border_visible,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-inline-end-style" => set_logical_border_visibility(
            value,
            &mut render_style.border_visible,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-color" => {
            if let Some(color) = parse_color(value) {
                render_style.border_color = Rect { left: color, right: color, top: color, bottom: color };
            }
        }
        "border-left-color" => set_if_some(parse_color(value), |color| render_style.border_color.left = color),
        "border-right-color" => set_if_some(parse_color(value), |color| render_style.border_color.right = color),
        "border-top-color" => set_if_some(parse_color(value), |color| render_style.border_color.top = color),
        "border-bottom-color" => set_if_some(parse_color(value), |color| render_style.border_color.bottom = color),
        "border-block-start-color" => set_logical_color(
            value,
            &mut render_style.border_color,
            LogicalSide::BlockStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-block-end-color" => set_logical_color(
            value,
            &mut render_style.border_color,
            LogicalSide::BlockEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-inline-start-color" => set_logical_color(
            value,
            &mut render_style.border_color,
            LogicalSide::InlineStart,
            render_style.writing_mode,
            render_style.direction,
        ),
        "border-inline-end-color" => set_logical_color(
            value,
            &mut render_style.border_color,
            LogicalSide::InlineEnd,
            render_style.writing_mode,
            render_style.direction,
        ),
        "background" | "background-color" => render_style.background = parse_color(value),
        "color" => {
            if let Some(color) = parse_color(value) {
                render_style.color = color;
            }
        }
        "writing-mode" => {
            if let Some(writing_mode) = parse_writing_mode(value) {
                render_style.writing_mode = writing_mode;
            }
        }
        "gap" => set_if_parse(value, |parsed| style.gap = Size { width: parsed, height: parsed }),
        "column-gap" => set_if_parse(value, |parsed| style.gap.width = parsed),
        "row-gap" => set_if_parse(value, |parsed| style.gap.height = parsed),
        _ => {}
    }
}

pub fn typed_initial_value(
    property_id: &lightningcss::properties::PropertyId<'_>,
    property_name: &str,
    keyword: &str,
) -> Option<Property<'static>> {
    let uses_initial = keyword == "initial" || keyword == "unset" && !is_inherited_property(property_name);
    if !uses_initial {
        return None;
    }
    let initial = initial_property_value(property_name)?;
    let property = Property::parse_string(
        property_id.clone().into_owned(),
        initial,
        CssParserOptions { error_recovery: false, ..CssParserOptions::default() },
    )
    .ok()?;
    (!matches!(property, Property::Unparsed(_) | Property::Custom(_))).then_some(property)
}

pub fn is_css_wide_keyword(value: &str) -> bool {
    matches!(value, "initial" | "inherit" | "unset" | "revert" | "revert-layer")
}

pub fn is_inherited_property(property: &str) -> bool {
    matches!(property, "color" | "direction" | "font-size" | "white-space" | "writing-mode")
}

pub fn apply_inherited_value(
    style: &mut Style,
    render_style: &mut RenderStyle,
    property: &str,
    inherited: &RenderStyle,
) {
    match property {
        "color" => render_style.color = inherited.color,
        "direction" => {
            style.direction = inherited.direction;
            render_style.direction = inherited.direction;
        }
        "font-size" => render_style.font_size = inherited.font_size,
        "white-space" => render_style.white_space_nowrap = inherited.white_space_nowrap,
        "writing-mode" => render_style.writing_mode = inherited.writing_mode,
        _ => {}
    }
}

pub fn initial_property_value(property: &str) -> Option<&'static str> {
    Some(match property {
        "display" => "block",
        "position" => "static",
        "box-sizing" => "content-box",
        "direction" => "ltr",
        "float" | "clear" => "none",
        "width" | "height" | "min-width" | "min-height" | "inline-size" | "block-size" | "min-inline-size"
        | "min-block-size" | "flex-basis" => "auto",
        "max-width" | "max-height" | "max-inline-size" | "max-block-size" => "auto",
        "aspect-ratio" => "auto",
        "top" | "right" | "bottom" | "left" | "inset-block-start" | "inset-block-end" | "inset-inline-start"
        | "inset-inline-end" => "auto",
        "flex-direction" => "row",
        "flex-wrap" => "nowrap",
        "flex-grow" => "0",
        "flex-shrink" => "1",
        "overflow-x" | "overflow-y" => "visible",
        "margin-left"
        | "margin-right"
        | "margin-top"
        | "margin-bottom"
        | "margin-block-start"
        | "margin-block-end"
        | "margin-inline-start"
        | "margin-inline-end"
        | "padding-left"
        | "padding-right"
        | "padding-top"
        | "padding-bottom"
        | "padding-block-start"
        | "padding-block-end"
        | "padding-inline-start"
        | "padding-inline-end"
        | "border-left-width"
        | "border-right-width"
        | "border-top-width"
        | "border-bottom-width"
        | "border-block-start-width"
        | "border-block-end-width"
        | "border-inline-start-width"
        | "border-inline-end-width"
        | "row-gap"
        | "column-gap" => "0",
        "border-left-style"
        | "border-right-style"
        | "border-top-style"
        | "border-bottom-style"
        | "border-block-start-style"
        | "border-block-end-style"
        | "border-inline-start-style"
        | "border-inline-end-style" => "none",
        "background-color" => "transparent",
        "color" => "black",
        "font-size" => "medium",
        "white-space" => "normal",
        "writing-mode" => "horizontal-tb",
        "grid-template-rows" | "grid-template-columns" | "grid-template-areas" => "none",
        "grid-auto-rows" | "grid-auto-columns" => "auto",
        "grid-auto-flow" => "row",
        "grid-row-start" | "grid-row-end" | "grid-column-start" | "grid-column-end" => "auto",
        _ => return None,
    })
}

pub fn declaration_font_size(declaration: &Declaration, parent_font_size: f32) -> Option<f32> {
    match declaration.parsed.as_ref() {
        Some(Property::FontSize(value)) => css_font_size(value, parent_font_size),
        _ => match declaration.value.as_str() {
            "initial" => Some(16.0),
            "inherit" | "unset" => Some(parent_font_size),
            _ => None,
        },
    }
}

pub fn declaration_direction(declaration: &Declaration) -> Option<gummy::Direction> {
    match declaration.parsed.as_ref() {
        Some(Property::Direction(lightningcss::properties::text::Direction::Ltr)) => Some(gummy::Direction::Ltr),
        Some(Property::Direction(lightningcss::properties::text::Direction::Rtl)) => Some(gummy::Direction::Rtl),
        _ if declaration.value == "initial" => Some(gummy::Direction::Ltr),
        _ => None,
    }
}

pub fn css_font_size(value: &lightningcss::properties::font::FontSize, parent_font_size: f32) -> Option<f32> {
    use lightningcss::properties::font::{AbsoluteFontSize, FontSize, RelativeFontSize};

    Some(match value {
        FontSize::Length(value) => css_length_percentage_to_px(value, parent_font_size)?,
        FontSize::Absolute(value) => match value {
            AbsoluteFontSize::XXSmall => 9.0,
            AbsoluteFontSize::XSmall => 10.0,
            AbsoluteFontSize::Small => 13.0,
            AbsoluteFontSize::Medium => 16.0,
            AbsoluteFontSize::Large => 18.0,
            AbsoluteFontSize::XLarge => 24.0,
            AbsoluteFontSize::XXLarge => 32.0,
            AbsoluteFontSize::XXXLarge => 48.0,
        },
        FontSize::Relative(RelativeFontSize::Smaller) => parent_font_size * 0.8,
        FontSize::Relative(RelativeFontSize::Larger) => parent_font_size * 1.2,
    })
}

pub fn css_length_percentage_to_px(value: &CssLengthPercentage, font_size: f32) -> Option<f32> {
    match value {
        DimensionPercentage::Dimension(value) => css_length_to_px(value, font_size),
        DimensionPercentage::Percentage(value) => Some(font_size * value.0),
        DimensionPercentage::Calc(_) => None,
    }
}

pub fn css_length_to_px(value: &LengthValue, font_size: f32) -> Option<f32> {
    if let Some(px) = value.to_px() {
        return Some(px);
    }

    let (value, unit) = value.to_unit_value();
    let factor = match unit {
        "em" | "rem" | "ch" | "ic" | "lh" | "rlh" => font_size,
        "ex" | "cap" => font_size / 2.0,
        "vw" | "svw" | "lvw" | "dvw" | "vi" => VIEWPORT_WIDTH as f32 / 100.0,
        "vh" | "svh" | "lvh" | "dvh" | "vb" => VIEWPORT_HEIGHT as f32 / 100.0,
        "vmin" | "svmin" | "lvmin" | "dvmin" => VIEWPORT_WIDTH.min(VIEWPORT_HEIGHT) as f32 / 100.0,
        "vmax" | "svmax" | "lvmax" | "dvmax" => VIEWPORT_WIDTH.max(VIEWPORT_HEIGHT) as f32 / 100.0,
        _ => return None,
    };
    Some(value * factor)
}

pub fn css_length_percentage(value: &CssLengthPercentage, font_size: f32) -> Option<LengthPercentage> {
    match value {
        DimensionPercentage::Dimension(value) => css_length_to_px(value, font_size).map(LengthPercentage::length),
        DimensionPercentage::Percentage(value) => Some(LengthPercentage::percent(value.0)),
        DimensionPercentage::Calc(_) => None,
    }
}

pub fn css_length_percentage_auto(
    value: &lightningcss::values::length::LengthPercentageOrAuto,
    font_size: f32,
) -> Option<LengthPercentageAuto> {
    use lightningcss::values::length::LengthPercentageOrAuto;
    match value {
        LengthPercentageOrAuto::Auto => Some(LengthPercentageAuto::auto()),
        LengthPercentageOrAuto::LengthPercentage(value) => css_length_percentage(value, font_size).map(Into::into),
    }
}

pub fn css_dimension(value: &lightningcss::properties::size::Size, font_size: f32) -> Option<Dimension> {
    use lightningcss::properties::size::Size;
    match value {
        Size::Auto => Some(Dimension::auto()),
        Size::LengthPercentage(value) => css_length_percentage(value, font_size).map(Into::into),
        Size::MinContent(_)
        | Size::MaxContent(_)
        | Size::FitContent(_)
        | Size::FitContentFunction(_)
        | Size::Stretch(_)
        | Size::Contain => Some(Dimension::auto()),
    }
}

pub fn css_max_dimension(value: &lightningcss::properties::size::MaxSize, font_size: f32) -> Option<Dimension> {
    use lightningcss::properties::size::MaxSize;
    match value {
        MaxSize::LengthPercentage(value) => css_length_percentage(value, font_size).map(Into::into),
        MaxSize::None
        | MaxSize::MinContent(_)
        | MaxSize::MaxContent(_)
        | MaxSize::FitContent(_)
        | MaxSize::FitContentFunction(_)
        | MaxSize::Stretch(_)
        | MaxSize::Contain => Some(Dimension::auto()),
    }
}

pub fn css_border_width(
    value: &lightningcss::properties::border::BorderSideWidth,
    font_size: f32,
) -> Option<LengthPercentage> {
    use lightningcss::{properties::border::BorderSideWidth, values::length::Length};
    match value {
        BorderSideWidth::Thin => Some(LengthPercentage::length(1.0)),
        BorderSideWidth::Medium => Some(LengthPercentage::length(3.0)),
        BorderSideWidth::Thick => Some(LengthPercentage::length(5.0)),
        BorderSideWidth::Length(Length::Value(value)) => {
            css_length_to_px(value, font_size).map(LengthPercentage::length)
        }
        BorderSideWidth::Length(Length::Calc(_)) => None,
    }
}

pub fn css_color(value: &CssColor) -> Option<Color> {
    let CssColor::RGBA(color) = value.clone().to_rgb().ok()? else {
        return None;
    };

    Some(Color::from_rgba8(color.red, color.green, color.blue, color.alpha))
}

pub fn resolve_css_color(value: &CssColor, current_color: Color) -> Option<Color> {
    if matches!(value, CssColor::CurrentColor) { Some(current_color) } else { css_color(value) }
}

pub fn css_gap(value: &lightningcss::properties::align::GapValue, font_size: f32) -> Option<LengthPercentage> {
    match value {
        lightningcss::properties::align::GapValue::Normal => Some(LengthPercentage::ZERO),
        lightningcss::properties::align::GapValue::LengthPercentage(value) => css_length_percentage(value, font_size),
    }
}

pub fn css_border_style_visible(value: &lightningcss::properties::border::LineStyle) -> bool {
    !matches!(
        value,
        lightningcss::properties::border::LineStyle::None | lightningcss::properties::border::LineStyle::Hidden
    )
}

pub fn css_non_auto_length(
    value: &lightningcss::values::length::LengthPercentageOrAuto,
    font_size: f32,
) -> Option<LengthPercentage> {
    use lightningcss::values::length::LengthPercentageOrAuto;
    match value {
        LengthPercentageOrAuto::Auto => None,
        LengthPercentageOrAuto::LengthPercentage(value) => css_length_percentage(value, font_size),
    }
}

pub fn css_display(value: &lightningcss::properties::display::Display) -> Display {
    use lightningcss::properties::display::{Display as CssDisplay, DisplayInside, DisplayKeyword};
    match value {
        CssDisplay::Keyword(DisplayKeyword::None) => Display::None,
        CssDisplay::Pair(pair) => match pair.inside {
            DisplayInside::Flex(_) | DisplayInside::Box(_) | DisplayInside::Table => Display::Flex,
            DisplayInside::Grid => Display::Grid,
            _ => Display::Block,
        },
        CssDisplay::Keyword(_) => Display::Block,
    }
}

pub fn css_display_is_inline(value: &lightningcss::properties::display::Display) -> bool {
    use lightningcss::properties::display::{Display as CssDisplay, DisplayOutside};
    matches!(value, CssDisplay::Pair(pair) if pair.outside == DisplayOutside::Inline)
}

pub fn css_display_is_table(value: &lightningcss::properties::display::Display) -> bool {
    use lightningcss::properties::display::{Display as CssDisplay, DisplayInside};
    matches!(value, CssDisplay::Pair(pair) if pair.inside == DisplayInside::Table)
}

pub fn css_position(value: &lightningcss::properties::position::Position) -> gummy::Position {
    use lightningcss::properties::position::Position;
    match value {
        Position::Absolute | Position::Fixed => gummy::Position::Absolute,
        Position::Static | Position::Relative | Position::Sticky(_) => gummy::Position::Relative,
    }
}

pub fn css_overflow(value: &lightningcss::properties::overflow::OverflowKeyword) -> gummy::Overflow {
    use lightningcss::properties::overflow::OverflowKeyword;
    match value {
        OverflowKeyword::Visible => gummy::Overflow::Visible,
        OverflowKeyword::Clip => gummy::Overflow::Clip,
        OverflowKeyword::Hidden => gummy::Overflow::Hidden,
        OverflowKeyword::Scroll | OverflowKeyword::Auto => gummy::Overflow::Scroll,
    }
}

pub fn css_flex_direction(value: &lightningcss::properties::flex::FlexDirection) -> gummy::FlexDirection {
    use lightningcss::properties::flex::FlexDirection;
    match value {
        FlexDirection::Row => gummy::FlexDirection::Row,
        FlexDirection::RowReverse => gummy::FlexDirection::RowReverse,
        FlexDirection::Column => gummy::FlexDirection::Column,
        FlexDirection::ColumnReverse => gummy::FlexDirection::ColumnReverse,
    }
}

pub fn css_flex_wrap(value: &lightningcss::properties::flex::FlexWrap) -> gummy::FlexWrap {
    use lightningcss::properties::flex::FlexWrap;
    match value {
        FlexWrap::NoWrap => gummy::FlexWrap::NoWrap,
        FlexWrap::Wrap => gummy::FlexWrap::Wrap,
        FlexWrap::WrapReverse => gummy::FlexWrap::WrapReverse,
    }
}

pub fn css_alignment_safety(
    value: Option<&lightningcss::properties::align::OverflowPosition>,
) -> gummy::AlignmentSafety {
    match value {
        Some(lightningcss::properties::align::OverflowPosition::Safe) => gummy::AlignmentSafety::Safe,
        Some(lightningcss::properties::align::OverflowPosition::Unsafe) | None => gummy::AlignmentSafety::Unsafe,
    }
}

pub fn css_self_position(value: &lightningcss::properties::align::SelfPosition) -> gummy::AlignItemsKeyword {
    use lightningcss::properties::align::SelfPosition;
    match value {
        SelfPosition::Center => gummy::AlignItemsKeyword::Center,
        SelfPosition::Start | SelfPosition::SelfStart => gummy::AlignItemsKeyword::Start,
        SelfPosition::End | SelfPosition::SelfEnd => gummy::AlignItemsKeyword::End,
        SelfPosition::FlexStart => gummy::AlignItemsKeyword::FlexStart,
        SelfPosition::FlexEnd => gummy::AlignItemsKeyword::FlexEnd,
    }
}

pub fn css_align_items(value: &lightningcss::properties::align::AlignItems) -> gummy::AlignItems {
    use lightningcss::properties::align::AlignItems;
    match value {
        AlignItems::Normal | AlignItems::Stretch => gummy::AlignItems::STRETCH,
        AlignItems::BaselinePosition(_) => gummy::AlignItems::BASELINE,
        AlignItems::SelfPosition { overflow, value } => {
            gummy::AlignItems { keyword: css_self_position(value), safety: css_alignment_safety(overflow.as_ref()) }
        }
    }
}

pub fn css_align_self(value: &lightningcss::properties::align::AlignSelf) -> Option<gummy::AlignSelf> {
    use lightningcss::properties::align::AlignSelf;
    Some(match value {
        AlignSelf::Auto => return None,
        AlignSelf::Normal | AlignSelf::Stretch => gummy::AlignSelf::STRETCH,
        AlignSelf::BaselinePosition(_) => gummy::AlignSelf::BASELINE,
        AlignSelf::SelfPosition { overflow, value } => {
            gummy::AlignSelf { keyword: css_self_position(value), safety: css_alignment_safety(overflow.as_ref()) }
        }
    })
}

pub fn css_justify_items(value: &lightningcss::properties::align::JustifyItems) -> gummy::JustifyItems {
    use lightningcss::properties::align::{JustifyItems, LegacyJustify};
    match value {
        JustifyItems::Normal | JustifyItems::Stretch => gummy::JustifyItems::STRETCH,
        JustifyItems::BaselinePosition(_) => gummy::JustifyItems::BASELINE,
        JustifyItems::SelfPosition { overflow, value } => {
            gummy::JustifyItems { keyword: css_self_position(value), safety: css_alignment_safety(overflow.as_ref()) }
        }
        JustifyItems::Left { overflow } => gummy::JustifyItems {
            keyword: gummy::AlignItemsKeyword::Start,
            safety: css_alignment_safety(overflow.as_ref()),
        },
        JustifyItems::Right { overflow } => gummy::JustifyItems {
            keyword: gummy::AlignItemsKeyword::End,
            safety: css_alignment_safety(overflow.as_ref()),
        },
        JustifyItems::Legacy(LegacyJustify::Left) => gummy::JustifyItems::START,
        JustifyItems::Legacy(LegacyJustify::Right) => gummy::JustifyItems::END,
        JustifyItems::Legacy(LegacyJustify::Center) => gummy::JustifyItems::CENTER,
    }
}

pub fn css_justify_self(value: &lightningcss::properties::align::JustifySelf) -> Option<gummy::JustifySelf> {
    use lightningcss::properties::align::JustifySelf;
    Some(match value {
        JustifySelf::Auto => return None,
        JustifySelf::Normal | JustifySelf::Stretch => gummy::JustifySelf::STRETCH,
        JustifySelf::BaselinePosition(_) => gummy::JustifySelf::BASELINE,
        JustifySelf::SelfPosition { overflow, value } => {
            gummy::JustifySelf { keyword: css_self_position(value), safety: css_alignment_safety(overflow.as_ref()) }
        }
        JustifySelf::Left { overflow } => gummy::JustifySelf {
            keyword: gummy::AlignItemsKeyword::Start,
            safety: css_alignment_safety(overflow.as_ref()),
        },
        JustifySelf::Right { overflow } => gummy::JustifySelf {
            keyword: gummy::AlignItemsKeyword::End,
            safety: css_alignment_safety(overflow.as_ref()),
        },
    })
}

pub fn css_content_position(value: &lightningcss::properties::align::ContentPosition) -> gummy::AlignContentKeyword {
    use lightningcss::properties::align::ContentPosition;
    match value {
        ContentPosition::Center => gummy::AlignContentKeyword::Center,
        ContentPosition::Start => gummy::AlignContentKeyword::Start,
        ContentPosition::End => gummy::AlignContentKeyword::End,
        ContentPosition::FlexStart => gummy::AlignContentKeyword::FlexStart,
        ContentPosition::FlexEnd => gummy::AlignContentKeyword::FlexEnd,
    }
}

pub fn css_content_distribution(
    value: &lightningcss::properties::align::ContentDistribution,
) -> gummy::AlignContentKeyword {
    use lightningcss::properties::align::ContentDistribution;
    match value {
        ContentDistribution::SpaceBetween => gummy::AlignContentKeyword::SpaceBetween,
        ContentDistribution::SpaceAround => gummy::AlignContentKeyword::SpaceAround,
        ContentDistribution::SpaceEvenly => gummy::AlignContentKeyword::SpaceEvenly,
        ContentDistribution::Stretch => gummy::AlignContentKeyword::Stretch,
    }
}

pub fn css_align_content(value: &lightningcss::properties::align::AlignContent) -> gummy::AlignContent {
    use lightningcss::properties::align::AlignContent;
    match value {
        AlignContent::Normal => gummy::AlignContent::STRETCH,
        AlignContent::BaselinePosition(_) => gummy::AlignContent::START,
        AlignContent::ContentDistribution(value) => {
            gummy::AlignContent { keyword: css_content_distribution(value), safety: gummy::AlignmentSafety::Unsafe }
        }
        AlignContent::ContentPosition { overflow, value } => gummy::AlignContent {
            keyword: css_content_position(value),
            safety: css_alignment_safety(overflow.as_ref()),
        },
    }
}

pub fn css_justify_content(value: &lightningcss::properties::align::JustifyContent) -> gummy::JustifyContent {
    use lightningcss::properties::align::JustifyContent;
    match value {
        JustifyContent::Normal => gummy::JustifyContent::STRETCH,
        JustifyContent::ContentDistribution(value) => {
            gummy::JustifyContent { keyword: css_content_distribution(value), safety: gummy::AlignmentSafety::Unsafe }
        }
        JustifyContent::ContentPosition { overflow, value } => gummy::JustifyContent {
            keyword: css_content_position(value),
            safety: css_alignment_safety(overflow.as_ref()),
        },
        JustifyContent::Left { overflow } => gummy::JustifyContent {
            keyword: gummy::AlignContentKeyword::Start,
            safety: css_alignment_safety(overflow.as_ref()),
        },
        JustifyContent::Right { overflow } => gummy::JustifyContent {
            keyword: gummy::AlignContentKeyword::End,
            safety: css_alignment_safety(overflow.as_ref()),
        },
    }
}

pub fn css_track_breadth(
    value: &lightningcss::properties::grid::TrackBreadth,
    font_size: f32,
) -> Option<TrackSizingFunction> {
    use lightningcss::properties::grid::TrackBreadth;
    Some(match value {
        TrackBreadth::Length(value) => css_length_percentage(value, font_size)?.into(),
        TrackBreadth::Flex(value) => TrackSizingFunction::from_fr(*value),
        TrackBreadth::MinContent => TrackSizingFunction::MIN_CONTENT,
        TrackBreadth::MaxContent => TrackSizingFunction::MAX_CONTENT,
        TrackBreadth::Auto => TrackSizingFunction::AUTO,
    })
}

pub fn css_min_track_breadth(
    value: &lightningcss::properties::grid::TrackBreadth,
    font_size: f32,
) -> Option<MinTrackSizingFunction> {
    use lightningcss::properties::grid::TrackBreadth;
    Some(match value {
        TrackBreadth::Length(value) => css_length_percentage(value, font_size)?.into(),
        TrackBreadth::MinContent => MinTrackSizingFunction::min_content(),
        TrackBreadth::MaxContent => MinTrackSizingFunction::max_content(),
        TrackBreadth::Auto => MinTrackSizingFunction::auto(),
        TrackBreadth::Flex(_) => return None,
    })
}

pub fn css_max_track_breadth(
    value: &lightningcss::properties::grid::TrackBreadth,
    font_size: f32,
) -> Option<MaxTrackSizingFunction> {
    use lightningcss::properties::grid::TrackBreadth;
    Some(match value {
        TrackBreadth::Length(value) => css_length_percentage(value, font_size)?.into(),
        TrackBreadth::Flex(value) => MaxTrackSizingFunction::from_fr(*value),
        TrackBreadth::MinContent => MaxTrackSizingFunction::min_content(),
        TrackBreadth::MaxContent => MaxTrackSizingFunction::max_content(),
        TrackBreadth::Auto => MaxTrackSizingFunction::auto(),
    })
}

pub fn css_track_size(
    value: &lightningcss::properties::grid::TrackSize,
    font_size: f32,
) -> Option<TrackSizingFunction> {
    use lightningcss::properties::grid::TrackSize;
    Some(match value {
        TrackSize::TrackBreadth(value) => css_track_breadth(value, font_size)?,
        TrackSize::MinMax { min, max } => TrackSizingFunction {
            min: css_min_track_breadth(min, font_size)?,
            max: css_max_track_breadth(max, font_size)?,
        },
        TrackSize::FitContent(value) => TrackSizingFunction::fit_content(css_length_percentage(value, font_size)?),
    })
}

pub fn css_line_names<S: CheapCloneStr>(values: &[lightningcss::values::ident::CustomIdentList<'_>]) -> Vec<Vec<S>> {
    values.iter().map(|names| names.iter().map(|name| S::from(name.0.as_ref())).collect()).collect()
}

pub fn set_css_grid_template<S: CheapCloneStr>(
    value: &lightningcss::properties::grid::TrackSizing<'_>,
    font_size: f32,
    tracks: &mut Vec<GridTemplateComponent<S>>,
    line_names: &mut Vec<Vec<S>>,
) {
    use lightningcss::properties::grid::{RepeatCount, TrackListItem, TrackSizing};
    let TrackSizing::TrackList(list) = value else {
        tracks.clear();
        line_names.clear();
        return;
    };

    let parsed = list
        .items
        .iter()
        .map(|item| match item {
            TrackListItem::TrackSize(value) => css_track_size(value, font_size).map(GridTemplateComponent::Single),
            TrackListItem::TrackRepeat(value) => {
                let count = match value.count {
                    RepeatCount::Number(value) => RepetitionCount::Count(u16::try_from(value).ok()?),
                    RepeatCount::AutoFill => RepetitionCount::AutoFill,
                    RepeatCount::AutoFit => RepetitionCount::AutoFit,
                };
                let tracks = value
                    .track_sizes
                    .iter()
                    .map(|value| css_track_size(value, font_size))
                    .collect::<Option<Vec<_>>>()?;
                Some(GridTemplateComponent::Repeat(GridTemplateRepetition {
                    count,
                    tracks,
                    line_names: css_line_names(&value.line_names),
                }))
            }
        })
        .collect::<Option<Vec<_>>>();
    if let Some(parsed) = parsed {
        *tracks = parsed;
        *line_names = css_line_names(&list.line_names);
    }
}

pub fn css_auto_tracks(
    value: &lightningcss::properties::grid::TrackSizeList,
    font_size: f32,
) -> Option<Vec<TrackSizingFunction>> {
    value.0.iter().map(|value| css_track_size(value, font_size)).collect()
}

pub fn css_grid_auto_flow(value: lightningcss::properties::grid::GridAutoFlow) -> gummy::GridAutoFlow {
    use lightningcss::properties::grid::GridAutoFlow;
    match (value.contains(GridAutoFlow::Column), value.contains(GridAutoFlow::Dense)) {
        (false, false) => gummy::GridAutoFlow::Row,
        (true, false) => gummy::GridAutoFlow::Column,
        (false, true) => gummy::GridAutoFlow::RowDense,
        (true, true) => gummy::GridAutoFlow::ColumnDense,
    }
}

pub fn css_grid_line<S: CheapCloneStr>(value: &lightningcss::properties::grid::GridLine<'_>) -> GridPlacement<S> {
    use lightningcss::properties::grid::GridLine;
    match value {
        GridLine::Auto => GridPlacement::Auto,
        GridLine::Area { name } => GridPlacement::NamedLine(S::from(name.0.as_ref()), 0),
        GridLine::Line { index, name: Some(name) } => GridPlacement::NamedLine(S::from(name.0.as_ref()), *index as i16),
        GridLine::Line { index, name: None } => GridPlacement::from_line_index(*index as i16),
        GridLine::Span { index, name: Some(name) } => {
            GridPlacement::NamedSpan(S::from(name.0.as_ref()), (*index).max(0) as u16)
        }
        GridLine::Span { index, name: None } => GridPlacement::Span((*index).max(0) as u16),
    }
}

pub fn set_css_grid_template_areas<S: CheapCloneStr>(
    value: &lightningcss::properties::grid::GridTemplateAreas,
    target: &mut Vec<GridTemplateArea<S>>,
) {
    use lightningcss::properties::grid::GridTemplateAreas;
    let GridTemplateAreas::Areas { columns, areas } = value else {
        target.clear();
        return;
    };
    let columns = *columns as usize;
    if columns == 0 || areas.len() % columns != 0 {
        return;
    }

    #[derive(Clone, Copy)]
    struct Bounds {
        row_start: usize,
        row_end: usize,
        column_start: usize,
        column_end: usize,
    }

    let mut bounds = HashMap::<&str, Bounds>::new();
    for (index, name) in areas.iter().enumerate() {
        let Some(name) = name.as_deref() else {
            continue;
        };
        let row = index / columns;
        let column = index % columns;
        bounds
            .entry(name)
            .and_modify(|bounds| {
                bounds.row_start = bounds.row_start.min(row);
                bounds.row_end = bounds.row_end.max(row + 1);
                bounds.column_start = bounds.column_start.min(column);
                bounds.column_end = bounds.column_end.max(column + 1);
            })
            .or_insert(Bounds { row_start: row, row_end: row + 1, column_start: column, column_end: column + 1 });
    }

    if bounds.iter().any(|(name, bounds)| {
        (bounds.row_start..bounds.row_end).any(|row| {
            (bounds.column_start..bounds.column_end)
                .any(|column| areas[row * columns + column].as_deref() != Some(*name))
        })
    }) {
        return;
    }

    let mut parsed = bounds
        .into_iter()
        .map(|(name, bounds)| GridTemplateArea {
            name: S::from(name),
            row_start: (bounds.row_start + 1) as u16,
            row_end: (bounds.row_end + 1) as u16,
            column_start: (bounds.column_start + 1) as u16,
            column_end: (bounds.column_end + 1) as u16,
        })
        .collect::<Vec<_>>();
    parsed.sort_by(|left, right| left.name.as_ref().cmp(right.name.as_ref()));
    *target = parsed;
}

pub fn set_logical_size_value<T>(value: T, writing_mode: WritingMode, inline_axis: bool, size: &mut Size<T>) {
    if writing_mode.is_vertical() == inline_axis {
        size.height = value;
    } else {
        size.width = value;
    }
}

pub fn set_logical_length_auto(
    value: &lightningcss::values::length::LengthPercentageOrAuto,
    font_size: f32,
    rect: &mut Rect<LengthPercentageAuto>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    if let Some(value) = css_length_percentage_auto(value, font_size) {
        set_rect_side(rect, physical_side(side, writing_mode, direction), value);
    }
}

pub fn set_logical_non_auto_length(
    value: &lightningcss::values::length::LengthPercentageOrAuto,
    font_size: f32,
    rect: &mut Rect<LengthPercentage>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    if let Some(value) = css_non_auto_length(value, font_size) {
        set_rect_side(rect, physical_side(side, writing_mode, direction), value);
    }
}

pub fn set_logical_border_width_value(
    value: &lightningcss::properties::border::BorderSideWidth,
    font_size: f32,
    rect: &mut Rect<LengthPercentage>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    if let Some(value) = css_border_width(value, font_size) {
        set_rect_side(rect, physical_side(side, writing_mode, direction), value);
    }
}

pub fn set_logical_border_visibility_value(
    value: &lightningcss::properties::border::LineStyle,
    rect: &mut Rect<bool>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    set_rect_side(rect, physical_side(side, writing_mode, direction), css_border_style_visible(value));
}

pub fn set_logical_color_value(
    value: &CssColor,
    current_color: Color,
    rect: &mut Rect<Color>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    if let Some(value) = resolve_css_color(value, current_color) {
        set_rect_side(rect, physical_side(side, writing_mode, direction), value);
    }
}

pub fn set_if_some<T>(value: Option<T>, set: impl FnOnce(T)) {
    if let Some(value) = value {
        set(value);
    }
}

pub fn parse_display(value: &str) -> Option<Display> {
    value.parse().ok().or_else(|| match value.trim() {
        "inline-flex" | "inline flex" => Some(Display::Flex),
        "inline-grid" | "inline grid" => Some(Display::Grid),
        "inline" | "inline-block" | "flow-root" | "list-item" | "table" => Some(Display::Block),
        _ => None,
    })
}

pub fn parse_position(value: &str) -> Option<gummy::Position> {
    value.parse().ok().or_else(|| match value.trim() {
        "static" | "sticky" => Some(gummy::Position::Relative),
        "fixed" => Some(gummy::Position::Absolute),
        _ => None,
    })
}

pub fn parse_overflow(value: &str) -> Option<gummy::Overflow> {
    value.parse().ok().or_else(|| value.trim().eq_ignore_ascii_case("auto").then_some(gummy::Overflow::Scroll))
}

pub fn parse_writing_mode(value: &str) -> Option<WritingMode> {
    match value.trim() {
        "horizontal-tb" => Some(WritingMode::HorizontalTb),
        "vertical-rl" | "sideways-rl" => Some(WritingMode::VerticalRl),
        "vertical-lr" | "sideways-lr" => Some(WritingMode::VerticalLr),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub enum LogicalSide {
    BlockStart,
    BlockEnd,
    InlineStart,
    InlineEnd,
}

#[derive(Clone, Copy)]
pub enum PhysicalSide {
    Top,
    Right,
    Bottom,
    Left,
}

pub fn physical_side(side: LogicalSide, writing_mode: WritingMode, direction: gummy::Direction) -> PhysicalSide {
    use LogicalSide::{BlockEnd, BlockStart, InlineEnd, InlineStart};
    use PhysicalSide::{Bottom, Left, Right, Top};
    use gummy::Direction::{Ltr, Rtl};

    match (writing_mode, direction, side) {
        (WritingMode::HorizontalTb, _, BlockStart) => Top,
        (WritingMode::HorizontalTb, _, BlockEnd) => Bottom,
        (WritingMode::HorizontalTb, Ltr, InlineStart) | (WritingMode::HorizontalTb, Rtl, InlineEnd) => Left,
        (WritingMode::HorizontalTb, Ltr, InlineEnd) | (WritingMode::HorizontalTb, Rtl, InlineStart) => Right,
        (WritingMode::VerticalRl, _, BlockStart) => Right,
        (WritingMode::VerticalRl, _, BlockEnd) => Left,
        (WritingMode::VerticalLr, _, BlockStart) => Left,
        (WritingMode::VerticalLr, _, BlockEnd) => Right,
        (WritingMode::VerticalRl | WritingMode::VerticalLr, Ltr, InlineStart)
        | (WritingMode::VerticalRl | WritingMode::VerticalLr, Rtl, InlineEnd) => Top,
        (WritingMode::VerticalRl | WritingMode::VerticalLr, Ltr, InlineEnd)
        | (WritingMode::VerticalRl | WritingMode::VerticalLr, Rtl, InlineStart) => Bottom,
    }
}

pub fn set_rect_side<T>(rect: &mut Rect<T>, side: PhysicalSide, value: T) {
    match side {
        PhysicalSide::Top => rect.top = value,
        PhysicalSide::Right => rect.right = value,
        PhysicalSide::Bottom => rect.bottom = value,
        PhysicalSide::Left => rect.left = value,
    }
}

pub fn set_logical_rect<T: std::str::FromStr>(
    value: &str,
    rect: &mut Rect<T>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    if let Some(value) = parse_style_value(value) {
        set_rect_side(rect, physical_side(side, writing_mode, direction), value);
    }
}

pub fn set_logical_border_width(
    value: &str,
    rect: &mut Rect<LengthPercentage>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    if let Some(value) = parse_border_width(value) {
        set_rect_side(rect, physical_side(side, writing_mode, direction), value);
    }
}

pub fn set_logical_color(
    value: &str,
    rect: &mut Rect<Color>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    if let Some(value) = parse_color(value) {
        set_rect_side(rect, physical_side(side, writing_mode, direction), value);
    }
}

pub fn set_logical_border_visibility(
    value: &str,
    rect: &mut Rect<bool>,
    side: LogicalSide,
    writing_mode: WritingMode,
    direction: gummy::Direction,
) {
    set_rect_side(rect, physical_side(side, writing_mode, direction), border_style_is_visible(value));
}

pub fn border_style_is_visible(value: &str) -> bool {
    !matches!(value.trim(), "none" | "hidden")
}

pub fn finalize_border_widths(style: &mut Style, render_style: &RenderStyle) {
    if !render_style.border_visible.left {
        style.border.left = LengthPercentage::ZERO;
    }
    if !render_style.border_visible.right {
        style.border.right = LengthPercentage::ZERO;
    }
    if !render_style.border_visible.top {
        style.border.top = LengthPercentage::ZERO;
    }
    if !render_style.border_visible.bottom {
        style.border.bottom = LengthPercentage::ZERO;
    }
}

pub fn set_logical_size<T: std::str::FromStr>(
    value: &str,
    writing_mode: WritingMode,
    inline_axis: bool,
    size: &mut Size<T>,
) {
    let Some(value) = parse_style_value(value) else {
        return;
    };
    if writing_mode.is_vertical() == inline_axis {
        size.height = value;
    } else {
        size.width = value;
    }
}

pub fn parse_aspect_ratio(value: &str) -> Option<Option<f32>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") || value.eq_ignore_ascii_case("none") {
        return Some(None);
    }

    let ratio = value.strip_prefix("auto ").unwrap_or(value);
    let (numerator, denominator) = ratio.split_once('/').unwrap_or((ratio, "1"));
    let numerator = numerator.trim().parse::<f32>().ok()?;
    let denominator = denominator.trim().parse::<f32>().ok()?;
    (numerator.is_finite() && denominator.is_finite() && numerator >= 0.0 && denominator > 0.0)
        .then_some(Some(numerator / denominator))
}

pub fn set_if_parse<T: std::str::FromStr>(value: &str, set: impl FnOnce(T)) {
    if let Some(parsed) = parse_style_value(value) {
        set(parsed);
    }
}

pub fn parse_style_value<T: std::str::FromStr>(value: &str) -> Option<T> {
    value.parse::<T>().ok().or_else(|| is_unitless_zero(value).then(|| "0px".parse::<T>().ok()).flatten())
}

pub fn is_unitless_zero(value: &str) -> bool {
    value.trim().parse::<f32>().is_ok_and(|number| number == 0.0)
}

pub fn set_rect(value: &str, rect: &mut Rect<LengthPercentage>) {
    if let Some([top, right, bottom, left]) = parse_box_values::<LengthPercentage>(value) {
        *rect = Rect { left, right, top, bottom };
    }
}

pub fn set_rect_auto(value: &str, rect: &mut Rect<LengthPercentageAuto>) {
    if let Some([top, right, bottom, left]) = parse_box_values::<LengthPercentageAuto>(value) {
        *rect = Rect { left, right, top, bottom };
    }
}

pub fn parse_box_values<T: Copy + std::str::FromStr>(value: &str) -> Option<[T; 4]> {
    let parts = value.split_ascii_whitespace().map(parse_style_value).collect::<Option<Vec<_>>>()?;
    match parts.as_slice() {
        [all] => Some([*all, *all, *all, *all]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left] => Some([*top, *right, *bottom, *left]),
        _ => None,
    }
}

pub fn parse_border_width(value: &str) -> Option<LengthPercentage> {
    value.split_ascii_whitespace().find_map(|part| match part {
        "thin" => "1px".parse().ok(),
        "medium" => "3px".parse().ok(),
        "thick" => "5px".parse().ok(),
        _ => parse_style_value(part),
    })
}

pub fn parse_color(value: &str) -> Option<Color> {
    let CssColor::RGBA(color) = CssColor::parse_string(value.trim()).ok()?.to_rgb().ok()? else {
        return None;
    };
    Some(Color::from_rgba8(color.red, color.green, color.blue, color.alpha))
}
