//! Transform-constraint decoding and world-space evaluation.

use super::{Component, Result, RuntimeError, TrackValue,
    boolean, core_is_transform_component, float, object_ids, property_ids, uint};
use crate::rive::{decode::Object, display_list::{Affine2, Point}};

#[derive(Debug, Clone, Copy)] enum Kind {
    Translation, Rotation, Scale,
    Transform { origin: Point, bounds: Point },
    Distance { distance: f32, mode: u32 },
}

#[derive(Debug, Clone, Copy)] pub(super) struct Constraint {
    kind: Kind, pub owner: u32, target: Option<u32>,
    strength: f32, copy_x: bool, copy_y: bool, factor_x: f32, factor_y: f32,
    offset: bool, source_local: bool, dest_local: bool, limits_local: bool,
    has_min_x: bool, has_max_x: bool, has_min_y: bool, has_max_y: bool,
    min_x: f32, max_x: f32, min_y: f32, max_y: f32,
}

#[derive(Clone, Copy)] struct Parts {
    x: f32, y: f32, scale_x: f32, scale_y: f32, rotation: f32, skew: f32,
}

impl Constraint {
    pub fn from_object(object: &Object) -> Result<Option<Self>> {
        let kind = match object.type_id.0 {
            object_ids::TRANSLATION_CONSTRAINT => Kind::Translation,
            object_ids::ROTATION_CONSTRAINT => Kind::Rotation,
            object_ids::SCALE_CONSTRAINT => Kind::Scale,
            object_ids::TRANSFORM_CONSTRAINT => Kind::Transform {
                origin: Point {
                    x: float(object, property_ids::TRANSFORMCONSTRAINT_ORIGINX)?,
                    y: float(object, property_ids::TRANSFORMCONSTRAINT_ORIGINY)?,
                }, bounds: Point::default(),
            },
            object_ids::DISTANCE_CONSTRAINT => Kind::Distance {
                distance: float(object, property_ids::DISTANCECONSTRAINT_DISTANCE)?,
                mode: uint(object, property_ids::DISTANCECONSTRAINT_MODEVALUE)?,
            },
            _ => return Ok(None),
        };
        let two_axes = matches!(kind, Kind::Translation | Kind::Scale);
        let target = uint(object, property_ids::TARGETEDCONSTRAINT_TARGETID)?;
        Ok(Some(Self {
            kind, owner: u32::MAX,
            target: (target != u32::MAX).then_some(target),
            strength: float(object, property_ids::CONSTRAINT_STRENGTH)?,
            copy_x: boolean(object, property_ids::DOESCOPY)?,
            copy_y: two_axes && boolean(object, property_ids::DOESCOPYY)?,
            factor_x: float(object, property_ids::COPYFACTOR)?,
            factor_y: if two_axes { float(object, property_ids::COPYFACTORY)? } else { 1.0 },
            offset: boolean(object, property_ids::TRANSFORMCOMPONENTCONSTRAINT_OFFSET)?,
            source_local: uint(object, property_ids::SOURCESPACEVALUE)? == 1,
            dest_local: uint(object, property_ids::DESTSPACEVALUE)? == 1,
            limits_local: uint(object, property_ids::MINMAXSPACEVALUE)? == 1,
            has_min_x: boolean(object, property_ids::MIN)?,
            has_max_x: boolean(object, property_ids::MAX)?,
            has_min_y: two_axes && boolean(object, property_ids::MINY)?,
            has_max_y: two_axes && boolean(object, property_ids::MAXY)?,
            min_x: float(object, property_ids::MINVALUE)?,
            max_x: float(object, property_ids::MAXVALUE)?,
            min_y: if two_axes { float(object, property_ids::MINVALUEY)? } else { 0.0 },
            max_y: if two_axes { float(object, property_ids::MAXVALUEY)? } else { 0.0 },
        }))
    }

    pub fn resolve(&mut self, component: u32, components: &[Component],
        objects: &[Object], obj_comps: &[Option<u32>], context_start: usize) -> Result<()> {
        self.owner = components[component as usize].parent
            .ok_or(RuntimeError::InvalidConstraintOwner(
                components[component as usize].obj_idx))?;
        if !core_is_transform_component(
            objects[components[self.owner as usize].obj_idx as usize].type_id.0) {
            return Err(RuntimeError::InvalidConstraintOwner(
                components[component as usize].obj_idx))
        }
        let Some(target_id) = self.target else { return Ok(()) };
        let target_obj = context_start.checked_add(target_id as usize)
            .ok_or(RuntimeError::InvalidConstraintTarget(target_id))?;
        let target = obj_comps.get(target_obj).copied().flatten()
            .filter(|&target| target != component && target != self.owner)
            .ok_or(RuntimeError::InvalidConstraintTarget(target_id))?;
        if !core_is_transform_component(
            objects[components[target as usize].obj_idx as usize].type_id.0) {
            return Err(RuntimeError::InvalidConstraintTarget(target_id))
        }
        if let Kind::Transform { bounds, .. } = &mut self.kind {
            let object = &objects[components[target as usize].obj_idx as usize];
            if matches!(object.type_id.0,
                object_ids::ARTBOARD | object_ids::LAYOUT_COMPONENT) {
                *bounds = Point {
                    x: float(object, property_ids::LAYOUTCOMPONENT_WIDTH)?,
                    y: float(object, property_ids::LAYOUTCOMPONENT_HEIGHT)?,
                };
            }
        }
        self.target = Some(target); Ok(())
    }

    pub fn set(&mut self, prop_id: u32, value: TrackValue) -> bool {
        match (prop_id, value) {
            (property_ids::CONSTRAINT_STRENGTH, TrackValue::Scalar(value)) =>
                self.strength = value,
            (property_ids::COPYFACTOR, TrackValue::Scalar(value)) =>
                self.factor_x = value,
            (property_ids::COPYFACTORY, TrackValue::Scalar(value)) =>
                self.factor_y = value,
            (property_ids::MINVALUE, TrackValue::Scalar(value)) =>
                self.min_x = value,
            (property_ids::MAXVALUE, TrackValue::Scalar(value)) =>
                self.max_x = value,
            (property_ids::MINVALUEY, TrackValue::Scalar(value)) =>
                self.min_y = value,
            (property_ids::MAXVALUEY, TrackValue::Scalar(value)) =>
                self.max_y = value,
            (property_ids::DOESCOPY, TrackValue::Bool(value)) => self.copy_x = value,
            (property_ids::DOESCOPYY, TrackValue::Bool(value)) => self.copy_y = value,
            (property_ids::TRANSFORMCOMPONENTCONSTRAINT_OFFSET, TrackValue::Bool(value)) =>
                self.offset = value,
            (property_ids::MIN, TrackValue::Bool(value)) => self.has_min_x = value,
            (property_ids::MAX, TrackValue::Bool(value)) => self.has_max_x = value,
            (property_ids::MINY, TrackValue::Bool(value)) => self.has_min_y = value,
            (property_ids::MAXY, TrackValue::Bool(value)) => self.has_max_y = value,
            (property_ids::SOURCESPACEVALUE, TrackValue::Uint(value)) =>
                self.source_local = value == 1,
            (property_ids::DESTSPACEVALUE, TrackValue::Uint(value)) =>
                self.dest_local = value == 1,
            (property_ids::MINMAXSPACEVALUE, TrackValue::Uint(value)) =>
                self.limits_local = value == 1,
            (property_ids::TRANSFORMCONSTRAINT_ORIGINX, TrackValue::Scalar(value)) =>
                if let Kind::Transform { origin, .. } = &mut self.kind {
                    origin.x = value
                } else { return false },
            (property_ids::TRANSFORMCONSTRAINT_ORIGINY, TrackValue::Scalar(value)) =>
                if let Kind::Transform { origin, .. } = &mut self.kind {
                    origin.y = value
                } else { return false },
            (property_ids::DISTANCECONSTRAINT_DISTANCE, TrackValue::Scalar(value)) =>
                if let Kind::Distance { distance, .. } = &mut self.kind {
                    *distance = value
                } else { return false },
            (property_ids::DISTANCECONSTRAINT_MODEVALUE, TrackValue::Uint(value)) =>
                if let Kind::Distance { mode, .. } = &mut self.kind {
                    *mode = value
                } else { return false },
            _ => return false,
        }   true
    }
}

pub(super) fn sort_constraints(components: &[Component],
    constraints: &mut Vec<u32>) -> Result<()> {
    let mut pending = vec![0u32; constraints.len()];
    let mut emitted = vec![false; constraints.len()];
    for (right_pos, &right) in constraints.iter().enumerate() {
        let right = components[right as usize].constraint().unwrap();
        for &left in constraints.iter() {
            if left == constraints[right_pos] { continue }
            let left = components[left as usize].constraint().unwrap();
            if depends_on(components, right, left) {
                pending[right_pos] += 1;
            }
        }
    }

    let mut sorted = Vec::with_capacity(constraints.len());
    while sorted.len() < constraints.len() {
        let Some((position, _)) = constraints.iter().enumerate()
            .find(|(position, _)| pending[*position] == 0 &&
                !emitted[*position]) else {
            let object = constraints.iter().enumerate()
                .find(|(position, _)| pending[*position] != 0)
                .map_or(0, |(_, &index)| components[index as usize].obj_idx);
            return Err(RuntimeError::ConstraintCycle(object))
        };
        let completed = constraints[position];
        emitted[position] = true;
        sorted.push(completed);
        let left = components[completed as usize].constraint().unwrap();
        for (right_pos, &right_index) in constraints.iter().enumerate() {
            if pending[right_pos] == 0 { continue }
            let right = components[right_index as usize].constraint().unwrap();
            if depends_on(components, right, left) {
                pending[right_pos] -= 1;
            }
        }
    }
    *constraints = sorted; Ok(())
}

pub(super) fn apply_constraints(components: &mut [Component], order: &[u32],
    constraints: &[u32], dirty: &mut [bool]) {
    for &index in constraints {
        let Some(constraint) = components[index as usize].constraint().copied() else { continue };
        if apply_constraint(components, constraint) {
            update_descendants(components, order, constraint.owner, dirty);
        }
    }
}

fn descends_from(components: &[Component], mut node: u32, ancestor: u32) -> bool {
    loop {
        if node == ancestor { return true }
        let Some(parent) = components[node as usize].parent else { return false };
        node = parent;
    }
}

fn depends_on(components: &[Component], right: &Constraint, left: &Constraint) -> bool {
    right.target.is_some_and(|target| descends_from(components, target, left.owner)) ||
        (right.owner != left.owner && descends_from(components, right.owner, left.owner))
}

fn apply_constraint(components: &mut [Component], constraint: Constraint) -> bool {
    let owner = constraint.owner as usize;
    let current = components[owner].world;
    if constraint.target.is_none() &&
        matches!(constraint.kind, Kind::Transform { .. } | Kind::Distance { .. }) {
        return false
    }
    let owner_parent = components[owner].parent
        .map_or(Affine2::default(), |parent| components[parent as usize].world);
    let source = if let Some(target) = constraint.target {
        let target = target as usize;
        let mut target_world = components[target].world;
        if let Kind::Transform { origin, bounds } = constraint.kind {
            target_world = target_world.then(Affine2 {
                tx: bounds.x * origin.x, ty: bounds.y * origin.y, ..Affine2::default()
            });
        }
        if constraint.source_local {
            let target_parent = components[target].parent
                .map_or(Affine2::default(), |parent| components[parent as usize].world);
            let Some(inverse) = inverse(target_parent) else { return false };
            inverse.then(target_world)
        } else { target_world }
    } else { current };
    let has_target = constraint.target.is_some();

    let next = match constraint.kind {
        Kind::Translation => constrain_translation(
            current, source, owner_parent, components[owner].transform, constraint, has_target),
        Kind::Rotation => constrain_rotation(
            current, source, owner_parent, components[owner].transform, constraint, has_target),
        Kind::Scale => constrain_scale(
            current, source, owner_parent, components[owner].transform, constraint, has_target),
        Kind::Transform { .. } => constrain_transform(current,
            if constraint.dest_local { owner_parent.then(source) } else { source },
            constraint.strength),
        Kind::Distance { distance, mode } =>
            constrain_distance(current, source, distance, mode, constraint.strength),
    };
    if next == current { return false }
    components[owner].world = next; true
}

fn constrain_transform(current: Affine2, target: Affine2, strength: f32) -> Affine2 {
    let (from, mut to) = (decompose(current), decompose(target));
    let tau = std::f32::consts::TAU;
    let mut difference = to.rotation.rem_euclid(tau) - from.rotation.rem_euclid(tau);
    if std::f32::consts::PI < difference { difference -= tau }
    else if difference < -std::f32::consts::PI { difference += tau }
    let inverse = 1.0 - strength;
    to.rotation = from.rotation + difference * strength;
    to.x = from.x * inverse + to.x * strength;
    to.y = from.y * inverse + to.y * strength;
    to.scale_x = from.scale_x * inverse + to.scale_x * strength;
    to.scale_y = from.scale_y * inverse + to.scale_y * strength;
    to.skew = from.skew * inverse + to.skew * strength;
    compose(to)
}

fn constrain_distance(current: Affine2, target: Affine2,
    distance: f32, mode: u32, strength: f32) -> Affine2 {
    let (dx, dy) = (current.tx - target.tx, current.ty - target.ty);
    let current_distance = dx.hypot(dy);
    if mode == 0 && current_distance < distance ||
       mode == 1 && distance < current_distance ||
       current_distance < 0.001 {
        return current
    }
    let scale = distance / current_distance;
    let (x, y) = (target.tx + dx * scale, target.ty + dy * scale);
    Affine2 { tx: current.tx + (x - current.tx) * strength,
        ty: current.ty + (y - current.ty) * strength, ..current }
}

fn constrain_translation(current: Affine2, source: Affine2, parent: Affine2,
    local: super::TransformValues, constraint: Constraint, has_target: bool) -> Affine2 {
    let mut target = Point { x: source.tx, y: source.ty };
    if has_target {
        if !constraint.copy_x {
            target.x = if constraint.dest_local { 0.0 } else { current.tx };
        } else {
            target.x *= constraint.factor_x;
            if constraint.offset { target.x += local.x }
        }
        if !constraint.copy_y {
            target.y = if constraint.dest_local { 0.0 } else { current.ty };
        } else {
            target.y *= constraint.factor_y;
            if constraint.offset { target.y += local.y }
        }
        if constraint.dest_local { target = parent.transform_point(target) }
    }
    let mut limited = if constraint.limits_local {
        inverse(parent).map_or(target, |inverse| inverse.transform_point(target))
    } else { target };
    limited.x = clamp(limited.x, constraint.has_min_x.then_some(constraint.min_x),
        constraint.has_max_x.then_some(constraint.max_x));
    limited.y = clamp(limited.y, constraint.has_min_y.then_some(constraint.min_y),
        constraint.has_max_y.then_some(constraint.max_y));
    if constraint.limits_local { limited = parent.transform_point(limited) }
    let strength = constraint.strength;
    Affine2 { tx: current.tx + (limited.x - current.tx) * strength,
        ty: current.ty + (limited.y - current.ty) * strength, ..current }
}

fn constrain_rotation(current: Affine2, source: Affine2, parent: Affine2,
    local: super::TransformValues, constraint: Constraint, has_target: bool) -> Affine2 {
    let current_parts = decompose(current);
    let mut target = decompose(source);
    if has_target {
        if !constraint.copy_x {
            target.rotation = if constraint.dest_local { 0.0 } else { current_parts.rotation };
        } else {
            target.rotation *= constraint.factor_x;
            if constraint.offset { target.rotation += local.rotation }
        }
        if constraint.dest_local { target = decompose(parent.then(compose(target))) }
    }
    if constraint.limits_local {
        let Some(inverse) = inverse(parent) else { return current };
        target = decompose(inverse.then(compose(target)));
    }
    target.rotation = clamp(target.rotation, constraint.has_min_x.then_some(constraint.min_x),
        constraint.has_max_x.then_some(constraint.max_x));
    if constraint.limits_local { target = decompose(parent.then(compose(target))) }
    let tau = std::f32::consts::TAU;
    let mut difference = target.rotation.rem_euclid(tau) -
        current_parts.rotation.rem_euclid(tau);
    if std::f32::consts::PI < difference { difference -= tau }
    else if difference < -std::f32::consts::PI { difference += tau }
    compose(Parts { rotation: current_parts.rotation + difference * constraint.strength,
        ..current_parts })
}

fn constrain_scale(current: Affine2, source: Affine2, parent: Affine2,
    local: super::TransformValues, constraint: Constraint, has_target: bool) -> Affine2 {
    let current_parts = decompose(current);
    let mut target = decompose(source);
    if has_target {
        if !constraint.copy_x {
            target.scale_x = if constraint.dest_local { 1.0 } else { current_parts.scale_x };
        } else {
            target.scale_x *= constraint.factor_x;
            if constraint.offset { target.scale_x *= local.scale_x }
        }
        if !constraint.copy_y {
            target.scale_y = if constraint.dest_local { 1.0 } else { current_parts.scale_y };
        } else {
            target.scale_y *= constraint.factor_y;
            if constraint.offset { target.scale_y *= local.scale_y }
        }
        if constraint.dest_local { target = decompose(parent.then(compose(target))) }
    }
    if constraint.limits_local {
        let Some(inverse) = inverse(parent) else { return current };
        target = decompose(inverse.then(compose(target)));
    }
    target.scale_x = clamp(target.scale_x, constraint.has_min_x.then_some(constraint.min_x),
        constraint.has_max_x.then_some(constraint.max_x));
    target.scale_y = clamp(target.scale_y, constraint.has_min_y.then_some(constraint.min_y),
        constraint.has_max_y.then_some(constraint.max_y));
    if constraint.limits_local { target = decompose(parent.then(compose(target))) }
    let strength = constraint.strength;
    compose(Parts {
        scale_x: current_parts.scale_x + (target.scale_x - current_parts.scale_x) * strength,
        scale_y: current_parts.scale_y + (target.scale_y - current_parts.scale_y) * strength,
        ..current_parts
    })
}

fn clamp(mut value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    if let Some(max) = max { value = value.min(max) }
    if let Some(min) = min { value = value.max(min) }   value
}

fn decompose(matrix: Affine2) -> Parts {
    let denom = matrix.xx * matrix.xx + matrix.yx * matrix.yx;
    let scale_x = denom.sqrt();
    Parts { x: matrix.tx, y: matrix.ty, scale_x,
        scale_y: if scale_x == 0.0 { 0.0 } else {
            (matrix.xx * matrix.yy - matrix.xy * matrix.yx) / scale_x
        },
        rotation: matrix.yx.atan2(matrix.xx),
        skew: (matrix.xx * matrix.xy + matrix.yx * matrix.yy).atan2(denom),
    }
}

fn compose(parts: Parts) -> Affine2 {
    let mut matrix = Affine2::from_transform(
        parts.x, parts.y, parts.rotation, parts.scale_x, parts.scale_y);
    if parts.skew != 0.0 {
        matrix.xy += matrix.xx * parts.skew;
        matrix.yy += matrix.yx * parts.skew;
    }   matrix
}

fn inverse(matrix: Affine2) -> Option<Affine2> {
    let determinant = matrix.xx * matrix.yy - matrix.yx * matrix.xy;
    if determinant == 0.0 { return None }
    let inverse = determinant.recip();
    Some(Affine2 {
        xx: matrix.yy * inverse, yx: -matrix.yx * inverse,
        xy: -matrix.xy * inverse, yy: matrix.xx * inverse,
        tx: (matrix.xy * matrix.ty - matrix.yy * matrix.tx) * inverse,
        ty: (matrix.yx * matrix.tx - matrix.xx * matrix.ty) * inverse,
    })
}

fn update_descendants(components: &mut [Component], order: &[u32], owner: u32,
    dirty: &mut [bool]) {
    dirty.fill(false);
    dirty[owner as usize] = true;
    for &index in order {
        if index == owner { continue }
        let component = &components[index as usize];
        let Some(parent) = component.parent else { continue };
        if !dirty[parent as usize] { continue }
        let world = components[parent as usize].world.then(component.transform.affine());
        let opacity = (components[parent as usize].world_opacity *
            component.transform.opacity).clamp(0.0, 1.0);
        components[index as usize].world = world;
        components[index as usize].world_opacity = opacity;
        dirty[index as usize] = true;
    }
}
