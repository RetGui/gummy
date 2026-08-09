//! A small `usvg` tree renderer backed by Vello CPU.

use usvg::tiny_skia_path::PathSegment;
use usvg::{Node, Paint, PaintOrder};
use vello_cpu::color::AlphaColor;
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Stroke};
use vello_cpu::peniko::Fill;
use vello_cpu::{Pixmap, RenderContext, Resources};

pub fn rasterize(tree: &usvg::Tree, max_width: u16, max_height: u16) -> Pixmap {
    let source_size = tree.size();
    let scale = (f32::from(max_width) / source_size.width()).min(f32::from(max_height) / source_size.height()).min(1.0);
    let width = (source_size.width() * scale).ceil().max(1.0) as u16;
    let height = (source_size.height() * scale).ceil().max(1.0) as u16;
    let output_transform = Affine::scale_non_uniform(
        f64::from(width) / f64::from(source_size.width()),
        f64::from(height) / f64::from(source_size.height()),
    );

    let mut renderer = RenderContext::new(width, height);
    let mut context = SvgContext::new(output_transform);
    render_group(&mut renderer, &mut context, tree.root());
    renderer.flush();

    let mut pixmap = Pixmap::new(width, height);
    renderer.render(&mut pixmap, &mut Resources::new());
    pixmap.recompute_may_have_transparency();
    pixmap
}

struct SvgContext {
    transforms: Vec<Affine>,
}

impl SvgContext {
    fn new(transform: Affine) -> Self {
        Self { transforms: vec![transform] }
    }

    fn push_transform(&mut self, transform: Affine) {
        self.transforms.push(self.transform() * transform);
    }

    fn pop_transform(&mut self) {
        self.transforms.pop();
    }

    fn transform(&self) -> Affine {
        *self.transforms.last().expect("the SVG transform stack is never empty")
    }
}

fn render_group(renderer: &mut RenderContext, context: &mut SvgContext, group: &usvg::Group) {
    context.push_transform(convert_transform(group.transform()));
    renderer.set_transform(context.transform());

    let clip_path = group.clip_path().map(|clip| {
        let mut path = BezPath::new();
        extract_clip_path(clip.root(), Affine::IDENTITY, &mut path);
        convert_transform(clip.transform()) * path
    });
    let opacity = (group.opacity().get() != 1.0).then_some(group.opacity().get());
    renderer.push_layer(clip_path.as_ref(), None, opacity, None, None);

    for child in group.children() {
        match child {
            Node::Group(group) => render_group(renderer, context, group),
            Node::Path(path) => render_path(renderer, context, path),
            Node::Image(_) | Node::Text(_) => {}
        }
    }

    context.pop_transform();
    renderer.pop_layer();
}

// `usvg` has already resolved basic SVG shapes, arcs, and relative coordinates into
// absolute path segments. Clip-path nesting and clip fill rules are outside the WPT
// image-sizing coverage handled by this runner.
fn extract_clip_path(group: &usvg::Group, transform: Affine, output: &mut BezPath) {
    let transform = transform * convert_transform(group.transform());
    for child in group.children() {
        match child {
            Node::Group(group) => extract_clip_path(group, transform, output),
            Node::Path(path) => output.extend(transform * convert_path(path)),
            Node::Image(_) | Node::Text(_) => {}
        }
    }
}

fn render_path(renderer: &mut RenderContext, context: &SvgContext, path: &usvg::Path) {
    if !path.is_visible() {
        return;
    }

    renderer.set_transform(context.transform());
    match path.paint_order() {
        PaintOrder::FillAndStroke => {
            fill_path(renderer, path);
            stroke_path(renderer, path);
        }
        PaintOrder::StrokeAndFill => {
            stroke_path(renderer, path);
            fill_path(renderer, path);
        }
    }
}

fn fill_path(renderer: &mut RenderContext, path: &usvg::Path) {
    let Some(fill) = path.fill() else { return };
    let Paint::Color(color) = fill.paint() else { return };

    renderer.set_fill_rule(match fill.rule() {
        usvg::FillRule::NonZero => Fill::NonZero,
        usvg::FillRule::EvenOdd => Fill::EvenOdd,
    });
    renderer.set_paint(AlphaColor::from_rgba8(color.red, color.green, color.blue, fill.opacity().to_u8()));
    renderer.fill_path(&convert_path(path));
}

fn stroke_path(renderer: &mut RenderContext, path: &usvg::Path) {
    let Some(stroke) = path.stroke() else { return };
    let Paint::Color(color) = stroke.paint() else { return };

    let cap = match stroke.linecap() {
        usvg::LineCap::Butt => Cap::Butt,
        usvg::LineCap::Round => Cap::Round,
        usvg::LineCap::Square => Cap::Square,
    };
    let join = match stroke.linejoin() {
        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => Join::Miter,
        usvg::LineJoin::Round => Join::Round,
        usvg::LineJoin::Bevel => Join::Bevel,
    };
    let mut stroke_style = Stroke::new(f64::from(stroke.width().get()))
        .with_caps(cap)
        .with_join(join)
        .with_miter_limit(f64::from(stroke.miterlimit().get()));
    if let Some(dashes) = stroke.dasharray() {
        stroke_style = stroke_style
            .with_dashes(f64::from(stroke.dashoffset()), dashes.iter().copied().map(f64::from).collect::<Vec<_>>());
    }
    renderer.set_stroke(stroke_style);
    renderer.set_paint(AlphaColor::from_rgba8(color.red, color.green, color.blue, stroke.opacity().to_u8()));
    renderer.stroke_path(&convert_path(path));
}

fn convert_transform(transform: usvg::Transform) -> Affine {
    Affine::new([
        f64::from(transform.sx),
        f64::from(transform.ky),
        f64::from(transform.kx),
        f64::from(transform.sy),
        f64::from(transform.tx),
        f64::from(transform.ty),
    ])
}

fn convert_path(path: &usvg::Path) -> BezPath {
    let mut output = BezPath::new();
    for segment in path.data().segments() {
        match segment {
            PathSegment::MoveTo(point) => output.move_to((point.x, point.y)),
            PathSegment::LineTo(point) => output.line_to((point.x, point.y)),
            PathSegment::QuadTo(control, point) => output.quad_to((control.x, control.y), (point.x, point.y)),
            PathSegment::CubicTo(control1, control2, point) => {
                output.curve_to((control1.x, control1.y), (control2.x, control2.y), (point.x, point.y));
            }
            PathSegment::Close => output.close_path(),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello_cpu::peniko::Color;

    #[test]
    fn renders_transformed_svg_with_vello_cpu() {
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">
            <g transform="translate(5 0)" opacity="0.5">
                <rect width="5" height="10" fill="#00ff00"/>
            </g>
        </svg>"##;
        let tree = usvg::Tree::from_str(source, &usvg::Options::default()).unwrap();
        let pixmap = rasterize(&tree, 20, 10);

        let transparent = Color::TRANSPARENT.premultiply().to_rgba8();
        let half_green = Color::from_rgba8(0, 255, 0, 128).premultiply().to_rgba8();
        assert_eq!(pixmap.data()[2 + 5 * 20], transparent);
        assert_eq!(pixmap.data()[7 + 5 * 20], half_green);
    }
}
