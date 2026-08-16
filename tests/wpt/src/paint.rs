use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use lightningcss::values::{
    angle::AnglePercentage,
    calc::{Calc, MathFunction},
    gradient::{
        Circle, ConicGradient, Ellipse, EndingShape, Gradient as CssGradient, GradientItem, LineDirection,
        LinearGradient, RadialGradient, ShapeExtent,
    },
    length::{Length, LengthPercentage as CssLengthPercentage},
    percentage::DimensionPercentage,
    position::{HorizontalPositionKeyword, Position, PositionComponent, VerticalPositionKeyword},
};
use parley::{PositionedLayoutItem, layout::AlignmentOptions};

use vello_cpu::peniko::{
    ColorStop as VelloColorStop, ColorStops, Extend, Gradient as VelloGradient, ImageSampler, LinearGradientPosition,
    RadialGradientPosition, SweepGradientPosition,
};
use vello_cpu::{Image, ImageSource, Pixmap, RenderContext, Resources, kurbo};

use crate::parse::{css_length_to_px, parse_and_layout_with_path, resolve_css_color};
use crate::{
    AhemFont, AhemTextLayout, BackgroundImage, Color, Document, ImageMeasureData, NodeContext, WritingMode, load_image,
};
use gummy::{AvailableSpace, NodeId, Rect, Size, Style};

pub fn read_html_document(path: &Path) -> anyhow::Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(decode_html_bytes(&bytes))
}

pub fn decode_html_bytes(bytes: &[u8]) -> String {
    if let Some(bytes) = bytes.strip_prefix(&[0xff, 0xfe]) {
        return decode_utf16(bytes, u16::from_le_bytes);
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xfe, 0xff]) {
        return decode_utf16(bytes, u16::from_be_bytes);
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

fn decode_utf16(bytes: &[u8], decode: fn([u8; 2]) -> u16) -> String {
    let code_units = bytes.as_chunks::<2>().0.iter().map(|bytes| decode(*bytes)).collect::<Vec<_>>();
    String::from_utf16_lossy(&code_units)
}

pub fn rasterize_tree(
    document: &mut Document,
    resources: &mut Resources,
    node: NodeId,
    parent_x: f32,
    parent_y: f32,
) -> anyhow::Result<()> {
    let layout = *document.tree.layout(node)?;
    let x = parent_x + layout.location.x;
    let y = parent_y + layout.location.y;
    let paint = document.paint.get(&node).cloned().unwrap_or_default();

    if let Some(background) = paint.background {
        fill_rect(&mut document.renderer, x, y, layout.size.width, layout.size.height, background);
    }
    paint_background_images(
        &mut document.renderer,
        &paint.background_images,
        document.source_path.as_deref(),
        kurbo::Rect::new(x as f64, y as f64, (x + layout.size.width) as f64, (y + layout.size.height) as f64),
        kurbo::Rect::new(
            (x + layout.border.left) as f64,
            (y + layout.border.top) as f64,
            (x + layout.size.width - layout.border.right) as f64,
            (y + layout.size.height - layout.border.bottom) as f64,
        ),
        paint.font_size,
        paint.color,
    )?;
    paint_borders(&mut document.renderer, x, y, &layout, &paint.border_color);
    if let Some(image) = &paint.image {
        draw_image(
            &mut document.renderer,
            image,
            x + layout.border.left + layout.padding.left,
            y + layout.border.top + layout.padding.top,
            layout.content_box_width(),
            layout.content_box_height(),
        );
    }

    let children = document.tree.children(node)?;
    if children.is_empty() {
        if let Some(text) = document.tree.get_node_context(node).and_then(|context| context.text.clone()) {
            paint_parley_text(
                &mut document.renderer,
                resources,
                x + layout.border.left + layout.padding.left,
                y + layout.border.top + layout.padding.top,
                layout.content_box_width(),
                layout.content_box_height(),
                &text,
                paint.color,
            );
        }
    } else {
        for child in children {
            rasterize_tree(document, resources, child, x, y)?;
        }
    }

    Ok(())
}

pub fn render_reftest_document(path: &Path, ahem_font: &AhemFont, browser_font: bool) -> anyhow::Result<Vec<u8>> {
    let html = read_html_document(path)?;
    let mut document = parse_and_layout_with_path(&html, Some(path), ahem_font, browser_font)?;
    let mut resources = Resources::new();
    let root = document.root;
    rasterize_tree(&mut document, &mut resources, root, 0.0, 0.0)?;

    let mut pixmap = Pixmap::new(document.renderer.width(), document.renderer.height());
    document.renderer.render(&mut pixmap, &mut resources);

    Ok(pixmap.data_as_u8_slice().to_vec())
}

pub fn paint_borders(renderer: &mut RenderContext, x: f32, y: f32, layout: &gummy::Layout, color: &Rect<Color>) {
    // The WPT layout runner deliberately flattens image-decorated, dotted,
    // dashed, rounded, and other fancy borders into solid square-cornered
    // rectangles.
    fill_rect(renderer, x, y, layout.size.width, layout.border.top, color.top);
    fill_rect(
        renderer,
        x,
        y + layout.size.height - layout.border.bottom,
        layout.size.width,
        layout.border.bottom,
        color.bottom,
    );
    fill_rect(renderer, x, y, layout.border.left, layout.size.height, color.left);
    fill_rect(
        renderer,
        x + layout.size.width - layout.border.right,
        y,
        layout.border.right,
        layout.size.height,
        color.right,
    );
}

pub fn paint_parley_text(
    renderer: &mut RenderContext,
    resources: &mut Resources,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    text: &AhemTextLayout,
    color: Color,
) {
    let mut layout = text.layout.clone();
    let inline_size = if text.writing_mode.is_vertical() { height } else { width };
    layout.break_all_lines(Some(inline_size.max(0.0)));
    layout.align(Some(inline_size.max(0.0)), text.text_alignment.parley(), AlignmentOptions::default());
    let transform = match text.writing_mode {
        WritingMode::HorizontalTb => kurbo::Affine::translate((x as f64, y as f64)),
        WritingMode::VerticalRl => {
            kurbo::Affine::translate(((x + width) as f64, y as f64))
                * kurbo::Affine::rotate(std::f64::consts::FRAC_PI_2)
        }
        WritingMode::VerticalLr => kurbo::Affine::new([0.0, 1.0, 1.0, 0.0, x as f64, y as f64]),
    };
    renderer.set_transform(transform);
    renderer.set_paint(color);
    renderer.set_aliasing_threshold(Some(128));
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            let glyphs =
                glyph_run.positioned_glyphs().map(|glyph| vello_cpu::Glyph { id: glyph.id, x: glyph.x, y: glyph.y });
            renderer
                .glyph_run(resources, run.font())
                .font_size(run.font_size())
                .normalized_coords(run.normalized_coords())
                .fill_glyphs(glyphs);
        }
    }
    renderer.set_aliasing_threshold(None);
    renderer.reset_transform();
}

pub fn fill_rect(renderer: &mut RenderContext, x: f32, y: f32, width: f32, height: f32, color: Color) {
    renderer.set_paint(color);
    renderer.fill_rect(&kurbo::Rect::new(x as f64, y as f64, (x + width) as f64, (y + height) as f64));
}

fn paint_background_images(
    renderer: &mut RenderContext,
    images: &[BackgroundImage],
    source_path: Option<&Path>,
    clip_rect: kurbo::Rect,
    positioning_rect: kurbo::Rect,
    font_size: f32,
    current_color: Color,
) -> anyhow::Result<()> {
    if clip_rect.width() <= 0.0
        || clip_rect.height() <= 0.0
        || positioning_rect.width() <= 0.0
        || positioning_rect.height() <= 0.0
    {
        return Ok(());
    }

    for image in images.iter().rev() {
        match image {
            BackgroundImage::Gradient(css_gradient) => {
                match prepare_gradient(css_gradient, positioning_rect, font_size, current_color) {
                    Some(PreparedPaint::Gradient { gradient, transform }) => {
                        renderer.set_paint(gradient);
                        renderer.set_paint_transform(transform);
                        renderer.fill_rect(&clip_rect);
                        renderer.reset_paint_transform();
                    }
                    Some(PreparedPaint::Solid(color)) => {
                        renderer.set_paint(color);
                        renderer.fill_rect(&clip_rect);
                    }
                    None => {}
                }
            }
            BackgroundImage::Url(src) => {
                let image = load_image(source_path, src)?;
                paint_repeating_image(renderer, &image, clip_rect, positioning_rect);
            }
        }
    }
    Ok(())
}

fn paint_repeating_image(
    renderer: &mut RenderContext,
    image: &crate::LoadedImage,
    clip_rect: kurbo::Rect,
    positioning_rect: kurbo::Rect,
) {
    if image.pixmap.width() == 0 || image.pixmap.height() == 0 {
        return;
    }
    let width = image.measure.size.width.unwrap_or(image.pixmap.width() as f32);
    let height = image.measure.size.height.unwrap_or(image.pixmap.height() as f32);
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    renderer.set_paint(Image {
        image: ImageSource::Pixmap(image.pixmap.clone()),
        sampler: ImageSampler { x_extend: Extend::Repeat, y_extend: Extend::Repeat, ..Default::default() },
    });
    renderer.set_paint_transform(
        kurbo::Affine::translate((positioning_rect.x0, positioning_rect.y0))
            * kurbo::Affine::scale_non_uniform(
                f64::from(width) / f64::from(image.pixmap.width()),
                f64::from(height) / f64::from(image.pixmap.height()),
            ),
    );
    renderer.fill_rect(&clip_rect);
    renderer.reset_paint_transform();
}

enum PreparedPaint {
    Gradient { gradient: VelloGradient, transform: kurbo::Affine },
    Solid(Color),
}

#[derive(Clone, Copy)]
struct ResolvedStop {
    position: f32,
    color: Color,
}

struct NormalizedStops {
    domain_start: f32,
    domain_end: f32,
    stops: ColorStops,
}

fn prepare_gradient(
    gradient: &CssGradient,
    rect: kurbo::Rect,
    font_size: f32,
    current_color: Color,
) -> Option<PreparedPaint> {
    match gradient {
        CssGradient::Linear(gradient) => prepare_linear_gradient(gradient, false, rect, font_size, current_color),
        CssGradient::RepeatingLinear(gradient) => {
            prepare_linear_gradient(gradient, true, rect, font_size, current_color)
        }
        CssGradient::Radial(gradient) => prepare_radial_gradient(gradient, false, rect, font_size, current_color),
        CssGradient::RepeatingRadial(gradient) => {
            prepare_radial_gradient(gradient, true, rect, font_size, current_color)
        }
        CssGradient::Conic(gradient) => prepare_conic_gradient(gradient, false, rect, font_size, current_color),
        CssGradient::RepeatingConic(gradient) => prepare_conic_gradient(gradient, true, rect, font_size, current_color),
        CssGradient::WebKitGradient(_) => None,
    }
}

fn prepare_linear_gradient(
    gradient: &LinearGradient,
    repeating: bool,
    rect: kurbo::Rect,
    font_size: f32,
    current_color: Color,
) -> Option<PreparedPaint> {
    let (start, end) = linear_gradient_line(&gradient.direction, rect)?;
    let delta = end - start;
    let line_length = delta.hypot() as f32;
    if line_length <= f32::EPSILON {
        return None;
    }

    let stops = resolve_stops(&gradient.items, current_color, |position| {
        resolve_length_percentage(position, line_length, font_size)
    })?;
    let Some(stops) = normalize_stops(stops, repeating) else {
        return solid_from_items(&gradient.items, current_color);
    };
    let adjusted_start = start + delta * f64::from(stops.domain_start);
    let adjusted_end = start + delta * f64::from(stops.domain_end);
    let gradient = VelloGradient {
        kind: LinearGradientPosition { start: adjusted_start, end: adjusted_end }.into(),
        stops: stops.stops,
        extend: if repeating { Extend::Repeat } else { Extend::Pad },
        ..Default::default()
    };
    Some(PreparedPaint::Gradient { gradient, transform: kurbo::Affine::IDENTITY })
}

fn linear_gradient_line(direction: &LineDirection, rect: kurbo::Rect) -> Option<(kurbo::Point, kurbo::Point)> {
    let width = rect.width();
    let height = rect.height();
    let (dx, dy) = match direction {
        LineDirection::Angle(angle) => {
            let angle = f64::from(angle.to_radians());
            (angle.sin(), -angle.cos())
        }
        LineDirection::Horizontal(HorizontalPositionKeyword::Left) => (-1.0, 0.0),
        LineDirection::Horizontal(HorizontalPositionKeyword::Right) => (1.0, 0.0),
        LineDirection::Vertical(VerticalPositionKeyword::Top) => (0.0, -1.0),
        LineDirection::Vertical(VerticalPositionKeyword::Bottom) => (0.0, 1.0),
        LineDirection::Corner { horizontal, vertical } => {
            let dx = match horizontal {
                HorizontalPositionKeyword::Left => -height,
                HorizontalPositionKeyword::Right => height,
            };
            let dy = match vertical {
                VerticalPositionKeyword::Top => -width,
                VerticalPositionKeyword::Bottom => width,
            };
            let length = dx.hypot(dy);
            if length <= f64::EPSILON {
                return None;
            }
            (dx / length, dy / length)
        }
    };
    let line_length = width * dx.abs() + height * dy.abs();
    if !line_length.is_finite() || line_length <= f64::EPSILON {
        return None;
    }
    let center = rect.center();
    let half_delta = kurbo::Vec2::new(dx, dy) * (line_length * 0.5);
    Some((center - half_delta, center + half_delta))
}

fn prepare_radial_gradient(
    gradient: &RadialGradient,
    repeating: bool,
    rect: kurbo::Rect,
    font_size: f32,
    current_color: Color,
) -> Option<PreparedPaint> {
    let center = resolve_position(&gradient.position, rect, font_size)?;
    let (radius_x, radius_y) = radial_radii(&gradient.shape, center, rect, font_size)?;
    if radius_x <= f64::EPSILON || radius_y <= f64::EPSILON {
        return solid_from_items(&gradient.items, current_color);
    }

    let normalized_diagonal = ((radius_x * radius_x + radius_y * radius_y) * 0.5).sqrt() as f32;
    let stops = resolve_stops(&gradient.items, current_color, |position| {
        resolve_length_percentage(position, normalized_diagonal, font_size)
    })?;
    let Some(stops) = normalize_stops(stops, repeating) else {
        return solid_from_items(&gradient.items, current_color);
    };
    if stops.domain_start < 0.0 {
        return None;
    }

    let gradient = VelloGradient {
        kind: RadialGradientPosition {
            start_center: kurbo::Point::ZERO,
            start_radius: stops.domain_start,
            end_center: kurbo::Point::ZERO,
            end_radius: stops.domain_end,
        }
        .into(),
        stops: stops.stops,
        extend: if repeating { Extend::Repeat } else { Extend::Pad },
        ..Default::default()
    };
    let transform =
        kurbo::Affine::translate((center.x, center.y)) * kurbo::Affine::scale_non_uniform(radius_x, radius_y);
    Some(PreparedPaint::Gradient { gradient, transform })
}

fn radial_radii(shape: &EndingShape, center: kurbo::Point, rect: kurbo::Rect, font_size: f32) -> Option<(f64, f64)> {
    let left = (center.x - rect.x0).abs();
    let right = (rect.x1 - center.x).abs();
    let top = (center.y - rect.y0).abs();
    let bottom = (rect.y1 - center.y).abs();
    let closest_x = left.min(right);
    let farthest_x = left.max(right);
    let closest_y = top.min(bottom);
    let farthest_y = top.max(bottom);

    match shape {
        EndingShape::Circle(Circle::Radius(Length::Value(radius))) => {
            let radius = f64::from(css_length_to_px(radius, font_size)?).max(0.0);
            Some((radius, radius))
        }
        EndingShape::Circle(Circle::Radius(Length::Calc(_))) => None,
        EndingShape::Circle(Circle::Extent(extent)) => {
            let radius = match extent {
                ShapeExtent::ClosestSide => closest_x.min(closest_y),
                ShapeExtent::FarthestSide => farthest_x.max(farthest_y),
                ShapeExtent::ClosestCorner => closest_x.hypot(closest_y),
                ShapeExtent::FarthestCorner => farthest_x.hypot(farthest_y),
            };
            Some((radius, radius))
        }
        EndingShape::Ellipse(Ellipse::Size { x, y }) => Some((
            f64::from(resolve_length_percentage_px(x, rect.width() as f32, font_size)?).max(0.0),
            f64::from(resolve_length_percentage_px(y, rect.height() as f32, font_size)?).max(0.0),
        )),
        EndingShape::Ellipse(Ellipse::Extent(extent)) => {
            let (mut radius_x, mut radius_y) = match extent {
                ShapeExtent::ClosestSide | ShapeExtent::ClosestCorner => (closest_x, closest_y),
                ShapeExtent::FarthestSide | ShapeExtent::FarthestCorner => (farthest_x, farthest_y),
            };
            if matches!(extent, ShapeExtent::ClosestCorner | ShapeExtent::FarthestCorner) {
                if radius_x <= f64::EPSILON || radius_y <= f64::EPSILON {
                    return Some((0.0, 0.0));
                }
                let (corner_x, corner_y) = match extent {
                    ShapeExtent::ClosestCorner => (closest_x, closest_y),
                    ShapeExtent::FarthestCorner => (farthest_x, farthest_y),
                    ShapeExtent::ClosestSide | ShapeExtent::FarthestSide => unreachable!(),
                };
                let scale = ((corner_x / radius_x).powi(2) + (corner_y / radius_y).powi(2)).sqrt();
                radius_x *= scale;
                radius_y *= scale;
            }
            Some((radius_x, radius_y))
        }
    }
}

fn prepare_conic_gradient(
    gradient: &ConicGradient,
    repeating: bool,
    rect: kurbo::Rect,
    font_size: f32,
    current_color: Color,
) -> Option<PreparedPaint> {
    let center = resolve_position(&gradient.position, rect, font_size)?;
    let stops = resolve_stops(&gradient.items, current_color, resolve_angle_percentage)?;
    let Some(stops) = normalize_stops(stops, repeating) else {
        return solid_from_items(&gradient.items, current_color);
    };
    let turn = std::f32::consts::TAU;
    let paint = VelloGradient {
        kind: SweepGradientPosition {
            center: kurbo::Point::ZERO,
            start_angle: stops.domain_start * turn,
            end_angle: stops.domain_end * turn,
        }
        .into(),
        stops: stops.stops,
        extend: if repeating { Extend::Repeat } else { Extend::Pad },
        ..Default::default()
    };
    // Vello's zero-angle ray points right. CSS's points up, and positive
    // angles rotate clockwise in the page's y-down coordinate system.
    let rotation = f64::from(gradient.angle.to_radians() - std::f32::consts::FRAC_PI_2);
    let transform = kurbo::Affine::translate((center.x, center.y)) * kurbo::Affine::rotate(rotation);
    Some(PreparedPaint::Gradient { gradient: paint, transform })
}

fn resolve_stops<D>(
    items: &[GradientItem<D>],
    current_color: Color,
    mut resolve_position: impl FnMut(&D) -> Option<f32>,
) -> Option<Vec<ResolvedStop>> {
    if items.iter().any(|item| matches!(item, GradientItem::Hint(_))) {
        return None;
    }
    let mut stops = Vec::new();
    for item in items {
        let GradientItem::ColorStop(stop) = item else { continue };
        let position = match &stop.position {
            Some(position) => Some(resolve_position(position)?),
            None => None,
        };
        stops.push((position, resolve_css_color(&stop.color, current_color)?));
    }
    if stops.len() < 2 {
        return None;
    }

    if stops[0].0.is_none() {
        stops[0].0 = Some(0.0);
    }
    let last = stops.len() - 1;
    if stops[last].0.is_none() {
        stops[last].0 = Some(1.0);
    }

    let mut previous_positioned = 0;
    for index in 1..stops.len() {
        let Some(mut position) = stops[index].0 else { continue };
        if !position.is_finite() {
            return None;
        }
        let previous = stops[previous_positioned].0?;
        position = position.max(previous);
        stops[index].0 = Some(position);
        let interval = index - previous_positioned;
        for offset in 1..interval {
            let fraction = offset as f32 / interval as f32;
            stops[previous_positioned + offset].0 = Some(previous + (position - previous) * fraction);
        }
        previous_positioned = index;
    }

    stops.into_iter().map(|(position, color)| Some(ResolvedStop { position: position?, color })).collect()
}

fn normalize_stops(stops: Vec<ResolvedStop>, repeating: bool) -> Option<NormalizedStops> {
    let first = stops.first()?.position;
    let last = stops.last()?.position;
    let (domain_start, domain_end) = if repeating { (first, last) } else { (first.min(0.0), last.max(1.0)) };
    let span = domain_end - domain_start;
    if !span.is_finite() || span <= f32::EPSILON {
        return None;
    }

    let stops = stops
        .into_iter()
        .map(|stop| VelloColorStop::from((((stop.position - domain_start) / span).clamp(0.0, 1.0), stop.color)))
        .collect::<Vec<_>>();
    Some(NormalizedStops { domain_start, domain_end, stops: ColorStops::from(stops.as_slice()) })
}

fn solid_from_items<D>(items: &[GradientItem<D>], current_color: Color) -> Option<PreparedPaint> {
    items.iter().rev().find_map(|item| match item {
        GradientItem::ColorStop(stop) => resolve_css_color(&stop.color, current_color).map(PreparedPaint::Solid),
        GradientItem::Hint(_) => None,
    })
}

fn resolve_length_percentage(value: &CssLengthPercentage, reference: f32, font_size: f32) -> Option<f32> {
    if reference <= f32::EPSILON {
        return None;
    }
    Some(resolve_length_percentage_px(value, reference, font_size)? / reference)
}

pub(crate) fn resolve_length_percentage_px(value: &CssLengthPercentage, reference: f32, font_size: f32) -> Option<f32> {
    match value {
        DimensionPercentage::Dimension(length) => css_length_to_px(length, font_size),
        DimensionPercentage::Percentage(percentage) => Some(reference * percentage.0),
        DimensionPercentage::Calc(calc) => {
            resolve_calc_expression(calc, &|value| resolve_length_percentage_px(value, reference, font_size))
        }
    }
}

pub(crate) fn resolve_angle_percentage(value: &AnglePercentage) -> Option<f32> {
    match value {
        DimensionPercentage::Dimension(angle) => Some(angle.to_radians() / std::f32::consts::TAU),
        DimensionPercentage::Percentage(percentage) => Some(percentage.0),
        DimensionPercentage::Calc(calc) => resolve_calc_expression(calc, &resolve_angle_percentage),
    }
}

fn resolve_calc_expression<V>(calc: &Calc<V>, resolve_value: &impl Fn(&V) -> Option<f32>) -> Option<f32> {
    let value = match calc {
        Calc::Value(value) => resolve_value(value)?,
        Calc::Number(value) => *value,
        Calc::Sum(left, right) => {
            resolve_calc_expression(left, resolve_value)? + resolve_calc_expression(right, resolve_value)?
        }
        Calc::Product(factor, value) => *factor * resolve_calc_expression(value, resolve_value)?,
        Calc::Function(function) => match function.as_ref() {
            MathFunction::Calc(value) => resolve_calc_expression(value, resolve_value)?,
            _ => return None,
        },
    };
    value.is_finite().then_some(value)
}

fn resolve_position(position: &Position, rect: kurbo::Rect, font_size: f32) -> Option<kurbo::Point> {
    let x = resolve_position_component(&position.x, rect.width() as f32, font_size, HorizontalPositionKeyword::Left)?;
    let y = resolve_position_component(&position.y, rect.height() as f32, font_size, VerticalPositionKeyword::Top)?;
    Some(kurbo::Point::new(rect.x0 + f64::from(x), rect.y0 + f64::from(y)))
}

fn resolve_position_component<S: PartialEq>(
    position: &PositionComponent<S>,
    extent: f32,
    font_size: f32,
    start_side: S,
) -> Option<f32> {
    match position {
        PositionComponent::Center => Some(extent * 0.5),
        PositionComponent::Length(value) => resolve_length_percentage_px(value, extent, font_size),
        PositionComponent::Side { side, offset } => {
            let offset = match offset {
                Some(offset) => resolve_length_percentage_px(offset, extent, font_size)?,
                None => 0.0,
            };
            Some(if *side == start_side { offset } else { extent - offset })
        }
    }
}

pub fn draw_image(renderer: &mut RenderContext, image: &Arc<Pixmap>, x: f32, y: f32, width: f32, height: f32) {
    if width <= 0.0 || height <= 0.0 || image.width() == 0 || image.height() == 0 {
        return;
    }
    renderer.set_paint(Image { image: ImageSource::Pixmap(image.clone()), sampler: Default::default() });
    renderer.set_paint_transform(
        kurbo::Affine::translate((x as f64, y as f64))
            * kurbo::Affine::scale_non_uniform(
                width as f64 / image.width() as f64,
                height as f64 / image.height() as f64,
            ),
    );
    renderer.fill_rect(&kurbo::Rect::new(x as f64, y as f64, (x + width) as f64, (y + height) as f64));
    renderer.reset_paint_transform();
}

pub fn measure_content(
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    _node_id: NodeId,
    context: Option<&mut NodeContext>,
    _style: &Style,
) -> Size<f32> {
    let Some(context) = context else {
        return known_dimensions.map(|dimension| dimension.unwrap_or(0.0));
    };
    if let Some(image) = context.image {
        return measure_image(image, known_dimensions, available_space);
    }
    if let Size { width: Some(width), height: Some(height) } = known_dimensions {
        return Size { width, height };
    }
    let Some(text) = &context.text else {
        return known_dimensions.map(|dimension| dimension.unwrap_or(0.0));
    };

    let (known_inline, known_block, available_inline) = match text.writing_mode {
        WritingMode::HorizontalTb => (known_dimensions.width, known_dimensions.height, available_space.width),
        WritingMode::VerticalRl | WritingMode::VerticalLr => {
            (known_dimensions.height, known_dimensions.width, available_space.height)
        }
    };
    let content_widths = text.layout.calculate_content_widths();
    let inline = known_inline.unwrap_or_else(|| match available_inline {
        AvailableSpace::Definite(width) => width.clamp(content_widths.min, content_widths.max),
        AvailableSpace::MinContent => content_widths.min,
        AvailableSpace::MaxContent => content_widths.max,
    });
    let mut layout = text.layout.clone();
    layout.break_all_lines(Some(inline.max(0.0)));
    let block = known_block.unwrap_or_else(|| layout.height());

    match text.writing_mode {
        WritingMode::HorizontalTb => Size { width: inline, height: block },
        WritingMode::VerticalRl | WritingMode::VerticalLr => Size { width: block, height: inline },
    }
}

fn measure_image(
    image: ImageMeasureData,
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
) -> Size<f32> {
    let intrinsic_measurement = known_dimensions == Size::NONE
        && matches!(available_space.width, AvailableSpace::Definite(0.0))
        && matches!(available_space.height, AvailableSpace::Definite(0.0));
    if intrinsic_measurement {
        return Size::intrinsic(image.size.width, image.size.height, image.aspect_ratio);
    }

    match known_dimensions {
        Size { width: Some(width), height: Some(height) } => Size { width, height },
        Size { width: Some(width), height: None } => {
            Size { width, height: image.aspect_ratio.map(|ratio| width / ratio).or(image.size.height).unwrap_or(0.0) }
        }
        Size { width: None, height: Some(height) } => {
            Size { width: image.aspect_ratio.map(|ratio| height * ratio).or(image.size.width).unwrap_or(0.0), height }
        }
        Size { width: None, height: None } => {
            let size = image.size.maybe_apply_aspect_ratio(image.aspect_ratio);
            Size { width: size.width.unwrap_or(0.0), height: size.height.unwrap_or(0.0) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_intrinsic_measurement_encodes_ratio_only_metadata() {
        let image = ImageMeasureData { size: Size::NONE, aspect_ratio: Some(2.0) };
        let measured = measure_image(
            image,
            Size::NONE,
            Size { width: AvailableSpace::Definite(0.0), height: AvailableSpace::Definite(0.0) },
        );

        assert_eq!(measured.decode_intrinsic_derived(), (None, None, Some(2.0)));
        assert_eq!(measured.width, -2.0);
        assert!(measured.height.is_nan());
    }

    #[test]
    fn ordinary_image_measurement_still_uses_known_axis() {
        let image = ImageMeasureData { size: Size::NONE, aspect_ratio: Some(2.0) };
        let measured = measure_image(
            image,
            Size { width: None, height: Some(25.0) },
            Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent },
        );

        assert_eq!(measured, Size { width: 50.0, height: 25.0 });
    }
}
