
//! Linear-animation discovery and keyframe evaluation.

use super::{decode::{Object, RiveFile, core_color_default, object_ids, property_ids},
    runtime::{Result, RuntimeError, float, track::TrackBinding, uint},
};
use crate::core::helpers::math::CubicBezierEasing;

#[derive(Debug, Clone, Copy)] enum Interpolation {
    Hold, Linear, Cubic { x1: f32, y1: f32, x2: f32, y2: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)] pub(super) enum TrackValue {
    Scalar(f32), Color(u32), Bool(bool), Uint(u32),
}

#[derive(Debug)] struct Keyframe {
    frame: u32, value: TrackValue, interp: Interpolation,
}

#[derive(Debug)] pub(super) struct RawTrack {
    pub component: u32, pub prop_id: u32,
    keyframes: Vec<Keyframe>,
}

impl RawTrack {
    pub fn value_type(&self) -> TrackValue { self.keyframes[0].value }
    pub fn bind(self, binding: TrackBinding) -> PropertyTrack {
        PropertyTrack { binding, keyframes: self.keyframes }
    }
}

#[derive(Debug)] pub(super) struct PropertyTrack {
    pub binding: TrackBinding,
    keyframes: Vec<Keyframe>,
}

#[derive(Debug)] pub(super) struct Animation<T> {
    pub name: Vec<u8>, pub duration: u32, pub fps: u32,
    pub speed: f32, pub loop_mode: u32,
    pub tracks: Vec<T>,
    pub geometries: Vec<u32>, pub gradients: Vec<u32>,
}

pub(super) type RawAnimation = Animation<RawTrack>;
pub(super) type LinearAnimation = Animation<PropertyTrack>;

pub(super) fn build_animations(file: &RiveFile, context_start: usize, context_end: usize,
    obj_comps: &[Option<u32>]) -> Result<Vec<RawAnimation>> {
    let mut current_animation = None;
    let mut animations: Vec<RawAnimation> = Vec::new();
    let (mut current_component, mut current_type, mut current_track) = (None, 0, None);

    // Keyed objects are encoded as a flat ordered stream; each entry changes the context for
    // the following properties/keyframes rather than referring to them by child collections.
    for object in &file.ocoll[context_start..context_end] { match object.type_id.0 {
        object_ids::LINEAR_ANIMATION => {
            animations.push(RawAnimation {
                name: object.bytes(property_ids::ANIMATION_NAME)?
                    .unwrap_or_default().to_vec(),
                duration: uint(object, property_ids::LINEARANIMATION_DURATION)?,
                fps: uint(object, property_ids::FPS)?,
                speed: float(object, property_ids::LINEARANIMATION_SPEED)?,
                loop_mode: uint(object, property_ids::LOOPVALUE)?,
                tracks: Vec::new(),
                geometries: Vec::new(), gradients: Vec::new(),
            });
            current_animation = Some(animations.len() - 1);
            current_component = None; current_track = None;
        }
        object_ids::KEYED_OBJECT => {
            let target = context_start.checked_add(
                uint(object, property_ids::KEYEDOBJECT_OBJECTID)? as usize);
            current_component = target.and_then(|index|
                obj_comps.get(index).copied().flatten());
            current_type = target.and_then(|index| file.ocoll.get(index))
                .map_or(0, |object| object.type_id.0);
            current_track = None;
        }
        object_ids::KEYED_PROPERTY => {
            current_track = match (current_animation, current_component) {
                (Some(animation), Some(component)) => {
                    animations[animation].tracks.push(RawTrack {
                        component,
                        prop_id: uint(object, property_ids::KEYEDPROPERTY_PROPERTYKEY)?,
                        keyframes: Vec::new(),
                    });
                    Some((animation, animations[animation].tracks.len() - 1))
                }   _ => None,
            };
        }
        object_ids::KEY_FRAME_DOUBLE => if let Some((animation, track)) = current_track {
            animations[animation].tracks[track].keyframes.push(Keyframe {
                frame: uint(object, property_ids::FRAME)?,
                value: TrackValue::Scalar(
                    float(object, property_ids::KEYFRAMEDOUBLE_VALUE)?),
                interp: keyframe_interpolation(file, context_start, object)?,
            });
        }
        object_ids::KEY_FRAME_COLOR => if let Some((animation, track)) = current_track {
            animations[animation].tracks[track].keyframes.push(Keyframe {
                frame: uint(object, property_ids::FRAME)?,
                value: TrackValue::Color(object.color(property_ids::KEYFRAMECOLOR_VALUE)?
                    .unwrap_or_else(||
                        core_color_default(property_ids::KEYFRAMECOLOR_VALUE))),
                interp: keyframe_interpolation(file, context_start, object)?,
            });
        }
        object_ids::KEY_FRAME_BOOL => if let Some((animation, track)) = current_track {
            animations[animation].tracks[track].keyframes.push(Keyframe {
                frame: uint(object, property_ids::FRAME)?,
                value: TrackValue::Bool(object.boolean(property_ids::KEYFRAMEBOOL_VALUE)?
                    .unwrap_or_else(||
                        super::decode::core_boolean_default(
                            property_ids::KEYFRAMEBOOL_VALUE))),
                interp: Interpolation::Hold,
            });
        }
        object_ids::KEY_FRAME_UINT => if let Some((animation, track)) = current_track {
            let value = uint(object, property_ids::KEYFRAMEUINT_VALUE)?;
            if animations[animation].tracks[track].prop_id == property_ids::POINTS {
                let count = if current_type == object_ids::STAR {
                    value.saturating_mul(2)
                } else { value };
                if u32::from(u16::MAX) < count {
                    return Err(RuntimeError::TooManyVertices(count))
                }
            } else if animations[animation].tracks[track].prop_id ==
                property_ids::TRIMPATH_MODEVALUE && !matches!(value, 1 | 2) {
                return Err(RuntimeError::InvalidTrimMode(value))
            }
            animations[animation].tracks[track].keyframes.push(Keyframe {
                frame: uint(object, property_ids::FRAME)?,
                value: TrackValue::Uint(value),
                interp: Interpolation::Hold,
            });
        }   _ => {}
    }}
    // Normalize once at load time so frame evaluation only touches animated components.
    for animation in &mut animations {
        animation.tracks.retain(|track| !track.keyframes.is_empty());
        for track in &mut animation.tracks {
            track.keyframes.sort_by_key(|keyframe| keyframe.frame);
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
    }   Ok(Interpolation::Cubic { x1, y1, x2, y2 })
}

pub(super) fn evaluate_track(track: &PropertyTrack, frame: f32) -> Option<TrackValue> {
    evaluate(&track.keyframes, frame)
}

fn evaluate(keyframes: &[Keyframe], frame: f32) -> Option<TrackValue> {
    // partition_point also gives stable last-keyframe-wins behavior for duplicate frame numbers.
    let first = keyframes.first()?;
    let upper = keyframes.partition_point(|keyframe| keyframe.frame as f32 <= frame);
    if  upper == 0 { return Some(first.value) }
    let current = &keyframes[upper - 1];
    let Some(next) = keyframes.get(upper) else { return Some(current.value) };
    if matches!(current.interp, Interpolation::Hold) || next.frame == current.frame {
        return Some(current.value)
    }
    let mut factor = ((frame - current.frame as f32) /
        (next.frame - current.frame) as f32).clamp(0.0, 1.0);
    if let Interpolation::Cubic { x1, y1, x2, y2 } = current.interp {
        factor = CubicBezierEasing::new((x1, y1), (x2, y2)).get_y(factor);
    }
    Some(match (current.value, next.value) {
        (TrackValue::Scalar(from), TrackValue::Scalar(to)) =>
            TrackValue::Scalar(from + (to - from) * factor),
        (TrackValue::Color(from), TrackValue::Color(to)) =>
            TrackValue::Color(lerp_color(from, to, factor)),
        (TrackValue::Bool(value), TrackValue::Bool(_)) => TrackValue::Bool(value),
        (TrackValue::Uint(value), TrackValue::Uint(_)) => TrackValue::Uint(value),
        _ => current.value,
    })
}

fn lerp_color(from: u32, to: u32, factor: f32) -> u32 {
    let channel = |shift: u32| {
        let from = ((from >> shift) & 0xff_u32) as f32;
        let to = ((to >> shift) & 0xff_u32) as f32;
        (from + (to - from) * factor).round().clamp(0.0, 255.0) as u32
    };  channel(24) << 24 | channel(16) << 16 | channel(8) << 8 | channel(0)
}

pub(super) fn mix_value(from: TrackValue, to: TrackValue, factor: f32) -> TrackValue {
    let factor = factor.clamp(0.0, 1.0);
    match (from, to) {
        (TrackValue::Scalar(from), TrackValue::Scalar(to)) =>
            TrackValue::Scalar(from + (to - from) * factor),
        (TrackValue::Color(from), TrackValue::Color(to)) =>
            TrackValue::Color(lerp_color(from, to, factor)),
        (_, to) if factor >= 0.5 => to,
        (from, _) => from,
    }
}

#[cfg(test)] mod tests { use super::*;

    fn track(interp: Interpolation) -> RawTrack {
        RawTrack { component: 0, prop_id: 0, keyframes: vec![
            Keyframe { frame:  0, value: TrackValue::Scalar(2.0), interp },
            Keyframe { frame: 10, value: TrackValue::Scalar(12.0),
                interp: Interpolation::Linear },
        ] }
    }

    #[test] fn evaluates_hold_linear_and_cubic_tracks() {
        assert_eq!(evaluate(&track(Interpolation::Hold).keyframes, 5.0),
            Some(TrackValue::Scalar(2.0)));
        assert_eq!(evaluate(&track(Interpolation::Linear).keyframes, 5.0),
            Some(TrackValue::Scalar(7.0)));
        let cubic = track(Interpolation::Cubic {
            x1: 1.0 / 3.0, y1: 1.0 / 3.0, x2: 2.0 / 3.0, y2: 2.0 / 3.0,
        });
        let Some(TrackValue::Scalar(value)) =
            evaluate(&cubic.keyframes, 5.0) else { panic!() };
        assert!((value - 7.0).abs() < 1e-4);
        assert_eq!(evaluate(&cubic.keyframes, -1.0), Some(TrackValue::Scalar(2.0)));
        assert_eq!(evaluate(&cubic.keyframes, 20.0), Some(TrackValue::Scalar(12.0)));
    }

    #[test] fn interpolates_argb_channels() {
        let track = RawTrack { component: 0, prop_id: 0, keyframes: vec![
                Keyframe { frame: 0, value: TrackValue::Color(0x0010_80ff),
                    interp: Interpolation::Linear },
                Keyframe { frame: 10, value: TrackValue::Color(0xfff0_0001),
                    interp: Interpolation::Linear },
        ] };
        assert_eq!(evaluate(&track.keyframes, 5.0),
            Some(TrackValue::Color(0x8080_4080)));
    }
}
