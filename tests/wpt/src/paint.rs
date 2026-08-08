use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;

use vello_cpu::{Image, ImageSource, Pixmap, RenderContext, Resources, kurbo};

use crate::parse::parse_and_layout_with_path;
use crate::{Color, Document, NodeContext, WritingMode};
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

pub fn rasterize_tree(document: &mut Document, node: NodeId, parent_x: f32, parent_y: f32) -> anyhow::Result<()> {
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
        if let Some(text) = &paint.text {
            paint_ahem_text(
                &mut document.renderer,
                x,
                y,
                layout.size.width,
                text,
                paint.font_size,
                paint.writing_mode,
                paint.color,
            );
        }
    } else {
        for child in children {
            rasterize_tree(document, child, x, y)?;
        }
    }

    Ok(())
}

pub fn render_reftest_document(path: &Path) -> anyhow::Result<Vec<u8>> {
    let html = read_html_document(path)?;
    let mut document = parse_and_layout_with_path(&html, Some(path))?;
    let root = document.root;
    rasterize_tree(&mut document, root, 0.0, 0.0)?;

    let mut pixmap = Pixmap::new(document.renderer.width(), document.renderer.height());
    document.renderer.render(&mut pixmap, &mut Resources::new());

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

#[allow(clippy::too_many_arguments)]
pub fn paint_ahem_text(
    renderer: &mut RenderContext,
    x: f32,
    y: f32,
    width: f32,
    text: &str,
    font_size: f32,
    writing_mode: WritingMode,
    color: Color,
) {
    let glyph = font_size;
    let glyphs_per_line = (width / glyph).floor().max(1.0) as usize;
    for (index, ch) in text.chars().enumerate() {
        if ch == '\n' {
            continue;
        }
        let line = index / glyphs_per_line;
        let column = index % glyphs_per_line;
        match writing_mode {
            WritingMode::HorizontalTb => {
                fill_rect(renderer, x + column as f32 * glyph, y + line as f32 * glyph, glyph, glyph, color)
            }
            WritingMode::VerticalRl | WritingMode::VerticalLr => {
                fill_rect(renderer, x + line as f32 * glyph, y + column as f32 * glyph, glyph, glyph, color)
            }
        }
    }
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
    if let Size { width: Some(width), height: Some(height) } = known_dimensions {
        return Size { width, height };
    }

    let Some(context) = context else {
        return known_dimensions.map(|dimension| dimension.unwrap_or(0.0));
    };
    if let Some(image) = context.image_size {
        return match known_dimensions {
            Size { width: Some(width), height: Some(height) } => Size { width, height },
            Size { width: Some(width), height: None } => Size { width, height: width * image.height / image.width },
            Size { width: None, height: Some(height) } => Size { width: height * image.width / image.height, height },
            Size { width: None, height: None } => image,
        };
    }
    let Some(text) = &context.text else {
        return known_dimensions.map(|dimension| dimension.unwrap_or(0.0));
    };

    let glyph = context.font_size;
    let glyph_count = text.chars().count();
    let max_inline = glyph_count as f32 * glyph;
    let min_inline =
        text.split_whitespace().map(|word| word.chars().count()).max().unwrap_or(glyph_count).max(1) as f32 * glyph;
    let inline = known_dimensions
        .width
        .unwrap_or_else(|| match available_space.width {
            AvailableSpace::Definite(width) => width.min(max_inline),
            AvailableSpace::MinContent => min_inline,
            AvailableSpace::MaxContent => max_inline,
        })
        .max(glyph.min(max_inline));
    let glyphs_per_line = (inline / glyph).floor().max(1.0) as usize;
    let line_count = glyph_count.div_ceil(glyphs_per_line).max(1);
    let block = known_dimensions.height.unwrap_or(line_count as f32 * glyph);

    match context.writing_mode {
        WritingMode::HorizontalTb => Size { width: inline, height: block },
        WritingMode::VerticalRl | WritingMode::VerticalLr => Size { width: block, height: inline },
    }
}
