use rand::distr::uniform::SampleRange;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use gummy::style::Style as GummyStyle;

use super::{BuildTree, BuildTreeExt, GenStyle};

pub struct Upstream03TreeBuilder<R: Rng, G: GenStyle<GummyStyle>> {
    rng: R,
    style_generator: G,
    tree: upstream_03::Taffy,
    root: upstream_03::prelude::Node,
}

// Implement the BuildTree trait
impl<R: Rng, G: GenStyle<GummyStyle>> BuildTree<R, G> for Upstream03TreeBuilder<R, G> {
    const NAME: &'static str = "Taffy 0.3";
    type Tree = upstream_03::Taffy;
    type Node = upstream_03::prelude::Node;

    fn with_rng(mut rng: R, mut style_generator: G) -> Self {
        let mut tree = upstream_03::Taffy::new();
        let root = tree.new_leaf(convert_style(style_generator.create_root_style(&mut rng))).unwrap();
        Upstream03TreeBuilder { rng, style_generator, tree, root }
    }

    fn compute_layout_inner(&mut self, available_width: Option<f32>, available_height: Option<f32>) {
        let available_space =
            upstream_03::geometry::Size { width: available_width.into(), height: available_height.into() };
        self.tree.compute_layout(self.root, available_space).unwrap();
    }

    fn random_usize(&mut self, range: impl SampleRange<usize>) -> usize {
        self.rng.random_range(range)
    }

    fn create_leaf_node(&mut self) -> Self::Node {
        let style = self.style_generator.create_leaf_style(&mut self.rng);
        self.tree.new_leaf(convert_style(style)).unwrap()
    }

    fn create_container_node(&mut self, children: &[Self::Node]) -> Self::Node {
        let style = self.style_generator.create_container_style(&mut self.rng);
        self.tree.new_with_children(convert_style(style), children).unwrap()
    }

    fn total_node_count(&mut self) -> usize {
        self.tree.total_node_count()
    }

    fn set_root_children(&mut self, children: &[Self::Node]) {
        self.tree.set_children(self.root, children).unwrap();
    }

    fn into_tree_and_root(self) -> (Self::Tree, Self::Node) {
        (self.tree, self.root)
    }
}

impl<G: GenStyle<GummyStyle>> BuildTreeExt<G> for Upstream03TreeBuilder<ChaCha8Rng, G> {}

fn convert_style(style: gummy::style::Style) -> upstream_03::style::Style {
    upstream_03::style::Style {
        display: convert_display(style.display),
        position: convert_position(style.position),
        inset: convert_rect(style.inset, convert_length_percentage_auto),
        margin: convert_rect(style.margin, convert_length_percentage_auto),
        padding: convert_rect(style.padding, convert_length_percentage),
        border: convert_rect(style.border, convert_length_percentage),
        size: convert_size(style.size, convert_dimension),
        min_size: convert_size(style.min_size, convert_dimension),
        max_size: convert_size(style.max_size, convert_dimension),
        aspect_ratio: style.aspect_ratio,
        gap: convert_size(style.gap, convert_length_percentage),
        // Alignment
        align_items: None,
        align_self: None,
        justify_items: None,
        justify_self: None,
        align_content: None,
        justify_content: None,
        // Flexbox
        flex_direction: convert_flex_direction(style.flex_direction),
        flex_wrap: convert_flex_wrap(style.flex_wrap),
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        flex_basis: convert_dimension(style.flex_basis),
        // Grid
        grid_template_rows: Vec::new(),
        grid_template_columns: Vec::new(),
        grid_auto_rows: Vec::new(),
        grid_auto_columns: Vec::new(),
        grid_auto_flow: upstream_03::style::GridAutoFlow::Row,
        grid_row: upstream_03::geometry::Line {
            start: upstream_03::style::GridPlacement::Auto,
            end: upstream_03::style::GridPlacement::Auto,
        },
        grid_column: upstream_03::geometry::Line {
            start: upstream_03::style::GridPlacement::Auto,
            end: upstream_03::style::GridPlacement::Auto,
        },
    }
}

fn convert_rect<T, U, F: Fn(T) -> U>(input: gummy::geometry::Rect<T>, map: F) -> upstream_03::geometry::Rect<U> {
    upstream_03::geometry::Rect {
        left: map(input.left),
        right: map(input.right),
        top: map(input.top),
        bottom: map(input.bottom),
    }
}

fn convert_size<T, U, F: Fn(T) -> U>(input: gummy::geometry::Size<T>, map: F) -> upstream_03::geometry::Size<U> {
    upstream_03::geometry::Size { width: map(input.width), height: map(input.height) }
}

fn convert_point<T, U, F: Fn(T) -> U>(input: gummy::geometry::Point<T>, map: F) -> upstream_03::geometry::Point<U> {
    upstream_03::geometry::Point { x: map(input.x), y: map(input.y) }
}

fn convert_dimension(input: gummy::style::Dimension) -> upstream_03::style::Dimension {
    let raw = input.into_raw();
    match raw.tag() {
        gummy::style::CompactLength::LENGTH_TAG => upstream_03::style::Dimension::Points(raw.value()),
        gummy::style::CompactLength::PERCENT_TAG => upstream_03::style::Dimension::Percent(raw.value()),
        gummy::style::CompactLength::AUTO_TAG => upstream_03::style::Dimension::Auto,
        _ => panic!("unsupported Dimension variant"),
    }
}

fn convert_length_percentage_auto(input: gummy::style::LengthPercentageAuto) -> upstream_03::style::LengthPercentageAuto {
    let raw = input.into_raw();
    match raw.tag() {
        gummy::style::CompactLength::LENGTH_TAG => upstream_03::style::LengthPercentageAuto::Points(raw.value()),
        gummy::style::CompactLength::PERCENT_TAG => upstream_03::style::LengthPercentageAuto::Percent(raw.value()),
        gummy::style::CompactLength::AUTO_TAG => upstream_03::style::LengthPercentageAuto::Auto,
        _ => panic!("unsupported LengthPercentageAuto variant"),
    }
}

fn convert_length_percentage(input: gummy::style::LengthPercentage) -> upstream_03::style::LengthPercentage {
    let raw = input.into_raw();
    match raw.tag() {
        gummy::style::CompactLength::LENGTH_TAG => upstream_03::style::LengthPercentage::Points(raw.value()),
        gummy::style::CompactLength::PERCENT_TAG => upstream_03::style::LengthPercentage::Percent(raw.value()),
        _ => panic!("unsupported LengthPercentage variant"),
    }
}

fn convert_display(input: gummy::style::Display) -> upstream_03::style::Display {
    match input {
        gummy::style::Display::None => upstream_03::style::Display::None,
        gummy::style::Display::Flex => upstream_03::style::Display::Flex,
        gummy::style::Display::Grid => upstream_03::style::Display::Grid,
        gummy::style::Display::Block => panic!("Block layout not implemented in Taffy 0.3"),
    }
}

fn convert_position(input: gummy::style::Position) -> upstream_03::style::Position {
    match input {
        gummy::style::Position::Relative => upstream_03::style::Position::Relative,
        gummy::style::Position::Absolute => upstream_03::style::Position::Absolute,
    }
}

fn convert_flex_direction(input: gummy::style::FlexDirection) -> upstream_03::style::FlexDirection {
    match input {
        gummy::style::FlexDirection::Row => upstream_03::style::FlexDirection::Row,
        gummy::style::FlexDirection::Column => upstream_03::style::FlexDirection::Column,
        gummy::style::FlexDirection::RowReverse => upstream_03::style::FlexDirection::RowReverse,
        gummy::style::FlexDirection::ColumnReverse => upstream_03::style::FlexDirection::ColumnReverse,
    }
}

fn convert_flex_wrap(input: gummy::style::FlexWrap) -> upstream_03::style::FlexWrap {
    match input {
        gummy::style::FlexWrap::NoWrap => upstream_03::style::FlexWrap::NoWrap,
        gummy::style::FlexWrap::Wrap => upstream_03::style::FlexWrap::Wrap,
        gummy::style::FlexWrap::WrapReverse => upstream_03::style::FlexWrap::WrapReverse,
    }
}
