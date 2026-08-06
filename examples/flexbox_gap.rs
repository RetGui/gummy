use gummy::prelude::*;

// Creates three 20px x 20px children, evenly spaced 10px apart from each other
// Thus the container is 80px x 20px.

fn main() -> Result<(), gummy::GummyError> {
    let mut gummy: GummyTree<()> = GummyTree::new();

    let child_style = Style { size: Size { width: length(20.0), height: length(20.0) }, ..Default::default() };
    let child0 = gummy.new_leaf(child_style.clone())?;
    let child1 = gummy.new_leaf(child_style.clone())?;
    let child2 = gummy.new_leaf(child_style.clone())?;

    let root = gummy.new_with_children(
        Style { gap: Size { width: length(10.0), height: zero() }, ..Default::default() },
        &[child0, child1, child2],
    )?;

    // Compute layout and print result
    gummy.compute_layout(root, Size::MAX_CONTENT)?;
    gummy.print_tree(root);

    Ok(())
}
