#[cfg(test)]
mod root_constraints {
    use gummy::prelude::{FromLength, FromPercent};
    use gummy::style_helpers::{length, GummyMaxContent};
    use gummy::{AvailableSpace, Rect, Size, Style, GummyTree};
    use gummy_test_helpers::new_test_tree;

    #[test]
    fn root_with_percentage_size() {
        let mut gummy = new_test_tree();
        let node = gummy
            .new_leaf(gummy::style::Style {
                size: gummy::geometry::Size {
                    width: gummy::style::Dimension::from_percent(1.0),
                    height: gummy::style::Dimension::from_percent(1.0),
                },
                ..Default::default()
            })
            .unwrap();

        gummy
            .compute_layout(
                node,
                gummy::geometry::Size {
                    width: AvailableSpace::Definite(100.0),
                    height: AvailableSpace::Definite(200.0),
                },
            )
            .unwrap();
        let layout = gummy.layout(node).unwrap();

        assert_eq!(layout.size.width, 100.0);
        assert_eq!(layout.size.height, 200.0);
    }

    #[test]
    fn root_with_no_size() {
        let mut gummy = new_test_tree();
        let node = gummy.new_leaf(gummy::style::Style::default()).unwrap();

        gummy
            .compute_layout(
                node,
                gummy::geometry::Size {
                    width: AvailableSpace::Definite(100.0),
                    height: AvailableSpace::Definite(100.0),
                },
            )
            .unwrap();
        let layout = gummy.layout(node).unwrap();

        assert_eq!(layout.size.width, 0.0);
        assert_eq!(layout.size.height, 0.0);
    }

    #[test]
    fn root_with_larger_size() {
        let mut gummy = new_test_tree();
        let node = gummy
            .new_leaf(gummy::style::Style {
                size: gummy::geometry::Size {
                    width: gummy::style::Dimension::from_length(200.0),
                    height: gummy::style::Dimension::from_length(200.0),
                },
                ..Default::default()
            })
            .unwrap();

        gummy
            .compute_layout(
                node,
                gummy::geometry::Size {
                    width: AvailableSpace::Definite(100.0),
                    height: AvailableSpace::Definite(100.0),
                },
            )
            .unwrap();
        let layout = gummy.layout(node).unwrap();

        assert_eq!(layout.size.width, 200.0);
        assert_eq!(layout.size.height, 200.0);
    }

    #[test]
    fn root_padding_and_border_larger_than_definite_size() {
        let mut tree: GummyTree<()> = GummyTree::with_capacity(16);

        let child = tree.new_leaf(Style::default()).unwrap();

        let root = tree
            .new_with_children(
                Style {
                    size: Size { width: length(10.0), height: length(10.0) },
                    padding: Rect { left: length(10.0), right: length(10.0), top: length(10.0), bottom: length(10.0) },

                    border: Rect { left: length(10.0), right: length(10.0), top: length(10.0), bottom: length(10.0) },
                    ..Default::default()
                },
                &[child],
            )
            .unwrap();

        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();

        let layout = tree.layout(root).unwrap();

        assert_eq!(layout.size.width, 40.0);
        assert_eq!(layout.size.height, 40.0);
    }
}
