use gummy::prelude::*;

fn main() -> Result<(), gummy::GummyError> {
    let mut gummy: GummyTree<()> = GummyTree::new();

    let child = gummy.new_leaf(Style {
        size: Size { width: Dimension::from_percent(0.5), height: Dimension::AUTO },
        ..Default::default()
    })?;

    let node = gummy.new_with_children(
        Style {
            size: Size { width: Dimension::from_length(100.0), height: Dimension::from_length(100.0) },
            justify_content: JustifyContent::CENTER,
            ..Default::default()
        },
        &[child],
    )?;

    println!("Compute layout with 100x100 viewport:");
    gummy.compute_layout(
        node,
        Size { height: AvailableSpace::Definite(100.0), width: AvailableSpace::Definite(100.0) },
    )?;
    println!("node: {:#?}", gummy.layout(node)?);
    println!("child: {:#?}", gummy.layout(child)?);

    println!("Compute layout with undefined (infinite) viewport:");
    gummy.compute_layout(node, Size::MAX_CONTENT)?;
    println!("node: {:#?}", gummy.layout(node)?);
    println!("child: {:#?}", gummy.layout(child)?);

    Ok(())
}
