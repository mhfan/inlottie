
#![allow(clippy::enum_variant_names)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Angle {
    #[serde(flatten)]
    pub effect_value: EffectValue,
    pub ty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<Value>,
}
#[doc = "An animatable property that holds an array of numbers"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnimatedProperty {
    #[serde(flatten)]
    pub subtype_0: AnimatedPropertySubtype0,
    #[serde(flatten)]
    pub subtype_1: AnimatedPropertySubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnimatedPropertySubtype0 {
    #[doc = "Whether the property is animated"]
    #[serde(default = "defaults::animated_property_subtype0_a")]
    pub a: IntBoolean,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ix: Option<u32>,
    #[doc = "One of the ID in the file's slots"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnimatedPropertySubtype1 {
    #[doc = "Array of keyframes"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub k: Vec<Keyframe>,
}
#[doc = "Animated property representing the text contents"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnimatedTextDocument {
    pub k: Vec<TextDocumentKeyframe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
}
#[doc = "Top level object, describing the animation"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Animation {
    #[serde(flatten)]
    pub visual_object: VisualObject,
    #[serde(flatten)]
    pub composition: Composition,
    #[doc = "List of assets that can be referenced by layers"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<AssetsItem>,
    #[doc = "Data defining text characters as lottie shapes. If present a player might only render characters defined here and nothing else."]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chars: Vec<CharacterData>,
    #[doc = "List of Extra compositions not referenced by anything"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comps: Vec<Precomposition>,
    #[doc = "Whether the animation has 3D layers"]
    #[serde(default = "defaults::animation_ddd")]
    pub ddd: IntBoolean,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fonts: Option<FontList>,
    pub fr: f32,
    #[doc = "Height of the animation"]
    pub h: u32,
    pub ip: f32,
    #[doc = "Markers defining named sections of the composition."]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mb: Option<MotionBlur>,
    #[doc = "Document metadata"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Metadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<UserMetadata>,
    pub op: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slots: Option<Slots>,
    #[serde(default = "defaults::animation_v")]
    pub v: String,
    #[doc = "Width of the animation"]
    pub w: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Asset {
    #[doc = "Unique identifier used by layers when referencing this asset"]
    pub id: String,
    #[doc = "Human readable name"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nm: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum AssetsItem {
    Image(Image),
    Precomposition(Precomposition),
    Sound(Sound),
    DataSource(DataSource),
}
#[doc = "A layer playing sounds"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioLayer {
    #[serde(flatten)]
    pub layer: Layer,
    pub au: AudioSettings,
    #[doc = "ID of the sound as specified in the assets"]
    #[serde(rename = "refId", default, skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    #[doc = "Layer type"]
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioSettings {
    pub lv: MultiDimensional,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BaseStroke {
    #[doc = "Dashed line definition"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub d: Vec<StrokeDash>,
    #[serde(default = "defaults::base_stroke_lc")]
    pub lc: LineCap,
    #[serde(default = "defaults::base_stroke_lj")]
    pub lj: LineJoin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ml: Option<f32>,
    #[doc = "Animatable alternative to ml"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ml2: Option<Value>,
    #[doc = "Opacity, 100 means fully opaque"]
    pub o: Value,
    #[doc = "Stroke width"]
    pub w: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BevelEmbossStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[doc = "Local lighting angle"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bs: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bt: Option<Value>,
    #[doc = "Use global light"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ga: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hc: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ho: Option<Value>,
    #[doc = "Local lighting altitude"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ll: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sc: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sf: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub so: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sr: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[doc = "Single bezier curve"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Bezier {
    #[serde(default)]
    pub c: bool,
    #[doc = "Array of points, each point is an array of coordinates.\nThese points are along the `in` tangents relative to the corresponding `v`."]
    pub i: Vec<Vec<f32>>,
    #[doc = "Array of points, each point is an array of coordinates.\nThese points are along the `out` tangents relative to the corresponding `v`."]
    pub o: Vec<Vec<f32>>,
    #[doc = "Array of points, each point is an array of coordinates.\nThese points are along the bezier path"]
    pub v: Vec<Vec<f32>>,
}
#[doc = "Layer and shape blend mode"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BlendMode {
    Normal(serde_json::Value),
    Multiply(serde_json::Value),
    Screen(serde_json::Value),
    Overlay(serde_json::Value),
    Darken(serde_json::Value),
    Lighten(serde_json::Value),
    ColorDodge(serde_json::Value),
    ColorBurn(serde_json::Value),
    HardLight(serde_json::Value),
    SoftLight(serde_json::Value),
    Difference(serde_json::Value),
    Exclusion(serde_json::Value),
    Hue(serde_json::Value),
    Saturation(serde_json::Value),
    Color(serde_json::Value),
    Luminosity(serde_json::Value),
    Add(serde_json::Value),
    HardMix(serde_json::Value),
}
#[doc = "3D Camera"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CameraLayer {
    #[serde(flatten)]
    pub layer: Layer,
    #[doc = "Layer transform"]
    pub ks: Transform,
    #[doc = "Distance from the Z=0 plane.\nSmall values yield a higher perspective effect."]
    pub pe: Value,
    #[doc = "Layer type"]
    pub ty: u32,
}
#[doc = "Defines character shapes"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterData {
    pub ch: String,
    pub data: Data,
    #[serde(rename = "fFamily")]
    pub f_family: String,
    pub size: f32,
    pub style: String,
    pub w: f32,
}
#[doc = "Defines a character as a precomp layer"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterPrecomp {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<f32>,
    #[doc = "Layer transform"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ks: Option<Transform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<f32>,
    #[doc = "ID of the precomp as specified in the assets"]
    #[serde(rename = "refId")]
    pub ref_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sr: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub st: Option<f32>,
}
#[doc = "Defines a character as shapes"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterShapes {
    #[doc = "Shapes forming the character"]
    pub shapes: ShapeList,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Checkbox {
    #[serde(flatten)]
    pub effect_value: EffectValue,
    pub ty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<Value>,
}
#[doc = "Color as a [r, g, b] array with values in [0, 1]"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Color(pub Vec<f32>);
impl From<Vec<f32>> for Color {
    fn from(value: Vec<f32>) -> Self {
        Self(value)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ColorOverlayStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub so: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[doc = "An animatable property that holds a Color"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ColorValue {
    #[serde(flatten)]
    pub subtype_0: AnimatedProperty,
    #[serde(flatten)]
    pub subtype_1: ColorValueSubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ColorValueSubtype1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k: Option<Color>,
}
#[doc = "How to stack copies in a repeater"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Composite {
    Above(serde_json::Value),
    Below(serde_json::Value),
}
#[doc = "Base class for layer holders"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Composition {
    pub layers: Vec<LayersItem>,
}
#[doc = "Some lottie files use `ty` = 5 for many different effects"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomEffect {
    #[serde(flatten)]
    pub effect: Effect,
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Data {
    Shapes(CharacterShapes),
    Precomp(CharacterPrecomp),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DataLayer {
    #[serde(flatten)]
    pub layer: Layer,
    #[doc = "ID of the data source in assets"]
    #[serde(rename = "refId", default, skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    #[doc = "Layer type"]
    pub ty: u32,
}
#[doc = "External data source, usually a JSON file"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DataSource {
    #[serde(flatten)]
    pub file_asset: FileAsset,
    pub t: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DisplacementMapEffect {
    #[serde(flatten)]
    pub effect: Effect,
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DropDown {
    #[serde(flatten)]
    pub effect_value: EffectValue,
    pub ty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<Value>,
}
#[doc = "Adds a shadow to the layer"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DropShadowEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DropShadowStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[doc = "Local light angle"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<Value>,
    #[doc = "Layer knowck out drop shadow"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lc: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[doc = "Blur size"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[doc = "Layer effect"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Effect {
    #[serde(flatten)]
    pub visual_object: VisualObject,
    pub ef: Vec<EffectValuesItem>,
    #[serde(default = "defaults::effect_en")]
    pub en: IntBoolean,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ix: Option<u32>,
    #[doc = "Number of values in `ef`"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub np: Option<u32>,
    #[doc = "Effect type"]
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EffectColor {
    #[serde(flatten)]
    pub effect_value: EffectValue,
    pub ty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<ColorValue>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EffectLayer {
    #[serde(flatten)]
    pub effect_value: EffectValue,
    pub ty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<Value>,
}
#[doc = "Value for an effect"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EffectValue {
    #[serde(flatten)]
    pub visual_object: VisualObject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ix: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<u32>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EffectValuesItem {
    NoValue(NoValue),
    Angle(Angle),
    Checkbox(Checkbox),
    EffectColor(EffectColor),
    DropDown(DropDown),
    Ignored(Ignored),
    EffectLayer(EffectLayer),
    Point(Point),
    Slider(Slider),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EffectsItem {
    CustomEffect(CustomEffect),
    DropShadowEffect(DropShadowEffect),
    FillEffect(FillEffect),
    GaussianBlurEffect(GaussianBlurEffect),
    Matte3Effect(Matte3Effect),
    ProLevelsEffect(ProLevelsEffect),
    StrokeEffect(StrokeEffect),
    TintEffect(TintEffect),
    TritoneEffect(TritoneEffect),
    RadialWipeEffect(RadialWipeEffect),
    WavyEffect(WavyEffect),
    PuppetEffect(PuppetEffect),
    SpherizeEffect(SpherizeEffect),
    MeshWarpEffect(MeshWarpEffect),
    DisplacementMapEffect(DisplacementMapEffect),
    TwirlEffect(TwirlEffect),
}
#[doc = "Ellipse shape"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Ellipse {
    #[serde(flatten)]
    pub shape: Shape,
    pub p: Position,
    pub s: MultiDimensional,
    pub ty: String,
}
#[doc = "Asset referencing a file"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileAsset {
    #[serde(flatten)]
    pub asset: Asset,
    #[doc = "Whether the file is embedded"]
    #[serde(default = "defaults::file_asset_e")]
    pub e: IntBoolean,
    #[doc = "Filename or data url"]
    pub p: String,
    #[doc = "Path to the directory containing a file"]
    #[serde(default)]
    pub u: String,
}
#[doc = "Solid fill color"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Fill {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    pub c: ColorValue,
    #[doc = "Opacity, 100 means fully opaque"]
    pub o: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<FillRule>,
    pub ty: String,
}
#[doc = "Replaces the whole layer with the given color"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FillEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "Rule used to handle multiple shapes rendered with the same fill object"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FillRule {
    NonZero(serde_json::Value),
    EvenOdd(serde_json::Value),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Font {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ascent: Option<f32>,
    #[doc = "CSS Class applied to text objects using this font"]
    #[serde(rename = "fClass", default, skip_serializing_if = "Option::is_none")]
    pub f_class: Option<String>,
    #[serde(rename = "fFamily")]
    pub f_family: String,
    #[doc = "Name used by text documents to reference this font, usually it's `fFamily` followed by `fStyle`"]
    #[serde(rename = "fName")]
    pub f_name: String,
    #[serde(rename = "fPath", default, skip_serializing_if = "Option::is_none")]
    pub f_path: Option<String>,
    #[serde(rename = "fStyle")]
    pub f_style: String,
    #[serde(rename = "fWeight", default, skip_serializing_if = "Option::is_none")]
    pub f_weight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<FontPathOrigin>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FontList {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<Font>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FontPathOrigin {
    Local(serde_json::Value),
    CssUrl(serde_json::Value),
    ScriptUrl(serde_json::Value),
    FontUrl(serde_json::Value),
}
#[doc = "Gaussian blur"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GaussianBlurEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Gradient {
    #[doc = "Highlight Angle, relative to the direction from `s` to `e`"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[doc = "End point for the gradient"]
    pub e: MultiDimensional,
    #[doc = "Gradient colors"]
    pub g: GradientColors,
    #[doc = "Highlight Length, as a percentage between `s` and `e`"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<Value>,
    #[doc = "Starting point for the gradient"]
    pub s: MultiDimensional,
    #[doc = "Type of the gradient"]
    #[serde(default = "defaults::gradient_t")]
    pub t: GradientType,
}
#[doc = "Represents colors and offsets in a gradient\n\nColors are represented as a flat list interleaving offsets and color components in weird ways\nThere are two possible layouts:\n\nWithout alpha, the colors are a sequence of offset, r, g, b\n\nWith alpha, same as above but at the end of the list there is a sequence of offset, alpha"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GradientColors {
    pub k: MultiDimensional,
    #[doc = "Number of colors in `k`"]
    pub p: u32,
}
#[doc = "Gradient fill"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GradientFill {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[serde(flatten)]
    pub gradient: Gradient,
    pub o: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<FillRule>,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GradientOverlayStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[doc = "Align with layer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub al: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gf: Option<GradientColors>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gs: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gt: Option<GradientType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub re: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[doc = "Gradient stroke"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GradientStroke {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[serde(flatten)]
    pub base_stroke: BaseStroke,
    #[serde(flatten)]
    pub gradient: Gradient,
    pub ty: String,
}
#[doc = "Type of a gradient"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GradientType {
    Linear(u32),
    Radial(u32),
}
#[doc = "Shape Element that can contain other shapes"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Group {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[doc = "Index used in expressions"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cix: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub it: Option<ShapeList>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub np: Option<f32>,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Ignored {
    #[serde(flatten)]
    pub effect_value: EffectValue,
    pub ty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<f32>,
}
#[doc = "External image"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Image {
    #[serde(flatten)]
    pub file_asset: FileAsset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f32>,
    #[doc = "Marks as part of an image sequence if present"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f32>,
}
#[doc = "Layer that shows an image asset"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageLayer {
    #[serde(flatten)]
    pub visual_layer: VisualLayer,
    #[doc = "ID of the image as specified in the assets"]
    #[serde(rename = "refId")]
    pub ref_id: String,
    #[doc = "Layer type"]
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InnerGlowStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub j: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sr: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InnerShadowStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[doc = "Local light angle"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[doc = "Blur size"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[doc = "Represents boolean values as an integer. 0 is false, 1 is true."]
#[derive(Clone, Debug, Serialize)]
pub struct IntBoolean(IntegerBoolean); // TODO:
impl Default for IntBoolean {
    fn default() -> Self {
        IntBoolean(IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
}
impl std::convert::TryFrom<IntegerBoolean> for IntBoolean {
    type Error = &'static str;
    fn try_from(value: IntegerBoolean) -> Result<Self, &'static str> {
        if ![
            IntegerBoolean::True(serde_json::from_str::<serde_json::Value>("0").unwrap()),
            IntegerBoolean::True(serde_json::from_str::<serde_json::Value>("1").unwrap()),
        ]
        .contains(&value)
        {
            Err("invalid value")
        } else {
            Ok(Self(value))
        }
    }
}
impl<'de> serde::Deserialize<'de> for IntBoolean {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::try_from(<IntegerBoolean>::deserialize(deserializer)?)
            .map_err(|e| <D::Error as serde::de::Error>::custom(e.to_string()))
    }
}
#[doc = "Represents boolean values as an integer. 0 is false, 1 is true."]
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum IntegerBoolean {
    True(serde_json::Value),
    False(serde_json::Value),
}
#[doc = "A Keyframes specifies the value at a specific time and the interpolation function to reach the next keyframe."]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Keyframe {
    #[serde(flatten)]
    pub subtype_0: KeyframeBase,
    #[serde(flatten)]
    pub subtype_1: KeyframeSubtype1,
    #[serde(flatten)]
    pub subtype_2: KeyframeSubtype2,
}
#[doc = "A Keyframes specifies the value at a specific time and the interpolation function to reach the next keyframe."]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyframeBase {
    #[serde(flatten)]
    pub subtype_0: KeyframeBaseSubtype0,
    #[serde(flatten)]
    pub subtype_1: KeyframeBaseSubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyframeBaseSubtype0 {
    #[serde(default = "defaults::keyframe_base_subtype0_h")]
    pub h: IntBoolean,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<f32>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyframeBaseSubtype1 {
    #[doc = "Easing tangent going into the next keyframe"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i: Option<KeyframeBezierHandle>,
    #[doc = "Easing tangent leaving the current keyframe"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<KeyframeBezierHandle>,
}
#[doc = "Bezier handle for keyframe interpolation"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyframeBezierHandle {
    #[doc = "Time component:\n0 means start time of the keyframe,\n1 means time of the next keyframe."]
    pub x: X,
    #[doc = "Value interpolation component:\n0 means start value of the keyframe,\n1 means value at the next keyframe."]
    pub y: Y,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyframeSubtype1 {
    #[doc = "Value at the end of the keyframe, note that this is deprecated and you should use `s` from the next keyframe to get this value"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub e: Vec<f32>,
    #[doc = "Value at this keyframe. Note the if the property is a scalar, keyframe values are still represented as arrays"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub s: Vec<f32>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyframeSubtype2 {
    #[doc = "Easing tangent going into the next keyframe"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i: Option<KeyframeBezierHandle>,
    #[doc = "Easing tangent leaving the current keyframe"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<KeyframeBezierHandle>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Layer {
    #[serde(flatten)]
    pub visual_object: VisualObject,
    #[doc = "Whether the layer is threedimensional"]
    #[serde(default = "defaults::layer_ddd")]
    pub ddd: IntBoolean,
    #[doc = "Whether the layer is hidden"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hd: Option<bool>,
    #[doc = "Index that can be used for parenting and referenced in expressions"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ind: Option<u32>,
    pub ip: f32,
    pub op: f32,
    #[doc = "Must be the `ind` property of another layer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sr: Option<f32>,
    pub st: f32,
    #[doc = "Layer Type"]
    pub ty: Type,
}
#[doc = "Style applied to a layer"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LayerStyle {
    #[serde(flatten)]
    pub visual_object: VisualObject,
    #[doc = "Style Type"]
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LayerStyleItem {
    StrokeStyle(StrokeStyle),
    DropShadowStyle(DropShadowStyle),
    InnerShadowStyle(InnerShadowStyle),
    OuterGlowStyle(OuterGlowStyle),
    InnerGlowStyle(InnerGlowStyle),
    BevelEmbossStyle(BevelEmbossStyle),
    SatinStyle(SatinStyle),
    ColorOverlayStyle(ColorOverlayStyle),
    GradientOverlayStyle(GradientOverlayStyle),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LayersItem {
    PrecompositionLayer(PrecompositionLayer),
    SolidColorLayer(SolidColorLayer),
    ImageLayer(ImageLayer),
    NullLayer(NullLayer),
    ShapeLayer(ShapeLayer),
    TextLayer(TextLayer),
    AudioLayer(AudioLayer),
    CameraLayer(CameraLayer),
    DataLayer(DataLayer),
}
#[doc = "Style at the end of a stoked line"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LineCap {
    Butt(serde_json::Value),
    Round(serde_json::Value),
    Square(serde_json::Value),
}
#[doc = "Style at a sharp corner of a stoked line"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LineJoin {
    Miter(serde_json::Value),
    Round(serde_json::Value),
    Bevel(serde_json::Value),
}
#[doc = "Defines named portions of the composition."]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Marker {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dr: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tm: Option<f32>,
}
#[doc = "Bezier shape used to mask/clip a layer"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Mask {
    #[serde(flatten)]
    pub visual_object: VisualObject,
    #[serde(default)]
    pub inv: bool,
    #[serde(default = "defaults::mask_mode")]
    pub mode: MaskMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pt: Option<ShapeProperty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Value>,
}
#[doc = "How masks interact with each other. See https://helpx.adobe.com/after-effects/using/alpha-channels-masks-mattes.html"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MaskMode {
    None(serde_json::Value),
    Add(serde_json::Value),
    Subtract(serde_json::Value),
    Intersect(serde_json::Value),
    Lighten(serde_json::Value),
    Darken(serde_json::Value),
    Difference(serde_json::Value),
}
#[doc = "Uses a layer as a mask"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Matte3Effect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "How a layer should mask another layer"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MatteMode {
    Normal(serde_json::Value),
    Alpha(serde_json::Value),
    InvertedAlpha(serde_json::Value),
    Luma(serde_json::Value),
    InvertedLuma(serde_json::Value),
}
#[doc = "Boolean operator on shapes"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Merge {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[serde(default = "defaults::merge_mm")]
    pub mm: MergeMode,
    pub ty: String,
}
#[doc = "Boolean operation on shapes"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MergeMode {
    Normal(serde_json::Value),
    Add(serde_json::Value),
    Subtract(serde_json::Value),
    Intersect(serde_json::Value),
    ExcludeIntersections(serde_json::Value),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MeshWarpEffect {
    #[serde(flatten)]
    pub effect: Effect,
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "Document metadata"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Metadata {
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub subtype_0: Option<MetadataSubtype0>,
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub subtype_1: Option<MetadataSubtype1>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetadataSubtype0 {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub k: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetadataSubtype1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Modifier(pub ShapeElement);
impl From<ShapeElement> for Modifier {
    fn from(value: ShapeElement) -> Self {
        Self(value)
    }
}
#[doc = "Motion blur settings"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MotionBlur {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asl: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sa: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sp: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spf: Option<f32>,
}
#[doc = "An animatable property that holds an array of numbers"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MultiDimensional {
    #[serde(flatten)]
    pub subtype_0: AnimatedProperty,
    #[serde(flatten)]
    pub subtype_1: MultiDimensionalSubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MultiDimensionalSubtype1 {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub k: Vec<f32>,
}
#[doc = "Represents a style for shapes without fill or stroke"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NoStyle {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NoValue(pub serde_json::Map<String, serde_json::Value>);
impl From<serde_json::Map<String, serde_json::Value>> for NoValue {
    fn from(value: serde_json::Map<String, serde_json::Value>) -> Self {
        Self(value)
    }
}
#[doc = "Layer with no data, useful to group layers together"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NullLayer {
    #[serde(flatten)]
    pub visual_layer: VisualLayer,
    #[doc = "Layer type"]
    pub ty: u32,
}
#[doc = "Interpolates the shape with its center point and bezier tangents with the opposite direction"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OffsetPath {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[serde(default = "defaults::offset_path_lj")]
    pub lj: LineJoin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ml: Option<Value>,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OuterGlowStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub j: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[doc = "Animatable Bezier curve"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Path {
    #[serde(flatten)]
    pub shape: Shape,
    #[doc = "Bezier path"]
    pub ks: ShapeProperty,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Point {
    #[serde(flatten)]
    pub effect_value: EffectValue,
    pub ty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<MultiDimensional>,
}
#[doc = "Star or regular polygon"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Polystar {
    #[serde(flatten)]
    pub subtype_0: Shape,
    #[serde(flatten)]
    pub subtype_1: PolystarSubtype1,
    #[serde(flatten)]
    pub subtype_2: PolystarSubtype2,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PolystarSubtype1 {
    pub or: Value,
    #[doc = "Outer Roundness as a percentage"]
    pub os: Value,
    pub p: Position,
    pub pt: Value,
    #[doc = "Rotation, clockwise in degrees"]
    pub r: Value,
    #[doc = "Star type, `1` for Star, `2` for Polygon"]
    #[serde(default = "defaults::polystar_subtype1_sy")]
    pub sy: StarType,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PolystarSubtype2 {
    pub ir: Value,
    #[doc = "Inner Roundness as a percentage"]
    pub is: Value,
}
#[doc = "An animatable property to represent a position in space"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Position {
    #[serde(flatten)]
    pub subtype_0: PositionSubtype0,
    #[serde(flatten)]
    pub subtype_1: PositionSubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PositionKeyframe {
    #[serde(flatten)]
    pub subtype_0: Keyframe,
    #[serde(flatten)]
    pub subtype_1: PositionKeyframeSubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PositionKeyframeSubtype1 {
    #[doc = "Tangent for values (eg: moving position around a curved path)"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ti: Vec<f32>,
    #[doc = "Tangent for values (eg: moving position around a curved path)"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub to: Vec<f32>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PositionSubtype0 {
    #[doc = "Whether the property is animated"]
    #[serde(default = "defaults::position_subtype0_a")]
    pub a: IntBoolean,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ix: Option<u32>,
    #[doc = "Number of components in the value arrays.\nIf present values will be truncated or expanded to match this length when accessed from expressions."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PositionSubtype1 {
    #[serde(flatten)]
    pub subtype_0: PositionSubtype1Subtype0,
    #[serde(flatten)]
    pub subtype_1: PositionSubtype1Subtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PositionSubtype1Subtype0 {
    #[doc = "Array of keyframes"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub k: Vec<PositionKeyframe>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PositionSubtype1Subtype1 {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub k: Vec<f32>,
}
#[doc = "Asset containing an animation that can be referenced by layers."]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Precomposition {
    #[serde(flatten)]
    pub asset: Asset,
    #[serde(flatten)]
    pub composition: Composition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fr: Option<f32>,
    #[doc = "Extra composition"]
    #[serde(default = "defaults::precomposition_xt")]
    pub xt: IntBoolean,
}
#[doc = "Layer that renders a Precomposition asset"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PrecompositionLayer {
    #[serde(flatten)]
    pub visual_layer: VisualLayer,
    #[doc = "Height of the clipping rect"]
    pub h: u32,
    #[doc = "ID of the precomp as specified in the assets"]
    #[serde(rename = "refId")]
    pub ref_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tm: Option<Value>,
    #[doc = "Layer type"]
    pub ty: u32,
    #[doc = "Width of the clipping rect"]
    pub w: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProLevelsEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "Interpolates the shape with its center point and bezier tangents with the opposite direction"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PuckerBloat {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[doc = "Amount as a percentage"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PuppetEffect {
    #[serde(flatten)]
    pub effect: Effect,
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RadialWipeEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "A simple rectangle shape"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rectangle {
    #[serde(flatten)]
    pub shape: Shape,
    #[doc = "Center of the rectangle"]
    pub p: Position,
    #[doc = "Rounded corners radius"]
    pub r: Value,
    pub s: MultiDimensional,
    pub ty: String,
}
#[doc = "Duplicates previous shapes in a group"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Repeater {
    #[serde(flatten)]
    pub modifier: Modifier,
    #[doc = "Number of copies"]
    pub c: Value,
    #[doc = "Stacking order"]
    #[serde(default = "defaults::repeater_m")]
    pub m: Composite,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[doc = "Transform applied to each copy"]
    pub tr: RepeaterTransform,
    pub ty: String,
}
#[doc = "Transform used by a repeater, the transform is applied to each subsequent repeated object."]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RepeaterTransform {
    #[serde(flatten)]
    pub transform: Transform,
    #[doc = "Opacity of the last repeated object."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eo: Option<Value>,
    #[doc = "Opacity of the first repeated object."]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub so: Option<Value>,
}
#[doc = "Rounds corners of other shapes"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RoundedCorners {
    #[serde(flatten)]
    pub modifier: Modifier,
    pub r: Value,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SatinStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<Value>,
    #[serde(rename = "in", default, skip_serializing_if = "Option::is_none")]
    pub in_: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[doc = "Drawable shape"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Shape {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[doc = "Direction the shape is drawn as, mostly relevant when using trim path"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<ShapeDirection>,
}
#[doc = "Drawing direction of the shape curve, useful for trim path"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ShapeDirection {
    Normal(serde_json::Value),
    Reversed(serde_json::Value),
}
#[doc = "Base class for all elements of ShapeLayer and Group"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapeElement {
    #[serde(flatten)]
    pub visual_object: VisualObject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<BlendMode>,
    #[doc = "CSS class used by the SVG renderer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cl: Option<String>,
    #[doc = "Whether the shape is hidden"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hd: Option<bool>,
    #[doc = "Index used in expressions"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ix: Option<u32>,
    #[doc = "`id` attribute used by the SVG renderer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ln: Option<String>,
    pub ty: ShapeType,
}
#[doc = "Keyframe holding Bezier objects"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapeKeyframe {
    #[serde(flatten)]
    pub keyframe_base: KeyframeBase,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub s: Vec<Bezier>,
}
#[doc = "Layer containing Shapes"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapeLayer {
    #[serde(flatten)]
    pub visual_layer: VisualLayer,
    pub shapes: ShapeList,
    #[doc = "Layer type"]
    pub ty: u32,
}
#[doc = "List of valid shapes"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapeList(pub Vec<ShapeListItem>);
impl From<Vec<ShapeListItem>> for ShapeList {
    fn from(value: Vec<ShapeListItem>) -> Self {
        Self(value)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ShapeListItem {
    Ellipse(Ellipse),
    Fill(Fill),
    GradientFill(GradientFill),
    GradientStroke(GradientStroke),
    Group(Group),
    Path(Path),
    Polystar(Polystar),
    PuckerBloat(PuckerBloat),
    Rectangle(Rectangle),
    Repeater(Repeater),
    RoundedCorners(RoundedCorners),
    Stroke(Stroke),
    ShapeTransform(ShapeTransform),
    Trim(Trim),
    Twist(Twist),
    Merge(Merge),
    OffsetPath(OffsetPath),
    ZigZag(ZigZag),
    NoStyle(NoStyle),
}
#[doc = "An animatable property that holds a Bezier"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapeProperty {
    #[serde(flatten)]
    pub subtype_0: ShapePropertySubtype0,
    #[serde(flatten)]
    pub subtype_1: ShapePropertySubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapePropertySubtype0 {
    #[doc = "Whether the property is animated"]
    #[serde(default = "defaults::shape_property_subtype0_a")]
    pub a: IntBoolean,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ix: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapePropertySubtype1 {
    #[serde(flatten)]
    pub subtype_0: ShapePropertySubtype1Subtype0,
    #[serde(flatten)]
    pub subtype_1: ShapePropertySubtype1Subtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapePropertySubtype1Subtype0 {
    #[doc = "Array of keyframes"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub k: Vec<ShapeKeyframe>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapePropertySubtype1Subtype1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k: Option<Bezier>,
}
#[doc = "Group transform"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapeTransform {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[serde(flatten)]
    pub transform: Transform,
    pub ty: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ShapeType {
    Rectangle(serde_json::Value),
    Ellipse(serde_json::Value),
    PolygonStar(serde_json::Value),
    Path(serde_json::Value),
    Fill(serde_json::Value),
    Stroke(serde_json::Value),
    GradientFill(serde_json::Value),
    GradientStroke(serde_json::Value),
    NoStyle(serde_json::Value),
    Group(serde_json::Value),
    Transform(serde_json::Value),
    RoundedCorners(serde_json::Value),
    PuckerBloat(serde_json::Value),
    Merge(serde_json::Value),
    Twist(serde_json::Value),
    OffsetPath(serde_json::Value),
    ZigZag(serde_json::Value),
    Repeater(serde_json::Value),
    Trim(serde_json::Value),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Slider {
    #[serde(flatten)]
    pub effect_value: EffectValue,
    pub ty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<Value>,
}
#[doc = "Available property overrides"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Slots {}
#[doc = "Layer with a solid color rectangle"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SolidColorLayer {
    #[serde(flatten)]
    pub visual_layer: VisualLayer,
    #[doc = "Color of the layer, unlike most other places, the color is a `#rrggbb` hex string"]
    pub sc: String,
    pub sh: f32,
    pub sw: f32,
    #[doc = "Layer type"]
    pub ty: u32,
}
#[doc = "External sound"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Sound(pub FileAsset);
impl From<FileAsset> for Sound {
    fn from(value: FileAsset) -> Self {
        Self(value)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SpherizeEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "An animatable property that is split into individually anaimated components"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SplitVector {
    pub s: bool,
    pub x: Value,
    pub y: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<Value>,
}
#[doc = "Star or Polygon"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StarType {
    Star(serde_json::Value),
    Polygon(serde_json::Value),
}
#[doc = "Solid stroke"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Stroke {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[serde(flatten)]
    pub base_stroke: BaseStroke,
    #[doc = "Stroke color"]
    pub c: ColorValue,
    pub ty: String,
}
#[doc = "An item used to described the dashe pattern in a stroked path"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StrokeDash {
    #[serde(flatten)]
    pub visual_object: VisualObject,
    #[serde(default = "defaults::stroke_dash_n")]
    pub n: StrokeDashType,
    #[doc = "Length of the dash"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<Value>,
}
#[doc = "Type of a dash item in a stroked line"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StrokeDashType {
    Dash(serde_json::Value),
    Gap(serde_json::Value),
    Offset(serde_json::Value),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StrokeEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "Stroke / frame"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StrokeStyle {
    #[serde(flatten)]
    pub layer_style: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<Value>,
    #[doc = "Layer Type"]
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextAlignmentOptions {
    #[doc = "Group alignment"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<MultiDimensional>,
    #[doc = "Anchor point grouping"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub g: Option<TextGrouping>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TextBased {
    Characters(serde_json::Value),
    CharacterExcludingSpaces(serde_json::Value),
    Words(serde_json::Value),
    Lines(serde_json::Value),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TextCaps {
    Regular(serde_json::Value),
    AllCaps(serde_json::Value),
    SmallCaps(serde_json::Value),
}
impl Default for TextCaps {
    fn default() -> Self {
        TextCaps::Regular(serde_json::from_str::<serde_json::Value>("0").unwrap())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextData {
    pub a: Vec<TextRange>,
    pub d: AnimatedTextDocument,
    pub m: TextAlignmentOptions,
    pub p: TextFollowPath,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca: Option<TextCaps>,
    pub f: String,
    pub fc: Color,
    #[serde(default = "defaults::text_document_j")]
    pub j: TextJustify,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lh: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ls: Option<f32>,
    #[doc = "Render stroke above the fill"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<bool>,
    #[doc = "Position of the box containing the text"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ps: Option<[f32; 2usize]>,
    pub s: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sc: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sw: Option<f32>,
    #[doc = "Size of the box containing the text"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sz: Option<[f32; 2usize]>,
    #[doc = "Text, note that newlines are encoded with \\r"]
    pub t: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tr: Option<f32>,
}
#[doc = "A keyframe containing a text document"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextDocumentKeyframe {
    pub s: TextDocument,
    pub t: f32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextFollowPath {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l: Option<Value>,
    #[doc = "Index of the mask to use"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TextGrouping {
    Characters(serde_json::Value),
    Word(serde_json::Value),
    Line(serde_json::Value),
    All(serde_json::Value),
}
#[doc = "Text alignment / justification"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TextJustify {
    Left(serde_json::Value),
    Right(serde_json::Value),
    Center(serde_json::Value),
    JustifyWithLastLineLeft(serde_json::Value),
    JustifyWithLastLineRight(serde_json::Value),
    JustifyWithLastLineCenter(serde_json::Value),
    JustifyWithLastLineFull(serde_json::Value),
}
#[doc = "Layer with some text"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextLayer {
    #[serde(flatten)]
    pub visual_layer: VisualLayer,
    pub t: TextData,
    #[doc = "Layer type"]
    pub ty: u32,
}
#[doc = "Range of text with custom animations and style"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<TextStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<TextRangeSelector>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextRangeSelector {
    pub a: Value,
    pub b: TextBased,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub e: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ne: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<TextRangeUnits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rn: Option<IntBoolean>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<Value>,
    pub sh: TextShape,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sm: Option<Value>,
    pub t: IntBoolean,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xe: Option<Value>,
}
#[doc = "Unit type for a text selector"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TextRangeUnits {
    Percent(serde_json::Value),
    Index(serde_json::Value),
}
#[doc = "Defines the function used to determine the interpolating factor on a text range selector."]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TextShape {
    Square(serde_json::Value),
    RampUp(serde_json::Value),
    RampDown(serde_json::Value),
    Triangle(serde_json::Value),
    Round(serde_json::Value),
    Smooth(serde_json::Value),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextStyle {
    #[serde(flatten)]
    pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bl: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fb: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fc: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fh: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fo: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fs: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ls: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sb: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sc: Option<ColorValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sh: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub so: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ss: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sw: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<Value>,
}
#[doc = "Colorizes the layer"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TintEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "Layer transform"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Transform {
    #[serde(flatten)]
    pub subtype_0: TransformSubtype0,
    #[serde(flatten)]
    pub subtype_1: TransformSubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransformSubtype0 {
    #[doc = "Anchor point: a position (relative to its parent) around which transformations are applied (ie: center for rotation / scale)"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Position>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<Value>,
    #[doc = "Scale factor, `[100, 100]` for no scaling"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<MultiDimensional>,
    #[doc = "Direction along which skew is applied, in degrees (`0` skews along the X axis, `90` along the Y axis)"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sa: Option<Value>,
    #[doc = "Skew amount as an angle in degrees"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sk: Option<Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransformSubtype1 {
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub subtype_0: Option<TransformSubtype1Subtype0>,
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub subtype_1: Option<TransformSubtype1Subtype1>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TransformSubtype1Subtype0 {
    Variant0 {
        #[doc = "Position / Translation"]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        p: Option<Position>,
    },
    Variant1 {
        #[doc = "Position / Translation with split components"]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        p: Option<SplitVector>,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TransformSubtype1Subtype1 {
    Variant0 {
        #[doc = "Rotation in degrees, clockwise"]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<Value>,
    },
    Variant1 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        or: Option<MultiDimensional>,
        #[doc = "Split rotation component"]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rx: Option<Value>,
        #[doc = "Split rotation component"]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ry: Option<Value>,
        #[doc = "Split rotation component, equivalent to `r` when not split"]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rz: Option<Value>,
    },
}
#[doc = "Trims shapes into a segment"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Trim {
    #[serde(flatten)]
    pub modifier: Modifier,
    #[doc = "Segment end"]
    pub e: Value,
    #[doc = "How to treat multiple copies"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<TrimMultipleShapes>,
    pub o: Value,
    #[doc = "Segment start"]
    pub s: Value,
    pub ty: String,
}
#[doc = "How to handle multiple shapes in trim path"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum TrimMultipleShapes {
    Individually(serde_json::Value),
    Simultaneously(serde_json::Value),
}
#[doc = "Maps layers colors based on bright/mid/dark colors"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TritoneEffect {
    #[serde(flatten)]
    pub effect: Effect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TwirlEffect {
    #[serde(flatten)]
    pub effect: Effect,
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Twist {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<MultiDimensional>,
    pub ty: String,
}
#[doc = "Layer Type"]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Type {
    PrecompositionLayer(serde_json::Value),
    SolidColorLayer(serde_json::Value),
    ImageLayer(serde_json::Value),
    NullLayer(serde_json::Value),
    ShapeLayer(serde_json::Value),
    TextLayer(serde_json::Value),
    AudioLayer(serde_json::Value),
    VideoPlaceholder(serde_json::Value),
    ImageSequence(serde_json::Value),
    VideoLayer(serde_json::Value),
    ImagePlaceholder(serde_json::Value),
    GuideLayer(serde_json::Value),
    AdjustmentLayer(serde_json::Value),
    Camera(serde_json::Value),
    LightLayer(serde_json::Value),
    DataLayer(serde_json::Value),
}
#[doc = "User-defined metadata"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserMetadata {
    #[serde(
        rename = "customProps",
        default,
        skip_serializing_if = "serde_json::Map::is_empty"
    )]
    pub custom_props: serde_json::Map<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}
#[doc = "An animatable property that holds a float"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Value {
    #[serde(flatten)]
    pub subtype_0: AnimatedProperty,
    #[serde(flatten)]
    pub subtype_1: ValueSubtype1,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ValueSubtype1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k: Option<f32>,
}
#[doc = "Layer used to affect visual elements"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VisualLayer {
    #[serde(flatten)]
    pub layer: Layer,
    #[doc = "If 1, The layer will rotate itself to match its animated position path"]
    #[serde(default = "defaults::visual_layer_ao")]
    pub ao: IntBoolean,
    #[serde(default = "defaults::visual_layer_bm")]
    pub bm: BlendMode,
    #[doc = "CSS class used by the SVG renderer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cl: Option<String>,
    #[doc = "This is deprecated in favour of `ct`"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cp: Option<bool>,
    #[doc = "Marks that transforms should be applied before masks"]
    #[serde(default = "defaults::visual_layer_ct")]
    pub ct: IntBoolean,
    #[doc = "List of layer effects"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ef: Vec<EffectsItem>,
    #[doc = "Whether the layer has masks applied"]
    #[serde(rename = "hasMask", default, skip_serializing_if = "Option::is_none")]
    pub has_mask: Option<bool>,
    #[doc = "Layer transform"]
    pub ks: Transform,
    #[doc = "`id` attribute used by the SVG renderer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ln: Option<String>,
    #[serde(
        rename = "masksProperties",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub masks_properties: Vec<Mask>,
    #[doc = "Whether motion blur is enabled for the layer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mb: Option<bool>,
    #[doc = "Styling effects for this layer"]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sy: Vec<LayerStyleItem>,
    #[doc = "If set to 1, it means a layer is using this layer as a track matte"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub td: Option<IntBoolean>,
    #[doc = "tag name used by the SVG renderer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tg: Option<String>,
    #[doc = "Index of the layer used as matte, if omitted assume the layer above the current one"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tp: Option<u32>,
    #[doc = "Defines the track matte mode for the layer"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tt: Option<MatteMode>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VisualObject {
    #[doc = "Match name, used in expressions"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mn: Option<String>,
    #[doc = "Name, as seen from editors and the like"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nm: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WavyEffect {
    #[serde(flatten)]
    pub effect: Effect,
    pub ef: Vec<serde_json::Value>,
    pub ty: u32,
}
#[doc = "Time component:\n0 means start time of the keyframe,\n1 means time of the next keyframe."]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum X {
    Variant0(Vec<f32>),
    Variant1(f32),
}
#[doc = "Value interpolation component:\n0 means start value of the keyframe,\n1 means value at the next keyframe."]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Y {
    Variant0(Vec<f32>),
    Variant1(f32),
}
#[doc = "Changes the edges of affected shapes into a series of peaks and valleys of uniform size"]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ZigZag {
    #[serde(flatten)]
    pub shape_element: ShapeElement,
    #[doc = "Point type (1 = corner, 2 = smooth)"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pt: Option<Value>,
    #[doc = "Number of ridges per segment"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<Value>,
    #[doc = "Distance between peaks and troughs"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<Value>,
    pub ty: String,
}
pub mod defaults {
    pub(super) fn animated_property_subtype0_a() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn animation_ddd() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn animation_v() -> String {
        "5.5.2".to_string()
    }
    pub(super) fn base_stroke_lc() -> super::LineCap {
        super::LineCap::Butt(serde_json::from_str::<serde_json::Value>("2").unwrap())
    }
    pub(super) fn base_stroke_lj() -> super::LineJoin {
        super::LineJoin::Miter(serde_json::from_str::<serde_json::Value>("2").unwrap())
    }
    pub(super) fn effect_en() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("1").unwrap(),
        ))
    }
    pub(super) fn file_asset_e() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn gradient_t() -> super::GradientType {
        super::GradientType::Linear(1_u32)
    }
    pub(super) fn keyframe_base_subtype0_h() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn layer_ddd() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn mask_mode() -> super::MaskMode {
        super::MaskMode::None(serde_json::from_str::<serde_json::Value>("\"i\"").unwrap())
    }
    pub(super) fn merge_mm() -> super::MergeMode {
        super::MergeMode::Normal(serde_json::from_str::<serde_json::Value>("1").unwrap())
    }
    pub(super) fn offset_path_lj() -> super::LineJoin {
        super::LineJoin::Miter(serde_json::from_str::<serde_json::Value>("2").unwrap())
    }
    pub(super) fn polystar_subtype1_sy() -> super::StarType {
        super::StarType::Star(serde_json::from_str::<serde_json::Value>("1").unwrap())
    }
    pub(super) fn position_subtype0_a() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn precomposition_xt() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn repeater_m() -> super::Composite {
        super::Composite::Above(serde_json::from_str::<serde_json::Value>("1").unwrap())
    }
    pub(super) fn shape_property_subtype0_a() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn stroke_dash_n() -> super::StrokeDashType {
        super::StrokeDashType::Dash(serde_json::from_str::<serde_json::Value>("\"d\"").unwrap())
    }
    pub(super) fn text_document_j() -> super::TextJustify {
        super::TextJustify::Left(serde_json::from_str::<serde_json::Value>("0").unwrap())
    }
    pub(super) fn visual_layer_ao() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
    pub(super) fn visual_layer_bm() -> super::BlendMode {
        super::BlendMode::Normal(serde_json::from_str::<serde_json::Value>("0").unwrap())
    }
    pub(super) fn visual_layer_ct() -> super::IntBoolean {
        super::IntBoolean(super::IntegerBoolean::True(
            serde_json::from_str::<serde_json::Value>("0").unwrap(),
        ))
    }
}
