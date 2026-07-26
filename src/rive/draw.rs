//! Draw grouping, Rive draw-rule ordering, and immutable display-list emission.

use std::mem;

use super::{
    DrawGroup, Result, Runtime, RuntimeError, uint,
    object_ids, property_ids,
    DisplayList, DrawItem, Paint, Shape,
};

impl Runtime {
    pub fn display_list(&self) -> DisplayList {
        let mut list = DisplayList::default();
        self.write_display_list(&mut list);
        list
    }

    pub fn write_display_list(&self, list: &mut DisplayList) {
        list.clear();
        let primitive_count = self.draw_groups.iter().filter(|group|
                0.0 < self.components[group.opacity_component as usize].world_opacity)
            .map(|group| group.paints.iter().filter_map(|&index|
                self.components[index as usize].paint.as_ref())
                .filter(|paint| visible_paint(paint)).count().max(1)).sum();
        list.items.reserve(primitive_count);

        // A DrawItem is a snapshot: cloning Paint and Shape data keeps an emitted list valid
        // while the retained Runtime advances to another animation frame.
        for group in &self.draw_groups {
            let opacity = self.components[group.opacity_component as usize].world_opacity;
            if  opacity <= 0.0 { continue }
            let shapes: std::sync::Arc<[_]> = group.components.iter().map(|&index| {
                let component = &self.components[index as usize];
                Shape { obj_idx: component.obj_idx, is_hole: component.is_hole,
                    trfm: component.world,
                    geom: component.geometry.as_ref().unwrap().clone() }
            }).collect();
            if group.paints.is_empty() {
                list.items.push(DrawItem {
                    obj_idx: group.obj_idx, opacity, shapes, paint: None });
            } else {
                let start = list.items.len();
                list.items.extend(group.paints.iter().filter_map(|&index| {
                    let paint = self.components[index as usize].paint.as_ref()?;
                    visible_paint(paint).then(|| DrawItem {
                        obj_idx: group.obj_idx, opacity,
                        shapes: shapes.clone(), paint: Some(paint.clone()),
                    })
                }));
                if  list.items.len() == start {
                    list.items.push(DrawItem {
                        obj_idx: group.obj_idx, opacity, shapes, paint: None });
                }
            }
        }
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
                    components: Vec::new(), paints: Vec::new()
                });
            } else if component.geometry.is_some() && shapes[index].is_none() {
                self.draw_groups.push(DrawGroup {
                    obj_idx: component.obj_idx, opacity_component: index as u32,
                    components: vec![index as u32], paints: Vec::new(),
                });
            }
        }
        for (index, component) in self.components.iter().enumerate() {
            let Some(shape) = shapes[index] else { continue };
            let group = &mut self.draw_groups[shape_groups[shape as usize].unwrap()];
            if component.geometry.is_some() { group.components.push(index as u32) }
            if component.paint.is_some() { group.paints.push(index as u32) }
        }
        self.draw_groups.retain(|group| !group.components.is_empty());
    }

    pub(super) fn apply_draw_rules(&mut self) -> Result<()> {
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
        let group_rules: Vec<_> = self.draw_groups.iter().map(|group| {
            let mut component = Some(group.opacity_component);
            while let Some(index) = component {
                if let Some(rules) = rules_by_owner[index as usize] { return Some(rules) }
                component = self.components[index as usize].parent;
            }   None
        }).collect();

        let mut before = vec![Vec::new(); self.draw_groups.len()];
        let mut after  = vec![Vec::new(); self.draw_groups.len()];
        let mut attached = vec![false; self.draw_groups.len()];
        for rule_index in rules_by_owner.into_iter().flatten() {
            let rule = &self.file.ocoll[self.components[rule_index as usize].obj_idx as usize];
            let target_id = uint(rule, property_ids::DRAWTARGETID)?;
            let Some(target_obj) = self.artboard_obj.checked_add(target_id) else { continue };
            let Some(target_component) = self.components.iter()
                .find(|component| component.obj_idx == target_obj) else { continue };
            let target = &self.file.ocoll[target_component.obj_idx as usize];
            if target.type_id.0 != object_ids::DRAW_TARGET { continue }

            let drawable_id = uint(target, property_ids::DRAWABLEID)?;
            let Some(drawable_obj) = self.artboard_obj
                .checked_add(drawable_id) else { continue };
            let Some(target_group) = self.draw_groups.iter()
                .position(|group| group.obj_idx == drawable_obj) else { continue };
            let moved: Vec<_> = group_rules.iter().enumerate()
                .filter(|(_, rules)| **rules == Some(rule_index))
                .map(|(index, _)| index).collect();
            if moved.is_empty() || moved.contains(&target_group) { continue }

            let placement = if uint(target, property_ids::PLACEMENTVALUE)? == 0 {
                &mut before[target_group]
            } else {
                &mut  after[target_group]
            };
            for &index in &moved {
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
        }
        self.draw_groups = output;  Ok(())
    }
}

fn visible_paint(paint: &Paint) -> bool { match paint {
    Paint::Stroke { width, .. } => 0.0 < *width,
    _ => true,
} }
