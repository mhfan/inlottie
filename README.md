
# lib/tool in Rust for [Lottie (Bodymovin) Animation](https://lottiefiles.com) file

The original [Lottie json shema](https://lottiefiles.github.io/lottie-docs/schema/) file was slightly modified so that the great [typify](https://github.com/oxidecomputer/typify) tool could run and convert it into Rust codebase. After much simplification and optimization work was done manually, finally, the `parser` seems to be very compatible to accept most of the [realword sample json files](https://github.com/zimond/lottie-rs/blob/main/fixtures) of [Lottie Animation](https://airbnb.design/lottie/) without too much compromise.

TODO: refer to [intvg](https://github.com/mhfan/intvg), development/implement a `renderer` based on [freetype2/ftgrays](https://gitlab.freedesktop.org/freetype/freetype/-/blob/master/src/smooth/ftgrays.c), [gpac/evg](https://github.com/gpac/gpac/tree/master/src/evg), [blend2d](https://github.com/blend2d/blend2d), HTML5/Web Canvas API or [femtovg](https://github.com/femtovg/femtovg); and a `viewer` based on [bevy engine](https://github.com/bevyengine/bevy) or [Dioxus](https://github.com/DioxusLabs/dioxus)? Then, enhance capability to parse/handle [dotLottie](https://dotlottie.io/structure/#dotlottie-structure).

## References

* <https://lottiefiles.github.io/lottie-docs/schema/>
* <https://github.com/LottieFiles/dotlottie-rs>

* <https://github.com/zimond/lottie-rs/blob/main/crates/model>
* <https://github.com/angular-rust/ux-animate/tree/main/src/runtime/lottie>
* <https://github.com/thorvg/thorvg/tree/main/src/loaders/lottie>
* <https://github.com/Samsung/rlottie/tree/master/src/lottie>
* <https://github.com/servo/pathfinder/tree/master/lottie>
* <https://transform.tools/json-to-rust-serde>
* <https://airbnb.design/lottie/>

* <https://github.com/Samsung/rlottie>
* <https://github.com/msrd0/rlottie-rs>
* <https://github.com/sammycage/plutovg>
* <https://github.com/micro-gl/micro-gl>
* <https://github.com/tseli0s/Prisma2D>
* <https://github.com/thorvg/thorvg>
* <https://www.amanithvg.com>
* <https://www.w3schools.com/tags/ref_canvas.asp>
* <https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API>
