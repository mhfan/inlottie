
use intvg::blend2d::*;

pub fn blend2d_logo(ctx: &mut BLContext) -> Result<(), BLErr> {
    //let mut img = BLImage::new(480, 480, BLFormat::BL_FORMAT_PRGB32); // 0xAARRGGBB
    ctx.clear_all()?;
    let mut radial = BLGradient::new(&BLRadialGradientValues::new(
        (180, 180).into(), (180, 180).into(), (180.0, 0.)))?;
    radial.add_stop(0.0, 0xFFFFFFFF.into())?;
    radial.add_stop(1.0, 0xFFFF6F3F.into())?;

    ctx.fill_geometry_ext(&BLCircle::new((180, 180).into(), 160.0), &radial)?;

    let mut linear = BLGradient::new(&BLLinearGradientValues::new(
        (195, 195).into(), (470, 470).into()))?;
    linear.add_stop(0.0, 0xFFFFFFFF.into())?;
    linear.add_stop(1.0, 0xFF3F9FFF.into())?;

    ctx.set_comp_op(BLCompOp::BL_COMP_OP_DIFFERENCE);
    ctx.fill_geometry_ext(
        &BLRoundRect::new(&(195, 195, 270, 270).into(), 25.0), &linear)?;
    ctx.set_comp_op(BLCompOp::BL_COMP_OP_SRC_OVER);   // restore to default

    //let _ = img.write_to_file("target/logo_b2d.png");
    Ok(())
}

pub fn render_nodes(blctx: &mut BLContext, mouse: (f32, f32),
    parent: &usvg::Group, trfm: &usvg::Transform) -> Result<(), BLErr> {
    fn convert_paint(paint: &usvg::Paint, opacity: usvg::Opacity,
        _trfm: &usvg::Transform) -> Result<Option<Box<dyn B2DStyle>>, BLErr> {
        fn convert_stops(grad: &mut BLGradient, stops: &[usvg::Stop],
            opacity: usvg::Opacity) -> Result<(), BLErr> {
            stops.iter().try_for_each(|stop| {   let color = stop.color();
                let color = (color.red, color.green, color.blue,
                    (stop.opacity() * opacity).to_u8()).into();
                grad.add_stop(stop.offset().get() as _, color)
            })
        }

        Ok(Some(match paint { usvg::Paint::Pattern(_) => { // trfm should be applied here
                eprintln!("Not support pattern painting"); return Ok(None) }
            // https://github.com/RazrFalcon/resvg/blob/master/crates/resvg/src/path.rs#L179
            usvg::Paint::Color(color) => Box::new(BLSolidColor::init_rgba32(
                    (color.red, color.green, color.blue, opacity.to_u8()).into())?),

            usvg::Paint::LinearGradient(grad) => {
                let mut linear = BLGradient::new(&BLLinearGradientValues::new(
                    (grad.x1(), grad.y1()).into(), (grad.x2(), grad.y2()).into()))?;
                convert_stops(&mut linear, grad.stops(), opacity)?;     Box::new(linear)
            }
            usvg::Paint::RadialGradient(grad) => {
                let mut radial = BLGradient::new(&BLRadialGradientValues::new(
                    (grad.cx(), grad.cy()).into(), (grad.fx(), grad.fy()).into(),
                    (grad.r().get() as _, 0.)))?;
                    //(grad.cx() - grad.fx()).hypot(grad.cy() - grad.fy())
                convert_stops(&mut radial, grad.stops(), opacity)?;     Box::new(radial)
            }
        }))
    }

    for child in parent.children() { match child {
        usvg::Node::Group(group) =>     // trfm is needed on rendering only
            render_nodes(blctx, mouse, group, &trfm.pre_concat(group.transform()))?,

        usvg::Node::Path(path) => if path.is_visible() {
            let tpath = if trfm.is_identity() { None
            } else { path.data().clone().transform(*trfm) };    // XXX:
            let mut fpath = BLPath::new();

            for seg in tpath.as_ref().unwrap_or(path.data()).segments() {
                use usvg::tiny_skia_path::PathSegment::*;
                match seg {     Close => fpath.close(),
                    MoveTo(pt) => fpath.move_to((pt.x, pt.y).into()),
                    LineTo(pt) => fpath.line_to((pt.x, pt.y).into()),

                    QuadTo(cp, end) =>
                        fpath.quad_to ((cp.x, cp.y).into(), (end.x, end.y).into()),
                    CubicTo(c1, c2, end) =>
                        fpath.cubic_to((c1.x, c1.y).into(),
                                       (c2.x, c2.y).into(), (end.x, end.y).into()),
                }
            }

            let fpaint = if let Some(fill) = path.fill() {
                blctx.set_fill_rule(match fill.rule() {
                    usvg::FillRule::NonZero => BLFillRule::BL_FILL_RULE_NON_ZERO,
                    usvg::FillRule::EvenOdd => BLFillRule::BL_FILL_RULE_EVEN_ODD,
                }); convert_paint(fill.paint(), fill.opacity(), trfm)?
            } else { None };

            let lpaint = if let Some(stroke) = path.stroke() {
                blctx.set_stroke_miter_limit(stroke.miterlimit().get() as _);
                blctx.set_stroke_width(stroke.width().get() as _);

                blctx.set_stroke_join(match stroke.linejoin() {
                    usvg::LineJoin::MiterClip => BLStrokeJoin::BL_STROKE_JOIN_MITER_CLIP,
                    usvg::LineJoin::Miter => BLStrokeJoin::BL_STROKE_JOIN_MITER_BEVEL,
                    usvg::LineJoin::Round => BLStrokeJoin::BL_STROKE_JOIN_ROUND,
                    usvg::LineJoin::Bevel => BLStrokeJoin::BL_STROKE_JOIN_BEVEL,
                });
                blctx.set_stroke_caps(match stroke.linecap () {
                    usvg::LineCap::Butt   => BLStrokeCap::BL_STROKE_CAP_BUTT,
                    usvg::LineCap::Round  => BLStrokeCap::BL_STROKE_CAP_ROUND,
                    usvg::LineCap::Square => BLStrokeCap::BL_STROKE_CAP_SQUARE,
                }); convert_paint(stroke.paint(), stroke.opacity(), trfm)?
            } else { None };

            match path.paint_order() {
                usvg::PaintOrder::FillAndStroke => {
                    if let Some(paint) = fpaint {
                        blctx.fill_geometry_ext(&fpath, paint.as_ref())?;
                    }
                    if let Some(paint) = lpaint {
                        blctx.stroke_geometry_ext(&fpath, paint.as_ref())?;
                    }
                }
                usvg::PaintOrder::StrokeAndFill => {
                    if let Some(paint) = lpaint {
                        blctx.stroke_geometry_ext(&fpath, paint.as_ref())?;
                    }
                    if let Some(paint) = fpaint {
                        blctx.fill_geometry_ext(&fpath, paint.as_ref())?;
                    }
                }
            }

            if  matches!(fpath.hit_test(mouse.into(),
                BLFillRule::BL_FILL_RULE_NON_ZERO), BLHitTest::BL_HIT_TEST_IN) {
                blctx.set_stroke_width(2. / blctx.get_transform(1).get_scaling().0);
                blctx.stroke_geometry_rgba32(&fpath, (32, 240, 32, 128).into())?;
            }
        }

        usvg::Node::Image(img) => if img.is_visible() {
            match img.kind() {
                usvg::ImageKind::GIF(_) | usvg::ImageKind::WEBP(_) |
                usvg::ImageKind::PNG(_) | usvg::ImageKind::JPEG(_) =>
                    eprintln!("Raster SVG images are not supported by the Blend2D viewer"),
                // https://github.com/linebender/vello_svg/blob/main/src/lib.rs#L212
                usvg::ImageKind::SVG(svg) => render_nodes(blctx, mouse, svg.root(), trfm)?,
            }
        }

        usvg::Node::Text(text) => { let group = text.flattened();
            render_nodes(blctx, mouse, group, &trfm.pre_concat(group.transform()))?;
        }
    } }   Ok(())
}
