mod common {
    pub mod image;
    pub mod text;
}
use common::image::{image_measure_function, ImageContext};
use common::text::{text_measure_function, FontMetrics, TextContext, WritingMode, LOREM_IPSUM};
use gummy::prelude::*;

enum NodeContext {
    Text(TextContext),
    Image(ImageContext),
}

fn measure_function(
    known_dimensions: gummy::geometry::Size<Option<f32>>,
    available_space: gummy::geometry::Size<gummy::style::AvailableSpace>,
    node_context: Option<&mut NodeContext>,
    font_metrics: &FontMetrics,
) -> Size<f32> {
    if let Size { width: Some(width), height: Some(height) } = known_dimensions {
        return Size { width, height };
    }

    match node_context {
        None => Size::ZERO,
        Some(NodeContext::Text(text_context)) => {
            text_measure_function(known_dimensions, available_space, &*text_context, font_metrics)
        }
        Some(NodeContext::Image(image_context)) => image_measure_function(known_dimensions, image_context),
    }
}

fn main() -> Result<(), gummy::GummyError> {
    let mut gummy: GummyTree<NodeContext> = GummyTree::new();

    let font_metrics = FontMetrics { char_width: 10.0, char_height: 10.0 };

    let text_node = gummy.new_leaf_with_context(
        Style::default(),
        NodeContext::Text(TextContext { text_content: LOREM_IPSUM.into(), writing_mode: WritingMode::Horizontal }),
    )?;

    let image_node = gummy
        .new_leaf_with_context(Style::default(), NodeContext::Image(ImageContext { width: 400.0, height: 300.0 }))?;

    let root = gummy.new_with_children(
        Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            size: Size { width: length(200.0), height: auto() },
            ..Default::default()
        },
        &[text_node, image_node],
    )?;

    // Compute layout and print result
    gummy.compute_layout_with_measure(
        root,
        Size::MAX_CONTENT,
        // Note: this closure is a FnMut closure and can be used to borrow external context for the duration of layout
        // For example, you may wish to borrow a global font registry and pass it into your text measuring function
        |known_dimensions, available_space, _node_id, node_context, _style| {
            measure_function(known_dimensions, available_space, node_context, &font_metrics)
        },
    )?;
    gummy.print_tree(root);

    Ok(())
}
