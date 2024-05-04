
use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

//  Represents boolean values as an integer. 0 is false, 1 is true.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(transparent)] pub struct IntBool(u8);

impl IntBool { #[inline] pub fn as_bool(&self) -> bool { (*self).into() } }
impl From<IntBool> for bool { #[inline] fn from(value: IntBool) -> Self { value.0 != 0 } }
impl From<bool> for IntBool { fn from(value: bool) -> Self { Self(if value { 1 } else { 0 }) } }

/* #[derive(Debug, Clone, Copy)] pub struct Rgb  { pub r: u8, pub g: u8, pub b: u8 }
impl Rgb {  pub fn new_u8 (r:  u8, g:  u8, b:  u8) -> Self { Self { r, g, b } }
            pub fn new_f32(r: f32, g: f32, b: f32) -> Self { Self {
        r: (r * 255.) as u8, g: (g * 255.) as u8, b: (b * 255.) as u8
    } }
} */

#[derive(Debug, Clone, Copy)] pub struct Rgba { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }
impl Default for Rgba { fn default() -> Self { Self { r: 0, g: 0, b: 0, a: 255 } } }

impl Rgba {
    pub fn new_u8 (r:  u8, g:  u8, b:  u8, a:  u8) -> Self { Self { r, g, b, a } }
    pub fn new_f32(r: f32, g: f32, b: f32, a: f32) -> Self { Self {
        r: (r * 255.) as u8, g: (g * 255.) as u8, b: (b * 255.) as u8, a: (a * 255.) as u8
    } }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = Vec::<f32>::deserialize(deserializer)?;
        assert!(2 < v.len() && v.len() < 5);
        Ok(Self::new_f32(v[0], v[1], v[2], v.get(3).cloned().unwrap_or(1.)))
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut v = vec![self.r as f32 / 255.,
            self.g as f32 / 255.0, self.b as f32 / 255.];
        if  self.a < 255 {  v.push(self.a as f32 / 255.); }    v.serialize(serializer)
    }
}

impl std::str::FromStr for Rgba { type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> { //assert!(s.len() == 7);
        let v = u32::from_str_radix(s.strip_prefix('#')
            .ok_or("not prefixed with '#'".to_owned())?, 16)
            .map_err(|err| err.to_string())?;

        let v = if s.len() == 7 { (v << 8) | 0xff } else { v };
        Ok(Self::new_u8((v >> 24) as u8, ((v >> 16) & 0xff) as u8,
            ((v >>  8) & 0xff) as u8, (v & 0xff) as u8))
    }
}

impl core::fmt::Display for Rgba {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, r"#{:2x}{:2x}{:2x}", self.r, self.g, self.b)?;
        if self.a < 255 { write!(f,  r"{:2x}", self.a)?; }  Ok(())
    }
}

pub(crate) fn str_to_rgba<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Rgba, D::Error> {
    String::deserialize(deserializer)?.parse().map_err(D::Error::custom)
}

pub(crate) fn str_from_rgba<S: Serializer>(c: &Rgba, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&c.to_string())
}

//pub type Vector2D = Vec<f32>; // euclid::default::Vector2D<f32>; // XXX: Position/Scale
#[derive(Debug, Clone, Copy)] pub struct Vector2D { pub x: f32, pub y: f32 } // Point/Size
impl From<(f32, f32)> for Vector2D {
    fn from(val: (f32, f32)) -> Self { Self { x: val.0, y: val.1 } }
}

impl<'de> Deserialize<'de> for Vector2D {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = Vec::<f32>::deserialize(deserializer)?;
        assert!(!v.is_empty() && v.len() < 4); // XXX: ignore extra 3rd value?
        Ok(Self { x: v[0], y: v.get(1).cloned().unwrap_or(0.) })
    }
}

impl Serialize for Vector2D {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        [self.x, self.y].serialize(serializer)
    }
}

#[derive(Clone, Debug)] pub struct ColorList(pub Vec<(f32, Rgba)>); // (offset, color)

impl<'de> Deserialize<'de> for ColorList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = Vec::<f32>::deserialize(deserializer)?;
        let len = data.len() as u32;

        let cnt = (len / 6) as usize; // XXX:
        let cnt = if len % 6 == 0 && !(len % 4 == 0 && (0..cnt).any(|i|
            data[i * 4] != data[cnt * 4 + i * 2])) { cnt as u32 } else { len / 4 };

        Ok(Self(if len == cnt * 4 { // Rgb color
            data.chunks(4).map(|chunk| (chunk[0],
                Rgba::new_f32(chunk[1], chunk[2], chunk[3], 1.))).collect()
        } else  if len == cnt * 4 + cnt * 2 { let cnt = (cnt * 4) as usize; // Rgba color
            data[0..cnt].chunks(4).zip(data[cnt..].chunks(2))
                .map(|(chunk, opacity)| (chunk[0], // == opacity[0]
                Rgba::new_f32(chunk[1], chunk[2], chunk[3], opacity[1]))).collect()
        } else { unreachable!() })) // issue_1732.json
    }
}

impl Serialize for ColorList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut data = self.0.iter().flat_map(|(offset, color)|
            vec![*offset, color.r as f32 / 255., color.g as f32 / 255.,
                          color.b as f32 / 255.]).collect::<Vec<_>>();

        if  self.0.iter().any(|(_, color)| color.a < 255) {
            data.extend(self.0.iter().flat_map(|(offset, color)|
                vec![*offset, color.a as f32 / 255.]));
        }   data.serialize(serializer)
    }
}

use crate::schema::*;

pub(crate) mod defaults { #![allow(unused)]
    pub fn time_stretch() -> f32 { 1.0 }
    pub fn animation_wh() -> u32 { 512 }
    pub fn animation_fr() -> f32 { 60. }
    pub fn animation_vs() -> String { "5.5.2".to_owned() }

    pub fn effect_en() -> super::IntBool { true.into() }
    pub fn precomp_op() -> f32 { 99999. }

    pub fn font_size()  -> f32 { 10. }
    //pub fn font_family() -> String { "sans".to_owned() }
    //pub fn font_style()  -> String { "Regular".to_owned() }
    //pub fn font_name()   -> String { "sans-Regular".to_owned() }

    use super::{Value, Animated2D};
    pub fn opacity() -> Value { Value::from_value(100.) }
    pub fn animated2d() -> Animated2D { Animated2D::from_value((100., 100.).into()) }

    pub fn is_default<T: Default + PartialEq>(v: &T) -> bool { *v == T::default() }
}

impl FontList { pub fn is_empty(&self) -> bool { self.list.is_empty() } }

pub trait Lerp { fn lerp(&self, other: &Self, t: f32) -> Self; }    // Linear intERPolation

impl Lerp for f32 {
    fn lerp(&self, other: &Self, t: f32) -> Self { self + (other - self) * t }
}

impl Lerp for Vector2D {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {  x: self.x + (other.x - self.x) * t,
                y: self.y + (other.y - self.y) * t, }
    }
}

impl Lerp for Rgba {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {  r: self.r + ((other.r - self.r) as f32 * t) as u8,
                g: self.g + ((other.g - self.g) as f32 * t) as u8,
                b: self.b + ((other.b - self.b) as f32 * t) as u8,
                a: self.a + ((other.a - self.a) as f32 * t) as u8,
        }
    }
}

impl Lerp for Bezier {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let closure =
            |val: (&Vector2D, &Vector2D)| val.0.lerp(val.1, t);
        Self { closed: self.closed,
            vp: self.vp.iter().zip(other.vp.iter()).map(closure).collect(),
            it: self.it.iter().zip(other.it.iter()).map(closure).collect(),
            ot: self.ot.iter().zip(other.it.iter()).map(closure).collect(),
        }
    }
}

/* impl Lerp for Vec<Bezier> {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self.iter().zip(other.iter())
            .map(|val| val.0.lerp(val.1, t)).collect()
    }
} */

impl Lerp for ColorList {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self(self.0.iter().zip(other.0.iter()).map(|val|
            (val.0.0 + (val.1.0 - val.0.0) * t, val.0.1.lerp(&val.1.1, t))).collect())
    }
}

impl Lerp for Vec<f32> {    // aka MultiDimensional
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self.iter().zip(other.iter()).map(|val| val.0.lerp(val.1, t)).collect()
    }
}

impl<T> KeyframeBase<T> {
    #[inline] pub fn as_array(&self) -> &[T] {
        if let Some(ArrayScalar::Array(val)) = &self.value { val } else {
            unreachable!("Expected array, encountered scalar or none") }
    }

    #[inline] pub fn as_scalar(&self) -> &T { // can be frequently called
        match &self.value { None => unreachable!(),
            Some(ArrayScalar::Scalar(val)) => val,
            Some(ArrayScalar::Array(val)) => { // Expected single-element array
                debug_assert!(val.len() == 1);  &val[0]
            }
        }
    }
}

impl Animation {
    pub fn from_reader<R: std::io::Read>(r: R) -> // TODO: print out summary here?
        Result<Self, serde_json::Error> { serde_json::from_reader(r) }
}

impl<T: Clone + Lerp> AnimatedProperty<T> { #[allow(unused)]
    pub fn from_value(val: T) -> Self {
        Self { animated: false.into(), keyframes: AnimatedValue::Static(val) }
    }

    // XXX: wrapped in Some, and use Cow<&T> to avoid unnecessary clone?
    pub fn get_value(&self, fnth: f32) -> T {
        match &self.keyframes {
            AnimatedValue::Static(val) => {
                debug_assert!(!self.animated.as_bool());    val.clone()
            }
            AnimatedValue::Animated(coll) => {
                debug_assert!(/* self.animated.as_bool() && */!coll.is_empty());
                if fnth <= coll[0].start { return coll[0].as_scalar().clone() }

                let mut len = coll.len() - 1;
                if coll[len].value.is_none() { if 0 < len { len -= 1; } else { unreachable!() } }
                if coll[len].start <= fnth { return coll[len].as_scalar().clone() }
                while 0 < len { len -= 1; if coll[len].start <= fnth { break } }
                // assert `coll` is sorted by `start` as well
                fn get_scalar(val: &ArrayScalar<f32>) -> f32 { match val {
                    ArrayScalar::Array(val) => val[0],
                    ArrayScalar::Scalar(val) => *val,
                } } // XXX: handle multiple dimention? https://lib.rs/keywords/cubic

                let kf =  &coll[len];
                let time = (fnth - kf.start) / (coll[len + 1].start - kf.start);
                use flo_curves::{bezier::Curve, BezierCurve, BezierCurveFactory, Coord2};

                let ctrl = kf.easing.as_ref().map_or(
                    (Coord2(0., 0.), Coord2(1., 1.)), |eh|
                    (Coord2(get_scalar(&eh.to.time) as _, get_scalar(&eh.to.factor) as _),
                     Coord2(get_scalar(&eh.ti.time) as _, get_scalar(&eh.ti.factor) as _)));

                let curve = Curve::from_points(Coord2(0., 0.), ctrl, Coord2(1., 1.));
                let time = curve.point_at_pos(time as _).1 as _;
                kf.as_scalar().lerp(coll[len + 1].as_scalar(), time)
            }   _ => unreachable!(),
        }
    }
}

impl<'de> Deserialize<'de> for LayersItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok( match value.get("ty").and_then(serde_json::Value::as_u64)
            .ok_or_else(|| D::Error::missing_field("ty"))? as u32 {

            0 => Self::PrecompLayer(PrecompLayer::
                deserialize(value).map_err(D::Error::custom)?),
            1 => Self::SolidColor(SolidLayer::deserialize(value).map_err(D::Error::custom)?),
            2 | 15 => Self::Image(ImageLayer::deserialize(value).map_err(D::Error::custom)?),
            3 => Self::Null(VisualLayer::deserialize(value).map_err(D::Error::custom)?),
            4 => Self::Shape(ShapeLayer::deserialize(value).map_err(D::Error::custom)?),
            5 => Self::Text ( TextLayer::deserialize(value).map_err(D::Error::custom)?),
            6 => Self::Audio(AudioLayer::deserialize(value).map_err(D::Error::custom)?),
           13 => Self::Camera(CameraLayer::deserialize(value).map_err(D::Error::custom)?),

            _ => unreachable!()
        })
    }
}

/* impl<'de, T, K> Deserialize<'de> for AnimatedValue<T, K>
    where T: Deserialize<'de>, K: Deserialize<'de> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;     //dbg!(&value);
        // "a" is in the same level as "k", but value is sub-level of "k"
        // XXX: Note some old animations might not have this
        Ok( match value.get("a").and_then(serde_json::Value::as_u64).unwrap_or(0) {
            1 => Self::Animated(Vec::<K>::deserialize(value).map_err(D::Error::custom)?),
            0 => Self::Static(T::deserialize(value).map_err(D::Error::custom)?),
            _ => unreachable!()
        })
    }
}

pub(crate) fn serialize_animated<S, T, K>(av: &AnimatedValue<T, K>, serializer: S) ->
    Result<S::Ok, S::Error> where S: Serializer, T: Serialize, K: Serialize {
    #[derive(Serialize)] struct AnimatedHelper<'a, T, K> { a: u8,
        #[serde(flatten)] content: &'a AnimatedValue<T, K>,
    }

    let item = match av {
        AnimatedValue::Animated(_) => AnimatedHelper { a: 1, content: av, },
        AnimatedValue::Static(_)   => AnimatedHelper { a: 0, content: av, },
    };  item.serialize(serializer)
} */

pub(crate) fn deserialize_strarray<'de, D: Deserializer<'de>>(d: D)
    -> Result<Vec<String>, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    if let Ok(v) = String::deserialize(&value) { Ok(vec![v]) } else {
        Vec::<String>::deserialize(value).map_err(D::Error::custom)
    }
}

impl<'de> Deserialize<'de> for EffectValuesItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok( match value.get("ty").and_then(serde_json::Value::as_u64)
            .ok_or_else(|| D::Error::missing_field("ty"))? as u32 {

            0 => Self::Slider(EffectValue::<Value>::
                deserialize(value).map_err(D::Error::custom)?),
            1 => Self::Angle (EffectValue::<Value>::
                deserialize(value).map_err(D::Error::custom)?),
            2 => Self::EffectColor(EffectValue::<ColorValue>::
                deserialize(value).map_err(D::Error::custom)?),
            3 => Self::Point(EffectValue::<Animated2D>::
                deserialize(value).map_err(D::Error::custom)?),
            4 => Self::Checkbox(EffectValue::<Value>::
                deserialize(value).map_err(D::Error::custom)?),
            //   Self::CustomEffect
            6 => Self::Ignored (EffectValue::<f32>::
                deserialize(value).map_err(D::Error::custom)?),
            7 => Self::DropDown(EffectValue::<Value>::
                deserialize(value).map_err(D::Error::custom)?),
           10 => Self::EffectLayer(EffectValue::<Value>::
                deserialize(value).map_err(D::Error::custom)?),
            //   Self::NoValue
            _ => unreachable!()
        })
    }
}

impl<'de> Deserialize<'de> for LayerStyleItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok( match value.get("ty").and_then(serde_json::Value::as_u64)
            .ok_or_else(|| D::Error::missing_field("ty"))? as u32 {

            0 => Self::Stroke(StrokeStyle::deserialize(value).map_err(D::Error::custom)?),
            1 => Self::DropShadow (DropShadowStyle::
                deserialize(value).map_err(D::Error::custom)?),
            2 => Self::InnerShadow(InnerShadowStyle::
                deserialize(value).map_err(D::Error::custom)?),
            3 => Self::OuterGlow(OuterGlowStyle::deserialize(value).map_err(D::Error::custom)?),
            4 => Self::InnerGlow(InnerGlowStyle::deserialize(value).map_err(D::Error::custom)?),
            5 => Self::BevelEmboss(BevelEmbossStyle::
                deserialize(value).map_err(D::Error::custom)?),
            6 => Self::Satin(SatinStyle::deserialize(value).map_err(D::Error::custom)?),
            7 => Self::ColorOverlay(ColorOverlayStyle::
                deserialize(value).map_err(D::Error::custom)?),
            8 => Self::GradientOverlay(GradientOverlayStyle::
                deserialize(value).map_err(D::Error::custom)?),

            _ => unreachable!()
        })
    }
}

#[derive(Clone, Debug, Serialize)] pub struct AnyAsset(AssetBase); //serde_json::Value
impl<'de> Deserialize<'de> for AnyAsset {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        //panic!("{}", value.to_string().get(0..20).unwrap());
        //let _ = Precomposition::deserialize(&value).unwrap();
        let value = AssetBase::deserialize(value).unwrap();
        panic!("Failed on asset: {{ id: {}, nm: {} }}", value.id, value.nm);
    }
}

#[derive(Clone, Debug, Serialize)] pub struct AnyValue(serde_json::Value);
impl<'de> Deserialize<'de> for AnyValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        panic!("{}", serde_json::Value::deserialize(d)?);
    }
}

#[cfg(test)] mod test { use super::*;
    use serde_test::{Token, assert_tokens/*, assert_de_tokens, assert_ser_tokens*/};

    #[test] fn test_enum_type() {
        let tokens = [
            Token::Struct { name: "Container", len: 1 },
            Token::Str("layers"),
            Token::Seq { len: Some(1) },
                Token::Map { len: None, },
                //Token::NewtypeVariant { name: "TestLayersItem", variant: "SomeLayer" },
                    //Token::Struct { name: "SomeLayer", len: 3 },
                    Token::Str("ty"),  Token::U32(0),
                    Token::Str("ind"), Token::U32(1),
                    Token::Str("nm"),  Token::String("name"),
                    //Token::StructEnd,
                Token::MapEnd,
            Token::SeqEnd,
            Token::StructEnd,
        ];

        let cont = Container { layers: vec![
            TestLayersItem::SomeLayer( SomeLayer { ind: 1, nm: "name".to_owned() }),
        ] };

        assert_tokens(&cont, &tokens);
        //assert_de_tokens (&cont, &tokens);
        //assert_ser_tokens(&cont, &tokens);
    }

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)] struct Container {
    #[serde(serialize_with = "serialize_with_type")] layers: Vec<TestLayersItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)] enum TestLayersItem { SomeLayer(SomeLayer),
    //Color(Rgba), //IntBool(IntBool), //Vector2D(Vector2D),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)] struct SomeLayer {
    ind: u32, nm: String,
}

impl<'de> Deserialize<'de> for TestLayersItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;

        Ok( match value.get("ty").and_then(serde_json::Value::as_u64)
            .ok_or_else(|| D::Error::missing_field("ty"))? as u32 {
            0 => Self::SomeLayer(SomeLayer::deserialize(value).map_err(D::Error::custom)?),

            _ => unreachable!()
        })
    }
}

fn serialize_with_type<S: Serializer>(layers: &[TestLayersItem],
    serializer: S) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)] struct TypedLayersItem<'a> { ty: u32,
        #[serde(flatten)] content: &'a TestLayersItem,
    }

    use serde::ser::SerializeSeq;
    let mut state = serializer.serialize_seq(Some(layers.len()))?;
    for layer in layers {
        let item = match layer {
            TestLayersItem::SomeLayer(_) => TypedLayersItem { ty: 0, content: layer, },
                //serializer.serialize_newtype_variant("ty", 0, "", layer),

            //_ => unreachable!()
        };  //item.serialize(serializer)
        state.serialize_element(&item)?;
    }   state.end()

    //let mut state = serializer.serialize_struct("Struct of Layer", fields_of_layer + 1)?;
    //state.serialize_field("ty", 0)?; state.serialize_fields_of(layer)?;   state.end()

    //serializer.serialize_str("ty: ")?; serializer.serialize_u32(0)?;
    //layer.serialize(serializer)   // flatten layer serialization
}

}
