
use super::*;
use serde::{ser::SerializeSeq, de::Error};
use serde::{Serialize, Deserialize, Deserializer, Serializer};
use crate::{Bezier, TextDocument, Vector2D, Easing, KeyFrame};

#[derive(Serialize, Deserialize, Clone)] #[serde(untagged)] pub enum Value {
    Primitive(f32), List(Vec<f32>), Bezier(Bezier),
    ComplexBezier(Vec<Bezier>), TextDocument(TextDocument),
}

impl Value {
    pub(crate) fn as_f32_vec(&self) -> Option<Vec<f32>> {
        Some(match self {
            Value::Primitive(p) => vec![*p],
            Value::List(l) => l.clone(),
            _ => return None,
        })
    }
}

pub fn bool_from_int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    match u8::deserialize(deserializer)? { 0 => Ok(false), 1 => Ok(true),
        other => Err(Error::invalid_value(
            serde::de::Unexpected::Unsigned(other as u64), &"zero or one")),
    }
}

pub fn int_from_bool<S: Serializer>(b: &bool, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u8(if *b { 1 } else { 0 })
}

pub fn array_to_rgba<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Rgba, D::Error> {
    Ok(<Rgba as FromTo<Value>>::from(Value::deserialize(deserializer)?))
}

pub fn str_to_rgba<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Rgba, D::Error> {
    Ok(String::deserialize(deserializer)?.parse().unwrap())
}

pub fn str_from_rgba<S: Serializer>(b: &Rgba, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&b.to_string())
}

pub fn array_from_rgba<S: Serializer>(b: &Rgba, serializer: S) -> Result<S::Ok, S::Error> {
    let a = [b.r as f32, b.g as f32, b.b as f32, b.a as f32];
    let mut seq = serializer.serialize_seq(Some(a.len()))?;
    seq.serialize_element(&a[0])?; seq.serialize_element(&a[1])?;
    seq.serialize_element(&a[2])?; seq.serialize_element(&a[3])?;   seq.end()
}

impl<'de> Deserialize<'de> for LayerContent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;

        Ok( match value.get("ty").and_then(serde_json::Value::as_u64).unwrap() {
                0 => LayerContent::PreCompositionRef(
                    PreCompositionRef::deserialize(value).map_err(D::Error::custom)?),
                1 => LayerContent::SolidColor(
                    SolidColor::deserialize(value).map_err(D::Error::custom)?),
                2 | 6 => LayerContent::MediaRef(
                    MediaRef::deserialize(value).map_err(D::Error::custom)?),
                3 => LayerContent::Empty,
                4 => {
                    let shapes = value.get("shapes")
                        .map(Vec::<ShapeLayer>::deserialize)
                        .transpose().unwrap_or_default().unwrap_or_default();
                    LayerContent::Shape(ShapeGroup { shapes })
                }
                5 => {
                    let v = value.get("t").ok_or_else(|| D::Error::missing_field("t"))?;
                    let v = TextAnimationData::deserialize(v).map_err(D::Error::custom)?;
                    LayerContent::Text(v)
                }
                // 7 => LayerContent::Null(Type3::deserialize(value).unwrap()),
                type_ => panic!("unsupported type {:?}", type_),
            },
        )
    }
}

impl Serialize for LayerContent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)] #[serde(untagged)] enum LayerContent_<'a> { // T1(&'a Type1),
            SolidColor(&'a SolidColor),
            Shape { shapes: &'a Vec<ShapeLayer> },
        }

        #[derive(Serialize)] struct TypedLayerContent<'a> {
            #[serde(rename = "ty")] t: u32,
            #[serde(flatten)] content: LayerContent_<'a>,
        }

        //serializer.serialize_str("ty: ")?;
        let msg = match self {
            LayerContent::SolidColor(sc) => {   //serializer.serialize_u32(1)?;
                //serde_json::json!({ "ty": 1, }).serialize(serializer)?;
                //sc.serialize(serializer)?;
                TypedLayerContent { t: 1, content: LayerContent_::SolidColor(sc) }
            }
            LayerContent::Shape(sg) => {        //serializer.serialize_u32(4)?;
                //serde_json::json!({ "ty": 4, }).serialize(serializer)?;
                //sg.serialize(serializer)?;
                TypedLayerContent { t: 4, content: LayerContent_::Shape { shapes: &sg.shapes } }
            }
            _ => unimplemented!(),  // TODO:
        };  msg.serialize(serializer)
    }
}

pub(crate) fn keyframes_from_array<'de, D, T>(deserializer: D) ->
    Result<Vec<KeyFrame<T>>, D::Error> where D: Deserializer<'de>, T: FromTo<Value> {
    Ok(AnimatedHelper::deserialize(deserializer)?.into())
}

pub fn array_from_keyframes<S: Serializer, T>(_b: &[KeyFrame<T>], _serializer: S) ->
    Result<S::Ok, S::Error> { todo!()
    /* match AnimatedHelper::from(b) {
        AnimatedHelper::Plain(data) => data.serialize(serializer),
        AnimatedHelper::AnimatedHelper(data) => {
            let mut seq = serializer.serialize_seq(Some(data.len()))?;
            for keyframe in data { seq.serialize_element(&keyframe)?; }     seq.end()
        }
    } */
}

pub fn default_vec2_100() -> Animated<Vector2D> {
    Animated { animated: false,
        keyframes: vec![KeyFrame::from_value(Vector2D::new(100.0, 100.0))],
    }
}

pub fn default_number_100() -> Animated<f32> {
    Animated { animated: false, keyframes: vec![KeyFrame::from_value(100.0)] }
}

struct NumberVistor;

impl<'de> serde::de::Visitor<'de> for NumberVistor { type Value = Option<u32>;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("u32 / f32")
    }

    fn visit_f32<E: Error>(self, v: f32) -> Result<Self::Value, E> { Ok(Some(v.round() as u32)) }
    fn visit_f64<E: Error>(self, v: f64) -> Result<Self::Value, E> { Ok(Some(v.round() as u32)) }
    fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> { Ok(Some(v as u32)) }
    fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> { Ok(Some(v as u32)) }
    fn visit_none<E: Error>(self) -> Result<Self::Value, E> { Ok(None) }
}

pub(crate) fn vec_from_array<'de, D: Deserializer<'de>>(deserializer: D) ->
    Result<Vec<Vector2D>, D::Error> {
    let result = Vec::<[f32; 2]>::deserialize(deserializer)?;
    Ok(result.into_iter().map(|f| f.into()).collect())
}

pub fn array_from_vec<S: Serializer>(data: &Vec<Vector2D>, serializer: S) ->
    Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_seq(Some(data.len()))?;
    for d in data { seq.serialize_element(&[d.x, d.y])?; }  seq.end()
}

pub(crate) fn array_from_array_or_number<'de, D: Deserializer<'de>>(deserializer: D) ->
    Result<Vec<f32>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Primitive(f) => vec![f],
        Value::List(f) => f,
        _ => unreachable!(),
    })
}

#[derive(Deserialize, Serialize)] pub(crate) struct ColorListHelper {
    #[serde(rename = "p")] color_count: usize,
    #[serde(rename = "k")] colors: Animated<Vec<f32>>,
}

impl From<ColorListHelper> for ColorList {
    fn from(helper: ColorListHelper) -> Self {
        let color_count = helper.color_count;
        ColorList { color_count,
            colors: Animated { animated: helper.colors.animated,
                keyframes: helper.colors.keyframes.into_iter()
                    .map(|keyframe| {
                        let start = f32_to_gradient_colors(&keyframe.start_value, color_count);
                        let end = f32_to_gradient_colors(&keyframe.end_value, color_count);
                        keyframe.alter_value(start, end)
                    }).collect()
            },
        }
    }
}

fn f32_to_gradient_colors(data: &Vec<f32>, color_count: usize) -> Vec<GradientColor> {
    if data.len() == color_count * 4 { // Rgb color
        data.chunks(4).map(|chunk| GradientColor {
                offset: chunk[0], color: Rgba::new_f32(chunk[1], chunk[2], chunk[3], 1.0),
        }).collect()
    } else if data.len() == color_count * 4 + color_count * 2 { // Rgba color
        data[0..(color_count * 4)].chunks(4).zip(data[(color_count * 4)..]
            .chunks(2)).map(|(chunk, opacity)| GradientColor {
                offset: chunk[0],
                color: Rgba::new_f32(chunk[1], chunk[2], chunk[3], opacity[1]),
            }).collect()
    } else { unimplemented!() }
}

impl From<ColorList> for ColorListHelper {
    fn from(list: ColorList) -> Self {
        ColorListHelper { color_count: list.color_count,
            colors: Animated { animated: list.colors.animated,
                keyframes: list.colors.keyframes.into_iter()
                    .map(|keyframe| {
                        let start = gradient_colors_to_f32(&keyframe.start_value);
                        let end = gradient_colors_to_f32(&keyframe.end_value);
                        keyframe.alter_value(start, end)
                    }).collect()
            }
        }
    }
}

fn gradient_colors_to_f32(data: &[GradientColor]) -> Vec<f32> {
    let mut start = data.iter().flat_map(|color| {
        vec![color.offset, color.color.r as f32 / 255.0,
             color.color.g as f32 / 255.0, color.color.b as f32 / 255.0]
    }).collect::<Vec<_>>();
    let start_has_opacity = data.iter().any(|color| color.color.a < 255);
    if  start_has_opacity {
        start.extend(data.iter().flat_map(|color|
            vec![color.offset, color.color.a as f32 / 255.0]));
    }   start
}

#[derive(Debug, Clone, Copy)] pub struct Rgba { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }

impl Rgba {
    pub fn new_f32(r: f32, g: f32, b: f32, a: f32) -> Rgba {
        Rgba {  r: (r * 255.0) as u8, g: (g * 255.0) as u8,
                b: (b * 255.0) as u8, a: (a * 255.0) as u8 }
    }

    pub fn new_u8(r: u8, g: u8, b: u8, a: u8) -> Rgba { Rgba { r, g, b, a } }
}

impl Default for Rgba { fn default() -> Self { Self { r: 0, g: 0, b: 0, a: 255 } } }

impl std::str::FromStr for Rgba { type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        if s.starts_with('#') { chars.next(); }
        let (rgb, a) = read_color::rgb_maybe_a(&mut chars).unwrap();
        Ok(Rgba::new_u8(rgb[0], rgb[1], rgb[2], a.unwrap_or(255)))
    }
}

impl ToString for Rgba { fn to_string(&self) -> String { todo!() } }

#[derive(Debug, Clone, Copy)] pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }

impl Rgb {
    pub fn new_f32(r: f32, g: f32, b: f32) -> Rgb {
        Rgb { r: (r * 255.0) as u8, g: (g * 255.0) as u8, b: (b * 255.0) as u8 }
    }

    pub fn new_u8(r: u8, g: u8, b: u8) -> Rgb { Rgb { r, g, b } }
}

impl FromTo<Value> for Rgba {
    fn from(v: Value) -> Self {
        let v = v.as_f32_vec().unwrap();
        if  v[0] > 1.0 && v[0] <= 255.0 {
            Rgba::new_u8(v[0] as u8, v[1] as u8, v[2] as u8,
                v.get(3).cloned().unwrap_or(255.0) as u8)
        } else {
            Rgba::new_f32(v[0], v[1], v[2], v.get(3).cloned().unwrap_or(1.0))
        }
    }

    fn to(self) -> Value {
        Value::List(vec![self.r as f32 / 255.0, self.g as f32 / 255.0,
                         self.b as f32 / 255.0, self.a as f32 / 255.0])
    }
}

pub trait FromTo<T> {
    fn to(self) -> T;
    fn from(v: T) -> Self;
}

impl FromTo<Value> for Vector2D {
    fn from(v: Value) -> Self {
        let v = v.as_f32_vec().unwrap();
        Vector2D::new(v[0], v.get(1).cloned().unwrap_or(0.0))
    }

    fn to(self) -> Value { todo!() }
}

impl FromTo<Value> for f32 {
    fn from(v: Value) -> Self { let v = v.as_f32_vec().unwrap(); v[0] }
    fn to(self) -> Value { Value::Primitive(self) }
}

impl FromTo<Value> for Rgb {
    fn from(v: Value) -> Self {
        let v = v.as_f32_vec().unwrap();
        if  v[0] > 1.0 && v[0] <= 255.0 {
            Rgb::new_u8(v[0] as u8, v[1] as u8, v[2] as u8)
        } else { Rgb::new_f32(v[0], v[1], v[2]) }
    }

    fn to(self) -> Value {
        Value::List(vec![self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0 ])
    }
}

impl FromTo<Value> for Vec<Bezier> {
    fn from(v: Value) -> Self {
        match v {
            Value::ComplexBezier(b) => b,
            Value::Bezier(b) => vec![b],
            _ => todo!(),
        }
    }

    fn to(self) -> Value { Value::ComplexBezier(self) }
}

impl FromTo<Value> for Vec<f32> {
    fn from(v: Value) -> Self {
        match v {
            Value::Primitive(f) => vec![f],
            Value::List(l) => l,
            _ => todo!(),
        }
    }

    fn to(self) -> Value { Value::List(self) }
}

impl FromTo<Value> for TextDocument {
    fn from(v: Value) -> Self {
        match v { Value::TextDocument(t) => t, _ => todo!() }
    }

    fn to(self) -> Value { Value::TextDocument(self) }
}

#[derive(Deserialize)] #[serde(transparent)]
pub(super) struct AnimatedHelper { data: TolerantAnimatedHelper }

#[derive(Deserialize)] #[serde(untagged)] enum TolerantAnimatedHelper { Plain(Value),
    AnimatedHelper(Vec<LegacyTolerantKeyFrame>),
}

fn default_none<T>() -> Option<T> { None }

#[derive(Deserialize, Default, Debug, Clone)] struct LegacyKeyFrame<T> {
    #[serde(rename = "s")] start_value: T,
    #[serde(rename = "e", default = "default_none")] end_value: Option<T>,
    #[serde(rename = "t", default)] start_frame: f32,
    #[serde(skip)] end_frame: f32,
    #[serde(rename = "o", default)] easing_out: Option<Easing>,
    #[serde(rename = "i", default)] easing_in:  Option<Easing>,
    #[serde(rename = "h", default, deserialize_with = "super::bool_from_int")] hold: bool,
}

#[allow(clippy::large_enum_variant)]
#[derive(Deserialize)] #[serde(untagged)] enum LegacyTolerantKeyFrame {
    LegacyKeyFrame(LegacyKeyFrame<Value>), TOnly { t: f32 },
}

impl<'a, T> From<&'a Vec<KeyFrame<T>>> for AnimatedHelper {
    fn from(_:   &'a Vec<KeyFrame<T>>) -> Self { todo!() }
}

impl<T: FromTo<Value>> From<AnimatedHelper> for Vec<KeyFrame<T>> {
    fn from(animated: AnimatedHelper) -> Self {
        match animated.data {
            TolerantAnimatedHelper::Plain(v) => {
                vec![KeyFrame { start_value: T::from(v.clone()), end_value: T::from(v),
                    start_frame: 0.0, end_frame: 0.0, easing_in: None, easing_out: None }]
            }
            TolerantAnimatedHelper::AnimatedHelper(v) => {
                let mut result: Vec<LegacyKeyFrame<Value>> = vec![];
                // Sometimes keyframes especially from TextData do not have an ending frame,
                // so we double check here to avoid removing them.
                let mut has_t_only_frame = false;
                for k in v {
                    match k {
                        LegacyTolerantKeyFrame::LegacyKeyFrame(mut k) => {
                            if let Some(prev) = result.last_mut() {
                                prev.end_frame = k.start_frame;
                            }
                            if k.hold { k.end_value = Some(k.start_value.clone()); }
                            result.push(k)
                        }
                        LegacyTolerantKeyFrame::TOnly { t } => {
                            if let Some(prev) = result.last_mut() {
                                prev.end_frame = t;
                            }
                            has_t_only_frame = true;    break;
                        }
                    }
                }
                if result.len() > 1 {
                    for i in 0..(result.len() - 1) {
                        if  result[i].end_value.is_none() {
                            result[i].end_value = Some(result[i + 1].start_value.clone());
                        }
                    }
                }
                if has_t_only_frame && result.last().map(|keyframe|
                    keyframe.end_value.is_none()).unwrap_or(false) { result.pop(); }
                result.into_iter().map(|keyframe| KeyFrame {
                        end_value: T::from(keyframe.end_value
                                .unwrap_or_else(|| keyframe.start_value.clone())),
                        start_value: T::from(keyframe.start_value),
                        start_frame: keyframe.start_frame,
                        end_frame: keyframe.end_frame.max(keyframe.start_frame),
                        easing_in: keyframe.easing_in, easing_out: keyframe.easing_out,
                }).collect()
            }
        }
    }
}
