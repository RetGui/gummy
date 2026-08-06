#[cfg(test)]
mod min_max_overrides {
    use gummy::prelude::*;
    use gummy_test_helpers::new_test_tree;

    #[test]
    fn min_overrides_max() {
        let mut gummy = new_test_tree();

        let child = gummy
            .new_leaf(Style {
                size: Size { width: Dimension::from_length(50.0), height: Dimension::from_length(50.0) },
                min_size: Size { width: Dimension::from_length(100.0), height: Dimension::from_length(100.0) },
                max_size: Size { width: Dimension::from_length(10.0), height: Dimension::from_length(10.0) },
                ..Default::default()
            })
            .unwrap();

        gummy
            .compute_layout(
                child,
                Size { width: AvailableSpace::Definite(100.0), height: AvailableSpace::Definite(100.0) },
            )
            .unwrap();

        assert_eq!(gummy.layout(child).unwrap().size, Size { width: 100.0, height: 100.0 });
    }

    #[test]
    fn max_overrides_size() {
        let mut gummy = new_test_tree();

        let child = gummy
            .new_leaf(Style {
                size: Size { width: Dimension::from_length(50.0), height: Dimension::from_length(50.0) },
                max_size: Size { width: Dimension::from_length(10.0), height: Dimension::from_length(10.0) },
                ..Default::default()
            })
            .unwrap();

        gummy
            .compute_layout(
                child,
                Size { width: AvailableSpace::Definite(100.0), height: AvailableSpace::Definite(100.0) },
            )
            .unwrap();

        assert_eq!(gummy.layout(child).unwrap().size, Size { width: 10.0, height: 10.0 });
    }

    #[test]
    fn min_overrides_size() {
        let mut gummy = new_test_tree();

        let child = gummy
            .new_leaf(Style {
                size: Size { width: Dimension::from_length(50.0), height: Dimension::from_length(50.0) },
                min_size: Size { width: Dimension::from_length(100.0), height: Dimension::from_length(100.0) },
                ..Default::default()
            })
            .unwrap();

        gummy
            .compute_layout(
                child,
                Size { width: AvailableSpace::Definite(100.0), height: AvailableSpace::Definite(100.0) },
            )
            .unwrap();

        assert_eq!(gummy.layout(child).unwrap().size, Size { width: 100.0, height: 100.0 });
    }
}
