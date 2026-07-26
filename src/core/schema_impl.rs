
use std::borrow::Cow;
use serde::{de::Error, ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use crate::core::{helpers::{math, IntBool}, schema::*};

pub(crate) fn des_static_value<'de, D, T>(d: D) -> Result<T, D::Error>
where D: Deserializer<'de>, T: Deserialize<'de> {
    #[derive(Deserialize)] #[serde(untagged)]
    enum StaticValue<T> { Direct(T), Singleton([T; 1]) }

    Ok(match StaticValue::deserialize(d)? {
        StaticValue::Direct(value) => value,
        StaticValue::Singleton([value]) => value,
    })
}

impl FontList { pub fn is_empty(&self) -> bool { self.list.is_empty() } }
impl<'de> Deserialize<'de> for TextGrouping {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = f64::deserialize(d)?;
        if !value.is_finite() || value.fract() != 0. {
            return Err(D::Error::custom("text grouping must be an integer"));
        }
        match value as u8 {
            1 => Ok(Self::Characters),
            2 => Ok(Self::Word),
            3 => Ok(Self::Line),
            4 => Ok(Self::All),
            _ => Err(D::Error::custom(format!("unknown text grouping {value}"))),
        }
    }
}

impl Animation {
    pub fn from_reader<R: std::io::Read>(r: R) -> Result<Self, serde_json::Error> {
        let mut value = serde_json::Value::deserialize(
            &mut serde_json::Deserializer::from_reader(r))?;
        let slots = value.as_object_mut().and_then(|animation| animation.remove("slots"));
        if let Some(serde_json::Value::Object(slots)) = &slots {
            if let Some(animation) = value.as_object_mut() {
                for value in animation.values_mut() {
                    resolve_slot_refs(value, slots, &mut Vec::new())?;
                }
            }
        }
        if let (Some(animation), Some(slots)) = (value.as_object_mut(), slots) {
            animation.insert("slots".to_owned(), slots);
        }
        Self::deserialize(value)
    }
}

fn resolve_slot_refs(value: &mut serde_json::Value, slots: &serde_json::Map<String,
    serde_json::Value>, stack: &mut Vec<String>) -> Result<(), serde_json::Error> {
    if let Some(id) = value.as_object().and_then(|object|
        object.get("sid")).and_then(serde_json::Value::as_str) {
        if let Some(replacement) = slots.get(id).and_then(|slot| slot.get("p")) {
            if stack.iter().any(|ancestor| ancestor == id) {
                return Err(serde_json::Error::custom(
                    format!("cyclic slot reference involving `{id}`")));
            }
            let mut replacement  = replacement.clone();
            stack.push(id.to_owned());
            resolve_slot_refs(&mut replacement, slots, stack)?;
            stack.pop();  *value = replacement;
            return Ok(());
        }
    }

    match value {
        serde_json::Value::Array(values) =>
            values.iter_mut().try_for_each(|value| resolve_slot_refs(value, slots, stack)),
        serde_json::Value::Object(object) =>
            object.values_mut().try_for_each(|value| resolve_slot_refs(value, slots, stack)),
        _ => Ok(()),
    }
}

pub(crate) fn des_nonempty_vec<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
    where D: Deserializer<'de>, T: Deserialize<'de> {
    let values = Vec::deserialize(d)?;
    if values.is_empty() { Err(D::Error::invalid_length(0, &"a non-empty array")) }
    else { Ok(values) }
}

pub(crate) fn des_strarray<'de, D: Deserializer<'de>>(d: D) ->
    Result<Vec<String>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    if let Ok(v) = String::deserialize(&value) { Ok(vec![v]) } else {
        Vec::<String>::deserialize(value).map_err(D::Error::custom)
    }
}

#[derive(Deserialize)] struct AnimatedPropertyRepr<T> {
    #[serde(rename = "k")] keyframes: Option<AnimatedValue<T>>, sid: Option<String>,
    #[cfg(feature = "expression")] #[serde(flatten)] expr: Option<Box<Expression>>,
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for AnimatedProperty<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = AnimatedPropertyRepr::<T>::deserialize(d)?;
        if value.sid.is_none() && value.keyframes.is_none() {
            return Err(D::Error::missing_field("k"));
        }
        // Some Bodymovin files contain stale `a` flags; the shape of `k` is authoritative.
        if let Some(AnimatedValue::Animated(keyframes)) = &value.keyframes {
            if keyframes[0].value.is_none() {
                return Err(D::Error::custom("first keyframe must contain a value"));
            }
            if keyframes[..keyframes.len() - 1].iter().any(|keyframe|
                keyframe.value.is_none()) {
                return Err(D::Error::custom("only the final keyframe may omit its value"));
            }
            if keyframes.iter().any(|keyframe| !keyframe.start.is_finite()) {
                return Err(D::Error::custom("keyframe times must be finite"));
            }
            if keyframes.windows(2).any(|pair| pair[1].start < pair[0].start) {
                return Err(D::Error::custom("keyframe times must be ordered"));
            }
        }
        let source = match (value.sid, value.keyframes) {
            (Some(id), fallback) => PropertySource::Slot { id: id.into(), fallback },
            (None, Some(value)) => PropertySource::Inline(value),
            (None, None) => unreachable!(),
        };
        Ok(Self { source, #[cfg(feature = "expression")] expr: value.expr, })
    }
}

impl<T: Serialize> Serialize for AnimatedProperty<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        let serialize_value = |map: &mut S::SerializeMap,
            keyframes: &AnimatedValue<T>| -> Result<(), S::Error> {
            map.serialize_entry("a",
                &IntBool::from(matches!(keyframes, AnimatedValue::Animated(_))))?;
            map.serialize_entry("k", keyframes)?;   Ok(())
        };
        match &self.source {
            PropertySource::Inline(value) => serialize_value(&mut map, value)?,
            PropertySource::Slot { id, fallback } => {
                if let Some(value) = fallback { serialize_value(&mut map, value)?; }
                map.serialize_entry("sid", id)?;
            },
        }

        #[cfg(feature = "expression")] if let Some(expr) = &self.expr {
            if !expr.x.is_empty() { map.serialize_entry("x", &expr.x)?; }
            if let Some(ix)  = expr.ix  { map.serialize_entry("ix", &ix)?; }
            if let Some(len) = expr.len { map.serialize_entry("l", &len)?; }
        }   map.end()
    }
}

impl<'de> Deserialize<'de> for GradientColors {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)] struct Repr {
            #[serde(rename = "p")] cnt: u32,
            #[serde(rename = "k")] cl: AnimatedProperty<Vec<f32>>,
        }

        let value = Repr::deserialize(d)?;
        let validate = |data: &[f32]| validate_gradient_data(data, value.cnt)
            .map_err(D::Error::custom);
        let validate_value = |animated: &AnimatedValue<Vec<f32>>| match animated {
            AnimatedValue::Static(data) => validate(data),
            AnimatedValue::Animated(keyframes) => keyframes.iter().try_for_each(|keyframe| {
                match &keyframe.value {
                    None => Ok(()),
                    Some(ArrayScalar::Scalar(data)) => validate(data),
                    Some(ArrayScalar::Array(data)) =>
                        data.iter().try_for_each(|data| validate(data)),
                }
            }),
        };
        match &value.cl.source {
            PropertySource::Inline(animated) => validate_value(animated)?,
            PropertySource::Slot { fallback: Some(animated), .. } => validate_value(animated)?,
            PropertySource::Slot { fallback: None, .. } => {}
        }
        Ok(Self { cnt: value.cnt, cl: value.cl })
    }
}

fn validate_gradient_data(data: &[f32], count: u32) -> Result<(), String> {
    let count = usize::try_from(count).map_err(|_| "gradient color count is too large")?;
    if count == 0 { return Err("gradient must contain at least one color".into()) }
    let color_len = count.checked_mul(4)
        .ok_or_else(|| "gradient color count is too large".to_owned())?;
    if data.len() < color_len || !(data.len() - color_len).is_multiple_of(2) {
        return Err(format!("invalid gradient data length {} for {count} colors", data.len()));
    }
    if data.iter().any(|value| !value.is_finite()) {
        return Err("gradient data must contain only finite numbers".into());
    }
    let valid_offsets = |data: &[f32], stride| data.chunks_exact(stride)
        .map(|stop| stop[0]).try_fold(None, |previous, offset| {
            if !(0. ..=1.).contains(&offset) || previous.is_some_and(|last| last > offset) {
                Err("gradient offsets must be ordered values in 0..=1")
            } else { Ok(Some(offset)) }
        }).map(|_| ());
    valid_offsets(&data[..color_len], 4).map_err(str::to_owned)?;
    valid_offsets(&data[color_len..], 2).map_err(str::to_owned)
}

impl<'de> Deserialize<'de> for AssetItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut value = serde_json::Value::deserialize(d)?;
        if value.get("layers").is_some() {
            return Precomp::deserialize(value).map(Self::Precomp).map_err(D::Error::custom);
        }

        match value.get("t").and_then(serde_json::Value::as_u64) {
            Some(3) => DataSource::deserialize(value)
                .map(Self::DataSource).map_err(D::Error::custom),
            Some(2) => FileAsset::deserialize(value).map(Self::Sound).map_err(D::Error::custom),
            Some(1) => {
                value.as_object_mut().expect("asset must be an object").remove("t");
                Image::deserialize(value).map(Self::Image).map_err(D::Error::custom)
            },
            Some(ty) => Err(D::Error::custom(format!("unknown asset type {ty}"))),
            None if ["w", "h", "sid"].iter().any(|key| value.get(key).is_some()) =>
                Image::deserialize(value).map(Self::Image).map_err(D::Error::custom),
            None if value.get("p").is_some() => FileAsset::deserialize(value)
                .map(Self::Sound).map_err(D::Error::custom),
            None => AnyAsset::deserialize(value).map(Self::DebugAny).map_err(D::Error::custom),
        }
    }
}

impl<'de> Deserialize<'de> for LayerItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok(match value.get("ty").and_then(serde_json::Value::as_u64)
            .ok_or_else(|| D::Error::missing_field("ty"))? {
            0 => Self::PrecompLayer(PrecompLayer::deserialize(value).map_err(D::Error::custom)?),
            1 => Self::SolidColor(SolidLayer::deserialize(value).map_err(D::Error::custom)?),
            2 => Self::Image(ImageLayer::deserialize(value).map_err(D::Error::custom)?),
            3 => Self::Null(VisualLayer::deserialize(value).map_err(D::Error::custom)?),
            4 => Self::Shape(ShapeLayer::deserialize(value).map_err(D::Error::custom)?),
            5 => Self::Text(Box::new(TextLayer::deserialize(value).map_err(D::Error::custom)?)),
            6 => Self::Audio(AudioLayer::deserialize(value).map_err(D::Error::custom)?),
           13 => Self::Camera(CameraLayer::deserialize(value).map_err(D::Error::custom)?),
           15 => Self::Data(ImageLayer::deserialize(value).map_err(D::Error::custom)?),
            ty => return Err(D::Error::custom(format!("unknown layer type {ty}"))),
        })
    }
}

impl<'de> Deserialize<'de> for EffectValueItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok(match value.get("ty").and_then(serde_json::Value::as_u64)
            .ok_or_else(|| D::Error::missing_field("ty"))? {
            0 => Self::Slider(EffectValue::<Value>::deserialize(value).map_err(D::Error::custom)?),
            1 => Self::Angle(EffectValue::<Value>::deserialize(value).map_err(D::Error::custom)?),
            2 => Self::EffectColor(EffectValue::<ColorValue>::
                deserialize(value).map_err(D::Error::custom)?),
            3 if value.pointer("/v/k").is_some_and(serde_json::Value::is_number) =>
                Self::Unsupported(value),
            3 => Self::Point(EffectValue::<Animated2D>::
                deserialize(value).map_err(D::Error::custom)?),
            4 => Self::Checkbox(EffectValue::<Value>::
                deserialize(value).map_err(D::Error::custom)?),
            6 => Self::Ignored(EffectValue::<f32>::deserialize(value).map_err(D::Error::custom)?),
            7 => Self::DropDown(EffectValue::<Value>::
                deserialize(value).map_err(D::Error::custom)?),
           10 => Self::EffectLayer(EffectValue::<Value>::
                deserialize(value).map_err(D::Error::custom)?),
            ty => return Err(D::Error::custom(format!("unknown effect value type {ty}"))),
        })
    }
}

impl<'de> Deserialize<'de> for LayerStyleItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok(match value.get("ty").and_then(serde_json::Value::as_u64)
            .ok_or_else(|| D::Error::missing_field("ty"))? {
            0 => Self::Stroke(StrokeStyle::deserialize(value).map_err(D::Error::custom)?),
            1 => Self::DropShadow(DropShadowStyle::deserialize(value).map_err(D::Error::custom)?),
            2 => Self::InnerShadow(InnerShadowStyle::deserialize(value).map_err(D::Error::custom)?),
            3 => Self::OuterGlow(OuterGlowStyle::deserialize(value).map_err(D::Error::custom)?),
            4 => Self::InnerGlow(InnerGlowStyle::deserialize(value).map_err(D::Error::custom)?),
            5 => Self::BevelEmboss(BevelEmbossStyle::
                deserialize(value).map_err(D::Error::custom)?),
            6 => Self::Satin(SatinStyle::deserialize(value).map_err(D::Error::custom)?),
            7 => Self::ColorOverlay(ColorOverlayStyle::
                deserialize(value).map_err(D::Error::custom)?),
            8 => Self::GradientOverlay(GradientOverlayStyle::
                deserialize(value).map_err(D::Error::custom)?),
            ty => return Err(D::Error::custom(format!("unknown layer style type {ty}"))),
        })
    }
}

/// Fallback for assets outside the variants currently modeled by [`AssetItem`].
#[derive(Serialize)] pub struct AnyAsset(AssetBase);
impl<'de> Deserialize<'de> for AnyAsset {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = AssetBase::deserialize(serde_json::Value::deserialize(d)?)
            .map_err(D::Error::custom)?;
        eprintln!("Failed with asset: {}", value.id);   Ok(Self(value))
    }
}

#[derive(Clone, Serialize)] pub struct AnyValue(serde_json::Value);
impl<'de> Deserialize<'de> for AnyValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        eprintln!("Unexpected value: {value}");     Ok(Self(value))
    }
}

impl<T> KeyframeBase<T> {
    pub fn as_array(&self) -> &[T] {
        if let Some(ArrayScalar::Array(val)) = &self.value { val } else {
            unreachable!("Expected array, encountered scalar or none") }
    }

    pub fn as_scalar(&self) -> &T {
        match &self.value { None => unreachable!(),
            Some(ArrayScalar::Scalar(val)) => val,
            Some(ArrayScalar::Array(val)) => &val[0],
        }
    }
}

impl ArrayScalar<f32> {
    // Scalar handles broadcast to every component; short arrays extend their last value.
    fn component(&self, index: usize) -> f32 { match self {
        Self::Scalar(value) => *value,
        Self::Array(values) => values.get(index).copied().unwrap_or_else(||
            *values.last().expect("ArrayScalar arrays are non-empty")),
    } }
}

impl EasingHandle {
    fn is_scalar(&self) -> bool {
        matches!(self.to.time,   ArrayScalar::Scalar(_)) &&
        matches!(self.to.factor, ArrayScalar::Scalar(_)) &&
        matches!(self.ti.time,   ArrayScalar::Scalar(_)) &&
        matches!(self.ti.factor, ArrayScalar::Scalar(_))
    }

    fn factor(&self, time: f32, component: usize) -> f32 {
        math::CubicBezierEasing::new(
            (self.to.time.component(component), self.to.factor.component(component)),
            (self.ti.time.component(component), self.ti.factor.component(component)),
        ).get_y(time)
    }
}

impl<T> AnimatedProperty<T> {
    pub fn from_value(val: T) -> Self {
        Self { source: PropertySource::Inline(AnimatedValue::Static(val)),
            #[cfg(feature = "expression")] expr: None,
        }
    }

    pub fn is_animated(&self) -> bool {
        matches!(&self.source,
            PropertySource::Inline(AnimatedValue::Animated(_)) |
            PropertySource::Slot { fallback: Some(AnimatedValue::Animated(_)), .. })
    }

    pub fn slot_id(&self) -> Option<&str> {
        match &self.source {
            PropertySource::Slot { id, .. } => Some(id),
            PropertySource::Inline(_) => None,
        }
    }
}

impl<T: Clone + math::Tween> AnimatedProperty<T> {
    pub(crate) fn try_get_value_cow(&self, fnth: f32) ->
        Result<Cow<'_, T>, UnresolvedSlot<'_>> {
        let keyframes = match &self.source {
            PropertySource::Inline(value) |
            PropertySource::Slot { fallback: Some(value), .. } => value,
            PropertySource::Slot { id, fallback: None } => return Err(UnresolvedSlot(id)),
        };
        Ok(match keyframes {
            AnimatedValue::Static(val) => Cow::Borrowed(val),
            AnimatedValue::Animated(coll) => {
                let mut current = coll.partition_point(|keyframe| keyframe.start <= fnth)
                    .saturating_sub(1);
                if fnth < coll[0].start { current = 0; }
                while coll[current].value.is_none() { current -= 1; }

                let keyframe = &coll[current];
                let Some(next) = coll.get(current + 1).filter(|next|
                   !keyframe.hold.as_bool() && keyframe.start < fnth && next.value.is_some())
                else { return Ok(Cow::Borrowed(keyframe.as_scalar())) };

                let duration = next.start - keyframe.start;
                if  duration <= 0. { return Ok(Cow::Borrowed(next.as_scalar())) }
                let time = ((fnth - keyframe.start) / duration).clamp(0., 1.);
                let (first, second) = (keyframe.as_scalar(), next.as_scalar());

                Cow::Owned(if let Some(extra) = &keyframe.pextra {
                    let factor = keyframe.easing.as_ref().map_or(time,
                        |easing| easing.factor(time, 0));
                    first.bezc(second, factor, extra)
                } else {
                    match keyframe.easing.as_deref() {
                        Some(easing) if easing.is_scalar() =>
                            first.lerp(second, easing.factor(time, 0)),
                        Some(easing) => {
                            let mut factor = |component| easing.factor(time, component);
                            first.lerp_by(second, &mut factor)
                        }
                        None => first.lerp(second, time),
                    }
                })
            }
        })
    }

    pub fn try_get_value(&self, fnth: f32) -> Result<T, UnresolvedSlot<'_>> {
        self.try_get_value_cow(fnth).map(Cow::into_owned)
    }

    pub(crate) fn get_value_cow(&self, fnth: f32) -> Cow<'_, T> {
        self.try_get_value_cow(fnth).unwrap_or_else(|slot|
            panic!("slot `{}` must be resolved before evaluation", slot.0))
    }

    pub fn get_value(&self, fnth: f32) -> T { self.get_value_cow(fnth).into_owned() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnresolvedSlot<'a>(pub &'a str);

impl std::fmt::Display for UnresolvedSlot<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "slot `{}` has not been resolved", self.0)
    }
}
impl std::error::Error for UnresolvedSlot<'_> {}

impl ShapeBase {
    pub fn is_ccw(&self) -> bool {
        self.dir.is_some_and(|d| matches!(d, ShapeDirection::Reversed))
    }
}

impl LayerItem {
    pub fn visual_layer(&self) -> Option<&VisualLayer> {
        Some(match self {
            Self::PrecompLayer(layer) => &layer.vl,
            Self::SolidColor(layer) => &layer.vl,
            Self::Shape(layer) => &layer.vl,
            Self::Image(layer) => &layer.vl,
            Self::Text(layer) => &layer.vl,
            Self::Data(layer) => &layer.vl,
            Self::Null(null) => null,
            Self::Audio(_) | Self::Camera(_) => return None,
        })
    }
}

impl VisualLayer {
    pub fn should_hide(&self, fnth: f32) -> bool {
        self.base.hd || fnth < self.base.ip || self.base.op <= fnth
    }
}

impl LayerInfo {
    pub fn local_frame(&self, global: f32) -> Option<f32> {
        let local = global / self.sr - self.st;
        (self.sr != 0. && self.sr.is_finite() && local.is_finite()).then_some(local)
    }
}

#[cfg(test)] mod tests { use super::*;
    use serde::ser::SerializeSeq;
    use serde_test::{assert_tokens, Token};

    #[test] fn layer_local_frame_applies_stretch_before_start_time() {
        let layer: LayerItem = serde_json::from_str(
            r#"{"ty":3,"st":10,"sr":2,"ip":0,"op":100,"ks":{}}"#).unwrap();
        let layer = layer.visual_layer().unwrap();

        assert_eq!(layer.base.local_frame(30.), Some(5.));
        assert!(!layer.should_hide(5.));

        let reversed: LayerItem = serde_json::from_str(
            r#"{"ty":3,"st":3,"sr":-2,"ip":0,"op":100,"ks":{}}"#).unwrap();
        assert_eq!(reversed.visual_layer().unwrap().base.local_frame(20.), Some(-13.));
    }

    #[test] fn layer_local_frame_rejects_invalid_time_stretch() {
        for sr in [0., f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut layer: LayerItem = serde_json::from_str(
                r#"{"ty":3,"st":0,"ip":0,"op":100,"ks":{}}"#).unwrap();
            let LayerItem::Null(layer) = &mut layer else { panic!() };
            layer.base.sr = sr;
            assert_eq!(layer.base.local_frame(10.), None);
        }
    }

    #[test] fn enum_deserialization_rejects_unknown_types() {
        assert!(serde_json::from_str::<LayerItem>(r#"{"ty":99}"#).is_err());
        assert!(serde_json::from_str::<EffectValueItem>(r#"{"ty":99}"#).is_err());
        assert!(serde_json::from_str::<LayerStyleItem>(r#"{"ty":99}"#).is_err());
    }

    #[test] fn point_effect_preserves_nonstandard_scalar_value() {
        let json = r#"{"ty":3,"v":{"k":0}}"#;
        let effect = serde_json::from_str::<EffectValueItem>(json).unwrap();
        assert!(matches!(effect, EffectValueItem::Unsupported(_)));
        assert_eq!(serde_json::to_string(&effect).unwrap(), json);
    }

    #[test] fn asset_variants_follow_the_community_schema() {
        let image = serde_json::from_str::<AssetItem>(r#"{"id":"image","p":"image.png","w":10}"#)
            .unwrap();
        assert!(matches!(image, AssetItem::Image(_)));

        let precomp = serde_json::from_str::<AssetItem>(
            r#"{"id":"comp","layers":[]}"#).unwrap();
        assert!(matches!(precomp, AssetItem::Precomp(_)));

        let data = serde_json::from_str::<AssetItem>(
            r#"{"id":"data","p":"data.json","t":3}"#).unwrap();
        assert!(matches!(data, AssetItem::DataSource(_)));

        let sound = serde_json::from_str::<AssetItem>(
            r#"{"id":"sound","p":"sound.mp3","t":2}"#).unwrap();
        assert!(matches!(sound, AssetItem::Sound(_)));

        let untyped_sound =
            serde_json::from_str::<AssetItem>(r#"{"id":"sound","p":"sound.mp3"}"#).unwrap();
        assert!(matches!(untyped_sound, AssetItem::Sound(_)));
    }

    #[test] fn animated_properties_reject_empty_keyframes() {
        assert!(serde_json::from_str::<Value>(r#"{"a":1,"k":[]}"#).is_err());
        assert!(serde_json::from_str::<Value>(
            r#"{"a":1,"k":[{"t":0,"s":[]}]}"#).is_err());
        assert!(serde_json::from_str::<Value>(
            r#"{"k":[{"t":0},{"t":1,"s":[1]}]}"#).is_err());
        assert!(serde_json::from_str::<Value>(
            r#"{"k":[{"t":0,"s":[0]},{"t":1},{"t":2,"s":[2]}]}"#).is_err());
        assert!(serde_json::from_str::<Value>(
            r#"{"k":[{"t":1,"s":[1]},{"t":0,"s":[0]}]}"#).is_err());
    }

    #[test] fn animated_properties_borrow_values_that_need_no_interpolation() {
        let static_value: Value = serde_json::from_str(r#"{"k":4}"#).unwrap();
        assert!(matches!(static_value.get_value_cow(0.), Cow::Borrowed(&4.)));

        let animated: Value = serde_json::from_str(
            r#"{"k":[{"t":0,"s":[2],"h":1},{"t":10,"s":[8]}]}"#).unwrap();
        assert!(matches!(animated.get_value_cow(5.), Cow::Borrowed(&2.)));
        assert!(matches!(animated.get_value_cow(10.), Cow::Borrowed(&8.)));
        assert!(matches!(animated.get_value_cow(15.), Cow::Borrowed(&8.)));

        let tweened: Value = serde_json::from_str(
            r#"{"k":[{"t":0,"s":[2]},{"t":10,"s":[8]}]}"#).unwrap();
        assert!(matches!(tweened.get_value_cow(5.), Cow::Owned(5.)));
    }

    #[test] fn animated_properties_apply_easing_per_component() {
        let position: Animated2D = serde_json::from_str(concat!(
            r#"{"k":[{"t":0,"s":[0,0],"#,
            r#""o":{"x":[0,0],"y":[0,1]},"i":{"x":[1,1],"y":[0,1]}},"#,
            r#"{"t":10,"s":[100,100]}]}"#,
        )).unwrap();
        let position = position.get_value(5.);
        assert!((position.x - 12.5).abs() < 1e-4);
        assert!((position.y - 87.5).abs() < 1e-4);

        let values: MultiD = serde_json::from_str(concat!(
            r#"{"k":[{"t":0,"s":[0,0,0],"#,
            r#""o":{"x":0,"y":[0,1]},"i":{"x":1,"y":[0,1]}},"#,
            r#"{"t":10,"s":[100,100,100]}]}"#,
        )).unwrap();
        let values = values.get_value(5.);
        assert!((values[0] - 12.5).abs() < 1e-4);
        assert!((values[1] - 87.5).abs() < 1e-4);
        assert!((values[2] - 87.5).abs() < 1e-4);

        let color: ColorValue = serde_json::from_str(concat!(
            r#"{"k":[{"t":0,"s":[0,0,0],"#,
            r#""o":{"x":0,"y":[0,0.5,1]},"i":{"x":1,"y":[0,0.5,1]}},"#,
            r#"{"t":10,"s":[1,1,1]}]}"#,
        )).unwrap();
        let color = color.get_value(5.);
        assert_eq!((color.r, color.g, color.b), (31, 127, 223));
    }

    #[test] fn animated_properties_handle_hold_duplicate_and_terminal_keyframes() {
        let duplicate: Value = serde_json::from_str(concat!(
            r#"{"k":[{"t":0,"s":[0]},{"t":10,"s":[100]},"#,
            r#"{"t":10,"s":[200]},{"t":20}]}"#,
        )).unwrap();
        assert_eq!(duplicate.get_value(5.), 50.);
        assert_eq!(duplicate.get_value(10.), 200.);
        assert_eq!(duplicate.get_value(15.), 200.);
        assert_eq!(duplicate.get_value(30.), 200.);

        let hold: Value = serde_json::from_str(
            r#"{"k":[{"t":0,"s":[10],"h":1},{"t":10,"s":[20]}]}"#).unwrap();
        assert_eq!(hold.get_value(5.), 10.);
        assert_eq!(hold.get_value(10.), 20.);
    }

    #[test] fn animated_properties_support_slots_and_normalize_the_animation_flag() {
        let property = serde_json::from_str::<Value>(r#"{"sid":"primary-color"}"#).unwrap();
        assert_eq!(property.slot_id(), Some("primary-color"));
        assert_eq!(property.try_get_value(0.), Err(UnresolvedSlot("primary-color")));
        assert_eq!(serde_json::to_string(&property).unwrap(), r#"{"sid":"primary-color"}"#);

        let fallback =
            serde_json::from_str::<Value>(r#"{"a":0,"k":42,"sid":"size"}"#).unwrap();
        assert_eq!(fallback.slot_id(), Some("size"));
        assert_eq!(fallback.try_get_value(0.), Ok(42.));

        let stale_static = serde_json::from_str::<Value>(r#"{"a":1,"k":42}"#).unwrap();
        assert!(!stale_static.is_animated());

        let singleton_static = serde_json::from_str::<Value>(r#"{"a":0,"k":[42]}"#).unwrap();
        assert_eq!(singleton_static.try_get_value(0.), Ok(42.));
        assert_eq!(serde_json::to_string(&singleton_static).unwrap(), r#"{"a":0,"k":42.0}"#);

        for property in [r#"{"a":0,"k":[{"t":0,"s":[42]},{"t":1,"s":[43]}]}"#,
                         r#"{"k":[{"t":0,"s":[42]},{"t":1,"s":[43]}]}"#, ] {
            assert!(serde_json::from_str::<Value>(property).unwrap().is_animated());
        }
    }

    #[test] fn text_grouping_accepts_integral_float_encoding() {
        assert!(serde_json::from_str::<TextAlignmentOptions>(r#"{"g":1.0}"#).is_ok());
        assert!(serde_json::from_str::<TextAlignmentOptions>(r#"{"g":1.5}"#).is_err());
    }

    #[test] fn animation_from_reader_resolves_slots_and_preserves_the_dictionary() {
        let json = br#"{ "slots": {
                "image": {"p": {"id":"resolved","p":"new.png","w":20,"h":10}}
            },
            "assets": [ {"id":"fallback","p":"old.png","w":1,"h":1,"sid":"image"} ]
        }"#;
        let animation = Animation::from_reader(&json[..]).unwrap();
        let AssetItem::Image(image) = &animation.assets[0] else {
            panic!("resolved asset must remain an image");
        };
        assert_eq!(image.file.url, "new.png");
        assert_eq!((image.w, image.h), (20., 10.));
        assert!(animation.slots.is_some());
    }

    #[test] fn slot_resolution_supports_chains_and_rejects_cycles() {
        let slots = serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(
            serde_json::json!({
                "first":  {"p": {"sid":"second","a":0,"k":1}},
                "second": {"p": {"a":0,"k":2}}
            })).unwrap();
        let mut value = serde_json::json!({"sid":"first","a":0,"k":0});
        resolve_slot_refs(&mut value, &slots, &mut Vec::new()).unwrap();
        assert_eq!(value, serde_json::json!({"a":0,"k":2}));

        let slots = serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(
            serde_json::json!({
                "first":  {"p": {"sid":"second"}},
                "second": {"p": {"sid":"first"}}
            })).unwrap();
        let mut value = serde_json::json!({"sid":"first"});
        assert!(resolve_slot_refs(&mut value, &slots, &mut Vec::new()).is_err());
    }

    #[test] fn enum_type_is_flattened_during_serialization() {
        let tokens = [
            Token::Struct { name: "Container", len: 1 },
            Token::Str("layers"),
            Token::Seq { len: Some(1) },
                Token::Map { len: None },
                    Token::Str("ty"),  Token::U32(0),
                    Token::Str("ind"), Token::U32(1),
                    Token::Str("nm"),  Token::String("name"),
                Token::MapEnd,
            Token::SeqEnd,
            Token::StructEnd,
        ];
        let container = Container { layers: vec![
            TestLayerItem::SomeLayer(SomeLayer { ind: 1, nm: "name".to_owned() }),
        ] };
        assert_tokens(&container, &tokens);
    }

    #[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
    struct Container {
        #[serde(serialize_with = "serialize_with_type")]
        layers: Vec<TestLayerItem>,
    }

    #[derive(Clone, Debug, PartialEq, Serialize)]
    #[serde(untagged)]
    enum TestLayerItem { SomeLayer(SomeLayer) }

    #[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
    struct SomeLayer { ind: u32, nm: String }

    impl<'de> Deserialize<'de> for TestLayerItem {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let value = serde_json::Value::deserialize(d)?;
            match value.get("ty").and_then(serde_json::Value::as_u64)
                .ok_or_else(|| D::Error::missing_field("ty"))? {
                0 => SomeLayer::deserialize(value).map(Self::SomeLayer).map_err(D::Error::custom),
                ty => Err(D::Error::custom(format!("unknown test layer type {ty}"))),
            }
        }
    }

    fn serialize_with_type<S: Serializer>(layers: &[TestLayerItem],
        serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct TypedLayerItem<'a> { ty: u32, #[serde(flatten)] content: &'a TestLayerItem }

        let mut state = serializer.serialize_seq(Some(layers.len()))?;
        for layer in layers {
            state.serialize_element(&TypedLayerItem { ty: 0, content: layer })?;
        }
        state.end()
    }
}
