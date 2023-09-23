
use serde::{de::Error, Serialize, Deserialize, Deserializer, Serializer};

//  Represents boolean values as an integer. 0 is false, 1 is true.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(transparent)] pub struct IntBool(u8);

impl From<IntBool> for bool { fn from(value: IntBool) -> Self { value.0 != 0 } }
impl From<bool> for IntBool { fn from(value: bool) -> Self { Self(if value { 1 } else { 0 }) } }

/* #[derive(Debug, Clone, Copy)] pub struct Rgb  { pub r: u8, pub g: u8, pub b: u8 }
impl Rgb {  pub fn new_u8 (r:  u8, g:  u8, b:  u8) -> Self { Self { r, g, b } }
            pub fn new_f32(r: f32, g: f32, b: f32) -> Self { Self {
        r: (r * 255.0) as u8, g: (g * 255.0) as u8, b: (b * 255.0) as u8
    } }
} */

#[derive(Debug, Clone, Copy)] pub struct Rgba { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }
impl Default for Rgba { fn default() -> Self { Self { r: 0, g: 0, b: 0, a: 255 } } }

impl Rgba {
    pub fn new_u8 (r:  u8, g:  u8, b:  u8, a:  u8) -> Self { Self { r, g, b, a } }
    pub fn new_f32(r: f32, g: f32, b: f32, a: f32) -> Self { Self {
        r: (r * 255.0) as u8, g: (g * 255.0) as u8, b: (b * 255.0) as u8, a: (a * 255.0) as u8
    } }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = Vec::<f32>::deserialize(deserializer)?;
        assert!(2 < v.len() && v.len() < 5);
        Ok(Self::new_f32(v[0], v[1], v[2], v.get(3).cloned().unwrap_or(1.0)))
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut v = vec![self.r as f32 / 255.0,
            self.g as f32 / 255.0, self.b as f32 / 255.0];
        if  self.a < 255 {  v.push(self.a as f32 / 255.0); }    v.serialize(serializer)
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
#[derive(Debug, Clone)] pub struct Vector2D { pub x: f32, pub y: f32 } // Point/Size

impl<'de> Deserialize<'de> for Vector2D {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = Vec::<f32>::deserialize(deserializer)?;
        assert!(!v.is_empty() && v.len() < 4); // XXX: just ignore extra 3rd value?
        Ok(Self { x: v[0], y: v.get(1).cloned().unwrap_or(0.0) })
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
                Rgba::new_f32(chunk[1], chunk[2], chunk[3], 1.0))).collect()
        } else  if len == cnt * 4 + cnt * 2 { let cnt = (cnt * 4) as usize; // Rgba color
            data[0..cnt].chunks(4).zip(data[cnt..].chunks(2))
                .map(|(chunk, opacity)| (chunk[0], // == opacity[0]
                Rgba::new_f32(chunk[1], chunk[2], chunk[3], opacity[1]))).collect()
        } else { unreachable!() }))
    }
}

impl Serialize for ColorList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut data = self.0.iter().flat_map(|(offset, color)|
            vec![*offset, color.r as f32 / 255.0, color.g as f32 / 255.0,
                          color.b as f32 / 255.0]).collect::<Vec<_>>();

        if  self.0.iter().any(|(_, color)| color.a < 255) {
            data.extend(self.0.iter().flat_map(|(offset, color)|
                vec![*offset, color.a as f32 / 255.0]));
        }   data.serialize(serializer)
    }
}

/* use crate::schema::{LayersItem, PrecompositionLayer};

impl<'de> Deserialize<'de> for LayersItem {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;

        Ok( match value.get("ty").and_then(serde_json::Value::as_u64)
            .ok_or_else(|| D::Error::missing_field("ty"))? {
            0 => Self::PrecompositionLayer(
                PrecompositionLayer::deserialize(value).map_err(D::Error::custom)?),

            _ => unreachable!()
        })
    }
}

impl Serialize for LayersItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::PrecompositionLayer(layer) =>
                serializer.serialize_newtype_variant("ty", 0, "", layer),

            _ => unreachable!()
        }

        //let mut state = serializer.serialize_struct("", 2)?;
        //state.serialize_field("ty", 0)?; state.serialize_field("", layer)?; state.end()

        //#[derive(Serialize)] #[serde(untagged)] enum LayersItemRef<'a> {
        //    SolidColor(&'a SolidColor),
        //}
        //#[derive(Serialize)] struct TypedLayersItem<'a> {
        //    ty: u32, #[serde(flatten)] content: LayersItemRef<'a>,
        //}

        //serializer.serialize_str("ty: ")?; serializer.serialize_u32(0)?;
        //layer.serialize(serializer)
    }
} */
