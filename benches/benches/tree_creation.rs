//! This file includes benchmarks for tree creation
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use gummy::prelude::*;
use gummy::style::Style;

#[cfg(feature = "yoga")]
use slotmap::SlotMap;
#[cfg(feature = "yoga")]
use gummy_benchmarks::yoga_helpers;
#[cfg(feature = "yoga")]
use yoga_helpers::yg;

/// Build a random leaf node
fn build_random_leaf(gummy: &mut GummyTree) -> NodeId {
    gummy.new_with_children(Style::DEFAULT, &[]).unwrap()
}

/// A tree with many children that have shallow depth
fn build_gummy_flat_hierarchy(total_node_count: u32, use_with_capacity: bool) -> (GummyTree, NodeId) {
    let mut gummy =
        if use_with_capacity { GummyTree::with_capacity(total_node_count as usize) } else { GummyTree::new() };
    let mut rng = ChaCha8Rng::seed_from_u64(12345);
    let mut children = Vec::new();
    let mut node_count = 0;

    while node_count < total_node_count {
        let sub_children_count = rng.random_range(1..=4);
        let sub_children: Vec<NodeId> = (0..sub_children_count).map(|_| build_random_leaf(&mut gummy)).collect();
        let node = gummy.new_with_children(Style::DEFAULT, &sub_children).unwrap();

        children.push(node);
        node_count += 1 + sub_children_count;
    }

    let root = gummy.new_with_children(Style::DEFAULT, children.as_slice()).unwrap();
    (gummy, root)
}

#[cfg(feature = "yoga")]
/// A tree with many children that have shallow depth
fn build_yoga_flat_hierarchy(total_node_count: u32) -> (yg::YogaTree, yg::NodeId) {
    let mut tree = SlotMap::new();
    let mut rng = ChaCha8Rng::seed_from_u64(12345);
    let mut children = Vec::new();
    let mut node_count = 0;

    while node_count < total_node_count {
        let sub_children_count = rng.random_range(1..=4);
        let sub_children: Vec<yg::NodeId> =
            (0..sub_children_count).map(|_| yoga_helpers::new_default_style_with_children(&mut tree, &[])).collect();
        let node = yoga_helpers::new_default_style_with_children(&mut tree, &sub_children);

        children.push(node);
        node_count += 1 + sub_children_count;
    }

    let root = yoga_helpers::new_default_style_with_children(&mut tree, &children);
    (tree, root)
}

fn gummy_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("Tree creation");
    for node_count in [1_000u32, 10_000, 100_000].iter() {
        #[cfg(feature = "yoga")]
        let benchmark_id = BenchmarkId::new(format!("Yoga"), node_count);
        #[cfg(feature = "yoga")]
        group.bench_with_input(benchmark_id, node_count, |b, &node_count| {
            b.iter(|| {
                let (gummy, root) = build_yoga_flat_hierarchy(node_count);
                std::hint::black_box(gummy);
                std::hint::black_box(root);
            })
        });
        let benchmark_id = BenchmarkId::new("GummyTree::new".to_string(), node_count);
        group.bench_with_input(benchmark_id, node_count, |b, &node_count| {
            b.iter(|| {
                let (tree, root) = build_gummy_flat_hierarchy(node_count, false);
                std::hint::black_box(tree);
                std::hint::black_box(root);
            })
        });

        let benchmark_id = BenchmarkId::new("GummyTree::with_capacity".to_string(), node_count);
        group.bench_with_input(benchmark_id, node_count, |b, &node_count| {
            b.iter(|| {
                let (tree, root) = build_gummy_flat_hierarchy(node_count, true);
                std::hint::black_box(tree);
                std::hint::black_box(root);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, gummy_benchmarks);
criterion_main!(benches);
