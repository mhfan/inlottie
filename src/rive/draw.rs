//! Draw grouping, Rive draw-rule ordering, and immutable display-list emission.

use std::{mem, sync::Arc};

use super::{ComponentPaint, DrawGroup, Result, Runtime, RuntimeError, uint, Shape,
    object_ids, property_ids, Affine2, Brush, Clip, DisplayList, DrawItem, Image, Paint,
};

impl Runtime {
    pub fn write_display_list(&self, list: &mut DisplayList) {
        list.clear();
        let primitive_count = self.draw_groups.iter().filter(|group|
                0.0 < self.components[group.opacity_component as usize].world_opacity)
            .map(|group| group.paints.iter().filter_map(|&index|
                self.components[index as usize].paint())
                .filter(|paint| visible_paint(paint)).count().max(1)).sum();
        list.reserve(primitive_count);
        let clip_paths: Vec<_> = self.components.iter().map(|component| {
            let clip = component.clip()?;
            clip.visible.then(|| Clip { obj_idx: component.obj_idx, scope: 0,
                rule: clip.rule, shapes: self.snapshot_shapes(&clip.shapes) })
        }).collect();

        // A DrawItem is a snapshot: cloning Paint and Shape data keeps an emitted list valid
        // while the retained Runtime advances to another animation frame.
        for group in &self.draw_groups {
            let opacity = self.components[group.opacity_component as usize].world_opacity;
            if  opacity <= 0.0 { continue }
            let shapes = self.snapshot_shapes(&group.components);
            let clips: Arc<[_]> = group.clips.iter()
                .filter_map(|&index| clip_paths[index as usize].clone()).collect();
            if let Some(index) = group.nested {
                self.write_nested(index, opacity, &clips, list); continue
            }
            if group.paints.is_empty() {
                list.push(DrawItem {
                    obj_idx: group.obj_idx, opacity, clips, shapes, paint: None,
                    image: group.image.map(|index| self.snapshot_image(index)) });
            } else {
                let start = list.len();
                list.extend(group.paints.iter().filter_map(|&index| {
                    let paint = self.components[index as usize].paint()?;
                    visible_paint(paint).then(|| DrawItem {
                        obj_idx: group.obj_idx, opacity,
                        clips: clips.clone(), shapes: shapes.clone(),
                        paint: Some(paint.value.clone()), image: None,
                    })
                }));
                if  list.len() == start {
                    list.push(DrawItem {
                        obj_idx: group.obj_idx, opacity, clips, shapes,
                        paint: None, image: None
                    });
                }
            }
        }
    }

    fn write_nested(&self, index: u32, opacity: f32,
        parent_clips: &[Clip], list: &mut DisplayList) {
        let nested = &self.nested[index as usize];
        let mut host = self.components[nested.host as usize].world;
        if let Some(origin) = nested.origin
            .and_then(|index| self.components[index as usize].nested_origin()) {
            let (width, height) = nested.runtime.artboard_size;
            host = host.then(Affine2 {
                tx: -origin.0.x * width, ty: -origin.0.y * height,
                ..Affine2::default()
            });
        }
        let scope = self.components[nested.host as usize].obj_idx + 1;
        let mut child = DisplayList::default();
        nested.runtime.write_display_list(&mut child);
        list.reserve(child.len());
        list.extend(child.into_items().map(|mut item| {
            item.opacity *= opacity;
            for shape in Arc::make_mut(&mut item.shapes) {
                shape.trfm = host.then(shape.trfm);
            }
            if let Some(image) = &mut item.image { image.trfm = host.then(image.trfm) }
            if let Some(paint) = &mut item.paint { transform_brush(paint, host) }
            let mut clips = Vec::with_capacity(parent_clips.len() + item.clips.len());
            clips.extend_from_slice(parent_clips);
            clips.extend(item.clips.iter().cloned().map(|mut clip| {
                clip.scope = scope;
                for shape in Arc::make_mut(&mut clip.shapes) {
                    shape.trfm = host.then(shape.trfm);
                }   clip
            }));
            item.clips = clips.into(); item
        }));
    }

    fn snapshot_image(&self, index: u32) -> Image {
        let component = &self.components[index as usize];
        let image = component.image().unwrap();
        Image { asset_id: image.asset_id, data: image.data.clone(),
            trfm: component.world, origin: image.origin }
    }

    fn snapshot_shapes(&self, indices: &[u32]) -> Arc<[Shape]> {
        indices.iter().map(|&index| {
                let component = &self.components[index as usize];
                Shape { obj_idx: component.obj_idx, is_hole: component.is_hole,
                    trfm: component.world,
                    geom: component.geom().unwrap().geometry().clone() }
        }).collect()
    }

    fn ancestor_of_type(&self, mut component: Option<u32>, type_id: u32) -> Option<u32> {
        while let Some(index) = component {
            let candidate = &self.components[index as usize];
            if self.file.ocoll[candidate.obj_idx as usize].type_id.0 == type_id {
                return Some(index)
            }   component = candidate.parent;
        }   None
    }

    pub(super) fn build_draw_groups(&mut self) {
        // Rive applies all paints under a Shape to the Shape's combined geometry collection.
        // Standalone geometry becomes its own unpainted group.
        let shapes: Vec<_> = self.components.iter().map(|component|
            self.ancestor_of_type(component.parent, object_ids::SHAPE)).collect();
        let mut shape_groups = vec![None; self.components.len()];
        for (index, component) in self.components.iter().enumerate() {
            let type_id = self.file.ocoll[component.obj_idx as usize].type_id.0;
            if  type_id == object_ids::SHAPE {
                shape_groups[index] = Some(self.draw_groups.len());
                self.draw_groups.push(DrawGroup {
                    obj_idx: component.obj_idx, opacity_component: index as u32,
                    components: Vec::new(), paints: Vec::new(), clips: Vec::new(),
                    image: None, nested: None,
                });
            } else if component.geom().is_some() && shapes[index].is_none() {
                self.draw_groups.push(DrawGroup {
                    obj_idx: component.obj_idx, opacity_component: index as u32,
                    components: vec![index as u32], paints: Vec::new(),
                    clips: Vec::new(), image: None, nested: None,
                });
            } else if component.image().is_some() {
                self.draw_groups.push(DrawGroup {
                    obj_idx: component.obj_idx, opacity_component: index as u32,
                    components: Vec::new(), paints: Vec::new(),
                    clips: Vec::new(), image: Some(index as u32), nested: None,
                });
            } else if type_id == object_ids::NESTED_ARTBOARD {
                if let Some(nested) = self.nested.iter()
                    .position(|nested| nested.host == index as u32) {
                    self.draw_groups.push(DrawGroup {
                        obj_idx: component.obj_idx, opacity_component: index as u32,
                        components: Vec::new(), paints: Vec::new(), clips: Vec::new(),
                        image: None, nested: Some(nested as u32),
                    });
                }
            }
        }
        for (index, component) in self.components.iter().enumerate() {
            let Some(shape) = shapes[index] else { continue };
            let group = &mut self.draw_groups[shape_groups[shape as usize].unwrap()];
            if component.geom().is_some() { group.components.push(index as u32) }
            if component.paint().is_some() { group.paints.push(index as u32) }
        }

        self.draw_groups.retain(|group| !group.components.is_empty() ||
                group.image.is_some() || group.nested.is_some());
    }

    pub(super) fn attach_clips(&mut self) {
        // Cache source membership; only geometry values and transforms change per frame.
        for clip in 0..self.components.len() {
            let Some(source) = self.components[clip].clip().map(|value| value.source) else {
                continue
            };
            let shapes = self.components.iter().enumerate().filter_map(|(index, component)|
                (component.geom().is_some() &&
                    is_descendant(&self.components, index as u32, source))
                    .then_some(index as u32)).collect();
            self.components[clip].clip_mut().unwrap().shapes = shapes;
        }
        // A clipping component affects every drawable in its parent's subtree.
        for (clip, component) in self.components.iter().enumerate() {
            if component.clip().is_none() { continue }
            let Some(owner) = component.parent else { continue };
            for group in &mut self.draw_groups {
                if is_descendant(&self.components, group.opacity_component, owner) {
                    group.clips.push(clip as u32);
                }
            }
        }
    }

    pub(super) fn apply_draw_rules(&mut self, obj_comps: &[Option<u32>]) -> Result<()> {
        // Convert before/after draw targets into graph edges, then emit a stable DFS order.
        let mut rules_by_owner = vec![None; self.components.len()];
        for (index, component) in self.components.iter().enumerate() {
            let object = &self.file.ocoll[component.obj_idx as usize];
            if object.type_id.0 == object_ids::DRAW_RULES {
                if let Some(owner) = component.parent {
                    rules_by_owner[owner as usize] = Some(index as u32);
                }
            }
        }
        let mut groups_by_rule = vec![Vec::new(); self.components.len()];
        for (group, value) in self.draw_groups.iter().enumerate() {
            let mut component = Some(value.opacity_component);
            while  let Some(index) = component {
                if let Some(rules) = rules_by_owner[index as usize] {
                    groups_by_rule[rules as usize].push(group);     break
                }
                component = self.components[index as usize].parent;
            }
        }
        let mut groups_by_obj = vec![None; self.file.ocoll.len()];
        for (group, value) in self.draw_groups.iter().enumerate() {
            groups_by_obj[value.obj_idx as usize] = Some(group);
        }

        let mut before = vec![Vec::new(); self.draw_groups.len()];
        let mut after  = vec![Vec::new(); self.draw_groups.len()];
        let mut attached = vec![false; self.draw_groups.len()];
        for rule_index in rules_by_owner.into_iter().flatten() {
            let rule = &self.file.ocoll[self.components[rule_index as usize].obj_idx as usize];
            let target_id = uint(rule, property_ids::DRAWTARGETID)?;
            let Some(target_obj) = self.artboard_obj.checked_add(target_id) else { continue };
            let Some(target_component) = obj_comps.get(target_obj as usize)
                .copied().flatten() else { continue };
            let target_component = &self.components[target_component as usize];
            let target = &self.file.ocoll[target_component.obj_idx as usize];
            if target.type_id.0 != object_ids::DRAW_TARGET { continue }

            let drawable_id = uint(target, property_ids::DRAWABLEID)?;
            let Some(drawable_obj) = self.artboard_obj
                .checked_add(drawable_id) else { continue };
            let Some(target_group) = groups_by_obj.get(drawable_obj as usize)
                .copied().flatten() else { continue };
            let moved = &groups_by_rule[rule_index as usize];
            if moved.is_empty() || moved.contains(&target_group) { continue }

            let placement = if uint(target, property_ids::PLACEMENTVALUE)? == 0 {
                &mut before[target_group]
            } else {
                &mut  after[target_group]
            };
            for &index in moved {
                if !attached[index] {
                    attached[index] = true;
                    placement.push(index);
                }
            }
        }

        fn emit(index: usize, groups: &mut [Option<DrawGroup>],
            before: &[Vec<usize>], after: &[Vec<usize>], state: &mut [u8],
            output: &mut Vec<DrawGroup>) -> Result<()> {
            // 0/1/2 are unvisited/visiting/emitted; a visiting edge is a draw-rule cycle.
            match state[index] {
                1 => return Err(RuntimeError::DrawOrderCycle(
                    groups[index].as_ref().map_or(0, |group| group.obj_idx))),
                2 => return Ok(()), _ => state[index] = 1,
            }
            for &child in &before[index] {
                emit(child, groups, before, after, state, output)?;
            }
            output.push(groups[index].take().unwrap());
            for &child in &after[index] {
                emit(child, groups, before, after, state, output)?;
            }   state[index] = 2;   Ok(())
        }

        let mut groups: Vec<_> = mem::take(&mut self.draw_groups)
            .into_iter().map(Some).collect();
        let mut state = vec![0; groups.len()];
        let mut output = Vec::with_capacity(groups.len());
        for index in 0..groups.len() {
            if !attached[index] {
                emit(index, &mut groups, &before, &after, &mut state, &mut output)?;
            }
        }
        if let Some(index) = state.iter().position(|&value| value == 0) {
            emit(index, &mut groups, &before, &after, &mut state, &mut output)?;
        }   self.draw_groups = output;  Ok(())
    }
}

fn is_descendant(components: &[super::Component], mut component: u32, ancestor: u32) -> bool {
    loop {
        if component == ancestor { return true }
        let Some(parent) = components[component as usize].parent else { return false };
        component = parent;
    }
}

fn visible_paint(paint: &ComponentPaint) -> bool {
    paint.visible && match &paint.value {
        Paint::Stroke { width, .. } => 0.0 < *width, _ => true,
    }
}

fn transform_brush(paint: &mut Paint, host: Affine2) {
    let brush = match paint {
        Paint::Fill { brush, .. } | Paint::Stroke { brush, .. } => brush,
    };
    match brush {
        Brush::LinearGradient { trfm, .. } |
        Brush::RadialGradient { trfm, .. } => *trfm = host.then(*trfm),
        Brush::Solid(_) => {}
    }
}
