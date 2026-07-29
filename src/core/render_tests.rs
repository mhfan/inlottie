use super::*;
use crate::core::schema::{MatteMode, VisualLayer};
use kurbo::ParamCurveArclen;

fn layer_world_matrices<MC: MatrixConv>(
    layers: &[LayerItem], global: f32,
    required: impl FnMut(&LayerItem) -> bool) -> Vec<Option<TM2DwO<MC>>> {
    CompositionState::new(layers).evaluate(layers, global, required)
        .into_iter().map(|world| match world {
        WorldState::Ready(world) => Some(world),
        WorldState::Pending | WorldState::Invalid => None,
    }).collect()
}

struct TestStyle;
impl StyleConv for TestStyle {
    fn solid_color(_: RGBA) -> Self { Self }
    fn linear_gradient(_: Vec2D, _: Vec2D, _: &[(f32, RGBA)]) -> Self { Self }
    fn radial_gradient(_: Vec2D, _: Vec2D, _: (f32, f32), _: &[(f32, RGBA)]) -> Self { Self }
}

#[derive(Default)] struct TestContext {
    clear: Option<RGBA>, clear_count: u32, draw_count: u32,
    current: kurbo::Affine, transforms: Vec<kurbo::Affine>,
    fills: Vec<(kurbo::Affine, Option<kurbo::Affine>)>,
    offscreens: u32, aborts: u32, masks: u32, mattes: u32, presents: u32,
    opacity: f32, drawn: Vec<f32>, discards: u32,
}
impl RenderContext for TestContext {
    type VGPath = BezPath;
    type VGStyle = TestStyle;
    type TM2D = kurbo::Affine;
    type State = (kurbo::Affine, f32);
    type Error = &'static str;

    fn get_size(&self) -> (u32, u32) { (1, 1) }
    fn clear_rect_with(&mut self, _: u32, _: u32, _: u32, _: u32,
        color: RGBA) -> Result<(), Self::Error> {
        self.clear = Some(color); self.clear_count += 1; Ok(())
    }
    fn save_state(&mut self) -> Result<Self::State, Self::Error> {
        Ok((self.current, self.opacity))
    }
    fn restore_state(&mut self,
        (transform, opacity): Self::State) -> Result<(), Self::Error> {
        self.current = transform; self.opacity = opacity; Ok(())
    }
    fn apply_transform(&mut self, transform: &Self::TM2D,
        opacity: Option<f32>) -> Result<(), Self::Error> {
        self.current = *transform; self.transforms.push(*transform);
        if let Some(opacity) = opacity { self.opacity = opacity } Ok(())
    }
    fn fill_stroke(&mut self, _: &Self::VGPath, relative: Option<&Self::TM2D>,
        _: &(Self::VGStyle, FSOpts)) -> Result<(), Self::Error> {
        self.draw_count += 1; self.drawn.push(self.opacity);
        self.fills.push((self.current, relative.copied())); Ok(())
    }
}
impl CompositeContext for TestContext {
    type Offscreen = u32;
    type Image = u32;

    fn begin_offscreen(&mut self) -> Result<Self::Offscreen, Self::Error> {
        self.offscreens += 1; Ok(self.offscreens)
    }
    fn abort_offscreen(&mut self, _: Self::Offscreen) { self.aborts += 1 }
    fn end_offscreen(&mut self, target: Self::Offscreen) ->
        Result<Self::Image, Self::Error> { Ok(target) }
    fn apply_masks(&mut self, image: Self::Image, _: &VisualLayer,
        _: &TM2DwO<Self::TM2D>, _: f32) -> Result<Self::Image, Self::Error> {
        self.masks += 1; Ok(image)
    }
    fn apply_matte(&mut self, content: Self::Image, _: Self::Image,
        _: MatteMode) -> Result<Self::Image, Self::Error> {
        self.mattes += 1; Ok(content)
    }
    fn present(&mut self, _: Self::Image) -> Result<(), Self::Error> {
        self.presents += 1; Ok(())
    }
    fn discard(&mut self, _: Self::Image) { self.discards += 1 }
}

fn line(length: f32, y: f32) -> BezPath {
    let mut path = BezPath::new();
    path.move_to((0., y)); path.line_to((length, y)); path
}

fn trim(start: f32, end: f32, offset: f32, multiple: u8) -> TrimPath {
    serde_json::from_str(&format!(r#"{{
        "s":{{"k":{start}}},"e":{{"k":{end}}},
        "o":{{"k":{offset}}},"m":{multiple}
    }}"#)).unwrap()
}

fn path_length(path: &BezPath) -> f32 {
    path.segments().map(|segment| segment.arclen(0.1)).sum::<f64>() as _
}

fn fill_style() -> DrawItem<BezPath, TestStyle, kurbo::Affine> {
    DrawItem::Style(Rc::new((TestStyle, FSOpts::Fill(FillRule::NonZero))))
}

#[test] fn composite_keeps_layer_masks_separate_from_track_matte_state() {
    let mask = r#""masksProperties":[
        {"mode":"a","pt":{"a":0,"k":{"i":[],"o":[],"v":[],"c":false}}}
    ]"#;
    let target: VisualLayer = serde_json::from_str(
        &format!(r#"{{"ind":1,"ip":0,"op":1,"ks":{{}},"tt":1,{mask}}}"#)).unwrap();
    let source: VisualLayer = serde_json::from_str(
        &format!(r#"{{"ind":2,"ip":0,"op":1,"ks":{{}},"td":1,{mask}}}"#)).unwrap();
    let (mut context, mut state) =
        (TestContext::default(), crate::core::composite::Compositor::default());

    state.render(&mut context, &target,
        &TM2DwO::default(), 0., |_| Ok(())).unwrap();
    assert_eq!((context.masks, context.mattes, context.presents), (1, 0, 0));

    state.render(&mut context, &source,
        &TM2DwO::default(), 0., |_| Ok(())).unwrap();
    assert_eq!((context.masks, context.mattes, context.presents), (2, 1, 1));
}

#[test] fn mask_data_is_applied_without_the_redundant_hint() {
    let layer: VisualLayer = serde_json::from_str(
        r#"{"ind":1,"ip":0,"op":1,"ks":{},"masksProperties":[
            {"mode":"a","pt":{"a":0,"k":{"i":[],"o":[],"v":[],"c":false}}}
        ]}"#).unwrap();
    let (mut context, mut state) =
        (TestContext::default(), crate::core::composite::Compositor::default());

    state.render(&mut context, &layer,
        &TM2DwO::default(), 0., |_| Ok(())).unwrap();

    assert_eq!((context.offscreens, context.masks, context.presents), (1, 1, 1));
}

#[test] fn composite_aborts_offscreen_when_layer_rendering_fails() {
    let layer: VisualLayer = serde_json::from_str(
        r#"{"ind":1,"ip":0,"op":1,"ks":{},"masksProperties":[
            {"mode":"a","pt":{"a":0,"k":{"i":[],"o":[],"v":[],"c":false}}}
        ]}"#).unwrap();
    let (mut context, mut state) =
        (TestContext::default(), crate::core::composite::Compositor::default());

    assert_eq!(state.render(&mut context, &layer,
        &TM2DwO::default(), 0., |_| Err("draw failed")), Err("draw failed"));
    assert_eq!((context.offscreens, context.aborts, context.presents), (1, 1, 0));
}

#[test] fn skipped_matte_source_does_not_bind_a_later_layer() {
    let target: VisualLayer = serde_json::from_str(
        r#"{"ind":1,"ip":0,"op":1,"ks":{},"tt":1}"#).unwrap();
    let source: VisualLayer = serde_json::from_str(
        r#"{"ind":2,"ip":0,"op":1,"ks":{},"td":1}"#).unwrap();
    let later: VisualLayer = serde_json::from_str(
        r#"{"ind":3,"ip":0,"op":1,"ks":{}}"#).unwrap();
    let (mut context, mut state) =
        (TestContext::default(), crate::core::composite::Compositor::default());

    state.render(&mut context, &target,
        &TM2DwO::default(), 0., |_| Ok(())).unwrap();
    state.skip(&mut context, &source);
    state.render(&mut context, &later,
        &TM2DwO::default(), 0., |_| Ok(())).unwrap();

    assert_eq!((context.discards, context.mattes, context.presents), (1, 0, 0));
}

#[test] fn layer_world_matrix_composes_parents_without_inheriting_opacity() {
    let animation: Animation = serde_json::from_str(r#"{ "layers": [
        {"ty":3,"ind":1,"hd":true,"st":0,"ip":0,"op":10,
            "ks":{"p":{"k":[10,0]},"o":{"k":25}}},
        {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,
            "ks":{"p":{"k":[0,20]},"o":{"k":50}}},
        {"ty":3,"ind":3,"parent":2,"st":0,"ip":0,"op":10,
            "ks":{"p":{"k":[3,4]},"o":{"k":80}}},
        {"ty":3,"ind":4,"parent":99,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[5,6]}}}
    ] }"#).unwrap();

    let matrices = layer_world_matrices::<kurbo::Affine>(
        &animation.layers, 0., |_| true);
    let child = matrices[2].as_ref().unwrap();
    assert_eq!(child.0.as_coeffs(), [1., 0., 0., 1., 13., 24.]);
    assert_eq!(child.1, 0.8);
    assert_eq!(matrices[3].as_ref().unwrap().0.as_coeffs(),
        [1., 0., 0., 1., 5., 6.]);
}

#[test] fn layer_world_matrix_skips_parent_cycles_and_their_descendants() {
    let animation: Animation = serde_json::from_str(r#"{ "layers": [
        {"ty":3,"ind":1,"parent":2,"st":0,"ip":0,"op":10,"ks":{}},
        {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,"ks":{}},
        {"ty":3,"ind":3,"parent":1,"st":0,"ip":0,"op":10,"ks":{}},
        {"ty":3,"ind":4,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[5,6]}}}
    ] }"#).unwrap();

    let mut runtime = CompositionState::new(&animation.layers);
    assert_eq!(runtime.parents,
        [Parent::Invalid, Parent::Invalid, Parent::Invalid, Parent::Root]);
    let worlds: Vec<WorldState<kurbo::Affine>> =
        runtime.evaluate(&animation.layers, 0., |_| true);
    assert!(worlds[..3].iter()
        .all(|world| matches!(world, WorldState::Pending)));
    let WorldState::Ready(world) = &worlds[3] else { panic!() };
    assert_eq!(world.0.as_coeffs(), [1., 0., 0., 1., 5., 6.]);
}

#[test] fn layer_world_matrix_only_evaluates_required_layers_and_their_parents() {
    let animation: Animation = serde_json::from_str(r#"{ "layers": [
        {"ty":3,"ind":1,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[10,0]}}},
        {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[0,20]}}},
        {"ty":3,"ind":3,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[30,40]}}}
    ] }"#).unwrap();

    let matrices = layer_world_matrices::<kurbo::Affine>(
        &animation.layers, 0., |layer|
            layer.visual_layer().is_some_and(|vl| vl.base.ind == Some(2)));
    assert!(matrices[0].is_some());
    assert_eq!(matrices[1].as_ref().unwrap().0.as_coeffs(),
        [1., 0., 0., 1., 10., 20.]);
    assert!(matrices[2].is_none());
}

#[test] fn layer_world_matrix_handles_deep_parent_chains_iteratively() {
    use std::fmt::Write;
    const COUNT: u32 = 4096;
    let mut json = String::from("{\"layers\":[");
    for id in 1..=COUNT {
        if 1 < id { json.push(','); }
        write!(json, "{{\"ty\":3,\"ind\":{id},\"st\":0,\"ip\":0,\"op\":10,\"ks\":{{}}")
            .unwrap();
        if 1 < id { write!(json, ",\"parent\":{}", id - 1).unwrap(); }
        json.push('}');
    }
    json.push_str("]}");
    let animation: Animation = serde_json::from_str(&json).unwrap();

    let matrices = layer_world_matrices::<kurbo::Affine>(
        &animation.layers, 0., |layer|
            layer.visual_layer().is_some_and(|vl| vl.base.ind == Some(COUNT)));
    assert!(matrices.iter().all(Option::is_some));
}

#[test] fn playback_starts_and_wraps_at_the_in_point() {
    let mut runtime = LottieRuntime::from_reader(
        &br#"{"ip":10,"op":12,"fr":1,"layers":[]}"#[..]).unwrap();
    let mut context = TestContext::default();

    assert!(runtime.render_next_frame(
        &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))).unwrap());
    assert_eq!(runtime.frame(), 11.);
    assert!(runtime.render_next_frame(
        &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))).unwrap());
    assert_eq!(runtime.frame(), 10.);

    assert!(runtime.render_next_frame(
        &mut context, 2., Some(RGBA::new_u8(0, 0, 0, 0))).unwrap());
    assert_eq!(runtime.frame(), 10.);
}

#[test] fn lottie_runtime_reuses_layer_graph_and_precomp_state() {
    let mut runtime = LottieRuntime::from_reader(&br##"{
        "ip":0,"op":10,"fr":1,
        "assets":[{"id":"nested","layers":[
            {"ty":1,"st":0,"ip":0,"op":10,
                "sw":1,"sh":1,"sc":"#000000","ks":{}}
        ]}],
        "layers":[{"ty":0,"refId":"nested","w":1,"h":1,
            "st":0,"ip":0,"op":10,"ks":{}}]
    }"##[..]).unwrap();
    let graph = runtime.root.parents.as_ptr();
    let child_graph = runtime.root.precomps[0].as_ref().unwrap()
        .composition.parents.as_ptr();
    let mut context = TestContext::default();

    assert!(runtime.render_next_frame(&mut context, 1., None).unwrap());
    assert!(runtime.render_next_frame(&mut context, 1., None).unwrap());
    assert_eq!(graph, runtime.root.parents.as_ptr());
    assert_eq!(child_graph, runtime.root.precomps[0].as_ref().unwrap()
        .composition.parents.as_ptr());
}

#[test] fn precomp_time_remap_uses_root_fps_after_layer_time_mapping() {
    let json = br##"{
        "fr":24,"ip":20,"op":40,
        "assets":[{"id":"nested","fr":99,"layers":[{
            "ty":4,"st":0,"ip":0,"op":100,
            "ks":{"p":{"k":[{"t":0,"s":[0,0]},{"t":24,"s":[100,0]}]}},
            "shapes":[
                {"ty":"rc","s":{"k":[1,1]},"p":{"k":[0,0]},"r":{"k":0}},
                {"ty":"fl","c":{"k":[1,0,0]},"o":{"k":100}}
            ]
        }]}],
        "layers":[{"ty":0,"refId":"nested","w":1,"h":1,
            "st":4,"sr":2,"ip":0,"op":40,"ks":{},
            "tm":{"k":[{"t":0,"s":[0]},{"t":12,"s":[1]}]}}]
    }"##;
    let mut runtime = LottieRuntime::from_reader(&json[..]).unwrap();
    let mut context = TestContext::default();

    assert!(runtime.render_next_frame(&mut context, 1. / 24., None).unwrap());
    assert!(context.transforms.iter().any(|transform|
        (transform.as_coeffs()[4] - 50.).abs() < 1e-4),
        "{:?}", context.transforms.iter().map(|tm| tm.as_coeffs()).collect::<Vec<_>>());
    let AssetItem::Precomp(precomp) = &runtime.animation.assets[0] else { panic!() };
    assert!(!serde_json::to_value(precomp).unwrap().as_object().unwrap().contains_key("fr"));
}

#[test] fn lottie_runtime_skips_recursive_precomp_references() {
    let runtime = LottieRuntime::from_reader(&br#"{
        "assets":[
            {"id":"a","layers":[{"ty":0,"refId":"b","w":1,"h":1,
                "ip":0,"op":1,"ks":{}}]},
            {"id":"b","layers":[{"ty":0,"refId":"a","w":1,"h":1,
                "ip":0,"op":1,"ks":{}}]}
        ],
        "layers":[{"ty":0,"refId":"a","w":1,"h":1,"ip":0,"op":1,"ks":{}}]
    }"#[..]).unwrap();

    let a = runtime.root.precomps[0].as_ref().unwrap();
    let b = a.composition.precomps[0].as_ref().unwrap();
    assert!(b.composition.precomps[0].is_none());
}

#[test] fn frame_clear_supports_transparent_color_and_preserve_modes() {
    let mut runtime = LottieRuntime::from_reader(
        &br#"{"ip":0,"op":10,"fr":1,"layers":[]}"#[..]).unwrap();
    let mut context = TestContext::default();

    assert!(runtime.render_next_frame(
        &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))).unwrap());
    let clear = context.clear.unwrap();
    assert_eq!((clear.r, clear.g, clear.b, clear.a), (0, 0, 0, 0));

    let red = RGBA::new_u8(255, 0, 0, 128);
    assert!(runtime.render_next_frame(&mut context, 1., Some(red)).unwrap());
    let clear = context.clear.unwrap();
    assert_eq!((clear.r, clear.g, clear.b, clear.a), (255, 0, 0, 128));

    assert!(runtime.render_next_frame(&mut context, 1., None).unwrap());
    assert_eq!(context.clear_count, 2);
}

#[test] fn recursive_render_restores_parent_opacity_between_siblings() {
    let path = || DrawItem::Shape(BezPath::new());
    let draws = vec![path(),
        DrawItem::Group(vec![path()],
            vec![TM2DwO(kurbo::Affine::IDENTITY, 0.4)]),
        fill_style(),
    ];
    let mut context = TestContext { opacity: 1., ..Default::default() };

    context.render_shapes(&TM2DwO(kurbo::Affine::IDENTITY, 0.5), &draws).unwrap();
    assert_eq!(context.drawn, [0.2, 0.5]);
    assert_eq!(context.opacity, 0.5);
}

#[test] fn outer_styles_keep_their_scope_transform_across_nested_groups() {
    let path = || DrawItem::Shape(BezPath::new());
    let group_matrix = kurbo::Affine::translate((20., 0.));
    let group = TM2DwO(group_matrix, 1.);
    let draws = [path(), DrawItem::Group(vec![path()], vec![group]),
        fill_style()];
    let scope = TM2DwO(kurbo::Affine::translate((10., 0.)), 1.);
    let mut context = TestContext::default();

    context.render_shapes(&scope, &draws).unwrap();
    assert_eq!(context.fills.len(), 2);
    assert_eq!(context.fills[0].0, scope.0);
    assert_eq!(context.fills[0].1, Some(group_matrix));
    assert_eq!(context.fills[1], (scope.0, None));
}

#[test] fn trim_range_normalizes_direction_and_negative_offset() {
    assert_eq!(normalize_trim(0., 0.5, -0.25), (0.75, 0.5));
    assert_eq!(normalize_trim(0., 0.5,  0.75), (0.75, 0.5));
    assert_eq!(normalize_trim(0.75, 0.25, 0.), (0.25, 0.5));
    assert_eq!(normalize_trim(0., 0.5, -1.25), (0.75, 0.5));
    assert_eq!(normalize_trim(0., 1.5,  0.25), (0.25, 1.0));
    assert_eq!(normalize_trim(-0.5, 0.5, 0.), (0., 0.5));
    assert_eq!(normalize_trim(1.5, 2., 0.), (0., 0.));
}

#[test] fn modifiers_keep_the_lazy_path_in_kurbo_form() {
    let mut path = PendingPath::<BezPath>::new(5);
    path.rect(0., 0., 10., 10.);
    assert!(matches!(path, PendingPath::Native(_)));

    path.round_corners(2.);
    assert!(matches!(path, PendingPath::Kurbo(_)));
    path.offset_path(1., super::super::schema::LineJoin::Round, 4.);
    assert!(matches!(path, PendingPath::Kurbo(_)));
    assert!(!path.into_native().is_empty());
}

#[test] fn sequential_trim_keeps_both_wrapped_parts_of_one_shape() {
    let trim = trim(0., 50., 270., 2);
    let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> =
        vec![DrawItem::Shape(line(100., 0.))];

    trim_shapes(&trim, &mut draws, 0.);
    let DrawItem::Shape(path) = &draws[0] else { panic!() };
    let segments = path.segments().collect::<Vec<_>>();
    assert_eq!(segments.len(), 2);
    use kurbo::ParamCurve;
    assert_eq!((segments[0].start().x, segments[0].end().x), (75., 100.));
    assert_eq!((segments[1].start().x, segments[1].end().x), (0., 25.));
}

#[test] fn sequential_trim_follows_reverse_render_order() {
    let trim = trim(0., 25., 0., 2);
    let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
        DrawItem::Shape(line(100., 0.)),
        DrawItem::Shape(line( 50., 1.)),
    ];

    trim_shapes(&trim, &mut draws, 0.);
    let [DrawItem::Shape(first), DrawItem::Shape(second)] = &draws[..] else { panic!() };
    assert!(first.is_empty());
    use kurbo::ParamCurve;
    let segment = second.segments().next().unwrap();
    assert_eq!((segment.start().x, segment.end().x), (0., 37.5));
}

#[test] fn sequential_trim_follows_nested_group_render_order() {
    let trim = trim(0., 25., 0., 2);
    let group = vec![
        DrawItem::Shape(line(30., 1.)),
        DrawItem::Shape(line(20., 2.)),
    ];
    let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
        DrawItem::Shape(line(100., 0.)),
        DrawItem::Group(group, vec![TM2DwO::default()]),
    ];

    trim_shapes(&trim, &mut draws, 0.);
    let [DrawItem::Shape(first), DrawItem::Group(group, _)] = &draws[..] else { panic!() };
    let [DrawItem::Shape(middle), DrawItem::Shape(last)] = &group[..] else { panic!() };
    assert!(first.is_empty());
    assert_eq!(last.segments().next().unwrap().arclen(0.1), 20.);
    assert_eq!(middle.segments().next().unwrap().arclen(0.1), 17.5);
}

#[test] fn simultaneous_trim_applies_the_range_to_each_path() {
    let trim = trim(25., 75., 0., 1);
    let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
        DrawItem::Shape(line(100., 0.)),
        DrawItem::Shape(line(40., 1.)),
    ];

    trim_shapes(&trim, &mut draws, 0.);
    let [DrawItem::Shape(first), DrawItem::Shape(second)] = &draws[..] else { panic!() };
    assert_eq!(first.segments().next().unwrap().arclen(0.1), 50.);
    assert_eq!(second.segments().next().unwrap().arclen(0.1), 20.);
}

#[test] fn sequential_trim_treats_repeater_copies_as_rendered_paths() {
    let trim = trim(0., 25., 0., 2);

    let mut after: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
        DrawItem::Group(vec![DrawItem::Shape(line(100., 0.))],
            vec![TM2DwO::default(); 4]),
    ];
    trim_shapes(&trim, &mut after, 0.);
    let DrawItem::Copies(copies) = &after[0] else { panic!() };
    assert_eq!(copies.len(), 4);
    for (index, (group, _)) in copies.iter().enumerate() {
        let [DrawItem::Shape(path)] = &group[..] else { panic!() };
        assert_eq!(path_length(path), if index == 3 { 100. } else { 0. });
    }
    after.push(fill_style());
    let mut context = TestContext::default();
    context.render_shapes(&TM2DwO::default(), &after).unwrap();
    assert_eq!(context.draw_count, 4);

    let mut before: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> =
        vec![DrawItem::Shape(line(100., 0.))];
    trim_shapes(&trim, &mut before, 0.);
    let DrawItem::Shape(path) = before.remove(0) else { panic!() };
    let batch: DrawItem<BezPath, TestStyle, kurbo::Affine> =
        DrawItem::Group(vec![DrawItem::Shape(path)], vec![TM2DwO::default(); 4]);
    let DrawItem::Group(group, transforms) = batch else { panic!() };
    let [DrawItem::Shape(path)] = &group[..] else { panic!() };
    assert_eq!(transforms.len(), 4);
    assert_eq!(path_length(path), 25.);
}

#[test] fn sequential_trim_measures_group_paths_before_group_transform() {
    let trim = trim(0., 25., 0., 2);
    let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
        DrawItem::Shape(line(100., 0.)),
        DrawItem::Group(vec![DrawItem::Shape(line(100., 0.))],
            vec![TM2DwO(kurbo::Affine::scale(10.), 1.)]),
    ];

    trim_shapes(&trim, &mut draws, 0.);
    let DrawItem::Group(group, _) = &draws[1] else { panic!() };
    let [DrawItem::Shape(path)] = &group[..] else { panic!() };
    assert_eq!(path.segments().next().unwrap().arclen(0.1), 50.);
}
