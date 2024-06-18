
# lib/tool in Rust for [Lottie (Bodymovin) Animation](https://lottiefiles.com)

The original [Lottie json shema](https://lottiefiles.github.io/lottie-docs/schema/) file was slightly modified so that the great [typify](https://github.com/oxidecomputer/typify) tool could run and convert it into Rust codebase. After extensive manual simplification and optimization efforts, finally, the `parser` seems to be very compatible to accept most of the [realword sample json files](https://github.com/zimond/lottie-rs/blob/main/fixtures) of [Lottie Animation](https://airbnb.design/lottie/) without too much compromise.

A simple and straightforward viewer/renderer for Lottie animation was implemented intuitively based on [femtovg](https://github.com/femtovg/femtovg), yet with many features (Text/Image/Audio/LayerEffect/etc.) to be developed/extended.

Besides, a player/renderer adapted to [rive-app](https://github.com/rive-app/rive-rs) for [**Rive** animation](https://rive.app) based on femtovg is also included, though is currently not capable to support `clip path` and `blend mode`.

TODO: refer to [intvg](https://github.com/mhfan/intvg), development/implement a `renderer` based on [gpac/evg](https://github.com/gpac/gpac/tree/master/src/evg), [blend2d](https://github.com/blend2d/blend2d), [HTML5/Web Canvas API](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API) to support most of Lottie and Rive animation features; and a `viewer` based on [bevy engine](https://github.com/bevyengine/bevy) or [Dioxus](https://github.com/DioxusLabs/dioxus)? Then, enhance capability to parse/handle [dotLottie](https://dotlottie.io/structure/#dotlottie-structure).

## Usages

```bash
    cargo r -- <path-to-lottie/svg>

    cargo r -F rive  -- <path-to-rive/lottie/svg>

    cargo r -F vello --bin vello -- <path-to-svg>

    cargo r -F b2d --bin blend2d -- <path-to-svg>
```

(with Drag & Drop support)

Note: To build for rive support, first remove comment of line "rive-rs = " in Cargo.toml. Since it isn't published on [crates.io](https://crates.io) yet.

## References

* <https://lottiefiles.github.io/lottie-docs/schema/>
* <https://github.com/LottieFiles/dotlottie-rs>

* <https://github.com/google/skia/tree/main/modules/skottie>
* <https://github.com/airbnb/lottie-web/>
* <https://github.com/linebender/velato>
* <https://github.com/Samsung/rlottie>
* <https://github.com/thorvg/thorvg>

* <https://github.com/zimond/lottie-rs/blob/main/crates/model>
* <https://github.com/angular-rust/ux-animate/tree/main/src/runtime/lottie>
* <https://github.com/servo/pathfinder/tree/master/lottie>
* <https://skia.org/docs/user/modules/skottie/>
* <https://transform.tools/json-to-rust-serde>
* <https://airbnb.design/lottie/>

* <https://github.com/sammycage/plutovg>
* <https://github.com/micro-gl/micro-gl>
* <https://github.com/tseli0s/Prisma2D>
* <https://www.amanithvg.com>

* **<https://www.w3.org/TR/compositing-1/>**
* <https://pomax.github.io/bezierinfo/>
* <https://2d.graphics/book/contents.html>
* <https://www.w3schools.com/tags/ref_canvas.asp>
