use rand::distr::uniform::SampleRange;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use gummy::style::Style as GummyStyle;
use gummy::tree::NodeId as GummyNodeId;
use gummy::GummyTree;

use super::{BuildTree, BuildTreeExt, GenStyle};

pub struct GummyTreeBuilder<R: Rng, G: GenStyle<GummyStyle>> {
    rng: R,
    style_generator: G,
    tree: GummyTree,
    root: GummyNodeId,
}

// Implement the BuildTree trait
impl<R: Rng, G: GenStyle<GummyStyle>> BuildTree<R, G> for GummyTreeBuilder<R, G> {
    const NAME: &'static str = "Gummy";
    type Tree = GummyTree;
    type Node = GummyNodeId;

    fn with_rng(mut rng: R, mut style_generator: G) -> Self {
        let mut tree = GummyTree::new();
        let root = tree.new_leaf(style_generator.create_root_style(&mut rng)).unwrap();
        GummyTreeBuilder { rng, style_generator, tree, root }
    }

    fn compute_layout_inner(&mut self, available_width: Option<f32>, available_height: Option<f32>) {
        let available_space = gummy::geometry::Size { width: available_width.into(), height: available_height.into() };
        self.tree.compute_layout(self.root, available_space).unwrap();
    }

    fn random_usize(&mut self, range: impl SampleRange<usize>) -> usize {
        self.rng.random_range(range)
    }

    fn create_leaf_node(&mut self) -> Self::Node {
        let style = self.style_generator.create_leaf_style(&mut self.rng);
        self.tree.new_leaf(style).unwrap()
    }

    fn create_container_node(&mut self, children: &[Self::Node]) -> Self::Node {
        let style = self.style_generator.create_container_style(&mut self.rng);
        self.tree.new_with_children(style, children).unwrap()
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

impl<G: GenStyle<GummyStyle>> BuildTreeExt<G> for GummyTreeBuilder<ChaCha8Rng, G> {}
