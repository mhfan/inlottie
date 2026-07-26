
//! Linear-animation discovery and scalar keyframe evaluation.

use super::{decode::{Object, RiveFile, object_ids, property_ids},
    runtime::{Result, RuntimeError, float, uint},
};

#[derive(Debug, Clone, Copy)] enum Interpolation {
    Hold, Linear, Cubic { x1: f32, y1: f32, x2: f32, y2: f32 },
}

#[derive(Debug)] struct Keyframe { frame: u32, value: f32, interp: Interpolation }

#[derive(Debug)] pub(super) struct PropertyTrack {
    // `slot` indexes LinearAnimation::components, avoiding a component-sized table per frame.
    pub component: u32, pub slot: u32, pub prop_id: u32,
    keyframes: Vec<Keyframe>,
}

#[derive(Debug)] pub(super) struct LinearAnimation {
    pub name: Vec<u8>, pub duration: u32, pub fps: u32,
    pub speed: f32, pub loop_mode: u32,
    pub tracks: Vec<PropertyTrack>, pub components: Vec<u32>,
}

pub(super) fn build_animations(file: &RiveFile, context_start: usize, context_end: usize,
    obj_comps: &[Option<u32>]) -> Result<Vec<LinearAnimation>> {
    let (mut current_animation, mut animations) = (None, Vec::new());
    let (mut current_component, mut current_track) = (None, None);

    // Keyed objects are encoded as a flat ordered stream; each entry changes the context for
    // the following properties/keyframes rather than referring to them by child collections.
    for object in &file.ocoll[context_start..context_end] { match object.type_id.0 {
        object_ids::LINEAR_ANIMATION => {
            animations.push(LinearAnimation {
                name: object.bytes(property_ids::ANIMATION_NAME)?
                    .unwrap_or_default().to_vec(),
                duration: uint(object, property_ids::LINEARANIMATION_DURATION)?,
                fps: uint(object, property_ids::FPS)?,
                speed: float(object, property_ids::LINEARANIMATION_SPEED)?,
                loop_mode: uint(object, property_ids::LOOPVALUE)?,
                tracks: Vec::new(), components: Vec::new(),
            });
            current_animation = Some(animations.len() - 1);
            current_component = None; current_track = None;
        }
        object_ids::KEYED_OBJECT => {
            let target = context_start.checked_add(
                uint(object, property_ids::KEYEDOBJECT_OBJECTID)? as usize);
            current_component = target.and_then(|index|
                obj_comps.get(index).copied().flatten());
            current_track = None;
        }
        object_ids::KEYED_PROPERTY => {
            current_track = match (current_animation, current_component) {
                (Some(animation), Some(component)) => {
                    animations[animation].tracks.push(PropertyTrack {
                        component, slot: 0,
                        prop_id: uint(object, property_ids::KEYEDPROPERTY_PROPERTYKEY)?,
                        keyframes: Vec::new(),
                    });
                    Some((animation, animations[animation].tracks.len() - 1))
                }   _ => None,
            };
        }
        // TODO: Add non-scalar keyed value types when color/bool/path animation is consumed.
        object_ids::KEY_FRAME_DOUBLE => if let Some((animation, track)) = current_track {
            animations[animation].tracks[track].keyframes.push(Keyframe {
                frame: uint(object, property_ids::FRAME)?,
                value: float(object, property_ids::KEYFRAMEDOUBLE_VALUE)?,
                interp: keyframe_interpolation(file, context_start, object)?,
            });
        }   _ => {}
    }}
    // Normalize once at load time so frame evaluation only touches animated components.
    for animation in &mut animations {
        animation.tracks.retain(|track| !track.keyframes.is_empty());
        for track in &mut animation.tracks {
            track.keyframes.sort_by_key(|keyframe| keyframe.frame);
            track.slot = animation.components.iter().position(|&component|
                component == track.component).unwrap_or_else(|| {
                    animation.components.push(track.component);
                    animation.components.len() - 1
                }) as u32;
        }
    }   Ok(animations)
}

fn keyframe_interpolation(file: &RiveFile, context_start: usize,
    keyframe: &Object) -> Result<Interpolation> {
    let kind = uint(keyframe, property_ids::INTERPOLATINGKEYFRAME_INTERPOLATIONTYPE)?;
    if  kind == 0 { return Ok(Interpolation::Hold) }
    if  kind == 1 { return Ok(Interpolation::Linear) }
    if  kind != 2 { return Err(RuntimeError::InvalidInterpolation(kind)) }

    let id = uint(keyframe, property_ids::INTERPOLATINGKEYFRAME_INTERPOLATORID)?;
    let interpolator = context_start.checked_add(id as usize)
        .and_then(|index| file.ocoll.get(index))
        .ok_or(RuntimeError::InvalidInterpolator(id))?;
    let props = if interpolator.type_id.0 == object_ids::CUBIC_INTERPOLATOR_COMPONENT {
        [property_ids::CUBICINTERPOLATORCOMPONENT_X1,
         property_ids::CUBICINTERPOLATORCOMPONENT_Y1,
         property_ids::CUBICINTERPOLATORCOMPONENT_X2,
         property_ids::CUBICINTERPOLATORCOMPONENT_Y2]
    } else if matches!(interpolator.type_id.0, object_ids::CUBIC_EASE_INTERPOLATOR |
        object_ids::CUBIC_VALUE_INTERPOLATOR | object_ids::CUBIC_INTERPOLATOR) {
        [property_ids::CUBICINTERPOLATOR_X1, property_ids::CUBICINTERPOLATOR_Y1,
         property_ids::CUBICINTERPOLATOR_X2, property_ids::CUBICINTERPOLATOR_Y2]
    } else { return Err(RuntimeError::InvalidInterpolator(id)) };
    let [x1, y1, x2, y2] = props;
    let (x1, y1, x2, y2) = (float(interpolator, x1)?, float(interpolator, y1)?,
        float(interpolator, x2)?, float(interpolator, y2)?);
    if [x1, y1, x2, y2].iter().any(|value| !value.is_finite()) {
        return Err(RuntimeError::InvalidInterpolator(id))
    }
    Ok(Interpolation::Cubic { x1, y1, x2, y2 })
}

pub(super) fn evaluate_track(track: &PropertyTrack, frame: f32) -> Option<f32> {
    // partition_point also gives stable last-keyframe-wins behavior for duplicate frame numbers.
    let first = track.keyframes.first()?;
    let upper = track.keyframes.partition_point(|keyframe| keyframe.frame as f32 <= frame);
    if upper == 0 { return Some(first.value) }
    let current = &track.keyframes[upper - 1];
    let Some(next) = track.keyframes.get(upper) else { return Some(current.value) };
    if matches!(current.interp, Interpolation::Hold) ||
        next.frame == current.frame {
        return Some(current.value)
    }
    let mut factor = ((frame - current.frame as f32) /
        (next.frame - current.frame) as f32).clamp(0.0, 1.0);
    if let Interpolation::Cubic { x1, y1, x2, y2 } = current.interp {
        // Invert the Bezier x coordinate. Newton is fast for regular curves; bisection is the
        // bounded fallback for flat derivatives or unusual but finite control points.
        let at = |t: f32, p1: f32, p2: f32|
            ((3.0 * p1 - 3.0 * p2 + 1.0) * t +
             (3.0 * p2 - 6.0 * p1)) * t * t + 3.0 * p1 * t;
        let slope = |t: f32, p1: f32, p2: f32|
            3.0 * (3.0 * p1 - 3.0 * p2 + 1.0) * t * t +
            2.0 * (3.0 * p2 - 6.0 * p1) * t + 3.0 * p1;
        let (x, mut parameter) = (factor, factor);
        for _ in 0..6 {
            let derivative = slope(parameter, x1, x2);
            if  derivative.abs() <= f32::EPSILON { break }
            parameter = (parameter - (at(parameter, x1, x2) - x) / derivative)
                .clamp(0.0, 1.0);
        }
        if (at(parameter, x1, x2) - x).abs() > 1e-5 {
            let (mut lower, mut upper) = (0.0, 1.0);
            for _ in 0..10 {
                if at(parameter, x1, x2) < x { lower = parameter } else { upper = parameter }
                parameter = (lower + upper) * 0.5;
            }
        }   factor = at(parameter, y1, y2);
    }   Some(current.value + (next.value - current.value) * factor)
}

#[cfg(test)] mod tests { use super::*;

    fn track(interp: Interpolation) -> PropertyTrack {
        PropertyTrack { component: 0, slot: 0, prop_id: 0, keyframes: vec![
            Keyframe { frame:  0, value:  2.0, interp },
            Keyframe { frame: 10, value: 12.0, interp: Interpolation::Linear },
        ] }
    }

    #[test] fn evaluates_hold_linear_and_cubic_tracks() {
        assert_eq!(evaluate_track(&track(Interpolation::Hold), 5.0), Some(2.0));
        assert_eq!(evaluate_track(&track(Interpolation::Linear), 5.0), Some(7.0));
        let cubic = track(Interpolation::Cubic {
            x1: 1.0 / 3.0, y1: 1.0 / 3.0, x2: 2.0 / 3.0, y2: 2.0 / 3.0,
        });
        assert!((evaluate_track(&cubic, 5.0).unwrap() - 7.0).abs() < 1e-4);
        assert_eq!(evaluate_track(&cubic, -1.0), Some(2.0));
        assert_eq!(evaluate_track(&cubic, 20.0), Some(12.0));
    }
}
