use std::path::Path;

use lightningcss::{
    properties::Property,
    values::{
        angle::AnglePercentage,
        gradient::{Circle, Ellipse, EndingShape, Gradient, GradientItem, RadialGradient},
        image::Image,
        length::{Length, LengthPercentage},
        position::{Position, PositionComponent},
    },
};
use scraper::{ElementRef, Html, Selector};

use crate::Declaration;
use crate::paint::{read_html_document, resolve_angle_percentage, resolve_length_percentage_px};
use crate::parse::{active_declarations_with_path, css_length_to_px, media_attribute_matches};

pub fn reason_for_pair(test: &Path, reference: &Path, wpt_dir: &Path) -> Option<String> {
    reason_for_test(test, wpt_dir).or_else(|| reason_for_reference(reference))
}

pub fn reason_for_test(test: &Path, wpt_dir: &Path) -> Option<String> {
    reason_for_path("Test", test, wpt_dir).or_else(|| reason_for_file("Test", test))
}

pub fn reason_for_reference(reference: &Path) -> Option<String> {
    reason_for_file("Reference", reference)
}

fn reason_for_file(role: &str, path: &Path) -> Option<String> {
    let html = read_html_document(path).ok()?;
    reason_for_document_with_path(role, &html, Some(path))
}

fn reason_for_path(role: &str, path: &Path, wpt_dir: &Path) -> Option<String> {
    let relative = path.strip_prefix(wpt_dir).unwrap_or(path);
    let components = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let css_index = components.iter().position(|component| component == "css")?;
    let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    let is_print_reftest =
        file_name.contains("-print.") || components.iter().skip(css_index + 1).any(|component| component == "print");
    if is_print_reftest {
        return Some(format!(
            "{role} is a WPT print reftest and requires paged-media fragment trees, which the screen layout runner does not provide."
        ));
    }
    let suite = components.get(css_index + 1)?.as_str();

    let scope = match suite {
        "compositing"
        | "css-backgrounds"
        | "css-color"
        | "css-color-adjust"
        | "css-color-hdr"
        | "css-forced-color-adjust"
        | "css-highlight-api"
        | "css-image-animation"
        | "css-images"
        | "css-masking"
        | "css-motion-path"
        | "css-scrollbars"
        | "css-shadow"
        | "css-text-decor"
        | "css-transforms"
        | "css-view-transitions"
        | "css-will-change"
        | "fill-stroke"
        | "filter-effects"
        | "motion" => "CSS painting or compositing, not box layout",
        "css-animations" | "css-easing" | "css-transitions" => {
            "an animation timeline, which the static runner does not provide"
        }
        "css-font-loading" | "css-fonts" | "css-size-adjust" | "woff2" => {
            "font loading and glyph shaping, which are delegated by the layout engine"
        }
        "css-speech" => "speech output, which a layout engine does not provide",
        "css-content" | "css-pseudo" => {
            "browser-generated content and pseudo-element trees, which are not supplied to the layout engine"
        }
        "css-forms" => "browser-provided native controls and user-interface painting",
        "css-overscroll-behavior" | "css-scroll-anchoring" | "css-scroll-snap" => {
            "browser scrolling state and interaction, which the static runner does not provide"
        }
        "css-round-display" | "css-viewport" => {
            "browser display-device or viewport configuration outside the fixed 800x600 layout context"
        }
        "css-layout-api" | "css-paint-api" => "a browser worklet runtime, which the layout runner does not provide",
        "css-cascade"
        | "css-conditional"
        | "css-device-adapt"
        | "css-env"
        | "css-link-params"
        | "css-mixins"
        | "css-namespaces"
        | "css-navigation"
        | "css-nesting"
        | "css-parser-api"
        | "css-properties-values-api"
        | "css-style-attr"
        | "css-syntax"
        | "css-typed-om"
        | "css-variables"
        | "cssom"
        | "cssom-view"
        | "fetching"
        | "geometry"
        | "mediaqueries"
        | "selectors" => "browser style-system or DOM APIs rather than layout",
        "css-page" | "css-gcpm" | "printing" => {
            "paged-media and fragment-tree support, which this screen layout runner does not provide"
        }
        "css-break" | "css-multicol" => {
            "fragment-tree layout, which is outside this engine's block/flex/grid algorithms"
        }
        "css-inline" | "css-rhythm" | "css-ruby" | "css-text" => {
            "inline formatting and glyph shaping, which the engine delegates to its measure callback"
        }
        "css-counter-styles" | "css-lists" => {
            "generated counters or list markers, which are outside the box layout engine"
        }
        "css-tables" => "CSS table formatting, which this block/flex/grid layout engine does not implement",
        "css2" => return reason_for_css2_path(role, components.get(css_index + 2).map(String::as_str)),
        _ => return None,
    };

    Some(format!("{role} belongs to {suite}, which requires {scope}."))
}

fn reason_for_css2_path(role: &str, section: Option<&str>) -> Option<String> {
    let section = section?;
    let scope = match section {
        "backgrounds" | "colors" | "stacking-context" | "visufx" | "zindex" | "zorder" => {
            "painting, clipping, or stacking rather than box layout"
        }
        "bidi-text" | "fonts" | "linebox" | "text" => {
            "inline formatting and glyph shaping, which the engine delegates to its measure callback"
        }
        "generate" | "generated-content" | "lists" => {
            "generated content or list markers, which are outside the box layout engine"
        }
        "page-box" | "pagination" => {
            "paged-media and fragment-tree support, which this screen layout runner does not provide"
        }
        "tables" => "CSS table formatting, which this block/flex/grid layout engine does not implement",
        "cascade" | "cascade-import" | "i18n" | "media" | "other-formats" | "selectors" | "syntax" | "ui" => {
            "browser parsing, style-system, or UI behavior rather than layout"
        }
        _ => return None,
    };
    Some(format!("{role} belongs to CSS2/{section}, which requires {scope}."))
}

fn reason_for_document_with_path(role: &str, html: &str, source_path: Option<&Path>) -> Option<String> {
    let document = Html::parse_document(html);

    if has_selector(&document, ".reftest-wait") {
        return Some(format!(
            "{role} uses reftest-wait and requires JavaScript/DOM execution; the runner renders a static document."
        ));
    }

    let script_selector = Selector::parse("script").unwrap();
    if document.select(&script_selector).any(|script| executable_script(&script)) {
        return Some(format!(
            "{role} contains executable JavaScript; the layout runner renders a static document without a DOM runtime."
        ));
    }

    if document
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
        .any(|element| element.value().attrs().any(|(name, _)| is_event_handler_attribute(name)))
    {
        return Some(format!(
            "{role} uses an event handler and requires browser event/DOM execution; the runner is static."
        ));
    }

    if let Some((flag, requirement)) = unsupported_wpt_flag(&document) {
        return Some(format!("{role} declares the WPT '{flag}' flag and requires {requirement}."));
    }

    if has_remote_stylesheet(&document) {
        return Some(format!(
            "{role} requires a remote or data stylesheet; the runner only resolves local WPT stylesheets."
        ));
    }
    if has_selector(&document, "base[href]") {
        return Some(format!(
            "{role} changes the document base URL, which requires browser URL resolution not provided by the runner."
        ));
    }

    for tag in ["picture", "svg", "canvas", "video", "audio", "iframe", "object", "embed", "math"] {
        if has_selector(&document, tag) {
            return Some(format!(
                "{role} uses <{tag}> and requires replaced-element intrinsic sizing or resource rendering outside the layout engine."
            ));
        }
    }

    for tag in ["input", "select", "textarea", "button", "fieldset", "meter", "progress", "details", "dialog"] {
        if has_selector(&document, tag) {
            return Some(format!(
                "{role} uses <{tag}> and requires browser-provided native control layout or painting."
            ));
        }
    }

    for tag in ["table", "caption", "colgroup", "thead", "tbody", "tfoot", "tr", "td", "th"] {
        if has_selector(&document, tag) {
            return Some(format!(
                "{role} uses <{tag}> and requires CSS table formatting, which this block/flex/grid layout engine does not implement."
            ));
        }
    }

    let embedded_css = strip_css_comments(&document_css(&document)).to_ascii_lowercase();
    if embedded_css.split("@import").skip(1).any(|import| {
        let import = import.split(';').next().unwrap_or(import);
        import.contains("http://")
            || import.contains("https://")
            || import.contains("url(//")
            || import.contains("url('//")
            || import.contains("url(\"//")
            || import.contains("data:")
    }) {
        return Some(format!(
            "{role} requires a remote or data @import stylesheet; the runner only resolves local WPT stylesheets."
        ));
    }
    if embedded_css.contains("@font-face") {
        return Some(format!(
            "{role} uses @font-face and requires font loading and glyph shaping outside the box layout engine."
        ));
    }

    let declarations = active_declarations_with_path(html, source_path).unwrap_or_default();
    if declarations.iter().any(|declaration| declaration.value.to_ascii_lowercase().contains("var(")) {
        return Some(format!(
            "{role} uses CSS custom-property substitution, which belongs to the browser style system rather than the layout engine."
        ));
    }
    if declaration_values(&declarations, "display").any(|value| {
        value
            .split_ascii_whitespace()
            .any(|keyword| keyword == "table" || keyword.starts_with("table-") || keyword == "inline-table")
    }) {
        return Some(format!(
            "{role} requires CSS table formatting, which this block/flex/grid layout engine does not implement."
        ));
    }
    if property_has_non_initial_value(&declarations, "animation-name", &["none"]) {
        return Some(format!("{role} requires animation timing, which the static layout runner does not provide."));
    }

    if let Some(feature) = unsupported_paint_feature(&declarations) {
        return Some(format!(
            "{role} uses '{feature}', which requires paint, clipping, or compositing behavior outside the layout engine."
        ));
    }

    None
}

fn executable_script(script: &ElementRef<'_>) -> bool {
    let Some(script_type) = script.value().attr("type") else {
        return true;
    };
    matches!(
        script_type.trim().to_ascii_lowercase().as_str(),
        "" | "module" | "text/javascript" | "application/javascript" | "text/ecmascript" | "application/ecmascript"
    )
}

fn is_event_handler_attribute(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "onabort"
            | "onanimationcancel"
            | "onanimationend"
            | "onanimationiteration"
            | "onanimationstart"
            | "onbeforeinput"
            | "onbeforetoggle"
            | "onblur"
            | "oncancel"
            | "onchange"
            | "onclick"
            | "onclose"
            | "oncontextmenu"
            | "oncopy"
            | "oncut"
            | "ondblclick"
            | "ondrag"
            | "ondragend"
            | "ondragenter"
            | "ondragleave"
            | "ondragover"
            | "ondragstart"
            | "ondrop"
            | "onerror"
            | "onfocus"
            | "oninput"
            | "oninvalid"
            | "onkeydown"
            | "onkeypress"
            | "onkeyup"
            | "onload"
            | "onmousedown"
            | "onmouseenter"
            | "onmouseleave"
            | "onmousemove"
            | "onmouseout"
            | "onmouseover"
            | "onmouseup"
            | "onpaste"
            | "onpointercancel"
            | "onpointerdown"
            | "onpointerenter"
            | "onpointerleave"
            | "onpointermove"
            | "onpointerout"
            | "onpointerover"
            | "onpointerup"
            | "onreset"
            | "onresize"
            | "onscroll"
            | "onsubmit"
            | "ontoggle"
            | "ontouchcancel"
            | "ontouchend"
            | "ontouchmove"
            | "ontouchstart"
            | "ontransitioncancel"
            | "ontransitionend"
            | "ontransitionrun"
            | "ontransitionstart"
            | "onwheel"
    )
}

fn unsupported_wpt_flag(document: &Html) -> Option<(String, &'static str)> {
    let selector = Selector::parse("meta[name]").unwrap();
    for meta in document.select(&selector) {
        if !meta.value().attr("name").is_some_and(|name| name.eq_ignore_ascii_case("flags")) {
            continue;
        }
        for flag in meta.value().attr("content").unwrap_or_default().split_ascii_whitespace() {
            let requirement = match flag.to_ascii_lowercase().as_str() {
                "animated" => "an animation clock",
                "dom" => "JavaScript and DOM mutation",
                "history" => "browser session history",
                "http" => "the WPT HTTP server and browser URL semantics",
                "interact" => "human interaction and browser event handling",
                "namespace" | "nonhtml" => "XML/XHTML namespace parsing; this runner parses documents as HTML",
                "paged" => "a paged-media layout context",
                "speech" => "speech output",
                "svg" => "SVG layout and vector painting",
                "userstyle" => "a browser user stylesheet",
                "font" => "an OS-installed font and real glyph shaping",
                _ => continue,
            };
            return Some((flag.to_string(), requirement));
        }
    }
    None
}

fn has_remote_stylesheet(document: &Html) -> bool {
    let selector = Selector::parse("link[href]").unwrap();
    document.select(&selector).any(|link| {
        let rel = link.value().attr("rel").unwrap_or_default();
        let has_relation =
            |expected: &str| rel.split_ascii_whitespace().any(|item| item.eq_ignore_ascii_case(expected));
        let media_matches =
            link.value().attr("media").filter(|media| !media.trim().is_empty()).is_none_or(media_attribute_matches);
        let stylesheet = has_relation("stylesheet")
            && !has_relation("alternate")
            && link.value().attr("disabled").is_none()
            && media_matches;
        let href = link.value().attr("href").unwrap_or_default().trim().to_ascii_lowercase();
        stylesheet
            && (href.starts_with("http://")
                || href.starts_with("https://")
                || href.starts_with("//")
                || href.starts_with("data:"))
    })
}

fn document_css(document: &Html) -> String {
    let mut css = String::new();
    let style_selector = Selector::parse("style").unwrap();
    for style in document.select(&style_selector) {
        css.push_str(&style.text().collect::<String>());
        css.push('\n');
    }
    let inline_selector = Selector::parse("[style]").unwrap();
    for element in document.select(&inline_selector) {
        css.push('{');
        css.push_str(element.value().attr("style").unwrap_or_default());
        css.push_str("}\n");
    }
    css
}

fn strip_css_comments(css: &str) -> String {
    let mut cleaned = String::with_capacity(css.len());
    let mut remainder = css;
    while let Some(start) = remainder.find("/*") {
        cleaned.push_str(&remainder[..start]);
        let Some(end) = remainder[start + 2..].find("*/") else {
            return cleaned;
        };
        remainder = &remainder[start + 2 + end + 2..];
    }
    cleaned.push_str(remainder);
    cleaned
}

fn unsupported_paint_feature(declarations: &[Declaration]) -> Option<&'static str> {
    if declarations
        .iter()
        .filter(|declaration| declaration.property == "background-image")
        .any(|declaration| !supported_background_image(declaration))
    {
        return Some("background-image");
    }

    let features: &[(&str, &[&str])] = &[
        ("background-position-x", &["0%", "left", "inherit"]),
        ("background-position-y", &["0%", "top", "inherit"]),
        ("background-size", &["auto", "auto auto", "inherit"]),
        ("background-repeat", &["repeat", "repeat repeat", "inherit"]),
        ("background-attachment", &["scroll", "inherit"]),
        ("background-origin", &["padding-box", "inherit"]),
        ("background-clip", &["border-box", "inherit"]),
        ("transform", &["none"]),
        ("perspective", &["none"]),
        ("filter", &["none"]),
        ("backdrop-filter", &["none"]),
        ("clip", &["auto"]),
        ("clip-path", &["none"]),
        ("mask", &["none"]),
        ("mask-image", &["none"]),
        ("visibility", &["visible"]),
        ("content", &["normal", "none"]),
    ];
    for (property, initial_values) in features {
        if property_has_non_initial_value(declarations, property, initial_values) {
            return Some(property);
        }
    }
    
    if declaration_values(declarations, "position").any(|value| value.trim() == "sticky") {
        return Some("sticky positioning");
    }
    None
}

fn supported_background_image(declaration: &Declaration) -> bool {
    match declaration.parsed.as_ref() {
        Some(Property::BackgroundImage(images)) => images.iter().all(|image| match image {
            Image::None => true,
            Image::Gradient(gradient) => supported_gradient(gradient),
            Image::Url(url) => supported_background_url(url.url.as_ref()),
            Image::ImageSet(_) => false,
        }),
        _ => css_value_is_initial(&declaration.value) || matches!(declaration.value.trim(), "none" | "inherit"),
    }
}

fn supported_background_url(url: &str) -> bool {
    let url = url.trim().to_ascii_lowercase();
    if url.starts_with("data:") {
        return url.starts_with("data:image/png;")
            || url.starts_with("data:image/png,")
            || url.starts_with("data:image/svg+xml;")
            || url.starts_with("data:image/svg+xml,");
    }
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") {
        return false;
    }
    let path = url.split(['?', '#']).next().unwrap_or(&url);
    path.ends_with(".png") || path.ends_with(".svg")
}

fn supported_gradient(gradient: &Gradient) -> bool {
    match gradient {
        Gradient::Linear(gradient) | Gradient::RepeatingLinear(gradient) => supported_length_stops(&gradient.items),
        Gradient::Radial(gradient) | Gradient::RepeatingRadial(gradient) => supported_radial_gradient(gradient),
        Gradient::Conic(gradient) | Gradient::RepeatingConic(gradient) => {
            supported_position(&gradient.position) && supported_angle_stops(&gradient.items)
        }
        Gradient::WebKitGradient(_) => false,
    }
}

fn supported_radial_gradient(gradient: &RadialGradient) -> bool {
    let shape_supported = match &gradient.shape {
        EndingShape::Circle(Circle::Radius(Length::Calc(_))) => false,
        EndingShape::Circle(Circle::Radius(Length::Value(radius))) => css_length_to_px(radius, 16.0).is_some(),
        EndingShape::Circle(Circle::Extent(_)) => true,
        EndingShape::Ellipse(Ellipse::Size { x, y }) => {
            supported_length_percentage(x) && supported_length_percentage(y)
        }
        EndingShape::Ellipse(Ellipse::Extent(_)) => true,
    };
    shape_supported && supported_position(&gradient.position) && supported_radial_stops(&gradient.items)
}

fn supported_length_stops(items: &[GradientItem<LengthPercentage>]) -> bool {
    supported_stops(items, supported_length_percentage)
}

fn supported_radial_stops(items: &[GradientItem<LengthPercentage>]) -> bool {
    supported_stops(items, |position| {
        resolve_length_percentage_px(position, 100.0, 16.0).is_some_and(|position| position >= 0.0)
    })
}

fn supported_angle_stops(items: &[GradientItem<AnglePercentage>]) -> bool {
    supported_stops(items, |position| resolve_angle_percentage(position).is_some())
}

fn supported_stops<D>(items: &[GradientItem<D>], supported_position: impl Fn(&D) -> bool) -> bool {
    items.iter().all(|item| match item {
        GradientItem::ColorStop(stop) => stop.position.as_ref().is_none_or(&supported_position),
        GradientItem::Hint(_) => false,
    })
}

fn supported_position(position: &Position) -> bool {
    supported_position_component(&position.x) && supported_position_component(&position.y)
}

fn supported_position_component<S>(position: &PositionComponent<S>) -> bool {
    match position {
        PositionComponent::Center => true,
        PositionComponent::Length(position) => supported_length_percentage(position),
        PositionComponent::Side { offset, .. } => offset.as_ref().is_none_or(supported_length_percentage),
    }
}

fn supported_length_percentage(value: &LengthPercentage) -> bool {
    resolve_length_percentage_px(value, 100.0, 16.0).is_some()
}

fn property_has_non_initial_value(declarations: &[Declaration], property: &str, initial_values: &[&str]) -> bool {
    declaration_values(declarations, property).any(|value| {
        let value = value.trim().trim_end_matches("!important").trim();
        !css_value_is_initial(value) && !initial_values.contains(&value)
    })
}

fn declaration_values<'a>(declarations: &'a [Declaration], property: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    declarations
        .iter()
        .filter(move |declaration| declaration.property == property)
        .map(|declaration| declaration.value.as_str())
}

fn css_value_is_initial(value: &str) -> bool {
    matches!(value.trim(), "initial" | "unset" | "revert" | "revert-layer")
}

fn has_selector(document: &Html, selector: &str) -> bool {
    document.select(&Selector::parse(selector).unwrap()).next().is_some()
}
