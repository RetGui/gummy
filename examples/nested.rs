use gummy::prelude::*;

fn main() -> Result<(), gummy::GummyError> {
    let mut gummy: GummyTree<()> = GummyTree::new();

    // left
    let child_t1 = gummy.new_leaf(Style {
        size: Size { width: Dimension::from_length(5.0), height: Dimension::from_length(5.0) },
        ..Default::default()
    })?;

    let div1 = gummy.new_with_children(
        Style {
            size: Size { width: Dimension::from_percent(0.5), height: Dimension::from_percent(1.0) },
            // justify_content: JustifyContent::CENTER,
            ..Default::default()
        },
        &[child_t1],
    )?;

    // right
    let child_t2 = gummy.new_leaf(Style {
        size: Size { width: Dimension::from_length(5.0), height: Dimension::from_length(5.0) },
        ..Default::default()
    })?;

    let div2 = gummy.new_with_children(
        Style {
            size: Size { width: Dimension::from_percent(0.5), height: Dimension::from_percent(1.0) },
            // justify_content: JustifyContent::CENTER,
            ..Default::default()
        },
        &[child_t2],
    )?;

    let container = gummy.new_with_children(
        Style {
            size: Size { width: Dimension::from_percent(1.0), height: Dimension::from_percent(1.0) },
            ..Default::default()
        },
        &[div1, div2],
    )?;

    gummy.compute_layout(
        container,
        Size { height: AvailableSpace::Definite(100.0), width: AvailableSpace::Definite(100.0) },
    )?;

    println!("node: {:#?}", gummy.layout(container)?);

    println!("div1: {:#?}", gummy.layout(div1)?);
    println!("div2: {:#?}", gummy.layout(div2)?);

    println!("child1: {:#?}", gummy.layout(child_t1)?);
    println!("child2: {:#?}", gummy.layout(child_t2)?);

    Ok(())
}
