
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

fn display_list(runtime: &Runtime) -> DisplayList {
    let mut list = DisplayList::default();
    runtime.write_display_list(&mut list);  list
}

fn prop(object: &mut Object, id: u32, value: f32) {
    object.add_prop(VarUInt(id), FieldValue::Float32(value));
}

fn uint_prop(object: &mut Object, id: u32, value: u32) {
    object.add_prop(VarUInt(id), FieldValue::VarUInt(VarUInt(value)));
}

fn constraint(type_id: u32, owner: u32, target: u32) -> Object {
    let mut object = parented(type_id, owner);
    uint_prop(&mut object, property_ids::TARGETEDCONSTRAINT_TARGETID, target);
    object
}

fn world(runtime: &Runtime, obj_idx: u32) -> Affine2 {
    runtime.components.iter().find(|component| component.obj_idx == obj_idx).unwrap().world
}

fn artboard() -> Object { Object::new_simple(object_ids::ARTBOARD) }

fn embedded_image_file(mut objects: Vec<Object>, bytes: &[u8]) -> RiveFile {
    let image_asset = Object::new_simple(object_ids::IMAGE_ASSET);
    let mut contents = Object::new_simple(object_ids::FILE_ASSET_CONTENTS);
    contents.add_prop(VarUInt(property_ids::BYTES), FieldValue::Bytes(bytes.to_vec()));
    let mut all = vec![image_asset, contents];
    all.append(&mut objects); file(all)
}

#[test] fn exposes_artboard_size() {
    let mut artboard = artboard();
    prop(&mut artboard, property_ids::LAYOUTCOMPONENT_WIDTH, 640.0);
    prop(&mut artboard, property_ids::LAYOUTCOMPONENT_HEIGHT, 360.0);
    assert_eq!(Runtime::from_file(file(vec![artboard])).unwrap().artboard_size(),
        (640.0, 360.0));
}

#[test] fn emits_embedded_image_instances() {
    let mut image = parented(object_ids::IMAGE, 0);
    uint_prop(&mut image, property_ids::IMAGE_ASSETID, 0);
    prop(&mut image, property_ids::NODE_X, 30.0);
    prop(&mut image, property_ids::NODE_Y, 40.0);
    prop(&mut image, property_ids::IMAGE_ORIGINX, 0.25);
    prop(&mut image, property_ids::IMAGE_ORIGINY, 0.75);
    let runtime = Runtime::from_file(
        embedded_image_file(vec![artboard(), image], b"encoded-image")).unwrap();
    let list = display_list(&runtime);
    let image = list[0].image.as_ref().unwrap();
    assert_eq!(image.asset_id, 0);
    assert_eq!(&*image.data, b"encoded-image");
    assert_eq!(image.origin, Point { x: 0.25, y: 0.75 });
    assert_eq!((image.trfm.tx, image.trfm.ty), (30.0, 40.0));
    assert!(runtime.is_fully_supported());
}

#[test] fn reports_missing_image_assets_and_animates_origin() {
    let mut missing = parented(object_ids::IMAGE, 0);
    uint_prop(&mut missing, property_ids::IMAGE_ASSETID, 1);
    let runtime = Runtime::from_file(
        embedded_image_file(vec![artboard(), missing], b"image")).unwrap();
    assert_eq!(runtime.unsupported_features(), &[UnsupportedFeature::Images]);
    assert!(display_list(&runtime).is_empty());

    let mut image = parented(object_ids::IMAGE, 0);
    uint_prop(&mut image, property_ids::IMAGE_ASSETID, 0);
    let objects = vec![artboard(), image, linear_animation(b"origin", 10, 10, 0),
        keyed_object(1), keyed_property(property_ids::IMAGE_ORIGINX),
        double_keyframe(0, 0.0, 1), double_keyframe(10, 1.0, 1)];
    let mut runtime = Runtime::from_file(embedded_image_file(objects, b"image")).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    assert!((display_list(&runtime)[0].image.as_ref().unwrap().origin.x - 0.5).abs() < 1e-5);
}

#[test] fn reports_textured_mesh_images_as_unsupported() {
    let mut image = parented(object_ids::IMAGE, 0);
    uint_prop(&mut image, property_ids::IMAGE_ASSETID, 0);
    let mesh = parented(object_ids::MESH, 1);
    let runtime = Runtime::from_file(
        embedded_image_file(vec![artboard(), image, mesh], b"image")).unwrap();
    assert_eq!(runtime.unsupported_features(), &[UnsupportedFeature::Images]);
    assert!(display_list(&runtime).is_empty());
}

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

fn color_keyframe(frame: u32, value: u32, interpolation: u32) -> Object {
    let mut keyframe = Object::new_simple(object_ids::KEY_FRAME_COLOR);
    uint_prop(&mut keyframe, property_ids::FRAME, frame);
    uint_prop(&mut keyframe,
        property_ids::INTERPOLATINGKEYFRAME_INTERPOLATIONTYPE, interpolation);
    keyframe.add_prop(VarUInt(property_ids::KEYFRAMECOLOR_VALUE),
        FieldValue::Color(value));   keyframe
}

fn bool_keyframe(frame: u32, value: bool) -> Object {
    let mut keyframe = Object::new_simple(object_ids::KEY_FRAME_BOOL);
    uint_prop(&mut keyframe, property_ids::FRAME, frame);
    keyframe.add_prop(VarUInt(property_ids::KEYFRAMEBOOL_VALUE),
        FieldValue::VarUInt(VarUInt(u32::from(value))));   keyframe
}

fn uint_keyframe(frame: u32, value: u32) -> Object {
    let mut keyframe = Object::new_simple(object_ids::KEY_FRAME_UINT);
    uint_prop(&mut keyframe, property_ids::FRAME, frame);
    uint_prop(&mut keyframe, property_ids::KEYFRAMEUINT_VALUE, value); keyframe
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

fn clipped_scene() -> Vec<Object> {
    let source = parented(object_ids::SHAPE, 0);
    let mut source_path = parented(object_ids::ELLIPSE, 1);
    prop(&mut source_path, property_ids::NODE_X, 12.0);
    prop(&mut source_path, property_ids::PARAMETRICPATH_WIDTH, 20.0);
    prop(&mut source_path, property_ids::PARAMETRICPATH_HEIGHT, 10.0);
    let owner = parented(object_ids::NODE, 0);
    let mut clip = parented(object_ids::CLIPPING_SHAPE, 3);
    uint_prop(&mut clip, property_ids::SOURCEID, 1);
    uint_prop(&mut clip, property_ids::CLIPPINGSHAPE_FILLRULE, 1);
    let target = parented(object_ids::SHAPE, 3);
    let target_path = parented(object_ids::RECTANGLE, 5);
    let fill = parented(object_ids::FILL, 5);
    vec![artboard(), source, source_path, owner, clip, target, target_path, fill]
}

#[test] fn emits_clipping_path_for_owner_subtree() {
    let runtime = Runtime::from_file(file(clipped_scene())).unwrap();
    let list = display_list(&runtime);
    let target = list.iter().find(|item| item.obj_idx == 5).unwrap();
    assert_eq!(target.clips.len(), 1);
    assert_eq!(target.clips[0].rule, FillRule::EvenOdd);
    assert_eq!(target.clips[0].shapes.len(), 1);
    assert_eq!(target.clips[0].shapes[0].obj_idx, 2);
    assert_eq!(target.clips[0].shapes[0].trfm.tx, 12.0);
    assert!(list.iter().find(|item| item.obj_idx == 1).unwrap().clips.is_empty());
}

#[test] fn animates_clipping_visibility() {
    let mut objects = clipped_scene();
    objects.extend([linear_animation(b"clip", 10, 10, 0), keyed_object(4),
        keyed_property(property_ids::CLIPPINGSHAPE_ISVISIBLE),
        bool_keyframe(0, true), bool_keyframe(10, false)]);
    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    assert_eq!(display_list(&runtime).iter()
        .find(|item| item.obj_idx == 5).unwrap().clips.len(), 1);
    runtime.advance(1.0);
    assert!(display_list(&runtime).iter()
        .find(|item| item.obj_idx == 5).unwrap().clips.is_empty());
}

#[test] fn rejects_missing_clipping_source() {
    let mut clip = parented(object_ids::CLIPPING_SHAPE, 0);
    uint_prop(&mut clip, property_ids::SOURCEID, 99);
    assert!(matches!(Runtime::from_file(file(vec![artboard(), clip])),
        Err(RuntimeError::InvalidClipSource { source_id: 99, .. })));
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
    let list = display_list(&runtime);
    assert_eq!(runtime.component_count(), 3);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].shapes[0].trfm.tx, 15.0);
    assert_eq!(list[0].shapes[0].trfm.ty, 20.0);
    assert_eq!(list[0].shapes[0].geom,
        Geometry::Ellipse(Rect { x: -20.0, y: -10.0, w: 40.0, h: 20.0 }));
}

#[test] fn reports_unsupported_rive_subsystems_once() {
    let runtime = Runtime::from_file(file(vec![artboard(),
        parented(object_ids::BONE,  0), parented(object_ids::SKIN, 0),
        parented(object_ids::IMAGE, 0), parented(object_ids::NESTED_ARTBOARD, 0),
        parented(object_ids::STATE_MACHINE,  0), parented(object_ids::TEXT, 0),
        parented(object_ids::I_K_CONSTRAINT, 0),
    ])).unwrap();
    assert_eq!(runtime.unsupported_features(), &[
        UnsupportedFeature::BonesAndSkins, UnsupportedFeature::AdvancedConstraints,
        UnsupportedFeature::Images, UnsupportedFeature::NestedArtboards,
        UnsupportedFeature::StateMachines, UnsupportedFeature::Text,
    ]);
    assert!(!runtime.is_fully_supported());
    assert!(Runtime::from_file(file(vec![artboard()])).unwrap().is_fully_supported());
}

#[test] fn applies_translation_rotation_and_scale_constraints() {
    let mut target = parented(object_ids::NODE, 0);
    prop(&mut target, property_ids::NODE_X, 100.0);
    prop(&mut target, property_ids::NODE_Y, 40.0);
    prop(&mut target, property_ids::TRANSFORMCOMPONENT_ROTATION, 1.0);
    prop(&mut target, property_ids::TRANSFORMCOMPONENT_SCALEX, 3.0);
    prop(&mut target, property_ids::TRANSFORMCOMPONENT_SCALEY, 5.0);
    let owner = parented(object_ids::SHAPE, 0);
    let ellipse = parented(object_ids::ELLIPSE, 2);
    let mut translation = constraint(object_ids::TRANSLATION_CONSTRAINT, 2, 1);
    prop(&mut translation, property_ids::CONSTRAINT_STRENGTH, 0.5);
    let rotation = constraint(object_ids::ROTATION_CONSTRAINT, 2, 1);
    let scale = constraint(object_ids::SCALE_CONSTRAINT, 2, 1);

    let runtime = Runtime::from_file(file(vec![artboard(), target, owner, ellipse,
        translation, rotation, scale])).unwrap();
    let matrix = world(&runtime, 2);
    assert!((matrix.tx - 50.0).abs() < 1e-5);
    assert!((matrix.ty - 20.0).abs() < 1e-5);
    assert!((matrix.yx.atan2(matrix.xx) - 1.0).abs() < 1e-5);
    assert!((matrix.xx.hypot(matrix.yx) - 3.0).abs() < 1e-5);
    assert!((world(&runtime, 3).tx - matrix.tx).abs() < 1e-5);
}

#[test] fn applies_constraint_limits_and_dependency_order() {
    let   owner = parented(object_ids::SHAPE, 0);
    let ellipse = parented(object_ids::ELLIPSE, 1);
    let  middle = parented(object_ids::NODE, 0);
    let mut target = parented(object_ids::NODE, 0);
    prop(&mut target, property_ids::NODE_X, 100.0);
    // Deliberately store the dependent constraint first.
    let mut dependent = constraint(object_ids::TRANSLATION_CONSTRAINT, 1, 3);
    uint_prop(&mut dependent, property_ids::MAX, 1);
    prop(&mut dependent, property_ids::MAXVALUE, 60.0);
    let source = constraint(object_ids::TRANSLATION_CONSTRAINT, 3, 4);

    let runtime = Runtime::from_file(file(vec![artboard(), owner, ellipse, middle,
        target, dependent, source])).unwrap();
    assert!((world(&runtime, 3).tx - 100.0).abs() < 1e-5);
    assert!((world(&runtime, 1).tx - 60.0).abs() < 1e-5);
    assert!((world(&runtime, 2).tx - 60.0).abs() < 1e-5);
}

#[test] fn animates_constraint_strength() {
    let mut target = parented(object_ids::NODE, 0);
    prop(&mut target, property_ids::NODE_X, 100.0);
    let   owner = parented(object_ids::SHAPE, 0);
    let ellipse = parented(object_ids::ELLIPSE, 2);
    let translation = constraint(object_ids::TRANSLATION_CONSTRAINT, 2, 1);
    let objects = vec![artboard(), target, owner, ellipse, translation,
        linear_animation(b"constraint", 10, 10, 0), keyed_object(4),
        keyed_property(property_ids::CONSTRAINT_STRENGTH),
        double_keyframe(0, 0.0, 1), double_keyframe(10, 1.0, 1)];
    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    assert!(world(&runtime, 2).tx.abs() < 1e-5);
    runtime.advance(0.5);
    assert!((world(&runtime, 2).tx - 50.0).abs() < 1e-5);
}

#[test] fn applies_transform_constraint_and_artboard_origin() {
    let mut artboard = artboard();
    prop(&mut artboard, property_ids::LAYOUTCOMPONENT_WIDTH, 200.0);
    prop(&mut artboard, property_ids::LAYOUTCOMPONENT_HEIGHT, 100.0);
    let   owner = parented(object_ids::SHAPE, 0);
    let ellipse = parented(object_ids::ELLIPSE, 1);
    let mut constraint = constraint(object_ids::TRANSFORM_CONSTRAINT, 1, 0);
    prop(&mut constraint, property_ids::TRANSFORMCONSTRAINT_ORIGINX, 0.5);
    prop(&mut constraint, property_ids::TRANSFORMCONSTRAINT_ORIGINY, 0.5);
    prop(&mut constraint, property_ids::CONSTRAINT_STRENGTH, 0.5);
    let runtime = Runtime::from_file(file(vec![
        artboard, owner, ellipse, constraint])).unwrap();
    let matrix = world(&runtime, 1);
    assert!((matrix.tx - 50.0).abs() < 1e-5);
    assert!((matrix.ty - 25.0).abs() < 1e-5);
    assert!((world(&runtime, 2).tx - 50.0).abs() < 1e-5);
}

#[test] fn transform_constraint_interpolates_the_complete_matrix() {
    let mut target = parented(object_ids::NODE, 0);
    prop(&mut target, property_ids::NODE_X, 100.0);
    prop(&mut target, property_ids::NODE_Y, 40.0);
    prop(&mut target, property_ids::TRANSFORMCOMPONENT_ROTATION, 1.0);
    prop(&mut target, property_ids::TRANSFORMCOMPONENT_SCALEX, 3.0);
    prop(&mut target, property_ids::TRANSFORMCOMPONENT_SCALEY, 5.0);
    let   owner = parented(object_ids::SHAPE, 0);
    let ellipse = parented(object_ids::ELLIPSE, 2);
    let mut constraint = constraint(object_ids::TRANSFORM_CONSTRAINT, 2, 1);
    prop(&mut constraint, property_ids::CONSTRAINT_STRENGTH, 0.5);
    let runtime = Runtime::from_file(file(vec![
        artboard(), target, owner, ellipse, constraint])).unwrap();
    let matrix = world(&runtime, 2);
    assert!((matrix.tx - 50.0).abs() < 1e-5);
    assert!((matrix.ty - 20.0).abs() < 1e-5);
    assert!((matrix.yx.atan2(matrix.xx) - 0.5).abs() < 1e-5);
    assert!((matrix.xx.hypot(matrix.yx) - 2.0).abs() < 1e-5);
    assert!((matrix.xy.hypot(matrix.yy) - 3.0).abs() < 1e-5);
}

#[test] fn applies_distance_constraint_modes() {
    fn constrained(mode: u32, owner_x: f32, distance: f32) -> f32 {
        let  target = parented(object_ids::NODE, 0);
        let mut owner = parented(object_ids::SHAPE, 0);
        prop(&mut owner, property_ids::NODE_X, owner_x);
        let ellipse = parented(object_ids::ELLIPSE, 2);
        let mut constraint = constraint(object_ids::DISTANCE_CONSTRAINT, 2, 1);
        prop(&mut constraint, property_ids::DISTANCECONSTRAINT_DISTANCE, distance);
        uint_prop(&mut constraint, property_ids::DISTANCECONSTRAINT_MODEVALUE, mode);
        world(&Runtime::from_file(file(vec![
            artboard(), target, owner, ellipse, constraint])).unwrap(), 2).tx
    }

    assert!((constrained(2, 40.0, 100.0) - 100.0).abs() < 1e-5);
    assert!((constrained(0, 40.0, 100.0) - 40.0).abs() < 1e-5);
    assert!((constrained(0, 140.0, 100.0) - 100.0).abs() < 1e-5);
    assert!((constrained(1, 40.0, 100.0) - 100.0).abs() < 1e-5);
    assert!((constrained(1, 140.0, 100.0) - 140.0).abs() < 1e-5);
}

#[test] fn animates_distance_constraint() {
    let  target = parented(object_ids::NODE, 0);
    let mut owner = parented(object_ids::SHAPE, 0);
    prop(&mut owner, property_ids::NODE_X, 100.0);
    let ellipse = parented(object_ids::ELLIPSE, 2);
    let mut distance = constraint(object_ids::DISTANCE_CONSTRAINT, 2, 1);
    uint_prop(&mut distance, property_ids::DISTANCECONSTRAINT_MODEVALUE, 2);
    let objects = vec![artboard(), target, owner, ellipse, distance,
        linear_animation(b"distance", 10, 10, 0), keyed_object(4),
        keyed_property(property_ids::DISTANCECONSTRAINT_DISTANCE),
        double_keyframe(0, 100.0, 1), double_keyframe(10, 200.0, 1)];
    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    assert!((world(&runtime, 2).tx - 150.0).abs() < 1e-5);
}

#[test] fn rejects_invalid_and_cyclic_constraints() {
    let  owner = parented(object_ids::NODE, 0);
    let invalid = constraint(object_ids::TRANSLATION_CONSTRAINT, 1, 1);
    assert!(matches!(Runtime::from_file(file(vec![artboard(), owner, invalid])),
        Err(RuntimeError::InvalidConstraintTarget(1))));

    let  first = parented(object_ids::NODE, 0);
    let second = parented(object_ids::NODE, 0);
    let first_constraint = constraint(object_ids::TRANSLATION_CONSTRAINT, 1, 2);
    let second_constraint = constraint(object_ids::TRANSLATION_CONSTRAINT, 2, 1);
    assert!(matches!(Runtime::from_file(file(vec![artboard(), first, second,
        first_constraint, second_constraint])), Err(RuntimeError::ConstraintCycle(_))));
}

#[test] fn targetless_constraint_still_applies_limits() {
    let mut owner = parented(object_ids::SHAPE, 0);
    prop(&mut owner, property_ids::NODE_X, 100.0);
    let   ellipse = parented(object_ids::ELLIPSE, 1);
    let mut limit = parented(object_ids::TRANSLATION_CONSTRAINT, 1);
    uint_prop(&mut limit, property_ids::MAX, 1);
    prop(&mut limit, property_ids::MAXVALUE, 60.0);
    let runtime = Runtime::from_file(file(vec![artboard(), owner, ellipse, limit])).unwrap();
    assert!((world(&runtime, 1).tx - 60.0).abs() < 1e-5);
    assert!((world(&runtime, 2).tx - 60.0).abs() < 1e-5);
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
    assert!((display_list(&runtime)[0].shapes[0].trfm.tx - 10.0)
        .abs() < 1e-6);
    assert!(runtime.advance(1.0));
    assert!((display_list(&runtime)[0].shapes[0].trfm.tx - 10.0)
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
    let x = display_list(&runtime)[0].shapes[0].trfm.tx;
    assert!((6.0..6.4).contains(&x), "{x}");
}

#[test] fn animates_stroke_width_and_trim_parameters() {
    let shape = parented(object_ids::SHAPE, 0);
    let  path = parented(object_ids::ELLIPSE, 1);
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
        &display_list(&runtime)[0].paint else { panic!() };
    assert!((*width - 4.0).abs() < 1e-6);
    let PathEffect::Trim { start, end, offset, .. } = effects[0] else { panic!() };
    assert!((start - 0.4).abs() < 1e-6);
    assert!((end - 0.6).abs() < 1e-6);
    assert!((offset - 0.1).abs() < 1e-6);
}

#[test] fn animates_parametric_geometry() {
    let mut rectangle = parented(object_ids::RECTANGLE, 0);
    prop(&mut rectangle, property_ids::PARAMETRICPATH_WIDTH, 10.0);
    prop(&mut rectangle, property_ids::PARAMETRICPATH_HEIGHT, 20.0);
    prop(&mut rectangle, property_ids::RECTANGLE_CORNERRADIUSTL, 1.0);
    let mut objects = vec![artboard(), rectangle,
        linear_animation(b"geometry", 10, 10, 0), keyed_object(1)];
    for (prop_id, from, to) in [
        (property_ids::PARAMETRICPATH_WIDTH, 10.0, 30.0),
        (property_ids::RECTANGLE_CORNERRADIUSTL, 1.0, 5.0), ] {
        objects.extend([keyed_property(prop_id), double_keyframe(0, from, 1),
            double_keyframe(10, to, 1)]);
    }

    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    let Geometry::RoundedRect { rect, radii } =
        display_list(&runtime)[0].shapes[0].geom else { panic!() };
    assert_eq!(rect, Rect { x: -10.0, y: -10.0, w: 20.0, h: 20.0 });
    assert_eq!(radii, CornerRadii { tl: 3.0, tr: 3.0, br: 3.0, bl: 3.0 });

    runtime.set_animation(0).unwrap();
    let Geometry::RoundedRect { rect, radii } =
        display_list(&runtime)[0].shapes[0].geom else { panic!() };
    assert_eq!(rect.w, 10.0);
    assert_eq!(radii, CornerRadii { tl: 1.0, tr: 1.0, br: 1.0, bl: 1.0 });
}

#[test] fn animates_gradient_stop_colors_and_positions() {
    let shape = parented(object_ids::SHAPE, 0);
    let ellipse = parented(object_ids::ELLIPSE, 1);
    let  fill = parented(object_ids::FILL, 1);
    let mut solid = parented(object_ids::SOLID_COLOR, 3);
    solid.add_prop(VarUInt(property_ids::SOLIDCOLOR_COLORVALUE),
        FieldValue::Color(0x0010_80ff));
    let gradient_fill = parented(object_ids::FILL, 1);
    let gradient = parented(object_ids::LINEAR_GRADIENT, 5);
    let mut stop = parented(object_ids::GRADIENT_STOP, 6);
    prop(&mut stop, property_ids::POSITION, 1.0);
    stop.add_prop(VarUInt(property_ids::GRADIENTSTOP_COLORVALUE),
        FieldValue::Color(0xff00_0000));
    let mut first_stop = parented(object_ids::GRADIENT_STOP, 6);
    first_stop.add_prop(VarUInt(property_ids::GRADIENTSTOP_COLORVALUE),
        FieldValue::Color(0xff12_3456));

    let objects = vec![artboard(), shape, ellipse, fill, solid,
        gradient_fill, gradient, stop, first_stop,
        linear_animation(b"color", 10, 10, 0),
        keyed_object(4), keyed_property(property_ids::SOLIDCOLOR_COLORVALUE),
        color_keyframe(0, 0x0010_80ff, 1),
        color_keyframe(10, 0xfff0_0001, 1),
        keyed_object(7), keyed_property(property_ids::GRADIENTSTOP_COLORVALUE),
        color_keyframe(0, 0xff00_0000, 1),
        color_keyframe(10, 0xffff_ffff, 1),
        keyed_property(property_ids::POSITION),
        double_keyframe(0, 1.0, 1), double_keyframe(10, -1.0, 1),
        linear_animation(b"idle", 10, 10, 0),
    ];

    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    let list = display_list(&runtime);
    assert!(matches!(&list[0].paint,
        Some(Paint::Fill { brush: Brush::Solid(0x8080_4080), .. })));
    let Some(Paint::Fill { brush: Brush::LinearGradient { stops, .. }, .. }) =
        &list[1].paint else { panic!() };
    assert_eq!(stops[0], GradientStop { pos: 0.0, color: 0xff80_8080 });
    assert_eq!(stops[1], GradientStop { pos: 0.0, color: 0xff12_3456 });

    runtime.set_animation(1).unwrap();
    let list = display_list(&runtime);
    assert!(matches!(&list[0].paint,
        Some(Paint::Fill { brush: Brush::Solid(0x0010_80ff), .. })));
    let Some(Paint::Fill { brush: Brush::LinearGradient { stops, .. }, .. }) =
        &list[1].paint else { panic!() };
    assert_eq!(stops[0], GradientStop { pos: 0.0, color: 0xff12_3456 });
    assert_eq!(stops[1], GradientStop { pos: 1.0, color: 0xff00_0000 });
}

#[test] fn applies_discrete_bool_and_uint_tracks() {
    let shape = parented(object_ids::SHAPE, 0);
    let  path = parented(object_ids::POINTS_PATH, 1);
    let  first = parented(object_ids::STRAIGHT_VERTEX, 2);
    let second = parented(object_ids::STRAIGHT_VERTEX, 2);
    let mut fill = parented(object_ids::FILL, 1);
    fill.add_prop(VarUInt(property_ids::SHAPEPAINT_ISVISIBLE),
        FieldValue::VarUInt(VarUInt(0)));
    let mut polygon = parented(object_ids::POLYGON, 0);
    prop(&mut polygon, property_ids::PARAMETRICPATH_WIDTH, 10.0);
    prop(&mut polygon, property_ids::PARAMETRICPATH_HEIGHT, 10.0);
    uint_prop(&mut polygon, property_ids::POINTS, 3);
    let objects = vec![artboard(), shape, path, first, second, fill, polygon,
        linear_animation(b"discrete", 10, 10, 0),
        keyed_object(5), keyed_property(property_ids::SHAPEPAINT_ISVISIBLE),
        bool_keyframe(0, false), bool_keyframe(5, true),
        keyed_object(2), keyed_property(property_ids::POINTSCOMMONPATH_ISCLOSED),
        bool_keyframe(0, false), bool_keyframe(5, true),
        keyed_object(6), keyed_property(property_ids::POINTS),
        uint_keyframe(0, 3), uint_keyframe(5, 5),
    ];

    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    let list = display_list(&runtime);
    assert!(list[0].paint.is_none());
    let Geometry::Path(path) = &list[0].shapes[0].geom else { panic!() };
    assert!(!matches!(path.cmd.last(), Some(PathCommand::Close)));
    let Geometry::Path(polygon) = &list[1].shapes[0].geom else { panic!() };
    assert_eq!(polygon.cmd.len(), 4);

    runtime.advance(0.6);
    let list = display_list(&runtime);
    assert!(matches!(list[0].paint, Some(Paint::Fill { .. })));
    let Geometry::Path(path) = &list[0].shapes[0].geom else { panic!() };
    assert!(matches!(path.cmd.last(), Some(PathCommand::Close)));
    let Geometry::Path(polygon) = &list[1].shapes[0].geom else { panic!() };
    assert_eq!(polygon.cmd.len(), 6);
}

#[test] fn animates_points_path_vertices_and_restores_them() {
    let shape = parented(object_ids::SHAPE, 0);
    let  path = parented(object_ids::POINTS_PATH, 1);
    let first = parented(object_ids::STRAIGHT_VERTEX, 2);
    let mut second = parented(object_ids::STRAIGHT_VERTEX, 2);
    prop(&mut second, property_ids::VERTEX_X, 10.0);
    let objects = vec![artboard(), shape, path, first, second,
        linear_animation(b"vertex", 10, 10, 0),
        keyed_object(3), keyed_property(property_ids::VERTEX_X),
        double_keyframe(0, 0.0, 1), double_keyframe(10, 4.0, 1),
        linear_animation(b"idle", 10, 10, 0),
    ];

    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    let list = display_list(&runtime);
    let Geometry::Path(path) = &list[0].shapes[0].geom else { panic!() };
    assert_eq!(path.cmd[0], PathCommand::MoveTo(Point { x: 2.0, y: 0.0 }));

    runtime.set_animation(1).unwrap();
    let list = display_list(&runtime);
    let Geometry::Path(path) = &list[0].shapes[0].geom else { panic!() };
    assert_eq!(path.cmd[0], PathCommand::MoveTo(Point { x: 0.0, y: 0.0 }));
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
    assert_eq!(display_list(&runtime)[0].shapes[0].trfm.tx, 20.0);
    runtime.set_animation(1).unwrap();
    let transform = display_list(&runtime)[0].shapes[0].trfm;
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
    assert_eq!(display_list(&runtime)[0].opacity, 1.0);
    runtime.advance(0.5);
    assert_eq!(display_list(&runtime)[0].opacity, 0.25);
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
        display_list(&second)[0].shapes[0].geom,
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
        &display_list(&runtime)[0].shapes[0].geom else { panic!() };
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
    let list = display_list(&runtime);
    let paths: Vec<_> = list.iter().map(|item| {
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
    assert_eq!(display_list(&runtime)[0].shapes[0].trfm.tx, 15.0);
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

    let star = parented(object_ids::STAR, 0);
    let animated = file(vec![artboard(), star,
        linear_animation(b"invalid", 10, 10, 0), keyed_object(1),
        keyed_property(property_ids::POINTS),
        uint_keyframe(0, u32::from(u16::MAX)),
    ]);
    assert!(matches!(Runtime::from_file(animated),
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
    let list = display_list(&runtime);
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].paint, Some(Paint::Fill {
        brush: Brush::Solid(0xff11_2233), rule: FillRule::EvenOdd,
        effects: [].into(),
    }));
    assert_eq!(list[1].paint, Some(Paint::Stroke {
        brush: Brush::Solid(0xff44_5566), width: 3.0,
        cap: StrokeCap::Butt, join: StrokeJoin::Miter,
        trfm_scale: true, effects: [].into(),
    }));
    assert!(std::sync::Arc::ptr_eq(
        &list[0].shapes, &list[1].shapes));
    let (Geometry::Path(fill_path), Geometry::Path(stroke_path)) =
        (&list[0].shapes[0].geom,
         &list[1].shapes[0].geom) else { panic!() };
    assert!(std::sync::Arc::ptr_eq(&fill_path.cmd, &stroke_path.cmd));
    assert_eq!(list[0].shapes[0].geom,
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
        &display_list(&runtime)[0].paint else { panic!() };
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

#[test] fn animates_dash_path_and_segments() {
    let shape = parented(object_ids::SHAPE, 0);
    let path = parented(object_ids::ELLIPSE, 1);
    let mut stroke = parented(object_ids::STROKE, 1);
    prop(&mut stroke, property_ids::THICKNESS, 2.0);
    let mut dash_path = parented(object_ids::DASH_PATH, 3);
    prop(&mut dash_path, property_ids::DASHPATH_OFFSET, 0.25);
    uint_prop(&mut dash_path, property_ids::OFFSETISPERCENTAGE, 1);
    let mut dash = parented(object_ids::DASH, 4);
    prop(&mut dash, property_ids::DASH_LENGTH, 4.0);
    let objects = vec![artboard(), shape, path, stroke, dash_path, dash,
        linear_animation(b"dash", 10, 10, 0),
        keyed_object(4), keyed_property(property_ids::DASHPATH_OFFSET),
        double_keyframe(0, 0.25, 1), double_keyframe(10, 0.75, 1),
        keyed_property(property_ids::OFFSETISPERCENTAGE),
        bool_keyframe(0, true), bool_keyframe(5, false),
        keyed_object(5), keyed_property(property_ids::DASH_LENGTH),
        double_keyframe(0, 4.0, 1), double_keyframe(10, 8.0, 1),
        keyed_property(property_ids::LENGTHISPERCENTAGE),
        bool_keyframe(0, false), bool_keyframe(5, true),
        linear_animation(b"idle", 10, 10, 0),
    ];

    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    let list = display_list(&runtime);
    let Some(Paint::Stroke { effects, .. }) = &list[0].paint else { panic!() };
    let PathEffect::Dash { offset, relative, segments } = &effects[0] else { panic!() };
    assert_eq!((*offset, *relative, segments[0]), (0.5, false,
        DashSegment { len: 6.0, relative: true }));

    runtime.set_animation(1).unwrap();
    let list = display_list(&runtime);
    let Some(Paint::Stroke { effects, .. }) = &list[0].paint else { panic!() };
    let PathEffect::Dash { offset, relative, segments } = &effects[0] else { panic!() };
    assert_eq!((*offset, *relative, segments[0]), (0.25, true,
        DashSegment { len: 4.0, relative: false }));
}

#[test] fn animates_discrete_paint_and_trim_properties() {
    let shape = parented(object_ids::SHAPE, 0);
    let path = parented(object_ids::ELLIPSE, 1);
    let fill = parented(object_ids::FILL, 1);
    let mut stroke = parented(object_ids::STROKE, 1);
    prop(&mut stroke, property_ids::THICKNESS, 1.0);
    let mut trim = parented(object_ids::TRIM_PATH, 4);
    uint_prop(&mut trim, property_ids::TRIMPATH_MODEVALUE, 1);
    let objects = vec![artboard(), shape, path, fill, stroke, trim,
        linear_animation(b"paint", 10, 10, 0),
        keyed_object(3), keyed_property(property_ids::FILL_FILLRULE),
        uint_keyframe(0, 0), uint_keyframe(5, 1),
        keyed_object(4), keyed_property(property_ids::CAP),
        uint_keyframe(0, 0), uint_keyframe(5, 1),
        keyed_property(property_ids::JOIN),
        uint_keyframe(0, 0), uint_keyframe(5, 2),
        keyed_property(property_ids::TRANSFORMAFFECTSSTROKE),
        bool_keyframe(0, true), bool_keyframe(5, false),
        keyed_object(5), keyed_property(property_ids::TRIMPATH_MODEVALUE),
        uint_keyframe(0, 1), uint_keyframe(5, 2),
        linear_animation(b"idle", 10, 10, 0),
    ];

    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    let list = display_list(&runtime);
    assert!(matches!(&list[0].paint,
        Some(Paint::Fill { rule: FillRule::EvenOdd, .. })));
    let Some(Paint::Stroke { cap, join, trfm_scale, effects, .. }) =
        &list[1].paint else { panic!() };
    assert_eq!((*cap, *join, *trfm_scale),
        (StrokeCap::Round, StrokeJoin::Bevel, false));
    assert!(matches!(effects[0],
        PathEffect::Trim { mode: TrimMode::Synchronized, .. }));

    runtime.set_animation(1).unwrap();
    let list = display_list(&runtime);
    assert!(matches!(&list[0].paint,
        Some(Paint::Fill { rule: FillRule::NonZero, .. })));
    let Some(Paint::Stroke { cap, join, trfm_scale, effects, .. }) =
        &list[1].paint else { panic!() };
    assert_eq!((*cap, *join, *trfm_scale),
        (StrokeCap::Butt, StrokeJoin::Miter, true));
    assert!(matches!(effects[0],
        PathEffect::Trim { mode: TrimMode::Sequential, .. }));
}

#[test] fn omits_non_positive_strokes() {
    let shape = parented(object_ids::SHAPE, 0);
    let path = parented(object_ids::ELLIPSE, 1);
    let mut stroke = parented(object_ids::STROKE, 1);
    prop(&mut stroke, property_ids::THICKNESS, 0.0);
    let runtime = Runtime::from_file(file(vec![artboard(), shape, path, stroke])).unwrap();
    assert!(display_list(&runtime)[0].paint.is_none());
}

#[test] fn rejects_invalid_trim_modes() {
    let shape = parented(object_ids::SHAPE, 0);
    let path = parented(object_ids::ELLIPSE, 1);
    let stroke = parented(object_ids::STROKE, 1);
    let mut trim = parented(object_ids::TRIM_PATH, 3);
    uint_prop(&mut trim, property_ids::TRIMPATH_MODEVALUE, 3);
    assert!(matches!(Runtime::from_file(file(vec![artboard(), shape, path, stroke, trim])),
        Err(RuntimeError::InvalidTrimMode(3))));

    let shape = parented(object_ids::SHAPE, 0);
    let stroke = parented(object_ids::STROKE, 1);
    let mut trim = parented(object_ids::TRIM_PATH, 2);
    uint_prop(&mut trim, property_ids::TRIMPATH_MODEVALUE, 1);
    let animated = file(vec![artboard(), shape, stroke, trim,
        linear_animation(b"invalid", 10, 10, 0), keyed_object(3),
        keyed_property(property_ids::TRIMPATH_MODEVALUE), uint_keyframe(0, 3)]);
    assert!(matches!(Runtime::from_file(animated),
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
    let list = display_list(&runtime);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].shapes.len(), 2);
    assert!(!list[0].shapes[0].is_hole);
    assert!( list[0].shapes[1].is_hole);
    assert!(matches!(list[0].paint, Some(Paint::Fill { .. })));
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
    assert_eq!(display_list(&runtime).iter()
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
    assert_eq!(display_list(&runtime).iter()
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
    let item = &display_list(&runtime)[0];
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
    }) = &display_list(&runtime)[0].paint else { panic!() };
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
    assert!(matches!(&display_list(&runtime)[0].paint,
        Some(Paint::Fill { brush: Brush::RadialGradient {
            center: Point { x: 2.0, y: 3.0 }, radius: 5.0, ..
        }, .. })));
}

#[test] fn animates_gradient_parameters_and_world_transform() {
    let parent  = parented(object_ids::NODE, 0);
    let ellipse = parented(object_ids::ELLIPSE, 2);
    let shape = parented(object_ids::SHAPE, 1);
    let fill  = parented(object_ids::FILL, 2);
    let mut gradient = parented(object_ids::RADIAL_GRADIENT, 4);
    prop(&mut gradient, property_ids::ENDX, 4.0);
    let stop = parented(object_ids::GRADIENT_STOP, 5);
    let mut objects = vec![artboard(), parent, shape, ellipse, fill, gradient, stop,
        linear_animation(b"gradient", 10, 10, 0),
        keyed_object(1), keyed_property(property_ids::NODE_X),
        double_keyframe(0, 0.0, 1), double_keyframe(10, 10.0, 1), keyed_object(5)];
    for (prop_id, from, to) in [
        (property_ids::STARTX, 0.0, 2.0), (property_ids::ENDX, 4.0, 8.0),
        (property_ids::LINEARGRADIENT_OPACITY, 1.0, 0.5), ] {
        objects.extend([keyed_property(prop_id), double_keyframe(0, from, 1),
            double_keyframe(10, to, 1)]);
    }
    objects.push(linear_animation(b"idle", 10, 10, 0));

    let mut runtime = Runtime::from_file(file(objects)).unwrap();
    runtime.set_animation(0).unwrap();
    runtime.advance(0.5);
    let list = display_list(&runtime);
    let Some(Paint::Fill { brush: Brush::RadialGradient {
        center, radius, trfm, opacity, .. }, .. }) =
        &list[0].paint else { panic!() };
    assert_eq!((*center, *radius, trfm.tx, *opacity),
        (Point { x: 1.0, y: 0.0 }, 5.0, 5.0, 0.75));

    runtime.set_animation(1).unwrap();
    let list = display_list(&runtime);
    let Some(Paint::Fill { brush: Brush::RadialGradient {
        center, radius, trfm, opacity, .. }, .. }) =
        &list[0].paint else { panic!() };
    assert_eq!((*center, *radius, trfm.tx, *opacity),
        (Point { x: 0.0, y: 0.0 }, 4.0, 0.0, 1.0));
}

#[test] fn omits_fully_transparent_shape() {
    let mut shape = parented(object_ids::SHAPE, 0);
    prop(&mut shape, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY, 0.0);
    let ellipse = parented(object_ids::ELLIPSE, 1);

    let runtime = Runtime::from_file(file(vec![artboard(), shape, ellipse])).unwrap();
    assert!(display_list(&runtime).is_empty());
}

#[test] fn imports_repository_sample() {
    let mut input = Cursor::new(include_bytes!("../../data/rating-animation.riv"));
    let file = RiveFile::read(&mut input).unwrap();
    let mut runtime = Runtime::from_file(file).unwrap();
    assert!(0 < runtime.component_count());
    assert!(0 < runtime.animation_count());
    runtime.set_animation(0).unwrap();
    assert!(runtime.advance(1.0 / 60.0));
    let list =  display_list(&runtime);
    assert!(list.iter().flat_map(|item| item.shapes.iter())
        .any(|geometry| matches!(&geometry.geom, Geometry::Path(_))));
    assert!(list.iter().any(|item| item.paint.is_some()));
}
