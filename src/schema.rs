
use serde::{Deserialize, Serialize};
use serde_repr::{Serialize_repr, Deserialize_repr}; // for the underlying repr of a C-like enum
use crate::helpers::{IntBool, Rgba, Vector2D, ColorList, AnyValue, AnyAsset, defaults};

/// Top level object, describing the animation.
/// https://lottiefiles.github.io/lottie-docs/schema/
#[derive(Deserialize, Serialize)] pub struct Animation {
    #[serde(skip)] pub elapsed: f32,    // for rendering
    #[serde(skip)] pub fnth: f32,

    #[serde(default = "defaults::animation_fr")] /** Framerate in FPS */ pub fr: f32,
    /// Whether the animation has 3D layers.
    /// Lottie doesn't actually support 3D stuff so this should always be `0`.
    #[serde(default)] pub ddd: IntBool,

    /// In  Point, which frame the animation starts at (usually `0`)
    #[serde(default)] pub ip: f32,
    /// Out Point, which frame the animation stops/loops at,
    /// which makes this the duration in frames when ip is `0`
    #[serde(default = "defaults::animation_fr")] pub op: f32,
    #[serde(default = "defaults::animation_wh")] pub  w: u32, // Width  of the animation
    #[serde(default = "defaults::animation_wh")] pub  h: u32, // Height of the animation

    #[serde(default)] pub layers: Vec<LayerItem>,
    /// List of assets that can be referenced by layers
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub assets: Vec<AssetItem>,

    #[serde(default, skip_serializing_if = "FontList::is_empty")] pub fonts: FontList,
    /// Data defining text characters as lottie shapes.
    /// If present a player might only render characters defined here and nothing else.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub chars: Vec<CharacterData>,

    //#[serde(flatten)] pub vo: VisualObject,
    //#[serde(default = "defaults::animation_vs")] /** Bodymovin/Lottie version */ pub v: String,

    /*/ List of Extra compositions not referenced by anything
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub comps: Vec<Precomp>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Document metadata
    pub meta: Option<Box<Metadata>>,
    #[serde(skip_serializing_if = "Option::is_none")] /// User-defined metadata
    pub metadata: Option<Box<UserMetadata>>, */

    /// Markers defining named sections of the composition.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub markers: Vec<Marker>,
    #[serde(skip_serializing_if = "Option::is_none")] pub mb: Option<MotionBlur>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Available property overrides
    #[cfg(feature = "expression")] pub slots: Option<Box<Slots>>,
}

#[derive(Serialize)] #[serde(untagged)] #[allow(clippy::large_enum_variant)]
/** Base class for layer holders */ pub enum LayerItem {
    /*  0 */PrecompLayer(PrecompLayer),
    /*  1 */SolidColor(SolidLayer),
    /*  2 */Image(ImageLayer),
    /*  4 */Shape(ShapeLayer),
    /*  6 */Audio(AudioLayer),
    /*  3 */Null(NullLayer),
    /*  5 */Text(Box<TextLayer>),

    //  7 */VideoPlaceholder,
    //  8 */ImageSequence,
    //  9 */Video,
    // 11 */Guide,
    // 12 */Adjustment,
    // 10 */ImagePlaceholder,

    /* 13 */Camera(CameraLayer),
    /* 15 */Data(DataLayer),
}

/// Layer with no data, useful to group layers together.
/// It's often used by animators as a parent to multiple other layers (see parenting).
type NullLayer = VisualLayer;
type DataLayer =  ImageLayer;   // refId of the data source in assets

/// Layer that shows an image asset
#[derive(Deserialize, Serialize)] pub struct ImageLayer {
    #[serde(flatten)] pub vl: VisualLayer,
    #[serde(rename = "refId")] pub rid: String,
}

/// Layer that renders a Precomposition asset
#[derive(Deserialize, Serialize)] pub struct PrecompLayer {
    #[serde(flatten)] pub vl: VisualLayer,
    /// ID of the precomp as specified in the assets
    #[serde(rename = "refId")] pub rid: String,
    /**  Width of the clipping rect */ pub w: u32,
    /** Height of the clipping rect */ pub h: u32,

    /// Time Remapping, The `tm` property maps the time in seconds of the precomposition
    /// to show. Basically you get the value of `tm` at the current frame, then assume
    /// that's a time in seconds since the start of the animation, and render
    /// the corresponding frame of the precomposition.
    #[serde(skip_serializing_if = "Option::is_none")] pub tm: Option<Value>,
}

use crate::helpers::{str_to_rgba, str_from_rgba, deserialize_strarray};

/// Layer with a solid color rectangle
#[derive(Deserialize, Serialize)] pub struct SolidLayer {
    #[serde(flatten)] pub vl: VisualLayer,
    /// Color of the layer, unlike most other places, the color is a `#rrggbb` hex string
    #[serde(deserialize_with = "str_to_rgba", serialize_with = "str_from_rgba")] pub sc: Rgba,
    pub sw: f32, // Width
    pub sh: f32, // Height
}

#[derive(Deserialize, Serialize)] /** A layer playing sounds */ pub struct AudioLayer {
    #[serde(flatten)] pub base: LayerInfo,
    /// ID of the sound as specified in the assets
    #[serde(rename = "refId")] pub rid: String,  // a workaround for issues missing `au`
    #[serde(skip_serializing_if = "Option::is_none")] pub au: Option<AudioSettings>,
}

#[derive(Deserialize, Serialize)] pub struct AudioSettings { pub lv: MultiD } // Level

#[derive(Deserialize, Serialize)] pub struct CameraLayer { // 3D Camera
    #[serde(flatten)] pub base: LayerInfo,
    pub ks: Transform, // Layer transform
    /// Distance from the (Z=0) plane. Small values yield a higher perspective effect.
    pub pe: Value,     // Perspective
}

/// Layer used to affect visual elements
#[derive(Deserialize, Serialize)] pub struct VisualLayer {
    #[serde(flatten)] pub base: LayerInfo,
    pub ks: Transform, // Layer transform

    //#[serde(skip_serializing)] pub cp: Option<bool>, // This is deprecated in favour of `ct`
    /// Collapse Transform, Marks that transforms should be applied before masks
    #[serde(default, skip_serializing_if = "defaults::is_default")] pub ct: IntBool,
    /// Auto Orient, When `true`, if the transform position is animated,
    /// it rotates the layer along the path the position follows.
    #[serde(default, skip_serializing_if = "defaults::is_default")] pub ao: IntBool,
    /// Whether motion blur is enabled for the layer
    #[serde(default, skip_serializing_if = "defaults::is_default")] pub mb: bool,
    #[serde(default, skip_serializing_if = "defaults::is_default", rename = "hasMask")]
    /** Whether the layer has masks applied */ pub has_mask: bool,
    #[serde(default, skip_serializing_if = "defaults::is_default")] pub bm: BlendMode,

    /// Defines the track matte mode for the layer.
    /// The way it works is the layer defining the mask has a `tt` attribute with the
    /// appropriate value. By defaults it affects the layer on top (the layer before it
    /// in the layer list, which has the `td` attribute), otherwise check the `tp` attribute.
    #[serde(skip_serializing_if = "Option::is_none")] pub tt: Option<MatteMode>,
    /// If set to `1`, it means a layer is using this layer as a track matte
    #[serde(skip_serializing_if = "Option::is_none")] pub td: Option<IntBool>,
    /// Index of the layer used as matte, if omitted assume the layer above the current one
    #[serde(skip_serializing_if = "Option::is_none")] pub tp: Option<u32>,

    /// A layer can have an array of masks that clip the contents of the layer to a shape.
    /// This is similar to mattes, but there are a few differences. With mattes, you use a
    /// layer to define the clipping area, while with masks you use an animated bezier curve.
    #[serde(default, skip_serializing_if = "Vec::is_empty",
        rename = "masksProperties")] pub masks_prop: Vec<Mask>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /** List of layer effects */ pub ef: Vec<Effect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /** Styling effects for this layer */ pub sy: Vec<LayerStyleItem>,

    #[serde(flatten, skip_serializing_if = "Option::is_none")] extra: Option<Box<SVGProp>>,
}

/// (tag name, `id` attribute, CSS class list) used by the SVG renderer
#[derive(Deserialize, Serialize)] pub struct SVGProp {
    #[serde(default, skip_serializing_if = "String::is_empty")] pub tg: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub ln: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub cl: String,
}

#[derive(Deserialize, Serialize)] pub struct LayerInfo {
    //* Layer type */ pub ty: u32
    /** Start Time */ pub st: f32, // XXX: what is `st` used for?
    /**  In Point  */ pub ip: f32,
    /** Out Point  */ pub op: f32,

    //#[serde(flatten)] pub vo: VisualObject,
    #[serde(default)] pub ddd: IntBool, // Whether the layer is 3D threedimensional
    #[serde(default, skip_serializing_if = "defaults::is_default")]
    /** Whether the layer is hidden */ pub hd: bool,
    /** Time Stretch */ #[serde(default = "defaults::time_stretch")] pub sr: f32,
    // XXX: how to limit to serialize with not default value?

    /// Index that can be used for parenting and referenced in expressions.
    /// Within a list of layers, the ind attribute (if present) must be unique.
    #[serde(skip_serializing_if = "Option::is_none")] pub ind: Option<u32>,
    /// Must be the `ind` property of another layer.
    /** Layers having a parent attribute matching another layer will inherit their parent's
        transform (except for opacity).  Basically you need multiply the transform matrix
        by the parent's transform matrix to get a child layer's final transform.
        The flat layer structure and flexible parenting allows more flexibility but it's
        different from the more traditional approach of nesting child layers inside the
        parent layer (like a folder structure).  One of the advantages of flexible parenting
        is you can have children of the same layer be intermixed with unrelated layers. */
    #[serde(skip_serializing_if = "Option::is_none")] pub parent: Option<u32>,
}

#[derive(Deserialize, Serialize)] pub struct VisualObject {
    //#[serde(default, skip_serializing_if = "String::is_empty")]
    //* Name, as seen from editors and the like */ pub nm: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg(feature = "expression")] /** Match name, used in expressions */ pub mn: String,
}

/// It has the properties from Visual Object and its own properties are all animated.
/// `a`, `p`, `s` are 2D Vector. // XXX: Transform for 3D layers (`ddd` == `1`)
/// will have `a` and `p` specified as 3D components. To make the anchor point
/// properly line up with the center of location, `p` and `a` should have the same value.
#[derive(Deserialize, Serialize)] pub struct Transform { // Layer transform
    /// Anchor point: a position (relative to its parent) around which
    /// transformations are applied (ie: center for rotation / scale)
    #[serde(skip_serializing_if = "Option::is_none", rename = "a")]
    pub anchor: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "p")]
    pub position: Option<Translation>,  // XXX: translate = position - anchor?

    // default = "defaults::animated2d", default = "defaults::opacity"
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")]
    /** Scale factor, `[100, 100]` for no scaling */ pub scale: Option<Animated2D>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "o")]
    /** Opacity, 0~100, `100` for fully opaque */  pub opacity: Option<Value>,

    /// Skew Axis, in degrees (`0` skews along the X axis, `90` along the Y axis)
    #[serde(skip_serializing_if = "Option::is_none", rename = "sa")]
    /** Direction along which skew is applied */ pub skew_axis: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "sk")]
    /** Skew amount as an angle in degrees    */ pub skew: Option<Value>,

    #[serde(flatten)] pub extra: TransRotation,
}

#[derive(Deserialize, Serialize)] #[serde(untagged)]
pub enum Translation { /** Position / Translation */ Normal(Position),
    /** Position / Translation with split components */ Split(Box<SplitVector>),
}

#[derive(Deserialize, Serialize)] #[serde(untagged)]
pub enum TransRotation {    Split3D(Box<SplitRotation>),
    Normal2D { #[serde(skip_serializing_if = "Option::is_none", rename = "r")]
        /** Rotation in degrees (0~360), clockwise */ rotation: Option<Value>,
    },  // should not be all optional in each variant, otherwise the first variant will be hit
}

/// XXX: Split rotation component, X/Y/Z (3D) Rotation and Orientation
#[derive(Deserialize, Serialize)] pub struct SplitRotation {
    #[serde(skip_serializing_if = "Option::is_none")] rx: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] ry: Option<Value>,
    /** equivalent to `r` when not split */ rz: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    /** Orientation, MultiDimentional    */ or: Option<MultiD>,
}

/// (In/Out) tangent for values (eg: moving position around a curved path)
#[derive(Deserialize, Serialize)] pub struct PositionExtra {
    //#[serde(flatten)] pub kf: KeyframeBase<Vector2D>, // PositionKeyframe

    pub ti: Vector2D, //Vec<f32>,
    pub to: Vector2D, //Vec<f32>,
}

/// An animatable property that is split into individually anaimated components
#[derive(Deserialize, Serialize)] pub struct SplitVector {
    pub x: Value, pub y: Value,
    #[serde(skip_serializing_if = "Option::is_none")]  pub z: Option<Value>,
    #[serde(rename = "s")] /** default `true` const */ pub split: bool,
}

pub type Value = AnimatedProperty<f32>;
pub type Position   = AnimatedProperty<Vector2D>;
pub type Animated2D = AnimatedProperty<Vector2D>;
pub type ColorValue = AnimatedProperty<Color>;
pub type MultiD = AnimatedProperty<Vec<f32>>;

/// Color as a [r, g, b] array with values in 0~1
/// Note sometimes you might find color values with 4 components
/// (the 4th being alpha) but most player ignore the last component.
type Color = Rgba; // Vec<f32>;

/// An animatable property that holds an array of numbers
#[derive(Deserialize, Serialize)] pub struct AnimatedProperty<T> {
    //#[serde(serialize_with = "crate::helpers::serialize_animated")]
    #[serde(rename = "k")] pub keyframes: AnimatedValue<T>,
    /// Whether the property is animated. Note some old animations might not have this
    #[serde(rename = "a", default)] pub animated: IntBool,

    #[cfg(feature = "expression")] #[serde(flatten)] expr: Option<Box<Expression>>,
}

#[derive(Deserialize, Serialize)] #[serde(untagged)]
pub enum AnimatedValue<T> { /**  `a` == `0` */ Static(T),
    /** Array of keyframes, when `a` == `1` */ Animated(Vec<KeyframeBase<T>>),
    /** Any unexpected value, used for debugging only */ DebugAny(Box<AnyValue>),
}

/** Properties can have expressions associated with them, when this is the case
    the value of such properties can be modified by using the expression.
    The expression language is based on JavaScript / ECMAScript.
    https://lottiefiles.github.io/lottie-docs/expressions/ */
#[derive(Deserialize, Serialize)] pub struct Expression { // XXX:
    #[serde(default, skip_serializing_if = "String::is_empty")] /** Expression */ pub x: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /** Slot ID, One of the ID in the file's slots */ pub sid: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    /** Property Index */ pub ix: Option<u32>,
    /// Number of components in the value arrays. If present values will be
    /// truncated or expanded to match this length when accessed from expressions.
    #[serde(skip_serializing_if = "Option::is_none", rename = "l")]
    /** Length, for Position and MultiDimentional */ pub len: Option<u32>,
}

/// A Keyframes specifies the value at a specific time and
/// the interpolation function to reach the next keyframe.
#[derive(Deserialize, Serialize)] pub struct KeyframeBase<T> {
    /// Keyframe time (in frames). They also have a final keyframe with only the `t` attribute
    /// and you need to determine its value based on the `s` value of the previous keyframe.
    #[serde(rename = "t")] pub start: f32,

    /// If `h` == `1`, you don't need `i` and `o`, as the property will
    /// keep the same value until the next keyframe.
    #[serde(rename = "h", default, skip_serializing_if = "defaults::is_default")]
    pub hold: IntBool,
    //#[serde(skip, deserialize_with = "deserialize_strarray")] pub n: Vec<String>,
    //  Value at the end of the keyframe, note that this is deprecated and
    //  you should use `s` from the next keyframe to get this value.
    //#[serde(skip_serializing)] pub e: Option<ArrayScalar<T>>,

    /// Value at this keyframe. Note the if the property is a scalar,
    /// keyframe values are still represented as arrays.
    #[serde(rename = "s")] pub value: Option<ArrayScalar<T>>, // a workaround for old file

    #[serde(flatten)] pub easing: Option<Box<EasingHandle>>,
    #[serde(flatten)] pub pextra: Option<PositionExtra>,
}

#[derive(Deserialize, Serialize)] pub struct EasingHandle {
    /// Easing tangent leaving the current keyframe
    #[serde(rename = "o")] pub to: BezierHandle,
    /// Easing tangent going into the next keyframe
    #[serde(rename = "i")] pub ti: BezierHandle,
}

/// Easing (Bezier) handles for keyframe interpolation
#[derive(Deserialize, Serialize)] pub struct BezierHandle {
    /// Time component: `0` means time of the current keyframe,
    /// `1` for the next keyframe. (range: 0~1)
    #[serde(rename = "x")] pub   time: ArrayScalar<f32>,
    /// Value interpolation factor component:
    /// `0` means value at the current keyframe, `1` for the next keyframe.
    #[serde(rename = "y")] pub factor: ArrayScalar<f32>,
}

#[derive(Deserialize, Serialize)] #[serde(untagged)]
pub enum ArrayScalar<T> { Array(Vec<T>), Scalar(T), }

#[derive(Clone, Copy, Default, PartialEq, Deserialize_repr, Serialize_repr)]
/** Layer and shape blend mode */ #[repr(u8)] pub enum BlendMode {
    #[default] Normal = 0, Multiply, Screen, Overlay, Darken, Lighten,
    ColorDodge, ColorBurn, HardLight, SoftLight, Difference, Exclusion,
    Hue, Saturation, Color, Luminosity, Add, HardMix,
}

/// How a layer should mask another layer
#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum MatteMode { Normal = 0, Alpha, InvertedAlpha, Luma, InvertedLuma, }

#[derive(Deserialize, Serialize)] /// Layer containing Shapes
pub struct ShapeLayer { #[serde(flatten)] pub vl: VisualLayer, pub shapes: ShapeList, }

type ShapeList = Vec<ShapeItem>; // List of valid shapes
/// These **shapes** only define path data, to actually show something,
/// they must be followed by some style shape.
/// Each **style** is applied to all preceding shapes in the same group / layer.
/// **Modifiers** process their siblings and alter the path defined by shapes.
#[derive(Deserialize, Serialize)] #[serde(tag = "ty")] pub enum ShapeItem {
    #[serde(rename = "rc")] Rectangle(Rectangle),           // Shapes:
    #[serde(rename = "sr")] Polystar(Box<Polystar>),
    #[serde(rename = "el")] Ellipse(Ellipse),
    #[serde(rename = "sh")] Path(FreePath),

    #[serde(rename = "no")] NoStyle(NoStyle),               // Styles:
    #[serde(rename = "fl")] Fill(FillStrokeGrad),
    #[serde(rename = "st")] Stroke(FillStrokeGrad),
    #[serde(rename = "gf")] GradientFill(FillStrokeGrad),
    #[serde(rename = "gs")] GradientStroke(FillStrokeGrad),

    /** A group is a shape that can contain other shapes (including other groups).
        The usual contents of a group are: `Shapes`, `Styles`, `Transform`.
        While the contents may vary, a group must always end with a `Transform` shape. */
    #[serde(rename = "gr")] Group(Group),
    #[serde(rename = "rp")] Repeater(Box<Repeater>),

    #[serde(rename = "rd")] RoundedCorners(RoundedCorners), // Modifiers:
    #[serde(rename = "pb")] PuckerBloat(PuckerBloat),
    #[serde(rename = "op")] OffsetPath(OffsetPath),
    #[serde(rename = "tm")] Trim(TrimPath),
    #[serde(rename = "tw")] Twist(Twist),
    #[serde(rename = "mm")] Merge(Merge),
    #[serde(rename = "zz")] ZigZag(ZigZag),

    #[serde(rename = "tr")] Transform(Box<TransformShape>),
}

#[derive(Deserialize, Serialize)] pub struct Ellipse {
    #[serde(flatten)] pub base: ShapeBase,
    #[serde(rename = "s")] pub size: Animated2D,
    #[serde(rename = "p")] pub pos: Position,
}

#[derive(Deserialize, Serialize)] pub struct Rectangle {
    #[serde(flatten)] pub base: ShapeBase,
    #[serde(rename = "s")] pub size: Animated2D,
    /** Center of the rectangle */ #[serde(rename = "p")] pub pos: Position,
    /** Rounded corners radius  */ #[serde(rename = "r")] pub rcr: Value,
}

#[derive(Deserialize, Serialize)] /** Star or regular polygon */ pub struct Polystar {
    #[serde(flatten)] pub base: ShapeBase,

    #[serde(rename = "p")] pub pos: Position,
    /** Points (count) */  pub  pt: Value,
    /** Outer Radius   */  pub  or: Value,
    /** Outer Roundness as a percentage */ pub os: Value,
    /** Rotation, clockwise in degrees  */ #[serde(rename = "r")] pub rotation: Value,

    /** Star type, `1` for Star, `2` for Polygon */ #[serde(default)] pub sy: StarType,
    /// Inner Radius, for Star only
    #[serde(skip_serializing_if = "Option::is_none")] pub ir: Option<Value>,
    /// Inner Roundness as a percentage, for Star only
    #[serde(skip_serializing_if = "Option::is_none")] pub is: Option<Value>,
}

#[derive(Deserialize, Serialize)] /// Animatable Bezier curve
pub struct FreePath {  #[serde(flatten)]        pub  base: ShapeBase,
    /** Bezier path */ #[serde(rename = "ks")]  pub shape: ShapeProperty,
}

#[derive(Clone, Copy, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum StarType { #[default] Star = 1, Polygon, }

/// Rule used to handle multiple shapes rendered with the same fill object
#[derive(Clone, Copy, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum FillRule {
    /** Everything is colored (You can think of this as an OR) */ #[default] NonZero = 1,
    /// Colored based on intersections and path direction, can be used to create 'holes'
    EvenOdd,
}

#[derive(Deserialize, Serialize)] pub struct Gradient {
    /** Start point for the gradient */ #[serde(rename = "s")] pub sp: Animated2D,
    /** End   point for the gradient */ #[serde(rename = "e")] pub ep: Animated2D,
    #[serde(rename = "g")] pub stops: GradientColors,

    #[serde(rename = "t", default)] pub r#type: GradientType,
    /// Highlight Length, as a percentage between `s` and `e`.
    /// If it's a radial gradient, `s` refers to the center of the gradient,
    /// and the style object may have these additional properties: `h` and `a`.
    /// Basically the radial highlight position is defined in polar coordinates relative to `s`.
    #[serde(rename = "h", skip_serializing_if = "Option::is_none")] pub hl: Option<Value>,
    /// Highlight Angle, relative to the direction from `s` to `e`
    #[serde(rename = "a", skip_serializing_if = "Option::is_none")] pub ha: Option<Value>,
}

/** Represents colors and offsets in a gradient. Colors are represented as a flat list
    interleaving offsets and color components in weird ways. There are two possible layouts:
    Without alpha, the colors are a sequence of 'offset, r, g, b'; With alpha,
    same as above but at the end of the list there is a sequence of 'offset, alpha'. */
#[derive(Deserialize, Serialize)] pub struct GradientColors {
    /** Number of colors in `k` */ #[serde(rename = "p")] pub cnt: u32,
    #[serde(rename = "k")] pub cl: AnimatedProperty<ColorList>, // MultiD
}

#[derive(Deserialize, Serialize)] pub struct FillStrokeGrad {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(flatten)] pub base: FillStrokeEnum,
    #[serde(flatten)] pub grad: ColorGradEnum,
    #[serde(rename = "o")] pub opacity: Value,
}

#[derive(Deserialize, Serialize)] #[serde(untagged)]
pub enum FillStrokeEnum { Stroke(Box<BaseStroke>), FillRule(FillRuleWrapper), }

#[derive(Deserialize, Serialize)] #[serde(untagged)]
pub enum ColorGradEnum { Color(ColorWrapper), Gradient(Box<Gradient>), }

#[derive(Deserialize, Serialize)] pub struct ColorWrapper {
    #[serde(rename = "c")] pub color: ColorValue
}

#[derive(Clone, Copy, Deserialize, Serialize)] pub struct FillRuleWrapper {
    #[serde(rename = "r", default)] pub rule: FillRule,
}

#[derive(Deserialize, Serialize)] pub struct BaseStroke {
    #[serde(rename = "w")] pub width: Value,    // `opacity` was moved out to unify struct

    #[serde(default)] pub lj: LineJoin,
    #[serde(default)] pub lc: LineCap,
    #[serde(default)] pub ml: f32,  /// Animatable alternative to `ml` (Miter Limit)
    #[serde(skip_serializing_if = "Option::is_none")] pub ml2: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "d")]
    pub dash: Vec<StrokeDash>, // Dashed line definition
}

/// An item used to described the dash pattern in a stroked path
#[derive(Deserialize, Serialize)] pub struct StrokeDash {
    //#[serde(flatten)] pub vo: VisualObject,     // Type of a dash item in a stroked line
    #[serde(rename = "n")] pub r#type: StrokeDashType,  /// Length of the dash
    #[serde(rename = "v")] pub  value: Value,
}

#[derive(Clone, Copy, Deserialize, Serialize, Default)] pub enum StrokeDashType {
    #[serde(rename = "d")] #[default] Length,
    #[serde(rename = "o")] Offset,
    #[serde(rename = "g")] Gap,
}

/// Style at the end of a stoked line
#[derive(Clone, Copy, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum LineCap  {  Butt = 1, #[default] Round, Square, }

/// Style at a sharp corner of a stoked line
#[derive(Clone, Copy, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum LineJoin { Miter = 1, #[default] Round,  Bevel, }

#[derive(Deserialize, Serialize)] pub struct ShapeBase { // Drawable shape
    #[serde(flatten)] pub elem: ShapeElement,
    /// Direction the shape is drawn as, mostly relevant when using trim path
    #[serde(skip_serializing_if = "Option::is_none", rename = "d")]
    pub dir: Option<ShapeDirection>,
}

/** Represents a style for shapes without fill or stroke */ type NoStyle = ShapeElement;

/// Base class for all elements of ShapeLayer and Group
#[derive(Deserialize, Serialize)] pub struct ShapeElement {
    //pub ty: String, // Shape type, handled via enum tag
    //#[serde(flatten)] pub vo: VisualObject,
    #[serde(default, skip_serializing_if = "defaults::is_default")] pub bm: Option<BlendMode>,
    #[serde(default, skip_serializing_if = "defaults::is_default")]
    /** Whether the shape is hidden */ pub hd: bool,

    #[serde(skip_serializing_if = "Option::is_none")] /// Index used in expressions
    #[cfg(feature = "expression")] pub ix: Option<u32>,
    //#[serde(default, skip_serializing_if = "String::is_empty")]
    //* `id` attribute used by the SVG renderer */ pub ln: String,
    //#[serde(default, skip_serializing_if = "String::is_empty")]
    //* CSS class list used by the SVG renderer */ pub cl: String,
}

/// Drawing direction of the shape curve, useful for trim path
#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum ShapeDirection {
    /** Usually clockwise */Normal = 1, /** Counter clockwise */Reversed = 3,
    Unknown2 = 2, Unknown0 = 0, // workaround with issue_1732.json, precomposition.json
}

pub type ShapeProperty = AnimatedProperty<Bezier>; // ShapeKeyframe

/// All fields are array of points (2D Vector), each point is an array of coordinates.
/// The `nth` bezier segment is defined as: `v[n], v[n]+o[n], v[n+1]+i[n+1], v[n+1]`
/// If the bezier is closed, you need an extra segment going from the last point to the first,
/// still following `i` and `o` appropriately. If you want linear bezier, you can have `i` and
/// `o` for a segment to be `[0, 0]`. If you want it quadratic, set them to 2/3rd of what the
/// quadratic control point would be. If you want a point to be smooth you need to make sure 
/// that `i = -o`. "A Primer on Bézier Curves": https://pomax.github.io/bezierinfo/
#[derive(Clone, Deserialize, Serialize)] /** Single bezier curve */ pub struct Bezier {
    #[serde(rename = "c", default)] pub closed: bool,
    /// These points are along the bezier path.
    #[serde(rename = "v")] pub vp: Vec<Vector2D>,
    /// These points are along the `in`  tangents relative to the corresponding `v`.
    #[serde(rename = "i")] pub it: Vec<Vector2D>, // Cubic control points, incoming tangent
    /// These points are along the `out` tangents relative to the corresponding `v`.
    #[serde(rename = "o")] pub ot: Vec<Vector2D>, // Cubic control points, outgoing tangent
}

#[derive(Deserialize, Serialize)] /** Group transform */ pub struct TransformShape {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(flatten)] pub trfm: Transform,
}

#[derive(Deserialize, Serialize)] pub struct RoundedCorners {
    #[serde(flatten)] pub elem: ShapeElement, /// Rounds corners of other shapes
    #[serde(rename = "r")] pub radius: Value,
}

/// Interpolates bezier vertices towards the center of the shape,
/// and tangent handles away from it (or vice-versa).
#[derive(Deserialize, Serialize)] pub struct PuckerBloat {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(skip_serializing_if = "Option::is_none", rename = "a")] /// Amount as a percentage
    pub amount: Option<Value>,
}

/// Interpolates the shape with its center point and bezier tangents with the opposite direction
#[derive(Deserialize, Serialize)] pub struct OffsetPath {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(default)] pub lj: LineJoin,
    #[serde(skip_serializing_if = "Option::is_none", rename = "a")] pub amount: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Miter Limit*/ pub ml: Option<Value>,
}

/// Duplicates previous shapes in a group
#[derive(Deserialize, Serialize)] pub struct Repeater {
    #[serde(flatten)] pub elem: ShapeElement,
    /** Number of copies */ #[serde(rename = "c")] pub cnt: Value,
    /** Stacking order   */ #[serde(rename = "m", default)] pub order: Composite,
    #[serde(rename = "o", skip_serializing_if = "Option::is_none")] pub offset: Option<Value>,
    /** Transform applied to each copy */ pub tr: RepeaterTransform,
}

/// How to stack copies in a repeater
#[derive(Clone, Copy, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum Composite { #[default] Above = 1, Below, }

/// Transform used by a repeater, the transform is applied to each subsequent repeated object.
#[derive(Deserialize, Serialize)] pub struct RepeaterTransform {
    #[serde(flatten)] pub trfm: Transform,
    /// Opacity of the first repeated object.
    #[serde(skip_serializing_if = "Option::is_none")] pub so: Option<Value>,
    /// Opacity of the  last repeated object.
    #[serde(skip_serializing_if = "Option::is_none")] pub eo: Option<Value>,
}

/// Shape Element that can contain other shapes
#[derive(Deserialize, Serialize)] pub struct Group {
    #[serde(flatten)] pub elem: ShapeElement,   /// Index used in expressions
    #[serde(skip_serializing_if = "Option::is_none", rename = "cix")]
    #[cfg(feature = "expression")] pub ix: Option<u32>,     /// Number Of Properties
    #[serde(skip_serializing_if = "Option::is_none")] pub np: Option<f32>,
    #[serde(skip_serializing_if =   "Vec::is_empty", rename =  "it")] pub shapes: ShapeList,
}

/// This is mostly useful for shapes with a stroke and not a fill.
/// It takes the path defined by shapes and only shows a segment of the resulting bezier
/// data. Also has the attributes from Modifier.
#[derive(Deserialize, Serialize)]
/** Trims shapes into a segment */ pub struct TrimPath {
    #[serde(flatten)] pub elem: ShapeElement,
    /** Segment start */ #[serde(rename = "s")] pub start: Value,
    /** Segment   end */ #[serde(rename = "e")] pub   end: Value,
    #[serde(rename = "o")] pub offset: Value,   /// How to treat multiple copies
    #[serde(rename = "m", skip_serializing_if = "Option::is_none")]
    pub multiple: Option<TrimMultiple>,
}

/// How to handle multiple shapes in trim path
#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TrimMultiple { Simultaneously = 1, Individually, }

#[derive(Deserialize, Serialize)] pub struct Twist {
    #[serde(flatten)] pub elem: ShapeElement,
    #[serde(rename = "a", skip_serializing_if = "Option::is_none")] pub angle: Option<Value>,
    #[serde(rename = "c", skip_serializing_if = "Option::is_none")]
    pub center: Option<Animated2D>,
}

#[derive(Deserialize, Serialize)] pub struct Merge {
    #[serde(flatten)] pub elem: ShapeElement,   /// Boolean operator on shapes
    #[serde(default)] pub mm: MergeMode,
}

#[derive(Clone, Copy, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum MergeMode { #[default] Normal = 1,
    Add, Subtract, Intersect, ExcludeIntersect,
}

/// Changes the edges of affected shapes into a series of peaks and valleys of uniform size
#[derive(Deserialize, Serialize)] pub struct ZigZag {
    #[serde(flatten)] pub elem: ShapeElement,   /// Frequency, Number of ridges per segment
    #[serde(skip_serializing_if = "Option::is_none", rename = "r")] pub freq: Option<Value>,
    /// Amplitude, Distance between peaks and troughs
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")] pub ampl: Option<Value>,
    /// Point type (`1` = corner, `2` = smooth)
    #[serde(skip_serializing_if = "Option::is_none")] pub   pt: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct TextLayer { // Layer with some text
    #[serde(flatten)] pub vl: VisualLayer, pub t: TextData,
}

/// Contains all the text data and animation
#[derive(Deserialize, Serialize)] pub struct TextData {
    #[serde(rename = "a")] pub ranges: Vec<TextRange>,
    #[serde(rename = "d")] pub    doc: AnimatedTextDocument,
    #[serde(rename = "m")] pub  align: TextAlignmentOptions,
    #[serde(rename = "p")] pub follow: TextFollowPath,
}

/// Range of text with custom animations and style
#[derive(Deserialize, Serialize)] pub struct TextRange {
    //#[serde(default, skip_serializing_if = "String::is_empty")] /** Name */ pub nm: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")]
    pub select: Option<TextRangeSelector>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "a")]
    pub  style: Option<TextStyle>,
}

#[derive(Deserialize, Serialize)] pub struct TextStyle {
    #[serde(flatten)] pub trfm: Transform,            /// Letter Spacing
    #[serde(skip_serializing_if = "Option::is_none", rename = "t")] pub spacing: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Blur
    pub bl: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Line Spacing
    pub ls: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Stroke Width
    pub sw: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")] /// Stroke Color
    pub sc: Option<ColorValue>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Stroke Hue
    pub sh: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Stroke Saturation
    pub ss: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Stroke Brightness
    pub sb: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Stroke Opacity
    pub so: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")] /// Fill Color
    pub fc: Option<ColorValue>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Fill Hue
    pub fh: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Fill Saturation
    pub fs: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Fill Brightness
    pub fb: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Fill Opacity
    pub fo: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct TextRangeSelector {
    #[serde(skip_serializing_if = "Option::is_none")] /// Randomize
    pub rn: Option<IntBool>,

    #[serde(rename = "t")] pub expressible: IntBool,
    #[serde(rename = "a")] pub  max_amount: Value,
    #[serde(rename = "b")] pub based: TextBased,
    pub sh: TextShape,

    #[serde(skip_serializing_if = "Option::is_none", rename = "o")] pub offset: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "r")] /// Range Units
    pub unit: Option<TextRangeUnits>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Selector Smoothness
    pub sm: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Max Ease */  pub xe: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Min Ease */  pub ne: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")] pub start: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "e")] pub   end: Option<Value>,
}

#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)] /// Unit type for a text selector
#[repr(u8)] pub enum TextRangeUnits { Percent = 1, Index, }

/// Animated property representing the text contents
#[derive(Deserialize, Serialize)] pub struct AnimatedTextDocument {
    /** A keyframe containing a text document */ pub k: Vec<TextDocumentKeyframe>,
    #[serde(default, skip_serializing_if = "String::is_empty")] /// Expression
    #[cfg(feature = "expression")] pub x: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] /// Slot ID
    #[cfg(feature = "expression")] pub sid: String,
}

#[derive(Deserialize, Serialize)] pub struct TextDocumentKeyframe {
    #[serde(rename = "s")] pub value: TextDocument,
    #[serde(rename = "t")] pub start: f32, // Time
}

#[derive(Deserialize, Serialize)] pub struct TextDocument {
    /// Text, note that newlines are encoded with \r
    #[serde(rename = "t")] pub ts: String,
    #[serde(rename = "f")] /** Font Family */ pub ff: String,
    /** Fill Color  */ pub fc: Color,
    #[serde(default = "defaults::font_size", rename = "s")] /** Font Size */ pub fs: f32,

    #[serde(skip_serializing_if = "Option::is_none")] /// Stroke Color
    pub sc: Option<Color>,
    /** Stroke Width */ #[serde(default)] pub sw: f32,
    #[serde(default, rename = "j")] pub justify: TextJustify,
    /// Render stroke above the fill
    #[serde(skip_serializing_if = "Option::is_none")] pub of: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Line Height */ pub lh: Option<f32>,
    /// Wrap size of the box containing the text
    #[serde(skip_serializing_if = "Option::is_none")] pub sz: Option<Vector2D>,
    /// Wrap position of the box containing the text
    #[serde(skip_serializing_if = "Option::is_none")] pub ps: Option<Vector2D>,
    #[serde(skip_serializing_if = "Option::is_none")] pub ca: Option<TextCaps>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Tracking */ pub tr: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Baseline Shift
    pub ls: Option<f32>,
}

#[derive(Clone, Copy, Default, Deserialize_repr, Serialize_repr)]
/** Text alignment / justification */ #[repr(u8)] pub enum TextJustify {
    #[default] Left = 0, Right, Center, JustifyWithLastLineLeft,
    JustifyWithLastLineRight, JustifyWithLastLineCenter, JustifyWithLastLineFull,
}

#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextCaps { Regular = 0, AllCaps, SmallCaps, }

/// Used to change the origin point for transformations,
/// such as Rotation, that may be applied to the text string.
/// The origin point for each character, word, or line can be changed.
#[derive(Deserialize, Serialize)] pub struct TextAlignmentOptions {
    #[serde(skip_serializing_if = "Option::is_none", rename = "a")] /// Group alignment
    pub align: Option<MultiD>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "g")] /// Anchor point grouping
    pub group: Option<TextGrouping>,
}

#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextGrouping { Characters = 1, Word, Line, All, }

#[derive(Deserialize, Serialize)] pub struct TextFollowPath {
    /// Mask, Index of the mask to use
    #[serde(skip_serializing_if = "Option::is_none", rename = "m")] pub mask: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "f")] /// First Margin
    pub fm: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "l")] /// Last  Margin
    pub lm: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "r")] /// Reverse Path
    pub reverse: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "a")] /// Force Alignment
    pub   align: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "p")] /// Perpendicular To Path
    pub    perp: Option<Value>,
}

#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextBased { Characters = 1, ExcludingSpaces, Words, Lines, }

/// Defines the function used to determine the interpolating factor on a text range selector.
#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum TextShape { Square = 1, RampUp, RampDown, Triangle, Round, Smooth, }

#[derive(Deserialize, Serialize)] #[serde(untagged)]
pub enum AssetItem { Image(Image), Sound(Sound), DataSource(DataSource),
    Precomp(Precomp), DebugAny(AnyAsset),
}

#[derive(Deserialize, Serialize)] pub struct Image { // External image
    #[serde(flatten)] pub file: FileAsset,
    #[serde(default)] pub w: f32, //  Width of the image
    #[serde(default)] pub h: f32, // Height of the image
    /// Marks as part of an image sequence if present, default "seq" const
    #[serde(skip_serializing_if = "Option::is_none", rename = "t")]
    pub seq: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")] /// Slot ID
    #[cfg(feature = "expression")] pub sid: String,
}

/// External data source, usually a JSON file
#[derive(Deserialize, Serialize)] pub struct DataSource {
    #[serde(flatten)] pub file: FileAsset,
    #[serde(rename = "t")] /** default `3` const */ pub r#type: u32,
}

type Sound = FileAsset; // External sound
#[derive(Deserialize, Serialize)] pub struct FileAsset {
    #[serde(rename = "p")] /** Filename or data url */ pub url: String,
    #[serde(flatten)] pub base: AssetBase,
    /// Path to the directory containing a file
    #[serde(rename = "u", default)] pub path: String,
    #[serde(rename = "e", default)] pub embedded: IntBool, // Whether the file is embedded
}

/// Asset containing an animation that can be referenced by (precomp) layers.
/// You can think of precompositions as self-contained animation within the main animation
/// file that can be referenced using precomp layers. Within a precomposition you can have
/// precomp layers showing other precompositions, as long as you don't create a dependency
/// cycle. https://lottiefiles.github.io/lottie-docs/breakdown/precomps/
#[derive(Deserialize, Serialize)] pub struct Precomp {
    #[serde(flatten)] pub base: AssetBase,
    pub layers: Vec<LayerItem>,
    #[serde(default = "defaults::animation_fr")] pub fr: f32,
    #[serde(default, rename = "xt")] /** Extra composition */ pub extra: IntBool,
}

#[derive(Deserialize, Serialize)] pub struct AssetBase {
    /** Unique identifier used by layers when referencing this asset */ pub id: String,
    //#[serde(default, skip_serializing_if = "String::is_empty")] /** Name */ pub nm: String,
}

#[derive(Default, Deserialize, Serialize)] pub struct FontList { pub list: Vec<Font>, }

/// Describes how a font with given settings should be loaded
#[derive(Deserialize, Serialize)] pub struct Font {
    /// Name used by text documents to reference this font,
    /// usually it's `fFamily` followed by `fStyle`
    #[serde(rename = "fName")]   pub   name: String, // default "sans-Regular"
    #[serde(rename = "fFamily")] pub family: String, // default "sans"
    #[serde(rename = "fStyle")]  pub  style: String, // default "Regular"

    /// CSS Class applied to text objects using this font
    #[serde(default, skip_serializing_if = "String::is_empty",
        rename = "fClass")]  pub  class: String,
    #[serde(default, skip_serializing_if = "String::is_empty",
        rename = "fPath")]   pub   path: String,
    #[serde(default, skip_serializing_if = "String::is_empty",
        rename = "fWeight")] pub weight: String,
    #[serde(default)] /** Text will be moved down based on this value */ pub ascent: f32,
    #[serde(skip_serializing_if = "Option::is_none")] pub origin: Option<FontPathOrigin>,
}

#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum FontPathOrigin { Local = 0, CssUrl, ScriptUrl, FontUrl, }

#[derive(Deserialize, Serialize)] /// Defines character shapes
pub struct CharacterData {
    #[serde(rename = "fFamily")] pub family: String,
    pub style: String,
    pub    ch: String,
    pub  data: ShapePrecomp,
    #[serde(rename = "w")] pub width: f32,
    pub  size: f32,
}

#[derive(Deserialize, Serialize)] #[serde(untagged)]
pub enum ShapePrecomp { Shapes(CharacterShapes), Precomp(Box<CharacterPrecomp>), }

/// Shapes forming the character
#[derive(Deserialize, Serialize)] pub struct CharacterShapes {
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub shapes: ShapeList,
}

/// Defines a character as a precomp layer
#[derive(Deserialize, Serialize)] pub struct CharacterPrecomp {
    /// ID of the precomp as specified in the assets
    #[serde(rename = "refId")] pub rid: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub ks: Option<Transform>,
    #[serde(default)] /** Frame when the layer becomes visible */ pub ip: f32,
    /// Out Point when the layer becomes invisible
    #[serde(default = "defaults::precomp_op")] pub op: f32,
    #[serde(default = "defaults::time_stretch")] /** Time Stretch */ pub sr: f32,
    #[serde(default)] /** Start Time */ pub st: f32,
}

/// Defines named portions of the composition.
#[derive(Deserialize, Serialize)] pub struct Marker {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /** Comment  */ pub cm: String,
    /** Time     */ #[serde(default)] pub tm: f32,
    /** Duration */ #[serde(default)] pub dr: f32,
}

#[derive(Deserialize, Serialize)] pub struct MotionBlur { // Motion blur settings
    /** Shutter Angle in degrees */ #[serde(default)] pub  sa: f32,
    /** Shutter Phase in degrees */ #[serde(default)] pub  sp: f32,
    /** Samples per Frame        */ #[serde(default)] pub spf: f32,
    /** Adaptive Sample Limit    */ #[serde(default)] pub asl: f32,
}

#[derive(Deserialize, Serialize)] pub struct Metadata {
    #[serde(default, skip_serializing_if = "String::is_empty")] pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty", rename = "d")] pub   desc: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /** Theme Color */ pub tc: String,
    #[serde(default, skip_serializing_if = "String::is_empty", rename = "g")]
    /** Software used to generate the file */ pub gen: String,
    #[serde(default, skip_serializing_if =    "Vec::is_empty", rename = "k",
        deserialize_with = "deserialize_strarray")] pub keywords: Vec<String>,
}

#[derive(Deserialize, Serialize)] pub struct UserMetadata {
    #[serde(default, rename = "customProps")] pub cprops: serde_json::Value, // XXX:
    #[serde(default, skip_serializing_if = "String::is_empty")] pub filename: String,
}

#[allow(unused)] type Slots = serde_json::Value; // XXX: Available property overrides
/* #[derive(Deserialize, Serialize)] pub struct Slots {
    // patternProperties: any of MultiD/ColorValue/Position/ShapeProperty/Value
} */

/// Layers can have post-processing effects applied to them.
/// Many effects have unused values which are labeled with a number.
#[derive(Deserialize, Serialize)] pub struct Effect { // Layer effect
    pub ef: Vec<EffectValueItem>,
    pub ty: EffectType,
    #[serde(default = "defaults::effect_en")] /** Enabled */ pub en: IntBool,

    //#[serde(flatten)] pub vo: VisualObject,
    /** Property Count, Number of values in `ef` */ #[serde(default)] pub np: u32,
    /** Effect Index */ #[serde(default)] pub ix: u32,
}

#[derive(Clone, Copy, Deserialize_repr, Serialize_repr)] #[repr(u8)] pub enum EffectType {
    /// Some lottie files use `ty` = 5 for many different effects.
    /// Sometimes these are used together with expressions.
    /*  5 */Custom = 5,
    /// Colorizes the layer. The layer is converted to grayscale,
    /// then black to white is mapped to the given color.
    /// The result is merged back with the original based on the intensity.
    /* 20 */Tint = 20,
    /// Replaces the whole layer with the given color,
    /// fill all opaque areas with a solid color
    /* 21 */Fill,
    /* 22 */Stroke,
    /// Converts the layer to greyscale, then maps layers colors
    ///   (applies the gradient) based on bright/mid/dark colors
    /* 23 */Tritone,
    /// Color correction levels. For more information refer to the After Effects Documentation.
    /// https://helpx.adobe.com/after-effects/using/color-correction-effects.html#levels_effect
    /* 24 */ProLevels,
    /// Adds a shadow to the layer
    /* 25 */DropShadow,
    /* 26 */RadialWipe,
    /* 27 */DisplacementMap,
    /// Uses a layer as a mask
    /* 28 */Matte3,
    /* 29 */GaussianBlur,
    /* 30 */Twirl,
    /* 31 */MeshWarp,
    /* 32 */Wavy,
    /* 33 */Spherize,
    /* 34 */Puppet,
    // Bulge, WaveWarp, ?
}

#[derive(Serialize)] #[serde(untagged)] pub enum EffectValueItem {
    /*  0 */Slider(EffectValue<Value>),
    /*  1 */Angle (EffectValue<Value>),
    /*  3 */Point (EffectValue<Animated2D>),
    /*  2 */EffectColor(EffectValue<ColorValue>),
    /*  4 */Checkbox(EffectValue<Value>),
    //  5 */CustomEffect(EffectValue<Value>),
    /*  6 */Ignored (EffectValue<f32>),
    /*  7 */DropDown(EffectValue<Value>),
    NoValue, // What is its usage/purpose?
    /* 10 */EffectLayer(EffectValue<Value>),
}

#[derive(Deserialize, Serialize)] pub struct EffectValue<T> {
    //* Effect (value) type */ pub ty: u32,
    /** Effect Index */ pub ix: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "v")] pub value: Option<T>,
    //#[serde(flatten)] pub vo: VisualObject,
}

#[derive(Serialize)] #[serde(untagged)] pub enum LayerStyleItem {
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

#[derive(Deserialize, Serialize)] pub struct StrokeStyle { // Stroke / frame
    //#[serde(flatten)] pub ls: LayerStyle,
    #[serde(skip_serializing_if = "Option::is_none", rename = "c")]
    pub color: Option<ColorValue>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")] pub size: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct DropShadowStyle {
    #[serde(flatten)] pub inner: InnerShadowStyle,
    /// Layer Conceal, Layer knowck out drop shadow
    #[serde(skip_serializing_if = "Option::is_none")] pub lc: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct InnerShadowStyle {
    //#[serde(flatten)] pub ls: LayerStyle,
    #[serde(skip_serializing_if = "Option::is_none", rename = "c")]
    pub color: Option<ColorValue>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "o")] pub opacity: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Choke Spread
    pub ch: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Blend Mode */ pub bm: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Noise */ pub no: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "a")] /// Local light angle
    pub angle: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")] /// Blur size
    pub bs: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "d")] pub distance: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct OuterGlowStyle {
    //#[serde(flatten)] pub ls: LayerStyle,
    #[serde(skip_serializing_if = "Option::is_none", rename = "c")]
     pub color: Option<ColorValue>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "o")] pub opacity: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Choke Spread
    pub ch: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Blend Mode */ pub bm: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Noise */ pub no: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "r")] pub  range: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "j")] pub jitter: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct InnerGlowStyle {
    #[serde(flatten)] pub outer: OuterGlowStyle,    /// Source
    #[serde(skip_serializing_if = "Option::is_none")] pub sr: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct BevelEmbossStyle {
    //#[serde(flatten)] pub ls: LayerStyle,
    #[serde(skip_serializing_if = "Option::is_none", rename = "a")] /// Local light angle
    pub angle: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")] pub size: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")] /// Bevel Style
    pub bs: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Technique
    pub bt: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Strength
    pub sr: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "sf")] pub softer: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Global Angle, Use global light
    pub ga: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Altitude, Local lighting altitude
    pub ll: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Highlight Mode
    pub hm: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Highlight Opacity
    pub ho: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Shadow Mode */ pub sm: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Shadow Opacity
    pub so: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Shadow Color
    pub sc: Option<ColorValue>,
    #[serde(skip_serializing_if = "Option::is_none")] /// Highlight Color
    pub hc: Option<ColorValue>,
}

#[derive(Deserialize, Serialize)] pub struct SatinStyle {
    //#[serde(flatten)] pub ls: LayerStyle,
    #[serde(skip_serializing_if = "Option::is_none", rename = "c")]
    pub color: Option<ColorValue>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "o")] pub opacity: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "a")] /// Local light angle
    pub angle: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")] pub size: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "d")] pub distance: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")] /** Blend Mode */ pub bm: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "in")] pub invert: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct ColorOverlayStyle {
    //#[serde(flatten)] pub ls: LayerStyle,
    #[serde(skip_serializing_if = "Option::is_none", rename = "c")]
    pub color: Option<ColorValue>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "so")] pub opacity: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Blend Mode */ pub bm: Option<Value>,
}

#[derive(Deserialize, Serialize)] pub struct GradientOverlayStyle {
    //#[serde(flatten)] pub ls: LayerStyle,
    #[serde(skip_serializing_if = "Option::is_none", rename = "o")] pub opacity: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "a")] /// Local light angle
    pub angle: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "s")] pub size: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Blend Mode */ pub bm: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "re")]
    pub reverse: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "al")] /// Align with layer
    pub align: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "of")] pub offset: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] /** Smoothness */ pub gs: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] pub gf: Option<GradientColors>,
    #[serde(skip_serializing_if = "Option::is_none")] pub gt: Option<GradientType>,
}

#[derive(Clone, Copy, Default, Deserialize_repr, Serialize_repr)]
#[repr(u8)] pub enum GradientType { #[default] Linear = 1, Radial, }

//type LayerStyle = VisualObject; // Style applied to a layer
#[derive(Deserialize, Serialize)] pub struct LayerStyle {
    //* Style type */ pub ty: u32
    //#[serde(flatten)] pub vo: VisualObject,
}

/// Bezier shape used to mask/clip a layer
#[derive(Deserialize, Serialize)] pub struct Mask {
    //#[serde(flatten)] pub vo: VisualObject,
    #[serde(default)] /** Inverted */ pub inv: bool,
    #[serde(default)] pub mode: MaskMode,
    #[serde(rename = "pt")] pub shape: ShapeProperty,
    #[serde(skip_serializing_if = "Option::is_none", rename =  "o")] pub opacity: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename =  "x")] pub  expand: Option<Value>,
}

/// How masks interact with each other. See
/// https://helpx.adobe.com/after-effects/using/alpha-channels-masks-mattes.html
#[derive(Clone, Copy, Default, Deserialize, Serialize)] pub enum MaskMode {
    #[serde(rename = "n")] None,
    #[serde(rename = "a")] Add,
    #[serde(rename = "s")] Subtract,
    #[serde(rename = "i")] #[default] Intersect,
    #[serde(rename = "l")] Lighten,
    #[serde(rename = "d")] Darken,
    #[serde(rename = "f")] Difference,
}

