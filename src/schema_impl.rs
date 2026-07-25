use serde::{de::Error, ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};

use crate::{helpers::{math, IntBool}, schema::*};

pub(crate) fn des_static_value<'de, D, T>(d: D) -> Result<T, D::Error>
where D: Deserializer<'de>, T: Deserialize<'de> {
    #[derive(Deserialize)] #[serde(untagged)]
    enum StaticValue<T> { Direct(T), Singleton([T; 1]) }

    Ok(match StaticValue::deserialize(d)? {
        StaticValue::Direct(value) => value,
        StaticValue::Singleton([value]) => value,
    })
}

impl FontList { #[inline] pub fn is_empty(&self) -> bool { self.list.is_empty() } }
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
    #[inline] fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        eprintln!("Unexpected value: {value}");     Ok(Self(value))
    }
}

impl<T> KeyframeBase<T> {
    #[inline] pub fn as_array(&self) -> &[T] {
        if let Some(ArrayScalar::Array(val)) = &self.value { val } else {
            unreachable!("Expected array, encountered scalar or none") }
    }

    #[inline] pub fn as_scalar(&self) -> &T {
        match &self.value { None => unreachable!(),
            Some(ArrayScalar::Scalar(val)) => val,
            Some(ArrayScalar::Array(val)) => &val[0],
        }
    }
}

impl<T> AnimatedProperty<T> {
    #[inline] pub fn from_value(val: T) -> Self {
        Self { source: PropertySource::Inline(AnimatedValue::Static(val)),
            #[cfg(feature = "expression")] expr: None,
        }
    }

    #[inline] pub fn is_animated(&self) -> bool {
        matches!(&self.source,
            PropertySource::Inline(AnimatedValue::Animated(_)) |
            PropertySource::Slot { fallback: Some(AnimatedValue::Animated(_)), .. })
    }

    #[inline] pub fn slot_id(&self) -> Option<&str> {
        match &self.source {
            PropertySource::Slot { id, .. } => Some(id),
            PropertySource::Inline(_) => None,
        }
    }
}

impl<T: Clone + math::Tween> AnimatedProperty<T> {
    pub fn try_get_value(&self, fnth: f32) -> Result<T, UnresolvedSlot<'_>> {
        let keyframes = match &self.source {
            PropertySource::Inline(value) |
            PropertySource::Slot { fallback: Some(value), .. } => value,
            PropertySource::Slot { id, fallback: None } => return Err(UnresolvedSlot(id)),
        };
        Ok(match keyframes {
            AnimatedValue::Static(val) => val.clone(),
            AnimatedValue::Animated(coll) => {
                if fnth <= coll[0].start { return Ok(coll[0].as_scalar().clone()) }

                let mut len = coll.len() - 1;
                if coll[len].value.is_none() { if 0 < len { len -= 1; } else { unreachable!() } }
                if coll[len].start <= fnth { return Ok(coll[len].as_scalar().clone()) }
                while 0 < len { len -= 1; if coll[len].start <= fnth { break } }

                #[inline] fn get_scalar(val: &ArrayScalar<f32>) -> f32 { match val {
                    ArrayScalar::Array(val) => val[0],
                    ArrayScalar::Scalar(val) => *val,
                } }

                let kf = &coll[len];
                if kf.hold.as_bool() { return Ok(kf.as_scalar().clone()) }
                let mut time = (fnth - kf.start) / (coll[len + 1].start - kf.start);

                if let Some((cp1, cp2)) = kf.easing.as_ref().map(|eh|
                    ((get_scalar(&eh.to.time) as _, get_scalar(&eh.to.factor) as _),
                     (get_scalar(&eh.ti.time) as _, get_scalar(&eh.ti.factor) as _))) {
                    time = math::CubicBezierEasing::new(cp1, cp2).get_y(time);
                }

                let (kf_prev, kf_next) = (kf.as_scalar(), coll[len + 1].as_scalar());
                if let Some(extra) = &kf.pextra {
                    kf_prev.bezc(kf_next, time, extra)
                } else { kf_prev.lerp(kf_next, time) }
            }
        })
    }

    #[inline] pub fn get_value(&self, fnth: f32) -> T {
        self.try_get_value(fnth).unwrap_or_else(|slot|
            panic!("slot `{}` must be resolved before evaluation", slot.0))
    }
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
    #[inline] pub fn is_ccw(&self) -> bool {
        self.dir.is_some_and(|d| matches!(d, ShapeDirection::Reversed))
    }
}

impl LayerItem {
    #[inline] pub fn visual_layer(&self) -> Option<&VisualLayer> {
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
    #[inline] pub fn should_hide(&self, fnth: f32) -> bool {
        self.base.hd || fnth < self.base.ip || self.base.op <= fnth || fnth < self.base.st
    }
}

#[cfg(test)] mod tests { use super::*;
    use serde::ser::SerializeSeq;
    use serde_test::{assert_tokens, Token};

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
