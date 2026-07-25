
use std::{error::Error as StdError, fmt};

use super::{display_list::{Affine2, CornerRadii, DisplayList, Geometry, Primitive, Rect},
    decode::{self, core_is_component, core_is_transform_component, object_ids,
        property_ids, DecodeError, Object, RiveFile},
};

pub type Result<T> = std::result::Result<T, RuntimeError>;

#[derive(Debug)] pub enum RuntimeError {
    Decode(DecodeError), TooManyObjects, ParentCycle(u32),
    InvalidParent { comp_id: u32, parent_id: u32 },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self {
        Self::Decode(error) => error.fmt(f),
        Self::TooManyObjects => f.write_str("Rive object count exceeds u32"),
        Self::InvalidParent { comp_id, parent_id } =>
            write!(f, "component {comp_id} references missing parent {parent_id}"),
        Self::ParentCycle(comp_id) => write!(f, "component parent cycle at {comp_id}"),
    } }
}

impl StdError for RuntimeError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self { Self::Decode(error) => Some(error), _ => None }
    }
}

impl From<DecodeError> for RuntimeError {
    fn from(error: DecodeError) -> Self { Self::Decode(error) }
}

#[derive(Debug, Clone)] struct Component {
    obj_idx: u32, parent: Option<u32>,
    geometry: Option<Geometry>,
    local: Affine2,
    world: Affine2,
}

/// Retained Rive scene state.
///
/// The first implementation resolves component transforms and emits static ellipse and
/// rectangle geometry. Animation, constraints, paints, clipping, text and state machines
/// can update this retained state without changing the display-list API.
#[derive(Debug)] pub struct Runtime {
    file: RiveFile, elapsed: f32,
    components: Vec<Component>,
    update_order: Vec<u32>,
}

impl Runtime {
    pub fn from_file(file: RiveFile) -> Result<Self> {
        let (mut components, mut parent_ids) = (Vec::new(), Vec::new());

        for (obj_idx, object) in file.ocoll.iter().enumerate() {
            if !core_is_component(object.type_id.0) { continue }
            let obj_idx =  u32::try_from(obj_idx).map_err(|_| RuntimeError::TooManyObjects)?;
            if  obj_idx == u32::MAX { return Err(RuntimeError::TooManyObjects) }

            let parent_id = object.varuint(property_ids::COMPONENT_PARENTID)?.unwrap_or(0);
            let geometry = match object.type_id.0 {
                object_ids::ELLIPSE => Some(Geometry::Ellipse(bounds(object)?)),
                object_ids::RECTANGLE => Some(Geometry::RoundedRect {
                    rect: bounds(object)?, radii: rectangle_radii(object)?,
                }), _ => None,
            };
            components.push(Component { obj_idx, parent: None,
                local: local_transform(object)?, world: Affine2::default(), geometry });
            parent_ids.push(parent_id);
        }

        for (index, parent_id) in parent_ids.into_iter().enumerate() {
            if  parent_id == 0 { continue }
            let parent = parent_id as usize - 1;
            if  components.len() <= parent {
                return Err(RuntimeError::InvalidParent {
                    comp_id: components[index].obj_idx + 1, parent_id })
            }   components[index].parent = Some(parent as u32);
        }

        let mut runtime = Self { file, components, update_order: Vec::new(), elapsed: 0.0 };
        runtime.validate_hierarchy()?;
        runtime.update_world_transforms();
        Ok(runtime)
    }

    pub fn file(&self) -> &RiveFile { &self.file }
    pub fn elapsed(&self) -> f32 { self.elapsed }
    pub fn component_count(&self) -> usize { self.components.len() }

    pub fn advance(&mut self, delta_seconds: f32) {
        self.elapsed += delta_seconds.max(0.0);
        self.update_world_transforms();
    }

    pub fn display_list(&self) -> DisplayList {
        let mut list = DisplayList::default();
        for component in &self.components {
            if let Some(geometry) = component.geometry {
                list.primitives.push(Primitive {
                    obj_idx: component.obj_idx, transform: component.world, geometry });
            }
        }   list
    }

    fn validate_hierarchy(&mut self) -> Result<()> {
        let mut state = vec![0u8; self.components.len()];
        for index in 0..self.components.len() {
            self.visit_component(index, &mut state)?;
        }   Ok(())
    }

    fn visit_component(&mut self, index: usize, state: &mut [u8]) -> Result<()> {
        match state[index] {
            2 => return Ok(()),
            1 => return Err(RuntimeError::ParentCycle(self.components[index].obj_idx + 1)),
            _ => state[index] = 1,
        }
        if let Some(parent) = self.components[index].parent {
            self.visit_component(parent as usize, state)?;
        }
        state[index] = 2;
        self.update_order.push(index as u32);
        Ok(())
    }

    fn update_world_transforms(&mut self) {
        for &index in &self.update_order {
            let index = index as usize;
            let component = &self.components[index];
            let world = component.parent.map_or(component.local, |parent|
                    self.components[parent as usize].world.then(component.local));
            self.components[index].world = world;
        }
    }
}

fn local_transform(object: &Object) -> decode::Result<Affine2> {
    if !core_is_transform_component(object.type_id.0) {
        return Ok(Affine2::default())
    }
    Ok(Affine2::from_transform(
        object.float(property_ids::NODE_X)?.unwrap_or(0.0),
        object.float(property_ids::NODE_Y)?.unwrap_or(0.0),
        object.float(property_ids::TRANSFORMCOMPONENT_ROTATION)?.unwrap_or(0.0),
        object.float(property_ids::TRANSFORMCOMPONENT_SCALEX)?.unwrap_or(1.0),
        object.float(property_ids::TRANSFORMCOMPONENT_SCALEY)?.unwrap_or(1.0),
    ))
}

fn bounds(object: &Object) -> decode::Result<Rect> {
    let width  = object.float(property_ids::PARAMETRICPATH_WIDTH)?.unwrap_or(0.0);
    let height = object.float(property_ids::PARAMETRICPATH_HEIGHT)?.unwrap_or(0.0);
    let origin_x = object.float(property_ids::PARAMETRICPATH_ORIGINX)?.unwrap_or(0.5);
    let origin_y = object.float(property_ids::PARAMETRICPATH_ORIGINY)?.unwrap_or(0.5);
    Ok(Rect { x: -width * origin_x, y: -height * origin_y, width, height, })
}

fn rectangle_radii(object: &Object) -> decode::Result<CornerRadii> {
    let top_left = object.float(property_ids::RECTANGLE_CORNERRADIUSTL)?.unwrap_or(0.0);
    let linked = object.boolean(property_ids::RECTANGLE_LINKCORNERRADIUS)?.unwrap_or(true);
    let radius = |prop_id| object.float(prop_id).map(|value| value.unwrap_or(0.0));
    Ok(if linked { CornerRadii { top_left, top_right: top_left,
            bottom_right: top_left, bottom_left: top_left,
    } } else { CornerRadii { top_left,
            top_right: radius(property_ids::RECTANGLE_CORNERRADIUSTR)?,
            bottom_right: radius(property_ids::RECTANGLE_CORNERRADIUSBR)?,
            bottom_left: radius(property_ids::RECTANGLE_CORNERRADIUSBL)?,
    } })
}

#[cfg(test)] mod tests { use super::*;
    use crate::rive::decode::{FieldValue, Header, VarUInt};
    use std::io::Cursor;

    fn file(objects: Vec<Object>) -> RiveFile { RiveFile {
            header: Header {
                majorv: VarUInt(1), minorv: VarUInt(0),
                fileid: VarUInt(0), toc: Vec::new(),
            },  ocoll: objects,
    } }

    fn prop(object: &mut Object, id: u32, value: f32) {
        object.add_prop(VarUInt(id), FieldValue::Float32(value));
    }

    #[test] fn emits_static_geometry_with_retained_parent_transforms() {
        let mut parent = Object::new_simple(object_ids::NODE);
        prop(&mut parent, property_ids::NODE_X, 10.0);
        prop(&mut parent, property_ids::NODE_Y, 20.0);

        let mut ellipse = Object::new_simple(object_ids::ELLIPSE);
        ellipse.add_prop(VarUInt(property_ids::COMPONENT_PARENTID),
            FieldValue::VarUInt(VarUInt(1)));
        prop(&mut ellipse, property_ids::NODE_X, 5.0);
        prop(&mut ellipse, property_ids::PARAMETRICPATH_WIDTH, 40.0);
        prop(&mut ellipse, property_ids::PARAMETRICPATH_HEIGHT, 20.0);

        let runtime = Runtime::from_file(file(vec![parent, ellipse])).unwrap();
        let list = runtime.display_list();
        assert_eq!(runtime.component_count(), 2);
        assert_eq!(list.primitives.len(), 1);
        assert_eq!(list.primitives[0].transform.tx, 15.0);
        assert_eq!(list.primitives[0].transform.ty, 20.0);
        assert_eq!(list.primitives[0].geometry,
            Geometry::Ellipse(Rect { x: -20.0, y: -10.0, width: 40.0, height: 20.0 }));
    }

    #[test] fn rectangle_defaults_to_linked_corner_radii() {
        let mut rectangle = Object::new_simple(object_ids::RECTANGLE);
        prop(&mut rectangle, property_ids::PARAMETRICPATH_WIDTH, 20.0);
        prop(&mut rectangle, property_ids::PARAMETRICPATH_HEIGHT, 10.0);
        prop(&mut rectangle, property_ids::RECTANGLE_CORNERRADIUSTL, 3.0);

        let runtime = Runtime::from_file(file(vec![rectangle])).unwrap();
        let Geometry::RoundedRect { radii, .. } =
            &runtime.display_list().primitives[0].geometry else { panic!() };
        assert_eq!(*radii, CornerRadii {
            top_left: 3.0, top_right: 3.0, bottom_right: 3.0, bottom_left: 3.0,
        });
    }

    #[test] fn resolves_parent_ids_through_component_indices() {
        let ignored = Object::new_simple(u32::MAX);
        let root = Object::new_simple(object_ids::NODE);
        let mut parent = Object::new_simple(object_ids::NODE);
        prop(&mut parent, property_ids::NODE_X, 10.0);
        let mut ellipse = Object::new_simple(object_ids::ELLIPSE);
        ellipse.add_prop(VarUInt(property_ids::COMPONENT_PARENTID),
            FieldValue::VarUInt(VarUInt(2)));
        prop(&mut ellipse, property_ids::NODE_X, 5.0);

        let runtime = Runtime::from_file(file(vec![ignored, root, parent, ellipse])).unwrap();
        assert_eq!(runtime.display_list().primitives[0].transform.tx, 15.0);
    }

    #[test] fn rejects_invalid_geometry_during_construction() {
        let mut ellipse = Object::new_simple(object_ids::ELLIPSE);
        ellipse.add_prop(VarUInt(property_ids::PARAMETRICPATH_WIDTH),
            FieldValue::VarUInt(VarUInt(10)));

        assert!(matches!(Runtime::from_file(file(vec![ellipse])),
            Err(RuntimeError::Decode(DecodeError::PropTypeMismatch { .. }))));
    }

    #[test] fn rejects_parent_cycles_during_construction() {
        let root = Object::new_simple(object_ids::NODE);
        let mut first = Object::new_simple(object_ids::NODE);
        first.add_prop(VarUInt(property_ids::COMPONENT_PARENTID),
            FieldValue::VarUInt(VarUInt(3)));
        let mut second = Object::new_simple(object_ids::NODE);
        second.add_prop(VarUInt(property_ids::COMPONENT_PARENTID),
            FieldValue::VarUInt(VarUInt(2)));

        assert!(matches!(Runtime::from_file(file(vec![root, first, second])),
            Err(RuntimeError::ParentCycle(2 | 3))));
    }

    #[test] fn imports_repository_sample() {
        let mut input = Cursor::new(include_bytes!("../../data/rating-animation.riv"));
        let file = RiveFile::read(&mut input).unwrap();
        let runtime = Runtime::from_file(file).unwrap();
        assert!(0 < runtime.component_count());
        runtime.display_list();
    }
}
