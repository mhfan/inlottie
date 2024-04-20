
use serde::{Deserialize, Serialize};
use serde_repr::{Serialize_repr, Deserialize_repr}; // for the underlying repr of a C-like enum
use crate::helpers::{IntBool, Rgba, Vector2D, ColorList, AnyValue, AnyAsset};

//  https://lottiefiles.github.io/lottie-docs/schema/

//  Top level object, describing the animation
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Animation {
    #[serde(default = "defaults::animation_fr")]
    pub fr: f32, // Framerate in frames per second
    #[serde(default)] pub ip: f32, // In  Point, which frame the animation starts at (usually 0)
    #[serde(default = "defaults::animation_fr")]
    pub op: f32, // Out Point, which frame the animation stops/loops at,
                 // which makes this the duration in frames when ip is 0
    #[serde(default = "defaults::animation_wh")]
    pub  w: u32, // Width  of the animation
    #[serde(default = "defaults::animation_wh")]
    pub  h: u32, // Height of the animation
    #[serde(default)] pub layers: Vec<LayersItem>,

    #[serde(flatten)] pub vo: VisualObject,
    #[serde(default = "defaults::animation_v")] pub v: String,
    #[serde(default)] pub ddd: IntBool, // Whether the animation has 3D layers

    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub assets: Vec<AssetsItem>,    // List of assets that can be referenced by layers
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fonts: Option<FontList>,    // how to skip serializing on fonts.list.is_empty?
    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub chars: Vec<CharacterData>,  // Data defining text characters as lottie shapes.
    //  If present a player might only render characters defined here and nothing else.
    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub comps: Vec<Precomposition>, // List of Extra compositions not referenced by anything

    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub markers: Vec<Marker>, // Markers defining named sections of the composition.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub mb: Option<MotionBlur>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Box<Metadata>>, // Document metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Box<UserMetadata>>, // User-defined metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slots: Option<Slots>,       // Available property overrides
}

#[derive(Clone, Debug, Serialize)] #[serde(untagged)] #[allow(clippy::large_enum_variant)]
pub enum LayersItem { // Base class for layer holders
    /*  0 */Precomposition(PrecompLayer),
    /*  1 */SolidColor (SolidColorLayer),
    /*  2 */Image(ImageLayer),
    /*  4 */Shape(ShapeLayer),
    /*  6 */Audio(AudioLayer),
    /*  3 */Null(NullLayer),
    /*  5 */Text(TextLayer),

    //  7 */VideoPlaceholder,
    //  8 */ImageSequence,
    //  9 */Video,
    // 11 */Guide,
    // 12 */Adjustment,
    // 10 */ImagePlaceholder,

    /* 13 */Camera(CameraLayer),
    /* 15 */Data(DataLayer),
}

type NullLayer = VisualLayer;   // Layer with no data, useful to group layers together
type DataLayer =  ImageLayer;   // refId of the data source in assets

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageLayer { // Layer that shows an image asset
    #[serde(flatten)] pub vl: VisualLayer,
    #[serde(default, rename = "refId")] pub rid: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PrecompLayer { // Layer that renders a Precomposition asset
    #[serde(flatten)] pub vl: VisualLayer,
    #[serde(rename = "refId")] pub rid: String, // ID of the precomp as specified in the assets
    pub w: u32, //  Width of the clipping rect
    pub h: u32, // Height of the clipping rect
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tm: Option<Value>, // Time Remapping
}

use crate::helpers::{str_to_rgba, str_from_rgba, deserialize_strarray};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SolidColorLayer { // Layer with a solid color rectangle
    #[serde(flatten)] pub vl: VisualLayer,
    //  Color of the layer, unlike most other places, the color is a `#rrggbb` hex string
    #[serde(deserialize_with = "str_to_rgba", serialize_with = "str_from_rgba")]
    pub sc: Rgba,
    pub sw: f32, // Width
    pub sh: f32, // Height
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioLayer { // A layer playing sounds
    #[serde(flatten)] pub layer: Layer,
    #[serde(rename = "refId", default, skip_serializing_if = "Option::is_none")]
    pub rid: Option<String>, // ID of the sound as specified in the assets
    #[serde(default)] pub  au: Option<AudioSettings>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioSettings { pub lv: MultiDimensional } // Level

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct CameraLayer { // 3D Camera
    #[serde(flatten)] pub layer: Layer,
    pub ks: Transform, // Layer transform
    //  Distance from the Z=0 plane. Small values yield a higher perspective effect.
    pub pe: Value,     // Perspective
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VisualLayer { // Layer used to affect visual elements
    #[serde(flatten)] pub layer: Layer,
    pub ks: Transform, // Layer transform

    //#[serde(default, skip_serializing)]
    //pub cp: Option<bool>, // This is deprecated in favour of `ct`
    //  Marks that transforms should be applied before masks
    #[serde(default)] pub ct: IntBool, // Collapse Transform
    //  If 1, The layer will rotate itself to match its animated position path
    #[serde(default)] pub ao: IntBool, // Auto Orient
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mb: Option<bool>, // Whether motion blur is enabled for the layer
    #[serde(default)] pub bm: BlendMode,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "hasMask")]
    pub has_mask: Option<bool>, // Whether the layer has masks applied

    //  If set to 1, it means a layer is using this layer as a track matte
    #[serde(default, skip_serializing_if = "Option::is_none")] pub td: Option<IntBool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tt: Option<MatteMode>,  // Defines the track matte mode for the layer
    //  Index of the layer used as matte, if omitted assume the layer above the current one
    #[serde(default, skip_serializing_if = "Option::is_none")] pub tp: Option<u32>,

    #[serde(default, skip_serializing_if =   "Vec::is_empty", rename = "masksProperties")]
    pub masks_prop: Vec<Mask>,

    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub ef: Vec<Effect>, // List of layer effects
    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub sy: Vec<LayerStyleItem>, // Styling effects for this layer

    #[serde(flatten, skip_serializing_if = "Option::is_none")] extra: Option<Box<SVGProp>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct SVGProp {
    //  (tag name, `id` attribute, CSS class list) used by the SVG renderer
    #[serde(default, skip_serializing_if = "String::is_empty")] pub tg: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub ln: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub cl: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Layer {
    pub ty: u32, // Layer type
    pub st: f32, // Start Time
    pub ip: f32, //  In Point
    pub op: f32, // Out Point

    #[serde(flatten)] pub vo: VisualObject,
    #[serde(default)] pub ddd: IntBool, // Whether the layer is threedimensional
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hd: Option<bool>, // Whether the layer is hidden
    #[serde(default = "defaults::time_stretch")] pub sr: f32,  // Time Stretch

    //  Index that can be used for parenting and referenced in expressions
    #[serde(default, skip_serializing_if = "Option::is_none")] pub ind: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>, // Must be the `ind` property of another layer
    /* Within a list of layers, the ind attribute (if present) must be unique.
     * Layers having a parent attribute matching another layer will inherit their parent's
     * transform (except for opacity).  Basically you need multiply the transform matrix
     * by the parent's transform matrix to get a child layer's final transform.
     * The flat layer structure and flexible parenting allows more flexibility but it's
     * different from the more traditional approach of nesting child layers inside the
     * parent layer (like a folder structure).  One of the advantages of flexible parenting
     * is you can have children of the same layer be intermixed with unrelated layers. */
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct VisualObject {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nm: Option<String>, // Name, as seen from editors and the like
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg(feature = "expression")] pub mn: Option<String>, // Match name, used in expressions
}

//  It has the properties from Visual Object and its own properties are all animated
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Transform { // Layer transform
    //  Anchor point: a position (relative to its parent) around which
    //  transformations are applied (ie: center for rotation / scale)
    #[serde(default, skip_serializing_if = "Option::is_none")] pub  a: Option<Position>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  p: Option<PositionTranslation>,

    //  XXX: Transform for 3D layers (ddd == 1) will have a and p specified as 3D components.
    //  To make the anchor point properly line up with the center of location,
    //  p and a should have the same value.  // a, p, s are 2D Vector

    // = "default_animated(Vector2D { x: 100.0, y: 100.0 })" // = "default_animated(100.0)"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  s: Option<Animated2D>, // Scale factor, `[100, 100]` for no scaling
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  o: Option<Value>, // Opacity, 0~100 `100` for fully opaque

    //  Skew Axis, Direction along which skew is applied,
    //  in degrees (`0` skews along the X axis, `90` along the Y axis)
    #[serde(default, skip_serializing_if = "Option::is_none")] pub sa: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sk: Option<Value>, // Skew amount as an angle in degrees

    #[serde(default, skip_serializing_if = "Option::is_none", flatten)]
    pub extra: Option<Box<TransformExtra>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)] #[serde(untagged)]
pub enum PositionTranslation { Normal(Position), // Position / Translation
    Split(Box<SplitVector>), // Position / Translation with split components
}

#[derive(Clone, Debug, Deserialize, Serialize)] #[serde(untagged)]
pub enum TransformExtra {
    Rotation { // Rotation in degrees (0~360), clockwise
        #[serde(default, skip_serializing_if = "Option::is_none")]  r: Option<Value>,
    },
    Split { // XXX: Split rotation component, X/Y/Z (3D) Rotation and Orientation
        #[serde(default, skip_serializing_if = "Option::is_none")] rx: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")] ry: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rz: Option<Value>, // equivalent to `r` when not split
        #[serde(default, skip_serializing_if = "Option::is_none")]
        or: Option<Animated2D>, // Orientation, MultiDimentional
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct PositionKeyframe {
    #[serde(flatten)] pub kf: KeyframeBase<Vector2D>,

    //  Tangent for values (eg: moving position around a curved path)
    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub ti: Vec<f32>, // Value  In Tangent
    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub to: Vec<f32>, // Value Out Tangent
}

//  An animatable property that is split into individually anaimated components
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct SplitVector {
    pub x: Value, pub y: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub z: Option<Value>,
    pub s: bool, // Split, default true const
}

pub type Value = AnimatedProperty<f32, KeyframeBase<Vec<f32>>>;
pub type Position = AnimatedProperty<Vector2D, PositionKeyframe>;
type MultiDimensional = AnimatedProperty<Vec<f32>>;
pub type Animated2D = AnimatedProperty<Vector2D>;
pub type ColorValue = AnimatedProperty<Color>;

type Color = Rgba; // Vec<f32>; // Color as a [r, g, b] array with values in [0, 1]
//  Note sometimes you might find color values with 4 components
//  (the 4th being alpha) but most player ignore the last component.

//  An animatable property that holds an array of numbers
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnimatedProperty<T, K = KeyframeBase<T>> {
    #[cfg(feature = "expression")] #[serde(flatten)] expr: Option<Box<Expression>>,

    //  Whether the property is animated. Note some old animations might not have this
    #[serde(default)] pub a: Option<IntBool>,
    //#[serde(serialize_with = "crate::helpers::serialize_animated")]
    pub k: AnimatedValue<T, K>,
}

#[derive(Clone, Debug, Deserialize, Serialize)] #[serde(untagged)]
pub enum AnimatedValue<T, K> { /* a == 0 */Static(T),
    /* a == 1 */Animated(Vec<K>), /* Array of keyframes */ DebugAny(AnyValue),
}

/*  Properties can have expressions associated with them, when this is the case
    the value of such properties can be modified by using the expression.
    The expression language is based on JavaScript / ECMAScript.
    https://lottiefiles.github.io/lottie-docs/expressions/ */
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Expression {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub   x: Option<String>, // Expression
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>, // Slot ID, One of the ID in the file's slots

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  ix: Option<u32>,    // Property Index
    //  Number of components in the value arrays. If present values will
    //  be truncated or expanded to match this length when accessed from expressions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub   l: Option<u32>,   // Length, for Position and MultiDimentional
}

//  A Keyframes specifies the value at a specific time and
//  the interpolation function to reach the next keyframe.
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct KeyframeBase<T> {
    #[serde(rename = "t")] pub time: f32, // start_frame
    //  They also have a final keyframe with only the t attribute and
    //  you need to determine its value based on the s value of the previous keyframe.
    //#[serde(skip)] pub final_frame: f32, // XXX:

    //#[serde(skip, deserialize_with = "deserialize_strarray")] pub n: Vec<String>,
    //  Value at the end of the keyframe, note that this is deprecated and
    //  you should use `s` from the next keyframe to get this value.
    //#[serde(default = "defaults::default_none", skip_serializing)] pub e: Option<T>,

    //  Value at this keyframe. Note the if the property is a scalar,
    //  keyframe values are still represented as arrays.
    //  XXX: make it ArrayOne here and AnimatedValue? for compatibility concern
    #[serde(rename = "s", default = "defaults::default_none")] pub start: Option<T>,

    #[serde(rename = "h", default)] pub hold: IntBool,
    #[serde(default, skip_serializing_if = "Option::is_none")] // XXX: if h != 1 or missing
    pub i: Option<KeyframeBezierHandle>, // Easing tangent going into the next keyframe
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<KeyframeBezierHandle>, // Easing tangent leaving the current keyframe
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyframeBezierHandle { // Easing Handles, Bezier handle for keyframe interpolation
    //  Time component: 0 means time of the current keyframe,
    pub x: ArrayOne, // 1 means time of the    next keyframe.
    //  Value interpolation factor component: 0 means value at the current keyframe,
    pub y: ArrayOne,                       // 1 means value at the    next keyframe.
}

#[derive(Clone, Debug, Deserialize, Serialize)] #[serde(untagged)]
pub enum ArrayOne { Array(Vec<f32>), One(f32), } // range: 0~1

#[derive(Clone, Debug, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum BlendMode { // Layer and shape blend mode
    #[default] Normal = 0, Multiply, Screen, Overlay, Darken, Lighten,
    ColorDodge, ColorBurn, HardLight, SoftLight, Difference, Exclusion,
    Hue, Saturation, Color, Luminosity, Add, HardMix,
}

//  How a layer should mask another layer
#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum MatteMode { Normal = 0, Alpha, InvertedAlpha, Luma, InvertedLuma, }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShapeLayer { // Layer containing Shapes
    #[serde(flatten)] pub vl: VisualLayer, pub shapes: ShapeList,
}

type ShapeList = Vec<ShapeListItem>; // List of valid shapes
#[derive(Clone, Debug, Deserialize, Serialize)] #[serde(tag = "ty")] pub enum ShapeListItem {
    #[serde(rename = "rc")] Rectangle(Rectangle),
    #[serde(rename = "sr")] Polystar(Polystar),
    #[serde(rename = "el")] Ellipse(Ellipse),
    #[serde(rename = "sh")] Path(Path),

    #[serde(rename = "fl")] Fill(Fill),
    #[serde(rename = "st")] Stroke(Stroke),
    #[serde(rename = "gf")] GradientFill(GradientFill),
    #[serde(rename = "gs")] GradientStroke(GradientStroke),
    #[serde(rename = "tr")] ShapeTransform(ShapeTransform),
    #[serde(rename = "rd")] RoundedCorners(RoundedCorners),
    #[serde(rename = "pb")] PuckerBloat(PuckerBloat),
    #[serde(rename = "op")] OffsetPath(OffsetPath),
    #[serde(rename = "rp")] Repeater(Repeater),
    #[serde(rename = "gr")] Group(Group),
    #[serde(rename = "tm")] Trim(Trim),
    #[serde(rename = "tw")] Twist(Twist),
    #[serde(rename = "mm")] Merge(Merge),
    #[serde(rename = "zz")] ZigZag(ZigZag),
    #[serde(rename = "no")] NoStyle(NoStyle),
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Ellipse {
    #[serde(flatten)] pub base: ShapeBase,
    pub s: Animated2D, // Size
    pub p: Position,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Rectangle { // Simple rectangle shape
    #[serde(flatten)] pub base: ShapeBase,
    pub s: Animated2D, // Size
    pub p: Position, // Center of the rectangle
    pub r: Value,    // Rounded corners radius
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Polystar { // Star or regular polygon
    #[serde(flatten)] pub base: ShapeBase,

    pub  p: Position,
    pub pt: Value, // Points (count)
    pub or: Value, // Outer Radius
    pub os: Value, // Outer Roundness as a percentage
    pub  r: Value, // Rotation, clockwise in degrees

    #[serde(default)] pub sy: StarType, // Star type, `1` for Star, `2` for Polygon
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir: Option<Value>, // Inner Radius // XXX: if sy == 1
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is: Option<Value>, // Inner Roundness as a percentage
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Path { // Animatable Bezier curve
    #[serde(flatten)] pub base: ShapeBase,
    #[serde(rename = "ks")] pub shape: ShapeProperty, // Bezier path
}

#[derive(Clone, Debug, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum StarType { #[default] Star = 1, Polygon, } // Star or Polygon

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Fill { // Solid fill color
    #[serde(flatten)] pub elem: ShapeElement,
    pub c: ColorValue, pub o: Value, // Opacity, 100 means fully opaque
    #[serde(default, skip_serializing_if = "Option::is_none")] pub r: Option<FillRule>,
}

//  Rule used to handle multiple shapes rendered with the same fill object
#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)] #[repr(u8)] pub enum FillRule {
    NonZero = 1, // Everything is colored (You can think of this as an OR)
    EvenOdd, // Colored based on intersections and path direction, can be used to create 'holes'
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Stroke { // Solid stroke
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(flatten)] pub base: BaseStroke,
    pub c: ColorValue, // Stroke color
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct GradientFill {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(flatten)] pub gradient: Gradient, pub o: Value, // Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")] pub r: Option<FillRule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Gradient {
    pub s: Animated2D, // Start point for the gradient
    pub e: Animated2D, // End   point for the gradient
    pub g: GradientColors,   // Gradient colors

    #[serde(default)] pub t: GradientType, // Type of the gradient
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<Value>,    // Highlight Length, as a percentage between `s` and `e`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>,    // Highlight Angle, relative to the direction from `s` to `e`
}

/*  Represents colors and offsets in a gradient. Colors are represented as a flat list
    interleaving offsets and color components in weird ways. There are two possible layouts:
    Without alpha, the colors are a sequence of 'offset, r, g, b';
    With alpha, same as above but at the end of the list there is a sequence of 'offset, alpha'.
 */
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct GradientColors {
    #[serde(rename = "p")] pub count: u32, // Number of colors in `k`
    pub k: AnimatedProperty<ColorList>, // MultiDimensional,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct GradientStroke {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(flatten)] pub base: BaseStroke,
    #[serde(flatten)] pub gradient: Gradient,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct BaseStroke {
    pub o: Value, // Opacity, 100 means fully opaque
    pub w: Value, // Stroke width

    #[serde(default)] pub lc: LineCap,
    #[serde(default)] pub lj: LineJoin,
    #[serde(default)] pub ml: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ml2: Option<Value>, // Animatable alternative to ml (Miter Limit)
    #[serde(default, skip_serializing_if =   "Vec::is_empty")]
    pub d: Vec<StrokeDash>, // Dashed line definition
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StrokeDash { // An item used to described the dashe pattern in a stroked path
    #[serde(flatten)] pub vo: VisualObject,
    #[serde(default)] pub n: StrokeDashType, // Type of a dash item in a stroked line
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "v")]
    pub length: Option<Value>, // Length of the dash
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)] pub enum StrokeDashType {
    #[serde(rename = "d")] #[default] Dash,
    #[serde(rename = "g")] Gap,
    #[serde(rename = "o")] Offset,
}

//  Style at the end of a stoked line
#[derive(Clone, Debug, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum LineCap  {  Butt = 1, #[default] Round, Square, }

//  Style at a sharp corner of a stoked line
#[derive(Clone, Debug, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum LineJoin { Miter = 1, #[default] Round,  Bevel, }

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct ShapeBase { // Drawable shape
    #[serde(flatten)] pub elem: ShapeElement,
    //  Direction the shape is drawn as, mostly relevant when using trim path
    #[serde(default, skip_serializing_if = "Option::is_none")] pub d: Option<ShapeDirection>,
}

type NoStyle = ShapeElement; // Represents a style for shapes without fill or stroke

//  Base class for all elements of ShapeLayer and Group
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct ShapeElement {
    //pub ty: String, // Shape type, handled via enum tag
    #[serde(flatten)] pub vo: VisualObject,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub bm: Option<BlendMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hd: Option<bool>,   // Whether the shape is hidden
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ix: Option<u32>,    // Index used in expressions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ln: Option<String>, // `id` attribute used by the SVG renderer
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cl: Option<String>, // CSS class list used by the SVG renderer
}

//  Drawing direction of the shape curve, useful for trim path
#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)] #[repr(u8)] pub enum ShapeDirection {
    /* Usually clockwise */Normal   = 1, /* counter clockwise */Reversed = 3,
    Unknown2 = 2, Unknown0 = 0, // XXX: issue_1732.json, precomposition.json
}

type ShapeProperty = AnimatedProperty<Bezier, KeyframeBase<Vec<Bezier>>>; // ShapeKeyframe

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Bezier { // Single bezier curve
    #[serde(default)] pub c: bool, // Closed
    //  All are array of points, each point is an array of coordinates.
    pub v: Vec<Vector2D>, // These points are along the bezier path.
    //  These points are along the `in`  tangents relative to the corresponding `v`.
    pub i: Vec<Vector2D>, // In  Tangent
    //  These points are along the `out` tangents relative to the corresponding `v`.
    pub o: Vec<Vector2D>, // Out Tangent // v, i, o are array of 2D Vector
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct ShapeTransform { // Group transform
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(flatten)] pub transform: Transform,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct RoundedCorners {
    #[serde(flatten)] pub elem: ShapeElement,
    pub r: Value, // Rounded // Rounds corners of other shapes
}

//  Interpolates the shape with its center point and bezier tangents with the opposite direction
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct PuckerBloat {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>, // Amount as a percentage
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct OffsetPath { //
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(default)] pub lj: LineJoin,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub  a: Option<Value>, // Amount
    #[serde(default, skip_serializing_if = "Option::is_none")] pub ml: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Repeater { // Duplicates previous shapes in a group
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(rename = "c")] pub copies: Value, // Number of copies
    #[serde(rename = "m", default)] pub stacking: Composite, // Stacking order
    #[serde(rename = "o", default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<Value>,
    pub tr: RepeaterTransform, // Transform applied to each copy
}

//  How to stack copies in a repeater
#[derive(Clone, Debug, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum Composite { #[default] Above = 1, Below, }

//  Transform used by a repeater, the transform is applied to each subsequent repeated object.
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct RepeaterTransform {
    #[serde(flatten)] pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub so: Option<Value>, // Opacity of the first repeated object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eo: Option<Value>, // Opacity of the last repeated object.
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Group { // Shape Element that can contain other shapes
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cix: Option<u32>, // Index used in expressions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  np: Option<f32>, // Number Of Properties
    #[serde(default, skip_serializing_if =   "Vec::is_empty")] pub it: ShapeList, // Shapes
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Trim { // Trims shapes into a segment
    #[serde(flatten)] pub elem: ShapeElement,
    pub s: Value, // Segment start
    pub e: Value, // Segment end
    pub o: Value, // Offset

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<TrimMultipleShapes>, // Multiple
}

//  How to handle multiple shapes in trim path
#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TrimMultipleShapes { Individually = 1, Simultaneously, }

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Twist {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>, // Angle
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<Animated2D>, // Center
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Merge {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(default)] pub mm: MergeMode, // Boolean operator on shapes
}

#[derive(Clone, Debug, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum MergeMode { #[default] Normal = 1,
    Add, Subtract, Intersect, ExcludeIntersections,
}

//  Changes the edges of affected shapes into a series of peaks and valleys of uniform size
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct ZigZag {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  r: Option<Value>, // Frequency, Number of ridges per segment
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  s: Option<Value>, // Amplitude, Distance between peaks and troughs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pt: Option<Value>, // Point type (1 = corner, 2 = smooth)
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct TextLayer { // Layer with some text
    #[serde(flatten)] pub vl: VisualLayer, pub t: TextData,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextData { // Contains all the text data and animation
    #[serde(rename = "a")] pub ranges: Vec<TextRange>,
    #[serde(rename = "d")] pub    doc: AnimatedTextDocument,
    #[serde(rename = "m")] pub  align: TextAlignmentOptions,
    #[serde(rename = "p")] pub follow: TextFollowPath,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TextRange { // Range of text with custom animations and style
    #[serde(default, skip_serializing_if = "Option::is_none")] pub nm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "s")]
    pub select: Option<TextRangeSelector>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "a")]
    pub  style: Option<TextStyle>,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct TextStyle {
    #[serde(flatten)] pub transform: Transform,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "t")]
    pub spacing: Option<Value>, // Letter Spacing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bl: Option<Value>, // Blur
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ls: Option<Value>, // Line Spacing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sw: Option<Value>, // Stroke Width

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sc: Option<ColorValue>, // Stroke Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sh: Option<Value>, // Stroke Hue
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ss: Option<Value>, // Stroke Saturation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sb: Option<Value>, // Stroke Brightness
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub so: Option<Value>, // Stroke Opacity

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fc: Option<ColorValue>, // Fill Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fh: Option<Value>, // Fill Hue
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fs: Option<Value>, // Fill Saturation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fb: Option<Value>, // Fill Brightness
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fo: Option<Value>, // Fill Opacity
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct TextRangeSelector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rn: Option<IntBool>, // Randomize

    #[serde(rename = "t")] pub expressible: IntBool, // Expressible
    #[serde(rename = "a")] pub  max_amount: Value,   // Max Amount
    #[serde(rename = "b")] pub based: TextBased,
    pub sh: TextShape,

    #[serde(default, skip_serializing_if = "Option::is_none")] pub o: Option<Value>, // Offset
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  r: Option<TextRangeUnits>, // Range Units
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sm: Option<Value>, // Selector Smoothness
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xe: Option<Value>, // Max Ease
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ne: Option<Value>, // Min Ease
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  s: Option<Value>, // Start
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  e: Option<Value>, // End
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextRangeUnits { Percent = 1, Index, } // Unit type for a text selector

//  Animated property representing the text contents
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct AnimatedTextDocument {
    pub k: Vec<TextDocumentKeyframe>, // A keyframe containing a text document
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>, // Expression
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct TextDocumentKeyframe {
    pub s: TextDocument, // Start
    pub t: f32, // Time
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct TextDocument {
    pub  t: String, // Text, note that newlines are encoded with \r
    pub  f: String, // Font Family
    pub fc: Color,  // Fill Color
    #[serde(default = "defaults::font_size")] pub  s: f32,    // Font Size

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sc: Option<Color>,  // Stroke Color
    #[serde(default)] pub sw: f32, // Stroke Width
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<bool>,   // Render stroke above the fill
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lh: Option<f32>,    // Line Height
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sz: Option<Vector2D>, // Size of the box containing the text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ps: Option<Vector2D>, // Position of the box containing the text
    #[serde(default)] pub j: TextJustify,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca: Option<TextCaps>, // TextCaps
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tr: Option<f32>,    // Tracking
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ls: Option<f32>,    // Baseline Shift
}

#[derive(Clone, Debug, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextJustify { // Text alignment / justification
    #[default] Left = 0, Right, Center, JustifyWithLastLineLeft,
    JustifyWithLastLineRight, JustifyWithLastLineCenter, JustifyWithLastLineFull,
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextCaps { Regular = 0, AllCaps, SmallCaps, }

//  Used to change the origin point for transformations,
//  such as Rotation, that may be applied to the text string.
//  The origin point for each character, word, or line can be changed.
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct TextAlignmentOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<MultiDimensional>,    // Group alignment // XXX:
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub g: Option<TextGrouping>,        // Anchor point grouping
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextGrouping { Characters = 1, Word, Line, All, }

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct TextFollowPath {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<u32>,   // Mask, Index of the mask to use
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f: Option<Value>, // First Margin
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l: Option<Value>, // Last  Margin
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<Value>, // Reverse Path
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<Value>, // Force Alignment
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p: Option<Value>, // Perpendicular To Path
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextBased { Characters = 1, CharacterExcludingSpaces, Words, Lines, }

//  Defines the function used to determine the interpolating factor on a text range selector.
#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextShape { Square = 1, RampUp, RampDown, Triangle, Round, Smooth, }

#[derive(Clone, Debug, Deserialize, Serialize)] #[serde(untagged)]
pub enum AssetsItem { Image(Image), Sound(Sound), DataSource(DataSource),
    Precomposition(Precomposition), DebugAny(AnyAsset),
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Image { // External image
    #[serde(flatten)] pub file: FileAsset,
    #[serde(default)] pub w: f32, //  Width of the image
    #[serde(default)] pub h: f32, // Height of the image
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<String>, // Marks as part of an image sequence if present, default "seq" const
}

//  External data source, usually a JSON file
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct DataSource {
    #[serde(flatten)] pub file: FileAsset, pub t: u32, // default 3 const
}

type Sound = FileAsset; // External sound
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct FileAsset {
    #[serde(rename = "p")] pub url: String, // Filename or data url
    #[serde(flatten)] pub asset: AssetBase,
    #[serde(rename = "u", default)] pub path: String, // Path to the directory containing a file
    #[serde(rename = "e", default)] pub embedded: IntBool, // Whether the file is embedded
}

//  Asset containing an animation that can be referenced by (precomp) layers.
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Precomposition {
    #[serde(flatten)] pub asset: AssetBase,
    pub layers: Vec<LayersItem>,
    #[serde(default = "defaults::animation_fr")] pub fr: f32,
    #[serde(default, rename = "xt")] pub extra: IntBool, // Extra composition
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct AssetBase {
    pub id: String, // Unique identifier used by layers when referencing this asset
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nm: Option<String>, // Human readable name
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct FontList { pub list: Vec<Font>, }

//  Describes how a font with given settings should be loaded
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Font {
    //  Name used by text documents to reference this font,
    //  usually it's `fFamily` followed by `fStyle`
    #[serde(rename = "fName")]   pub   name: String, // default "sans-Regular"
    #[serde(rename = "fFamily")] pub family: String, // default "sans"
    #[serde(rename = "fStyle")]  pub  style: String, // default "Regular"

    #[serde(rename = "fClass",  default, skip_serializing_if = "Option::is_none")]
    pub  class: Option<String>, // CSS Class applied to text objects using this font
    #[serde(rename = "fPath",   default, skip_serializing_if = "Option::is_none")]
    pub   path: Option<String>,
    #[serde(rename = "fWeight", default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    #[serde(default)] pub ascent: f32, // Text will be moved down based on this value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<FontPathOrigin>,
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum FontPathOrigin { Local = 0, CssUrl, ScriptUrl, FontUrl, }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterData { // Defines character shapes
    #[serde(rename = "fFamily")] pub family: String,
    pub style: String,
    pub    ch: String,
    pub  data: ShapePrecomp,
    #[serde(rename = "w")] pub width: f32,
    pub  size: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)] #[serde(untagged)]
pub enum ShapePrecomp { Shapes(CharacterShapes), Precomp(Box<CharacterPrecomp>), }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterShapes { // Shapes forming the character
    #[serde(default, skip_serializing_if =   "Vec::is_empty")] pub shapes: ShapeList,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterPrecomp { // Defines a character as a precomp layer
    #[serde(rename = "refId")] pub rid: String, // ID of the precomp as specified in the assets
    #[serde(default, skip_serializing_if = "Option::is_none")] pub ks: Option<Transform>,
    #[serde(default)] pub ip: f32, // Frame when the layer becomes visible
    #[serde(default = "defaults::precomp_op")]
    pub op: f32, // Out Point when the layer becomes invisible
    #[serde(default = "defaults::time_stretch")] pub sr: f32, // Time Stretch
    #[serde(default)] pub st: f32, // Start Time
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Marker { // Defines named portions of the composition.
    #[serde(default, skip_serializing_if = "Option::is_none")] pub cm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub tm: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub dr: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct MotionBlur { // Motion blur settings
    #[serde(default)] pub  sa: f32, // Shutter Angle in degrees
    #[serde(default)] pub  sp: f32, // Shutter Phase in degrees
    #[serde(default)] pub spf: f32, // Samples per Frame
    #[serde(default)] pub asl: f32, // Adaptive Sample Limit
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Metadata {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "a")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "d")]
    pub   desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  tc: Option<String>, // Theme Color
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "g")]
    pub gen: Option<String>, // Software used to generate the file
    #[serde(default, skip_serializing_if =   "Vec::is_empty", rename = "k",
        deserialize_with = "deserialize_strarray")] pub keywords: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct UserMetadata {
    #[serde(rename = "customProps", default)] pub cprops: serde_json::Value, // XXX:
    #[serde(default, skip_serializing_if = "Option::is_none")] pub filename: Option<String>,
}

type Slots = serde_json::Value; // XXX: Available property overrides
/* #[derive(Clone, Debug, Deserialize, Serialize)] pub struct Slots {
    // patternProperties: any of MultiDimensional/ColorValue/Position/ShapeProperty/Value
} */

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct Effect { // Layer effect
    pub ef: Vec<EffectValuesItem>,
    pub ty: EffectType,
    #[serde(default = "defaults::effect_en")] pub en: IntBool, // Enabled

    #[serde(flatten)] pub vo: VisualObject,
    #[serde(default)] pub np: u32, // Property Count, Number of values in `ef`
    #[serde(default)] pub ix: u32, // Effect Index
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)] #[repr(u8)] pub enum EffectType {
    /*  5 */Custom = 5, // Some lottie files use `ty` = 5 for many different effects
    /* 20 */Tint = 20,  // Colorizes the layer
    /* 21 */Fill,       // Replaces the whole layer with the given color
    /* 22 */Stroke,
    /* 23 */Tritone,    // Maps layers colors based on bright/mid/dark colors
    /* 24 */ProLevels,
    /* 25 */DropShadow, // Adds a shadow to the layer
    /* 26 */RadialWipe,
    /* 27 */DisplacementMap,
    /* 28 */Matte3,     // Uses a layer as a mask
    /* 29 */GaussianBlur,
    /* 30 */Twirl,
    /* 31 */MeshWarp,
    /* 32 */Wavy,
    /* 33 */Spherize,
    /* 34 */Puppet,
}

#[derive(Clone, Debug, Serialize)] #[serde(untagged)] pub enum EffectValuesItem {
    /*  0 */Slider(EffectValue<Value>),
    /*  1 */Angle (EffectValue<Value>),
    /*  3 */Point (EffectValue<Animated2D>),
    /*  2 */EffectColor(EffectValue<ColorValue>),
    /*  4 */Checkbox(EffectValue<Value>),
    //  5 */CustomEffect(Effect),
    /*  6 */Ignored (EffectValue<f32>),
    /*  7 */DropDown(EffectValue<Value>),
    NoValue, // What use/purpose?
    /* 10 */EffectLayer(EffectValue<Value>),
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct EffectValue<T> {
    pub ty: u32, // Effect (value) type
    #[serde(default)] pub ix: u32, // Effect Index
    #[serde(default = "defaults::default_none",
        skip_serializing_if = "Option::is_none")] pub v: Option<T>,
    #[serde(flatten)] pub vo: VisualObject,
}

#[derive(Clone, Debug, Serialize)] #[serde(untagged)] pub enum LayerStyleItem {
    /* 2 */InnerShadow(InnerShadowStyle),
    /* 1 */DropShadow  (DropShadowStyle),
    /* 3 */OuterGlow(OuterGlowStyle),
    /* 4 */InnerGlow(InnerGlowStyle),
    /* 0 */Stroke(StrokeStyle),
    /* 6 */Satin  (SatinStyle),
    /* 5 */BevelEmboss  (BevelEmbossStyle),
    /* 7 */ColorOverlay(ColorOverlayStyle),
    /* 8 */GradientOverlay(GradientOverlayStyle),
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct StrokeStyle { // Stroke / frame
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<ColorValue>,  // Color
    #[serde(default, skip_serializing_if = "Option::is_none")] pub s: Option<Value>, // Size
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct DropShadowStyle {
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  c: Option<ColorValue>, // Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  o: Option<Value>,  // Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  a: Option<Value>,  // Angle, Local light angle
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  s: Option<Value>,  // Blur size
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  d: Option<Value>,  // Distance
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<Value>,  // Choke Spread
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,  // Blend Mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,  // Noise

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lc: Option<Value>,  // Layer Conceal, Layer knowck out drop shadow
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct InnerShadowStyle {
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  c: Option<ColorValue>, // Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  o: Option<Value>,  // Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  a: Option<Value>,  // Angle, Local light angle
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  s: Option<Value>,  // Blur size
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  d: Option<Value>,  // Distance
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<Value>,  // Choke Spread
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,  // Blend Mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,  // Noise
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct OuterGlowStyle {
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  c: Option<ColorValue>, // Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  o: Option<Value>,  // Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  r: Option<Value>,  // Range
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  j: Option<Value>,  // Jitter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<Value>,  // Choke Spread
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,  // Blend Mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,  // Noise
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct InnerGlowStyle {
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  c: Option<ColorValue>, // Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  o: Option<Value>,  // Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  r: Option<Value>,  // Range
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  j: Option<Value>,  // Jitter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ch: Option<Value>,  // Choke Spread
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,  // Blend Mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<Value>,  // Noise

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sr: Option<Value>,  // Source
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct BevelEmbossStyle {
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  a: Option<Value>,  // Angle, Local light angle
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  s: Option<Value>,  // size
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bs: Option<Value>,  // Bevel Style
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bt: Option<Value>,  // Technique
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sr: Option<Value>,  // Strength
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sf: Option<Value>,  // Soften
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ga: Option<Value>,  // Global Angle, Use global light
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ll: Option<Value>,  // Altitude, Local lighting altitude
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hm: Option<Value>,  // Highlight Mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ho: Option<Value>,  // Highlight Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sm: Option<Value>,  // Shadow Mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub so: Option<Value>,  // Shadow Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sc: Option<ColorValue>, // Shadow Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hc: Option<ColorValue>, // Highlight Color
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct SatinStyle {
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  c: Option<ColorValue>, // Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  o: Option<Value>,  // Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  a: Option<Value>,  // Angle, Local light angle
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  s: Option<Value>,  // size
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  d: Option<Value>,  // Distance

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,  // Blend Mode
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "in")]
    pub invert: Option<Value>,  // Invert
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct ColorOverlayStyle {
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  c: Option<ColorValue>, // Color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub so: Option<Value>,  // Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,  // Blend Mode
}

#[derive(Clone, Debug, Deserialize, Serialize)] pub struct GradientOverlayStyle {
    #[serde(flatten)] pub ls: LayerStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  o: Option<Value>,  // Opacity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  a: Option<Value>,  // Angle, Local light angle
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub  s: Option<Value>,  // size
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bm: Option<Value>,  // Blend Mode
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub re: Option<Value>,  // Reverse
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub al: Option<Value>,  // Align with layer
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<Value>,  // Offset
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gs: Option<Value>,  // Smoothness
    #[serde(default, skip_serializing_if = "Option::is_none")] pub gf: Option<GradientColors>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub gt: Option<GradientType>,
}

#[derive(Clone, Debug, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum GradientType { #[default] Linear = 1, Radial, } // Type of a gradient

//type LayerStyle = VisualObject; // Style applied to a layer
#[derive(Clone, Debug, Deserialize, Serialize)] pub struct LayerStyle {
    pub ty: u32, // Style type
    #[serde(flatten)] pub vo: VisualObject,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Mask { // Bezier shape used to mask/clip a layer
    #[serde(flatten)] pub vo: VisualObject,
    #[serde(default)] pub inv: bool, // Inverted
    #[serde(default)] pub mode: MaskMode,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "pt")]
    pub shape_prop: Option<ShapeProperty>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename =  "o")]
    pub opacity: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename =  "x")]
    pub  expand: Option<Value>,
}

//  How masks interact with each other. See
//  https://helpx.adobe.com/after-effects/using/alpha-channels-masks-mattes.html
#[derive(Clone, Debug, Deserialize, Serialize, Default)] pub enum MaskMode {
    #[serde(rename = "n")] None,
    #[serde(rename = "a")] Add,
    #[serde(rename = "s")] Subtract,
    #[serde(rename = "i")] #[default] Intersect,
    #[serde(rename = "l")] Lighten,
    #[serde(rename = "d")] Darken,
    #[serde(rename = "f")] Difference,
}

pub(crate) mod defaults {
    pub(super) fn animation_wh() -> u32 { 512 }
    pub(super) fn animation_fr() -> f32 { 60.0 }
    pub(super) fn animation_v() -> String { "5.5.2".to_owned() }
    pub(super) fn effect_en() -> super::IntBool { true.into() }
    pub(super) fn default_none<T>() -> Option<T> { None }
    pub(super) fn time_stretch() -> f32 { 1.0 }
    pub(super) fn precomp_op() -> f32 { 99999.0 }
    pub(super) fn font_size()  -> f32 { 10.0 }
    //pub(super) fn font_family() -> String { "sans".to_owned() }
    //pub(super) fn font_name()   -> String { "sans-Regular".to_owned() }
    //pub(super) fn font_style()  -> String { "Regular".to_owned() }
}
