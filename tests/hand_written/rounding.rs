use gummy::prelude::*;
use gummy_test_helpers::new_test_tree;

#[test]
fn rounding_doesnt_leave_gaps() {
    // First create an instance of GummyTree
    let mut gummy = new_test_tree();

    let w_square = Size { width: length(100.3), height: length(100.3) };
    let child_a = gummy.new_leaf(Style { size: w_square, ..Default::default() }).unwrap();
    let child_b = gummy.new_leaf(Style { size: w_square, ..Default::default() }).unwrap();

    let root_node = gummy
        .new_with_children(
            Style {
                size: Size { width: length(963.3333), height: length(1000.) },
                justify_content: Some(JustifyContent::CENTER),
                ..Default::default()
            },
            &[child_a, child_b],
        )
        .unwrap();

    gummy.compute_layout(root_node, Size::MAX_CONTENT).unwrap();
    gummy.print_tree(root_node);

    let layout_a = gummy.layout(child_a).unwrap();
    let layout_b = gummy.layout(child_b).unwrap();
    assert_eq!(layout_a.location.x + layout_a.size.width, layout_b.location.x);
}
