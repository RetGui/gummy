use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use parley::{PositionedLayoutItem, layout::AlignmentOptions};

use vello_cpu::{Image, ImageSource, Pixmap, RenderContext, Resources, kurbo};

use crate::parse::parse_and_layout_with_path;
use crate::{AhemFont, AhemTextLayout, Color, Document, ImageMeasureData, NodeContext, WritingMode};
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
