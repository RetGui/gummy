#![allow(dead_code)]

use rand::distr::uniform::SampleRange;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use slotmap::{DefaultKey, SlotMap};

use super::{BuildTree, BuildTreeExt, GenStyle};

pub mod yg {
    pub use ordered_float::OrderedFloat;
    pub use slotmap::{DefaultKey, SlotMap};
    pub use yoga::types::*;
    pub use yoga::Node;

    pub type NodeId = DefaultKey;
    pub type YogaTree = SlotMap<DefaultKey, Node>;
}
mod tf {
    pub use gummy::prelude::*;
}
use tf::Style as GummyStyle;

pub struct YogaTreeBuilder<R: Rng, G: GenStyle<GummyStyle>> {
    rng: R,
    style_generator: G,
    tree: yg::YogaTree,
    root: yg::DefaultKey,
}

// Implement the BuildTree trait
impl<R: Rng, G: GenStyle<GummyStyle>> BuildTree<R, G> for YogaTreeBuilder<R, G> {
    const NAME: &'static str = "Yoga";
    type Tree = yg::YogaTree;
    type Node = DefaultKey;

    fn with_rng(mut rng: R, mut style_generator: G) -> Self {
        let mut tree = SlotMap::new();
        let root = create_yg_node(&mut tree, &style_generator.create_root_style(&mut rng), &[]);
        YogaTreeBuilder { rng, style_generator, tree, root }
    }

    fn compute_layout_inner(&mut self, available_width: Option<f32>, available_height: Option<f32>) {
        self.tree[self.root].calculate_layout(
            available_width.unwrap_or(f32::INFINITY),
            available_height.unwrap_or(f32::INFINITY),
            yg::Direction::LTR,
        )
    }

    fn random_usize(&mut self, range: impl SampleRange<usize>) -> usize {
        self.rng.random_range(range)
    }

    fn create_leaf_node(&mut self) -> Self::Node {
        let style = self.style_generator.create_leaf_style(&mut self.rng);
        create_yg_node(&mut self.tree, &style, &[])
    }

    fn create_container_node(&mut self, children: &[Self::Node]) -> Self::Node {
        let style = self.style_generator.create_container_style(&mut self.rng);
        create_yg_node(&mut self.tree, &style, &children)
    }

    fn set_root_children(&mut self, children: &[Self::Node]) {
        set_node_children(&mut self.tree, self.root, &children);
    }

    fn total_node_count(&mut self) -> usize {
        self.tree.len()
    }

    fn into_tree_and_root(self) -> (Self::Tree, Self::Node) {
        (self.tree, self.root)
    }
}

impl<G: GenStyle<GummyStyle>> BuildTreeExt<G> for YogaTreeBuilder<ChaCha8Rng, G> {}

// impl<R: Rng, G: GenStyle<GummyStyle>> YogaTreeBuilder<R, G> {
//     /// Create a YogaTreeBuilder with a standard rng from a style generator
//     fn new<NG: GenStyle<GummyStyle>>(mut style_generator: NG) -> YogaTreeBuilder<ChaCha8Rng, NG> {
//         let mut rng = ChaCha8Rng::seed_from_u64(STANDARD_RNG_SEED);
//         let mut tree = SlotMap::new();
//         let root = create_yg_node(&mut tree, &style_generator.create_root_style(&mut rng), &[]);
//         YogaTreeBuilder { rng, style_generator, tree, root }
//     }

//     /// Create a YogaTreeBuilder with a standard rng from a style generator
//     fn with_seed<NG: GenStyle<GummyStyle>>(seed: u64, mut style_generator: NG) -> YogaTreeBuilder<ChaCha8Rng, NG> {
//         let mut rng = ChaCha8Rng::seed_from_u64(seed);
//         let mut tree = SlotMap::new();
//         let root = create_yg_node(&mut tree, &style_generator.create_root_style(&mut rng), &[]);
//         YogaTreeBuilder { rng, style_generator, tree, root }
//     }

//     /// Create a YogaTreeBuilder from a random number generator and a style generator
//     fn with_rng<NR: Rng, NG: GenStyle<GummyStyle>>(mut rng: NR, mut style_generator: NG) -> YogaTreeBuilder<NR, NG> {
//         let mut tree = SlotMap::new();
//         let root = create_yg_node(&mut tree, &style_generator.create_root_style(&mut rng), &[]);
//         YogaTreeBuilder { rng, style_generator, tree, root }
//     }
// }

fn create_yg_node(tree: &mut yg::YogaTree, style: &tf::Style, children: &[yg::DefaultKey]) -> yg::DefaultKey {
    let mut node = yg::Node::new();
    apply_gummy_style(&mut node, &style);
    for (i, child) in children.into_iter().enumerate() {
        node.insert_child(&mut tree[*child], i);
    }
    tree.insert(node)
}

pub fn new_default_style_with_children(tree: &mut yg::YogaTree, children: &[yg::DefaultKey]) -> yg::DefaultKey {
    let mut node = yg::Node::new();
    for (i, child) in children.into_iter().enumerate() {
        node.insert_child(&mut tree[*child], i);
    }
    tree.insert(node)
}

fn set_node_children(tree: &mut yg::YogaTree, node_id: yg::DefaultKey, children: &[yg::DefaultKey]) {
    // TODO: clear existing children.
    for (i, child_id) in children.into_iter().enumerate() {
        let [node, child] = tree.get_disjoint_mut([node_id, *child_id]).unwrap();
        node.insert_child(child, i);
    }
}

fn into_yg_units(dim: impl Into<tf::Dimension>) -> yg::StyleUnit {
    let dim: tf::Dimension = dim.into();
    let val = dim.into_raw().value();
    match dim.into_raw().tag() {
        tf::CompactLength::AUTO_TAG => yg::StyleUnit::Auto,
        tf::CompactLength::LENGTH_TAG => yg::StyleUnit::Point(yg::OrderedFloat(val)),
        tf::CompactLength::PERCENT_TAG => yg::StyleUnit::Percent(yg::OrderedFloat(val)),
        _ => unreachable!(),
    }
}

fn into_pixels(dim: impl Into<tf::Dimension>) -> f32 {
    dim.into().into_option().unwrap_or(0.0)
}

fn items_into_align(align: tf::AlignItems) -> yg::Align {
    // Yoga has no safe/unsafe overflow-position concept.
    match align {
        tf::AlignItems::Normal => yg::Align::Auto,
        tf::AlignItems::FlexStart | tf::AlignItems::SafeFlexStart => yg::Align::FlexStart,
        tf::AlignItems::FlexEnd | tf::AlignItems::SafeFlexEnd => yg::Align::FlexEnd,
        tf::AlignItems::Center | tf::AlignItems::SafeCenter => yg::Align::Center,
        tf::AlignItems::Baseline => yg::Align::Baseline,
        tf::AlignItems::Stretch => yg::Align::Stretch,
        tf::AlignItems::Start
        | tf::AlignItems::End
        | tf::AlignItems::SelfStart
        | tf::AlignItems::SelfEnd
        | tf::AlignItems::SafeStart
        | tf::AlignItems::SafeEnd
        | tf::AlignItems::SafeSelfStart
        | tf::AlignItems::SafeSelfEnd => unimplemented!(),
    }
}

fn self_into_align(align: tf::AlignSelf) -> yg::Align {
    // Yoga has no safe/unsafe overflow-position concept.
    match align {
        tf::AlignSelf::Normal | tf::AlignSelf::Auto => yg::Align::Auto,
        tf::AlignSelf::FlexStart | tf::AlignSelf::SafeFlexStart => yg::Align::FlexStart,
        tf::AlignSelf::FlexEnd | tf::AlignSelf::SafeFlexEnd => yg::Align::FlexEnd,
        tf::AlignSelf::Center | tf::AlignSelf::SafeCenter => yg::Align::Center,
        tf::AlignSelf::Baseline => yg::Align::Baseline,
        tf::AlignSelf::Stretch => yg::Align::Stretch,
        tf::AlignSelf::Start
        | tf::AlignSelf::End
        | tf::AlignSelf::SelfStart
        | tf::AlignSelf::SelfEnd
        | tf::AlignSelf::SafeStart
        | tf::AlignSelf::SafeEnd
        | tf::AlignSelf::SafeSelfStart
        | tf::AlignSelf::SafeSelfEnd => unimplemented!(),
    }
}

fn content_into_align(align: tf::AlignContent) -> yg::Align {
    match align {
        tf::AlignContent::Normal => yg::Align::Auto,
        tf::AlignContent::FlexStart
        | tf::AlignContent::Start
        | tf::AlignContent::SafeFlexStart
        | tf::AlignContent::SafeStart => yg::Align::FlexStart,
        tf::AlignContent::FlexEnd
        | tf::AlignContent::End
        | tf::AlignContent::SafeFlexEnd
        | tf::AlignContent::SafeEnd => yg::Align::FlexEnd,
        tf::AlignContent::Center | tf::AlignContent::SafeCenter => yg::Align::Center,
        tf::AlignContent::Stretch => yg::Align::Stretch,
        tf::AlignContent::SpaceBetween => yg::Align::SpaceBetween,
        tf::AlignContent::SpaceAround => yg::Align::SpaceAround,
        tf::AlignContent::SpaceEvenly => unimplemented!(),
    }
}

fn content_into_justify(align: tf::JustifyContent) -> yg::Justify {
    match align {
        tf::JustifyContent::Normal
        | tf::JustifyContent::FlexStart
        | tf::JustifyContent::Start
        | tf::JustifyContent::SafeFlexStart
        | tf::JustifyContent::SafeStart => yg::Justify::FlexStart,
        tf::JustifyContent::FlexEnd
        | tf::JustifyContent::End
        | tf::JustifyContent::SafeFlexEnd
        | tf::JustifyContent::SafeEnd => yg::Justify::FlexEnd,
        tf::JustifyContent::Center | tf::JustifyContent::SafeCenter => yg::Justify::Center,
        tf::JustifyContent::SpaceBetween => yg::Justify::SpaceBetween,
        tf::JustifyContent::SpaceAround => yg::Justify::SpaceAround,
        tf::JustifyContent::Stretch | tf::JustifyContent::SpaceEvenly => unimplemented!(),
    }
}

fn apply_gummy_style(node: &mut yg::Node, style: &tf::Style) {
    // display
    node.set_display(match style.display {
        tf::Display::None => yg::Display::None,
        tf::Display::Flex => yg::Display::Flex,
        tf::Display::Grid => panic!("Yoga does not support CSS Grid layout"),
        tf::Display::Block => panic!("Yoga does not support CSS Block layout"),
    });

    // box_sizing
    node.set_box_sizing(match style.box_sizing {
        tf::BoxSizing::BorderBox => yg::BoxSizing::BorderBox,
        tf::BoxSizing::ContentBox => yg::BoxSizing::ContentBox,
    });

    // position
    node.set_position_type(match style.position {
        tf::Position::Relative => yg::PositionType::Relative,
        tf::Position::Absolute => yg::PositionType::Absolute,
    });
    // inset
    node.set_position(yg::Edge::Left, into_yg_units(style.inset.left));
    node.set_position(yg::Edge::Right, into_yg_units(style.inset.right));
    node.set_position(yg::Edge::Top, into_yg_units(style.inset.top));
    node.set_position(yg::Edge::Bottom, into_yg_units(style.inset.bottom));

    // sizes
    node.set_width(into_yg_units(style.size.width));
    node.set_height(into_yg_units(style.size.height));
    node.set_min_width(into_yg_units(style.min_size.width));
    node.set_min_height(into_yg_units(style.min_size.height));
    node.set_max_width(into_yg_units(style.max_size.width));
    node.set_max_height(into_yg_units(style.max_size.height));

    // aspect_ratio
    if let Some(aspect_ratio) = style.aspect_ratio {
        node.set_aspect_ratio(aspect_ratio);
    }

    // spacing
    node.set_padding(yg::Edge::Left, into_yg_units(style.padding.left));
    node.set_padding(yg::Edge::Right, into_yg_units(style.padding.right));
    node.set_padding(yg::Edge::Top, into_yg_units(style.padding.top));
    node.set_padding(yg::Edge::Bottom, into_yg_units(style.padding.bottom));
    node.set_margin(yg::Edge::Left, into_yg_units(style.margin.left));
    node.set_margin(yg::Edge::Right, into_yg_units(style.margin.right));
    node.set_margin(yg::Edge::Top, into_yg_units(style.margin.top));
    node.set_margin(yg::Edge::Bottom, into_yg_units(style.margin.bottom));
    node.set_border(yg::Edge::Left, into_pixels(style.border.left));
    node.set_border(yg::Edge::Right, into_pixels(style.border.right));
    node.set_border(yg::Edge::Top, into_pixels(style.border.top));
    node.set_border(yg::Edge::Bottom, into_pixels(style.border.bottom));

    // alignment
    node.set_align_items(items_into_align(style.align_items));
    node.set_align_self(self_into_align(style.align_self));
    node.set_align_content(content_into_align(style.align_content));
    node.set_justify_content(content_into_justify(style.justify_content));

    // gap
    node.set_column_gap(into_yg_units(style.gap.width));
    node.set_row_gap(into_yg_units(style.gap.height));

    // flex
    node.set_flex_direction(match style.flex_direction {
        tf::FlexDirection::Row => yg::FlexDirection::Row,
        tf::FlexDirection::Column => yg::FlexDirection::Column,
        tf::FlexDirection::RowReverse => yg::FlexDirection::RowReverse,
        tf::FlexDirection::ColumnReverse => yg::FlexDirection::ColumnReverse,
    });
    node.set_flex_wrap(match style.flex_wrap {
        tf::FlexWrap::NoWrap => yg::Wrap::NoWrap,
        tf::FlexWrap::Wrap => yg::Wrap::Wrap,
        tf::FlexWrap::WrapReverse => yg::Wrap::WrapReverse,
    });
    node.set_flex_basis(into_yg_units(style.flex_basis));
    node.set_flex_grow(style.flex_grow);
    node.set_flex_shrink(style.flex_shrink);
}
