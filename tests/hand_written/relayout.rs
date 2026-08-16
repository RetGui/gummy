use gummy::prelude::*;
use gummy_test_helpers::new_test_tree;

#[test]
fn relayout() {
    let mut gummy = new_test_tree();
    let node1 = gummy
        .new_leaf(gummy::style::Style {
            size: gummy::geometry::Size { width: length(8.0), height: length(80.0) },
            ..Default::default()
        })
        .unwrap();
    let node0 = gummy
        .new_with_children(
            gummy::style::Style {
                align_self: gummy::prelude::AlignSelf::CENTER,
                size: gummy::geometry::Size { width: Dimension::AUTO, height: Dimension::AUTO },
                // size: gummy::geometry::Size { width: Dimension::Percent(1.0), height: Dimension::Percent(1.0) },
                ..Default::default()
            },
            &[node1],
        )
        .unwrap();
    let node = gummy
        .new_with_children(
            gummy::style::Style {
                size: gummy::geometry::Size {
                    width: Dimension::from_percent(1f32),
                    height: Dimension::from_percent(1f32),
                },
                ..Default::default()
            },
            &[node0],
        )
        .unwrap();
    gummy
        .compute_layout(
            node,
            gummy::geometry::Size { width: AvailableSpace::Definite(100f32), height: AvailableSpace::Definite(100f32) },
        )
        .unwrap();
    let initial = gummy.layout(node).unwrap().location;
    let initial0 = gummy.layout(node0).unwrap().location;
    let initial1 = gummy.layout(node1).unwrap().location;
    for _ in 1..10 {
        gummy
            .compute_layout(
                node,
                gummy::geometry::Size {
                    width: AvailableSpace::Definite(100f32),
                    height: AvailableSpace::Definite(100f32),
                },
            )
            .unwrap();
        assert_eq!(gummy.layout(node).unwrap().location, initial);
        assert_eq!(gummy.layout(node0).unwrap().location, initial0);
        assert_eq!(gummy.layout(node1).unwrap().location, initial1);
    }
}

#[test]
fn toggle_root_display_none() {
    let hidden_style = Style {
        display: Display::None,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    let flex_style = Style {
        display: Display::Flex,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    // Setup
    let mut gummy = new_test_tree();
    let node = gummy.new_leaf(hidden_style.clone()).unwrap();

    // Layout 1 (None)
    gummy.compute_layout(node, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);

    // Layout 2 (Flex)
    gummy.set_style(node, flex_style).unwrap();
    gummy.compute_layout(node, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 100.0);
    assert_eq!(layout.size.height, 100.0);

    // Layout 3 (None)
    gummy.set_style(node, hidden_style).unwrap();
    gummy.compute_layout(node, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);
}

#[test]
fn toggle_root_display_none_with_children() {
    use gummy::prelude::*;

    let mut gummy = new_test_tree();

    let child = gummy
        .new_leaf(Style { size: Size { width: length(800.0), height: length(100.0) }, ..Default::default() })
        .unwrap();

    let parent = gummy
        .new_with_children(
            Style { size: Size { width: length(800.0), height: length(100.0) }, ..Default::default() },
            &[child],
        )
        .unwrap();

    let root = gummy.new_with_children(Style::default(), &[parent]).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(gummy.layout(child).unwrap().size.width, 800.0);
    assert_eq!(gummy.layout(child).unwrap().size.height, 100.0);

    gummy.set_style(root, Style { display: Display::None, ..Default::default() }).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(gummy.layout(child).unwrap().size.width, 0.0);
    assert_eq!(gummy.layout(child).unwrap().size.height, 0.0);

    gummy.set_style(root, Style::default()).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(gummy.layout(parent).unwrap().size.width, 800.0);
    assert_eq!(gummy.layout(parent).unwrap().size.height, 100.0);
    assert_eq!(gummy.layout(child).unwrap().size.width, 800.0);
    assert_eq!(gummy.layout(child).unwrap().size.height, 100.0);
}

#[test]
fn toggle_flex_child_display_none() {
    let hidden_style = Style {
        display: Display::None,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    let flex_style = Style {
        display: Display::Flex,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    // Setup
    let mut gummy = new_test_tree();
    let node = gummy.new_leaf(hidden_style.clone()).unwrap();
    let root = gummy.new_with_children(flex_style.clone(), &[node]).unwrap();

    // Layout 1 (None)
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);

    // Layout 2 (Flex)
    gummy.set_style(node, flex_style).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 100.0);
    assert_eq!(layout.size.height, 100.0);

    // Layout 3 (None)
    gummy.set_style(node, hidden_style).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);
}

#[test]
fn toggle_flex_container_display_none() {
    let hidden_style = Style {
        display: Display::None,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    let flex_style = Style {
        display: Display::Flex,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    // Setup
    let mut gummy = new_test_tree();
    let node = gummy.new_leaf(hidden_style.clone()).unwrap();
    let root = gummy.new_with_children(hidden_style.clone(), &[node]).unwrap();

    // Layout 1 (None)
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(root).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);

    // Layout 2 (Flex)
    gummy.set_style(root, flex_style).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(root).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 100.0);
    assert_eq!(layout.size.height, 100.0);

    // Layout 3 (None)
    gummy.set_style(root, hidden_style).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(root).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);
}

#[test]
fn toggle_grid_child_display_none() {
    let hidden_style = Style {
        display: Display::None,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    let grid_style = Style {
        display: Display::Grid,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    // Setup
    let mut gummy = new_test_tree();
    let node = gummy.new_leaf(hidden_style.clone()).unwrap();
    let root = gummy.new_with_children(grid_style.clone(), &[node]).unwrap();

    // Layout 1 (None)
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);

    // Layout 2 (Flex)
    gummy.set_style(node, grid_style).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 100.0);
    assert_eq!(layout.size.height, 100.0);

    // Layout 3 (None)
    gummy.set_style(node, hidden_style).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(node).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);
}

#[test]
fn toggle_grid_container_display_none() {
    let hidden_style = Style {
        display: Display::None,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    let grid_style = Style {
        display: Display::Grid,
        size: Size { width: length(100.0), height: length(100.0) },
        ..Default::default()
    };

    // Setup
    let mut gummy = new_test_tree();
    let node = gummy.new_leaf(hidden_style.clone()).unwrap();
    let root = gummy.new_with_children(hidden_style.clone(), &[node]).unwrap();

    // Layout 1 (None)
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(root).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);

    // Layout 2 (Flex)
    gummy.set_style(root, grid_style).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(root).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 100.0);
    assert_eq!(layout.size.height, 100.0);

    // Layout 3 (None)
    gummy.set_style(root, hidden_style).unwrap();
    gummy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = gummy.layout(root).unwrap();
    assert_eq!(layout.location.x, 0.0);
    assert_eq!(layout.location.y, 0.0);
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);
}

#[test]
fn relayout_is_stable_with_rounding() {
    let mut gummy = new_test_tree();
    gummy.enable_rounding();

    // <div style="width: 1920px; height: 1080px">
    //     <div style="width: 100%; left: 1.5px">
    //         <div style="width: 150px; justify-content: end">
    //             <div style="min-width: 300px" />
    //         </div>
    //     </div>
    // </div>

    let inner =
        gummy.new_leaf(Style { min_size: Size { width: length(300.), height: auto() }, ..Default::default() }).unwrap();
    let wrapper = gummy
        .new_with_children(
            Style {
                size: Size { width: length(150.), height: auto() },
                justify_content: JustifyContent::END,
                ..Default::default()
            },
            &[inner],
        )
        .unwrap();
    let outer = gummy
        .new_with_children(
            Style {
                size: Size { width: percent(1.), height: auto() },
                inset: Rect { left: length(1.5), right: auto(), top: auto(), bottom: auto() },
                ..Default::default()
            },
            &[wrapper],
        )
        .unwrap();
    let root = gummy
        .new_with_children(
            Style { size: Size { width: length(1920.), height: length(1080.) }, ..Default::default() },
            &[outer],
        )
        .unwrap();

    // Compute and assert initial layout.

    gummy.compute_layout(root, Size::MAX_CONTENT).ok();
    gummy.print_tree(root);

    let initial_root_layout = gummy.layout(root).unwrap().clone();
    assert_eq!(initial_root_layout.location.x, 0.0);
    assert_eq!(initial_root_layout.location.y, 0.0);
    assert_eq!(initial_root_layout.size.width, 1920.0);
    assert_eq!(initial_root_layout.size.height, 1080.0);

    let initial_outer_layout = gummy.layout(outer).unwrap().clone();
    assert_eq!(initial_outer_layout.location.x, 2.0);
    assert_eq!(initial_outer_layout.location.y, 0.0);
    assert_eq!(initial_outer_layout.size.width, 1920.0);
    assert_eq!(initial_outer_layout.size.height, 1080.0);

    let initial_wrapper_layout = gummy.layout(wrapper).unwrap().clone();
    assert_eq!(initial_wrapper_layout.location.x, 0.0);
    assert_eq!(initial_wrapper_layout.location.y, 0.0);
    assert_eq!(initial_wrapper_layout.size.width, 150.0);
    assert_eq!(initial_wrapper_layout.size.height, 1080.0);

    let initial_inner_layout = gummy.layout(inner).unwrap().clone();
    assert_eq!(initial_inner_layout.location.x, -150.0);
    assert_eq!(initial_inner_layout.location.y, 0.0);
    assert_eq!(initial_inner_layout.size.width, 300.0);
    assert_eq!(initial_inner_layout.size.height, 1080.0);

    // Recompute and assert that new layout marks initial layout each time
    for _ in 0..5 {
        gummy.mark_dirty(root).ok();
        gummy.compute_layout(root, Size::MAX_CONTENT).ok();
        gummy.print_tree(root);

        let root_layout = gummy.layout(root).unwrap();
        assert_eq!(initial_root_layout.location.x, root_layout.location.x);
        assert_eq!(initial_root_layout.location.y, root_layout.location.y);
        assert_eq!(initial_root_layout.size.width, root_layout.size.width);
        assert_eq!(initial_root_layout.size.height, root_layout.size.height);
        let outer_layout = gummy.layout(outer).unwrap();
        assert_eq!(initial_outer_layout.location.x, outer_layout.location.x);
        assert_eq!(initial_outer_layout.location.y, outer_layout.location.y);
        assert_eq!(initial_outer_layout.size.width, outer_layout.size.width);
        assert_eq!(initial_outer_layout.size.height, outer_layout.size.height);
        let wrapper_layout = gummy.layout(wrapper).unwrap();
        assert_eq!(initial_wrapper_layout.location.x, wrapper_layout.location.x);
        assert_eq!(initial_wrapper_layout.location.x, wrapper_layout.location.y);
        assert_eq!(initial_wrapper_layout.size.width, wrapper_layout.size.width);
        assert_eq!(initial_wrapper_layout.size.height, wrapper_layout.size.height);
        let inner_layout = gummy.layout(inner).unwrap();
        assert_eq!(initial_inner_layout.location.x, inner_layout.location.x);
        assert_eq!(initial_inner_layout.location.y, inner_layout.location.y);
        assert_eq!(initial_inner_layout.size.width, inner_layout.size.width);
        assert_eq!(initial_inner_layout.size.height, inner_layout.size.height);
    }
}
