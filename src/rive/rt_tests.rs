
use super::*;
use std::io::Cursor;
use crate::rive::{decode::{FieldValue, Header, VarUInt},
    display_list::{CornerRadii, Path, PathCommand, Rect},
};

fn file(objects: Vec<Object>) -> RiveFile { RiveFile {
        header: Header {
            majorv: VarUInt(1), minorv: VarUInt(0),
            fileid: VarUInt(0), toc: Vec::new(),
        },  ocoll: objects,
} }

fn prop(object: &mut Object, id: u32, value: f32) {
    object.add_prop(VarUInt(id), FieldValue::Float32(value));
}

fn uint_prop(object: &mut Object, id: u32, value: u32) {
    object.add_prop(VarUInt(id), FieldValue::VarUInt(VarUInt(value)));
}

fn artboard() -> Object { Object::new_simple(object_ids::ARTBOARD) }

fn parented(type_id: u32, parent: u32) -> Object {
    let mut object = Object::new_simple(type_id);
    object.add_prop(VarUInt(property_ids::COMPONENT_PARENTID),
        FieldValue::VarUInt(VarUInt(parent)));  object
}

fn linear_animation(name: &[u8], duration: u32, fps: u32, loop_mode: u32) -> Object {
    let mut animation = Object::new_simple(object_ids::LINEAR_ANIMATION);
    animation.add_prop(VarUInt(property_ids::ANIMATION_NAME),
        FieldValue::Bytes(name.to_vec()));
    uint_prop(&mut animation, property_ids::LINEARANIMATION_DURATION, duration);
    uint_prop(&mut animation, property_ids::FPS, fps);
    uint_prop(&mut animation, property_ids::LOOPVALUE, loop_mode);
    animation
}

fn keyed_object(object_id: u32) -> Object {
    let mut object = Object::new_simple(object_ids::KEYED_OBJECT);
    uint_prop(&mut object, property_ids::KEYEDOBJECT_OBJECTID, object_id); object
}

fn keyed_property(prop_id: u32) -> Object {
    let mut object = Object::new_simple(object_ids::KEYED_PROPERTY);
    uint_prop(&mut object, property_ids::KEYEDPROPERTY_PROPERTYKEY, prop_id); object
}

fn double_keyframe(frame: u32, value: f32, interpolation: u32) -> Object {
    let mut keyframe = Object::new_simple(object_ids::KEY_FRAME_DOUBLE);
    uint_prop(&mut keyframe, property_ids::FRAME, frame);
    uint_prop(&mut keyframe,
        property_ids::INTERPOLATINGKEYFRAME_INTERPOLATIONTYPE, interpolation);
    prop(&mut keyframe, property_ids::KEYFRAMEDOUBLE_VALUE, value); keyframe
}

fn cubic_interpolator(x1: f32, y1: f32, x2: f32, y2: f32) -> Object {
    let mut interpolator = Object::new_simple(object_ids::CUBIC_INTERPOLATOR);
    prop(&mut interpolator, property_ids::CUBICINTERPOLATOR_X1, x1);
    prop(&mut interpolator, property_ids::CUBICINTERPOLATOR_Y1, y1);
    prop(&mut interpolator, property_ids::CUBICINTERPOLATOR_X2, x2);
    prop(&mut interpolator, property_ids::CUBICINTERPOLATOR_Y2, y2);
    interpolator
}

fn cubic_keyframe(frame: u32, value: f32, interpolator_id: u32) -> Object {
    let mut keyframe = double_keyframe(frame, value, 2);
    uint_prop(&mut keyframe, property_ids::INTERPOLATINGKEYFRAME_INTERPOLATORID,
        interpolator_id);   keyframe
}

#[test] fn emits_static_geometry_with_retained_parent_transforms() {
    let mut parent = parented(object_ids::NODE, 0);
    prop(&mut parent, property_ids::NODE_X, 10.0);
    prop(&mut parent, property_ids::NODE_Y, 20.0);

    let mut ellipse = parented(object_ids::ELLIPSE, 1);
    prop(&mut ellipse, property_ids::NODE_X, 5.0);
    prop(&mut ellipse, property_ids::PARAMETRICPATH_WIDTH,  40.0);
    prop(&mut ellipse, property_ids::PARAMETRICPATH_HEIGHT, 20.0);

    let runtime = Runtime::from_file(file(vec![artboard(), parent, ellipse])).unwrap();
    let list = runtime.display_list();
    assert_eq!(runtime.component_count(), 3);
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].shapes[0].trfm.tx, 15.0);
    assert_eq!(list.items[0].shapes[0].trfm.ty, 20.0);
    assert_eq!(list.items[0].shapes[0].geom,
        Geometry::Ellipse(Rect { x: -20.0, y: -10.0, w: 40.0, h: 20.0 }));
}

#[test] fn discovers_selects_and_advances_linear_animation() {
    let mut ellipse = parented(object_ids::ELLIPSE, 0);
    prop(&mut ellipse, property_ids::PARAMETRICPATH_WIDTH, 10.0);
    let objects = vec![artboard(), ellipse,
        linear_animation(b"move", 10, 10, 1),
        keyed_object(1), keyed_property(property_ids::NODE_X),
        double_keyframe(0, 0.0, 1), double_keyframe(10, 20.0, 1),
    ];
    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    assert_eq!(runtime.animation_count(), 1);
    assert_eq!(runtime.animation(0), Some(AnimationInfo {
        name: b"move", duration: 10, fps: 10, speed: 1.0, loop_mode: 1,
    }));
    assert!(runtime.set_animation_by_name(b"move").is_ok());

    assert!(runtime.advance(0.5));
    assert_eq!(runtime.elapsed(), 0.5);
    assert!((runtime.display_list().items[0].shapes[0].trfm.tx - 10.0)
        .abs() < 1e-6);
    assert!(runtime.advance(1.0));
    assert!((runtime.display_list().items[0].shapes[0].trfm.tx - 10.0)
        .abs() < 1e-6);
    assert!(!runtime.advance(0.0));
    assert!(matches!(runtime.set_animation(1), Err(RuntimeError::AnimationNotFound(1))));
}

#[test] fn cubic_keyframes_resolve_and_apply_their_interpolator() {
    let mut ellipse = parented(object_ids::ELLIPSE, 0);
    prop(&mut ellipse, property_ids::PARAMETRICPATH_WIDTH, 10.0);
    let objects = vec![artboard(), ellipse,
        cubic_interpolator(0.42, 0.0, 1.0, 1.0),
        linear_animation(b"ease", 10, 10, 0),
        keyed_object(1), keyed_property(property_ids::NODE_X),
        cubic_keyframe(0, 0.0, 2), double_keyframe(10, 20.0, 1),
    ];
    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    let x = runtime.display_list().items[0].shapes[0].trfm.tx;
    assert!((6.0..6.4).contains(&x), "{x}");
}

#[test] fn animates_stroke_width_and_trim_parameters() {
    let shape = parented(object_ids::SHAPE, 0);
    let path  = parented(object_ids::ELLIPSE, 1);
    let mut stroke = parented(object_ids::STROKE, 1);
    prop(&mut stroke, property_ids::THICKNESS, 0.0);
    let mut trim = parented(object_ids::TRIM_PATH, 3);
    prop(&mut trim, property_ids::TRIMPATH_START, 0.2);
    prop(&mut trim, property_ids::TRIMPATH_END, 0.8);
    prop(&mut trim, property_ids::TRIMPATH_OFFSET, -0.1);
    uint_prop(&mut trim, property_ids::TRIMPATH_MODEVALUE, 1);

    let mut objects = vec![artboard(), shape, path, stroke, trim,
        linear_animation(b"paint", 10, 10, 0), keyed_object(3)];
    objects.extend([keyed_property(property_ids::THICKNESS),
        double_keyframe(0, 0.0, 1), double_keyframe(10, 8.0, 1)]);
    objects.push(keyed_object(4));
    for (prop_id, from, to) in [
        (property_ids::TRIMPATH_START, 0.2, 0.6),
        (property_ids::TRIMPATH_END, 0.8, 0.4),
        (property_ids::TRIMPATH_OFFSET, -0.1, 0.3)] {
        objects.extend([keyed_property(prop_id), double_keyframe(0, from, 1),
            double_keyframe(10, to, 1)]);
    }

    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    let Some(Paint::Stroke { width, effects, .. }) =
        &runtime.display_list().items[0].paint else { panic!() };
    assert!((*width - 4.0).abs() < 1e-6);
    let PathEffect::Trim { start, end, offset, .. } = effects[0] else { panic!() };
    assert!((start - 0.4).abs() < 1e-6);
    assert!((end - 0.6).abs() < 1e-6);
    assert!((offset - 0.1).abs() < 1e-6);
}

#[test] fn switching_animation_restores_previous_targets() {
    let ellipse = parented(object_ids::ELLIPSE, 0);
    let objects = vec![artboard(), ellipse,
        linear_animation(b"x", 10, 10, 0), keyed_object(1),
        keyed_property(property_ids::NODE_X), double_keyframe(0, 20.0, 1),
        linear_animation(b"y", 10, 10, 0), keyed_object(1),
        keyed_property(property_ids::NODE_Y), double_keyframe(0, 30.0, 1)];
    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    assert_eq!(runtime.display_list().items[0].shapes[0].trfm.tx, 20.0);
    runtime.set_animation(1).unwrap();
    let transform = runtime.display_list().items[0].shapes[0].trfm;
    assert_eq!((transform.tx, transform.ty), (0.0, 30.0));
}

#[test] fn rejects_unknown_keyframe_interpolation_and_cubic_reference() {
    let animation_file = |interpolation, interpolator| file(vec![
        artboard(), parented(object_ids::ELLIPSE, 0),
        linear_animation(b"invalid", 10, 10, 0),
        keyed_object(1), keyed_property(property_ids::NODE_X), {
            let mut keyframe = double_keyframe(0, 0.0, interpolation);
            if let Some(id) = interpolator {
                uint_prop(&mut keyframe,
                    property_ids::INTERPOLATINGKEYFRAME_INTERPOLATORID, id);
            }   keyframe
        },
    ]);
    assert!(matches!(Runtime::from_file(animation_file(3, None)),
        Err(RuntimeError::InvalidInterpolation(3))));
    assert!(matches!(Runtime::from_file(animation_file(2, Some(99))),
        Err(RuntimeError::InvalidInterpolator(99))));
}

#[test] fn hold_keyframes_update_opacity_and_propagate_to_children() {
    let parent = parented(object_ids::NODE, 0);
    let mut ellipse = parented(object_ids::ELLIPSE, 1);
    prop(&mut ellipse, property_ids::PARAMETRICPATH_WIDTH, 10.0);
    let objects = vec![artboard(), parent, ellipse,
        linear_animation(b"hide", 10, 10, 0), keyed_object(1),
        keyed_property(property_ids::WORLDTRANSFORMCOMPONENT_OPACITY),
        double_keyframe(0, 1.0, 0), double_keyframe(5, 0.25, 0),
    ];
    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.25);
    assert_eq!(runtime.display_list().items[0].opacity, 1.0);
    runtime.advance(0.5);
    assert_eq!(runtime.display_list().items[0].opacity, 0.25);
}

#[test] fn selects_one_artboard_without_crossing_contexts() {
    let objects = || {
        let mut first = parented(object_ids::ELLIPSE, 0);
        prop(&mut first, property_ids::PARAMETRICPATH_WIDTH, 10.0);
        let mut second = parented(object_ids::ELLIPSE, 0);
        prop(&mut second, property_ids::PARAMETRICPATH_WIDTH, 20.0);
        vec![artboard(), first, artboard(), second]
    };

    let first = Runtime::from_file(file(objects())).unwrap();
    assert_eq!(first.artboard_object_index(), 0);
    assert_eq!(first.component_count(), 2);
    let second = Runtime::from_artboard(file(objects()), 1).unwrap();
    assert_eq!(second.artboard_object_index(), 2);
    assert_eq!(second.component_count(), 2);
    assert!(matches!(
        second.display_list().items[0].shapes[0].geom,
        Geometry::Ellipse(Rect { w: 20.0, .. })));
    assert!(matches!(Runtime::from_artboard(file(objects()), 2),
        Err(RuntimeError::ArtboardNotFound(2))));
}

#[test] fn rectangle_defaults_to_linked_corner_radii() {
    let mut rectangle = parented(object_ids::RECTANGLE, 0);
    prop(&mut rectangle, property_ids::PARAMETRICPATH_WIDTH,  20.0);
    prop(&mut rectangle, property_ids::PARAMETRICPATH_HEIGHT, 10.0);
    prop(&mut rectangle, property_ids::RECTANGLE_CORNERRADIUSTL, 3.0);

    let runtime = Runtime::from_file(file(vec![artboard(), rectangle])).unwrap();
    let Geometry::RoundedRect { radii, .. } =
        &runtime.display_list().items[0].shapes[0].geom else { panic!() };
    assert_eq!(*radii, CornerRadii {
        tl: 3.0, tr: 3.0, br: 3.0, bl: 3.0,
    });
}

#[test] fn builds_triangle_polygon_and_star_paths() {
    let mut triangle = parented(object_ids::TRIANGLE, 0);
    prop(&mut triangle, property_ids::PARAMETRICPATH_WIDTH,  20.0);
    prop(&mut triangle, property_ids::PARAMETRICPATH_HEIGHT, 10.0);

    let mut polygon = parented(object_ids::POLYGON, 0);
    prop(&mut polygon, property_ids::PARAMETRICPATH_WIDTH,  20.0);
    prop(&mut polygon, property_ids::PARAMETRICPATH_HEIGHT, 10.0);
    uint_prop(&mut polygon, property_ids::POINTS, 4);
    prop(&mut polygon, property_ids::CORNERRADIUS, 1.0);

    let mut star = parented(object_ids::STAR, 0);
    prop(&mut star, property_ids::PARAMETRICPATH_WIDTH,  20.0);
    prop(&mut star, property_ids::PARAMETRICPATH_HEIGHT, 10.0);
    uint_prop(&mut star, property_ids::POINTS, 5);
    prop(&mut star, property_ids::INNERRADIUS, 0.5);

    let runtime = Runtime::from_file(file(vec![artboard(),
        triangle, polygon, star])).unwrap();
    let list = runtime.display_list();
    let paths: Vec<_> = list.items.iter().map(|item| {
        let Geometry::Path(path) = &item.shapes[0].geom else { panic!() };
        path
    }).collect();
    assert_eq!(&*paths[0].cmd, &[
        PathCommand::MoveTo(Point { x: 0.0, y: -5.0 }),
        PathCommand::LineTo(Point { x: 10.0, y: 5.0 }),
        PathCommand::LineTo(Point { x: -10.0, y: 5.0 }),
        PathCommand::Close,
    ]);
    assert_eq!(paths[1].cmd.iter()
        .filter(|command| matches!(command, PathCommand::CubicTo { .. })).count(), 4);
    assert_eq!(paths[2].cmd.len(), 11);
    let PathCommand::MoveTo(first) = paths[2].cmd[0] else { panic!() };
    assert!(first.x.abs() < 1e-6 && first.y == -5.0);
}

#[test] fn resolves_parent_ids_through_component_indices() {
    let ignored = Object::new_simple(u32::MAX);
    let mut parent  = parented(object_ids::NODE, 0);
    prop(&mut parent, property_ids::NODE_X, 10.0);
    let mut ellipse = parented(object_ids::ELLIPSE, 2);
    prop(&mut ellipse, property_ids::NODE_X, 5.0);

    let runtime =
        Runtime::from_file(file(vec![artboard(), ignored, parent, ellipse])).unwrap();
    assert_eq!(runtime.display_list().items[0].shapes[0].trfm.tx, 15.0);
}

#[test] fn rejects_invalid_geometry_during_construction() {
    let mut ellipse = parented(object_ids::ELLIPSE, 0);
    ellipse.add_prop(VarUInt(property_ids::PARAMETRICPATH_WIDTH),
        FieldValue::VarUInt(VarUInt(10)));

    assert!(matches!(Runtime::from_file(file(vec![artboard(), ellipse])),
            Err(RuntimeError::Decode(DecodeError::PropTypeMismatch { .. }))));
}

#[test] fn rejects_excessive_parametric_vertex_counts() {
    let mut star = parented(object_ids::STAR, 0);
    uint_prop(&mut star, property_ids::POINTS, u32::from(u16::MAX));
    assert!(matches!(Runtime::from_file(file(vec![artboard(), star])),
        Err(RuntimeError::TooManyVertices(131_070))));
}

#[test] fn builds_points_path_with_fill_and_stroke() {
    let shape = parented(object_ids::SHAPE, 0);
    let mut path = parented(object_ids::POINTS_PATH, 1);
    path.add_prop(VarUInt(property_ids::POINTSCOMMONPATH_ISCLOSED),
        FieldValue::VarUInt(VarUInt(1)));

    let mut first  = parented(object_ids::STRAIGHT_VERTEX, 2);
    prop(&mut first, property_ids::VERTEX_X, 10.0);
    let mut second = parented(object_ids::STRAIGHT_VERTEX, 2);
    prop(&mut second, property_ids::VERTEX_Y, 20.0);

    let mut fill = parented(object_ids::FILL, 1);
    fill.add_prop(VarUInt(property_ids::FILL_FILLRULE),
        FieldValue::VarUInt(VarUInt(1)));
    let mut fill_color = parented(object_ids::SOLID_COLOR, 5);
    fill_color.add_prop(VarUInt(property_ids::SOLIDCOLOR_COLORVALUE),
        FieldValue::Color(0xff11_2233));

    let mut stroke = parented(object_ids::STROKE, 1);
    prop(&mut stroke, property_ids::THICKNESS, 3.0);
    let mut stroke_color = parented(object_ids::SOLID_COLOR, 7);
    stroke_color.add_prop(VarUInt(property_ids::SOLIDCOLOR_COLORVALUE),
        FieldValue::Color(0xff44_5566));

    let runtime = Runtime::from_file(file(vec![artboard(), shape, path, first, second,
        fill, fill_color, stroke, stroke_color])).unwrap();
    let list = runtime.display_list();
    assert_eq!(list.items.len(), 2);
    assert_eq!(list.items[0].paint, Some(Paint::Fill {
        brush: Brush::Solid(0xff11_2233), rule: FillRule::EvenOdd,
        effects: [].into(),
    }));
    assert_eq!(list.items[1].paint, Some(Paint::Stroke {
        brush: Brush::Solid(0xff44_5566), width: 3.0,
        cap: StrokeCap::Butt, join: StrokeJoin::Miter,
        trfm_scale: true, effects: [].into(),
    }));
    assert!(std::sync::Arc::ptr_eq(
        &list.items[0].shapes, &list.items[1].shapes));
    let (Geometry::Path(fill_path), Geometry::Path(stroke_path)) =
        (&list.items[0].shapes[0].geom,
         &list.items[1].shapes[0].geom) else { panic!() };
    assert!(std::sync::Arc::ptr_eq(&fill_path.cmd, &stroke_path.cmd));
    assert_eq!(list.items[0].shapes[0].geom,
        Geometry::Path(Path { cmd: vec![
        PathCommand::MoveTo(Point { x: 10.0, y:  0.0 }),
        PathCommand::LineTo(Point { x:  0.0, y: 20.0 }),
        PathCommand::Close,
    ].into() }));
}

#[test] fn preserves_trim_dash_and_stroke_transform_semantics() {
    let shape = parented(object_ids::SHAPE, 0);
    let path = parented(object_ids::POINTS_PATH, 1);
    let first = parented(object_ids::STRAIGHT_VERTEX, 2);
    let second = parented(object_ids::STRAIGHT_VERTEX, 2);
    let mut stroke = parented(object_ids::STROKE, 1);
    prop(&mut stroke, property_ids::THICKNESS, 2.0);
    uint_prop(&mut stroke, property_ids::TRANSFORMAFFECTSSTROKE, 0);

    let mut trim = parented(object_ids::TRIM_PATH, 5);
    prop(&mut trim, property_ids::TRIMPATH_START, 0.2);
    prop(&mut trim, property_ids::TRIMPATH_END, 0.8);
    prop(&mut trim, property_ids::TRIMPATH_OFFSET, -0.1);
    uint_prop(&mut trim, property_ids::TRIMPATH_MODEVALUE, 2);

    let mut dash_path = parented(object_ids::DASH_PATH, 5);
    prop(&mut dash_path, property_ids::DASHPATH_OFFSET, 0.25);
    uint_prop(&mut dash_path, property_ids::OFFSETISPERCENTAGE, 1);
    let mut dash = parented(object_ids::DASH, 7);
    prop(&mut dash, property_ids::DASH_LENGTH, 4.0);
    let mut gap = parented(object_ids::DASH, 7);
    prop(&mut gap, property_ids::DASH_LENGTH, 0.1);
    uint_prop(&mut gap, property_ids::LENGTHISPERCENTAGE, 1);

    let runtime = Runtime::from_file(file(vec![artboard(), shape, path, first, second,
        stroke, trim, dash_path, dash, gap])).unwrap();
    let Some(Paint::Stroke { width, trfm_scale: transform_affects, effects, .. }) =
        &runtime.display_list().items[0].paint else { panic!() };
    assert_eq!((*width, *transform_affects), (2.0, false));
    assert_eq!(&**effects, &[
        PathEffect::Trim { start: 0.2, end: 0.8, offset: -0.1,
            mode: TrimMode::Synchronized },
        PathEffect::Dash { offset: 0.25, relative: true, segments: vec![
            DashSegment { len: 4.0, relative: false },
            DashSegment { len: 0.1, relative: true },
        ].into() },
    ]);
}

#[test] fn omits_non_positive_strokes() {
    let shape = parented(object_ids::SHAPE, 0);
    let path = parented(object_ids::ELLIPSE, 1);
    let mut stroke = parented(object_ids::STROKE, 1);
    prop(&mut stroke, property_ids::THICKNESS, 0.0);
    let runtime = Runtime::from_file(file(vec![artboard(), shape, path, stroke])).unwrap();
    assert!(runtime.display_list().items[0].paint.is_none());
}

#[test] fn rejects_invalid_trim_modes() {
    let shape = parented(object_ids::SHAPE, 0);
    let path = parented(object_ids::ELLIPSE, 1);
    let stroke = parented(object_ids::STROKE, 1);
    let mut trim = parented(object_ids::TRIM_PATH, 3);
    uint_prop(&mut trim, property_ids::TRIMPATH_MODEVALUE, 3);
    assert!(matches!(Runtime::from_file(file(vec![artboard(), shape, path, stroke, trim])),
        Err(RuntimeError::InvalidTrimMode(3))));
}

#[test] fn rejects_parent_cycles_during_construction() {
    let first  = parented(object_ids::NODE, 2);
    let second = parented(object_ids::NODE, 1);

    assert!(matches!(Runtime::from_file(file(vec![artboard(), first, second])),
        Err(RuntimeError::ParentCycle(2 | 3))));
}

#[test] fn combines_shape_paths_before_applying_paint() {
    let shape = parented(object_ids::SHAPE, 0);
    let path1 = parented(object_ids::POINTS_PATH, 1);
    let vertex1 = parented(object_ids::STRAIGHT_VERTEX, 2);
    let vertex2 = parented(object_ids::STRAIGHT_VERTEX, 2);

    let mut path2 = parented(object_ids::POINTS_PATH, 1);
    uint_prop(&mut path2, property_ids::ISHOLE, 1);
    let vertex3 = parented(object_ids::STRAIGHT_VERTEX, 5);
    let vertex4 = parented(object_ids::STRAIGHT_VERTEX, 5);

    let fill  = parented(object_ids::FILL, 1);
    let color = parented(object_ids::SOLID_COLOR, 8);

    let runtime = Runtime::from_file(file(vec![artboard(), shape,
        path1, vertex1, vertex2, path2, vertex3, vertex4, fill, color])).unwrap();
    let list = runtime.display_list();
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].shapes.len(), 2);
    assert!(!list.items[0].shapes[0].is_hole);
    assert!( list.items[0].shapes[1].is_hole);
    assert!(matches!(list.items[0].paint, Some(Paint::Fill { .. })));
}

#[test] fn draw_rules_move_shape_after_target() {
    let owner = parented(object_ids::NODE, 0);
    let moved = parented(object_ids::SHAPE, 1);
    let moved_geometry = parented(object_ids::ELLIPSE, 2);
    let mut rules = parented(object_ids::DRAW_RULES, 1);
    uint_prop(&mut rules, property_ids::DRAWTARGETID, 5);
    let mut target = parented(object_ids::DRAW_TARGET, 4);
    uint_prop(&mut target, property_ids::DRAWABLEID, 6);
    uint_prop(&mut target, property_ids::PLACEMENTVALUE, 1);
    let target_shape = parented(object_ids::SHAPE, 0);
    let target_geometry = parented(object_ids::ELLIPSE, 6);

    let runtime = Runtime::from_file(file(vec![artboard(), owner, moved, moved_geometry,
        rules, target, target_shape, target_geometry])).unwrap();
    assert_eq!(runtime.display_list().items.iter()
        .map(|item| item.obj_idx).collect::<Vec<_>>(), [6, 2]);
}

#[test] fn nested_draw_rules_move_attached_blocks_together() {
    let owner_a = parented(object_ids::NODE, 0);
    let shape_a = parented(object_ids::SHAPE, 1);
    let geometry_a = parented(object_ids::ELLIPSE, 2);
    let mut rules_a = parented(object_ids::DRAW_RULES, 1);
    uint_prop(&mut rules_a, property_ids::DRAWTARGETID, 5);
    let mut target_a = parented(object_ids::DRAW_TARGET, 4);
    uint_prop(&mut target_a, property_ids::DRAWABLEID, 7);
    uint_prop(&mut target_a, property_ids::PLACEMENTVALUE, 1);

    let owner_b = parented(object_ids::NODE, 0);
    let shape_b = parented(object_ids::SHAPE, 6);
    let geometry_b = parented(object_ids::ELLIPSE, 7);
    let mut rules_b = parented(object_ids::DRAW_RULES, 6);
    uint_prop(&mut rules_b, property_ids::DRAWTARGETID, 10);
    let mut target_b = parented(object_ids::DRAW_TARGET, 9);
    uint_prop(&mut target_b, property_ids::DRAWABLEID, 11);
    uint_prop(&mut target_b, property_ids::PLACEMENTVALUE, 1);

    let shape_c = parented(object_ids::SHAPE, 0);
    let geometry_c = parented(object_ids::ELLIPSE, 11);
    let runtime = Runtime::from_file(file(vec![artboard(),
        owner_a, shape_a, geometry_a, rules_a, target_a,
        owner_b, shape_b, geometry_b, rules_b, target_b,
        shape_c, geometry_c])).unwrap();
    assert_eq!(runtime.display_list().items.iter()
        .map(|item| item.obj_idx).collect::<Vec<_>>(), [11, 7, 2]);
}

#[test] fn rejects_draw_rule_cycles() {
    let owner_a = parented(object_ids::NODE, 0);
    let shape_a = parented(object_ids::SHAPE, 1);
    let geometry_a = parented(object_ids::ELLIPSE, 2);
    let mut rules_a = parented(object_ids::DRAW_RULES, 1);
    uint_prop(&mut rules_a, property_ids::DRAWTARGETID, 5);
    let mut target_a = parented(object_ids::DRAW_TARGET, 4);
    uint_prop(&mut target_a, property_ids::DRAWABLEID, 7);

    let owner_b = parented(object_ids::NODE, 0);
    let shape_b = parented(object_ids::SHAPE, 6);
    let geometry_b = parented(object_ids::ELLIPSE, 7);
    let mut rules_b = parented(object_ids::DRAW_RULES, 6);
    uint_prop(&mut rules_b, property_ids::DRAWTARGETID, 10);
    let mut target_b = parented(object_ids::DRAW_TARGET, 9);
    uint_prop(&mut target_b, property_ids::DRAWABLEID, 2);

    assert!(matches!(Runtime::from_file(file(vec![artboard(),
        owner_a, shape_a, geometry_a, rules_a, target_a,
        owner_b, shape_b, geometry_b, rules_b, target_b])),
        Err(RuntimeError::DrawOrderCycle(2 | 7))));
}

#[test] fn inherits_shape_opacity_without_rewriting_color_alpha() {
    let mut parent = parented(object_ids::NODE, 0);
    prop(&mut parent, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY, 0.5);
    let mut shape = parented(object_ids::SHAPE, 1);
    prop(&mut shape, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY, 0.4);
    let ellipse = parented(object_ids::ELLIPSE, 2);
    let fill = parented(object_ids::FILL, 2);
    let mut color = parented(object_ids::SOLID_COLOR, 4);
    color.add_prop(VarUInt(property_ids::SOLIDCOLOR_COLORVALUE),
        FieldValue::Color(0x8011_2233));

    let runtime = Runtime::from_file(file(vec![
        artboard(), parent, shape, ellipse, fill, color])).unwrap();
    let item = &runtime.display_list().items[0];
    assert!((item.opacity - 0.2).abs() < f32::EPSILON);
    assert!(matches!(&item.paint,
        Some(Paint::Fill { brush: Brush::Solid(0x8011_2233), .. })));
}

#[test] fn builds_sorted_linear_gradient_in_shape_space() {
    let mut shape = parented(object_ids::SHAPE, 0);
    prop(&mut shape, property_ids::NODE_X, 12.0);
    prop(&mut shape, property_ids::NODE_Y, 34.0);
    let ellipse = parented(object_ids::ELLIPSE, 1);
    let fill = parented(object_ids::FILL, 1);
    let mut gradient = parented(object_ids::LINEAR_GRADIENT, 3);
    prop(&mut gradient, property_ids::STARTX, 1.0);
    prop(&mut gradient, property_ids::STARTY, 2.0);
    prop(&mut gradient, property_ids::ENDX, 8.0);
    prop(&mut gradient, property_ids::ENDY, 9.0);
    prop(&mut gradient, property_ids::LINEARGRADIENT_OPACITY, 0.5);
    let mut last = parented(object_ids::GRADIENT_STOP, 4);
    prop(&mut last, property_ids::POSITION, 1.5);
    last.add_prop(VarUInt(property_ids::GRADIENTSTOP_COLORVALUE),
        FieldValue::Color(0xffaa_bbcc));
    let mut first = parented(object_ids::GRADIENT_STOP, 4);
    prop(&mut first, property_ids::POSITION, -0.5);
    first.add_prop(VarUInt(property_ids::GRADIENTSTOP_COLORVALUE),
        FieldValue::Color(0xff11_2233));

    let runtime = Runtime::from_file(file(vec![
        artboard(), shape, ellipse, fill, gradient, last, first])).unwrap();
    let Some(Paint::Fill { brush: Brush::LinearGradient {
        start, end, trfm: transform, opacity, stops }, ..
    }) = &runtime.display_list().items[0].paint else { panic!() };
    assert_eq!((*start, *end), (Point { x: 1.0, y: 2.0 }, Point { x: 8.0, y: 9.0 }));
    assert_eq!((transform.tx, transform.ty), (12.0, 34.0));
    assert_eq!(*opacity, 0.5);
    assert_eq!(&**stops, &[GradientStop { pos: 0.0, color: 0xff11_2233 },
                           GradientStop { pos: 1.0, color: 0xffaa_bbcc }, ]);
}

#[test] fn builds_radial_gradient_radius_from_end_point() {
    let ellipse = parented(object_ids::ELLIPSE, 1);
    let shape = parented(object_ids::SHAPE, 0);
    let fill  = parented(object_ids::FILL, 1);
    let mut gradient = parented(object_ids::RADIAL_GRADIENT, 3);
    prop(&mut gradient, property_ids::STARTX, 2.0);
    prop(&mut gradient, property_ids::STARTY, 3.0);
    prop(&mut gradient, property_ids::ENDX, 5.0);
    prop(&mut gradient, property_ids::ENDY, 7.0);

    let runtime = Runtime::from_file(file(vec![
        artboard(), shape, ellipse, fill, gradient])).unwrap();
    assert!(matches!(&runtime.display_list().items[0].paint,
        Some(Paint::Fill { brush: Brush::RadialGradient {
            center: Point { x: 2.0, y: 3.0 }, radius: 5.0, ..
        }, .. })));
}

#[test] fn omits_fully_transparent_shape() {
    let mut shape = parented(object_ids::SHAPE, 0);
    prop(&mut shape, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY, 0.0);
    let ellipse = parented(object_ids::ELLIPSE, 1);

    let runtime = Runtime::from_file(file(vec![artboard(), shape, ellipse])).unwrap();
    assert!(runtime.display_list().items.is_empty());
}

#[test] fn imports_repository_sample() {
    let mut input = Cursor::new(include_bytes!("../../data/rating-animation.riv"));
    let file = RiveFile::read(&mut input).unwrap();
    let mut runtime = Runtime::from_file(file).unwrap();
    assert!(0 < runtime.component_count());
    assert!(0 < runtime.animation_count());
    runtime.set_animation(0).unwrap();
    assert!(runtime.advance(1.0 / 60.0));
    let list =  runtime.display_list();
    assert!(list.items.iter().flat_map(|item| item.shapes.iter())
        .any(|geometry| matches!(&geometry.geom, Geometry::Path(_))));
    assert!(list.items.iter().any(|item| item.paint.is_some()));
}
