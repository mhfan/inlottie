
use std::io::{Read, Result, Error, ErrorKind::InvalidData, Seek};

/// ## Rive runtime format:
/// Binary representation of Artboards, Shapes, Animations, State Machines, etc.
/// The format was designed to provide a balance of quick load times, small file sizes,
/// and flexibility with regards to future changes/addition of features.
/// https://rive.app/community/doc/format/docxcTF9lJxR
///
/// ### Binary Types:
/// A binary reader for Rive runtime files needs to be able to read these data types
/// from the stream. **Byte order is little endian.**
///
/// - varuint ([LEB128](https://en.wikipedia.org/wiki/LEB128) variable encoded unsigned integer)
/// - string (u32 followed by utf-8 encoded byte array of provided length)
/// - u32, f32
///
/// https://github.com/rive-app/rive-runtime/blob/master/src/core/binary_reader.cpp
///
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct VarUInt(pub u32); // u64/u128?

impl VarUInt {
    #[inline] pub fn new(value: u32) -> Self { Self(value) }
    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let (mut val, mut cnt) = (0u32, 0);
        while cnt < (std::mem::size_of::<VarUInt>() * 8) as u32 {
            let mut buf = [0u8];
            reader.read_exact(&mut buf)?;
            if cnt == 28 && 0x0f < buf[0] {
                return Err(Error::new(InvalidData, "VarUInt too large for u32"));
            }
            val |= ((buf[0] & 0x7F) as u32) << cnt;
            if buf[0] < 0x80 { return Ok(Self(val)) }   cnt += 7;
        }   Err(Error::new(InvalidData, "VarUInt too large for u32"))
    }
}

pub struct BinaryReader<R: Read> { reader: R, }

impl<R: Read> BinaryReader<R> {
    #[inline] pub fn new(reader: R) -> Self { Self { reader } }
    #[inline] pub fn read_varuint(&mut self) -> Result<VarUInt> {
        VarUInt::read(&mut self.reader)
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        let mut buffer = [0u8; 4];
        self.reader.read_exact(&mut buffer)?;
        Ok(u32::from_le_bytes(buffer))
    }

    pub fn read_f32(&mut self) -> Result<f32> {
        let mut buffer = [0u8; 4];
        self.reader.read_exact(&mut buffer)?;
        Ok(f32::from_le_bytes(buffer))
    }

    pub fn read_bytes(&mut self) -> Result<Vec<u8>> {
        let length = self.read_varuint()?.0 as usize;
        let mut buffer = vec![0u8; length];
        self.reader.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    pub fn read_magic(&mut self) -> Result<()> {
        let mut magic = [0u8; 4];
        self.reader.read_exact(&mut magic)?;

        if magic != *b"RIVE" {
            return Err(Error::new(InvalidData, "Invalid Rive file magic number"));
        }   Ok(())
    }
}

/// ### Header:
/// A ToC (table of contents/field definition) is provided which allows the runtime to
/// understand how it can skip over properties and objects it may not understand. This is
/// part of what makes the format resilient to future changes/feature additions to the editor.
/// An older runtime can at least attempt to load an older file and display it without
/// the objects and properties it doesn't understand.
pub struct Header {
    //pub magic: [u8; 4], // Fingerprint: 0x52 0x49 0x56 0x45 / "RIVE"
    /// Major versions are not cross-compatible.
    pub majorv: VarUInt,
    /// Minor version changes are compatible with each other provided the major version is
    /// the same. However, certain newer features may not be available if the runtime is of
    /// a different minor version.
    pub minorv: VarUInt,
    /// a unique identifier for the file that in the future will be able to
    /// be used to distinguish the file
    pub fileid: VarUInt,

    /// The Table of Contents section of the header is a list of the properties in the file
    /// along with their backing type. This allows the runtime to read past properties it
    /// wishes to skip or doesn't understand. It does this by providing the backing type
    /// for each property ID.
    ///
    /// The list of known properties is serialized as a sequence of variable unsigned integers
    /// with a 0 terminator. A valid property key is distinguished by a non-zero unsigned
    /// integer id/key. Following the properties is a bit array which is composed of the read
    /// property count / 4 bytes. Every property gets 2 bits to define which backing type
    /// deserializer can be used to read past it.
    pub toc: HashMap<VarUInt, FieldType>,
}   use std::collections::HashMap;

impl Header {
    pub fn read<R: Read>(reader: &mut BinaryReader<R>) -> Result<Self> {
        let majorv = reader.read_varuint()?;
        let minorv = reader.read_varuint()?;
        let fileid = reader.read_varuint()?;

        let mut prop_keys = Vec::new();
        loop {  let  key = reader.read_varuint()?;
            if  key.0 == 0 { break }   prop_keys.push(key);
        }

        let mut toc = create_core_toc(); // XXX:
        //let mut toc = HashMap::with_capacity(prop_keys.len());
        let (mut current_uint, mut bit_position) = (None, 0);

        for key in &prop_keys {
            if  current_uint.is_none() || bit_position > 30 {
                current_uint = Some(reader.read_u32()?);
                bit_position = 0;
            }

            if let Some(uint_value) = current_uint {
                let field_index = ((uint_value >> bit_position) & 0x03) as u8;
                let field_type = unsafe {
                    std::mem::transmute::<u8, FieldType>(field_index) };
                //if !toc.contains_key(key) { toc.insert(*key, field_type); }
                toc.entry(*key).or_insert(field_type);  bit_position += 2;
            }
        }       Ok(Self { majorv, minorv, fileid, toc })
    }

    #[inline] pub fn get_prop_type(&self, prop_key: VarUInt) -> Option<FieldType> {
        self.toc.get(&prop_key).copied()
    }
}

/// ### Field Types:
/// There are 5 fundamental backing types but they are serialized in 4 different ways.
/// Knowing how the type is serialized allows the runtime to know how to read it in.
/// Even if it reads the wrong value or interprets it incorrectly, the important aspect
/// is being able to read past it so the rest of the file can be read in safely.
///
/// For example, a boolean can be read as an unsigned integer as the backing type and
/// serializer is compatible. Even though reading the boolean as an integer will not
/// provide the valid value for the property, the runtime can still just read past it.
#[derive(Debug, Clone, Copy, PartialEq)] #[repr(u8)]
pub enum FieldType { UIntBool = 0, String, Float, Color } // 1 byte, 2 bits used

/// ## Content:
/// The rest of the file is simply a list of objects, each containing a list of their
/// properties and values. An object is represented as a varuint type key. It is immediately
/// followed by the list of properties. Properties are terminated with a 0 varuint. If a non 0
/// value is read, it is expected to the type key for the property. If the runtime knows the
/// type key, it will know the backing type and how to decode it. The bytes following the type
/// key will be one of the binary types specified earlier. If it is unknown, it can determine
/// from the ToC what the backing type is and read past it.
#[derive(Debug, PartialEq)] pub enum FieldValue {
    VarUInt(VarUInt), Bytes(Vec<u8>), Float32(f32), Color(u32),
}

impl FieldValue {
    pub fn read_with_type<R: Read>(reader: &mut BinaryReader<R>,
        field_type: FieldType) -> Result<Self> {
        match field_type {
            FieldType::UIntBool => { Ok(Self::VarUInt(reader.read_varuint()?)) },
            FieldType::String   => { Ok(Self::Bytes  (reader.read_bytes()?)) },
            FieldType::Float => { Ok(Self::Float32(reader.read_f32()?)) },
            FieldType::Color => { Ok(Self::Color  (reader.read_u32()?)) },
        }
    }

    #[inline] pub fn get_type(&self) -> FieldType { match self {
        Self::VarUInt(_) => FieldType::UIntBool,
        Self::Float32(_) => FieldType::Float,
        Self::Bytes(_)   => FieldType::String,
        Self::Color(_)   => FieldType::Color,
    } }
}

/// Example Serialized Object:
/// Data    Type/Size       Description
/// 2       varuint         object of type 2 (Node)
/// 13      varuint         X  property for the Node
/// 100.0   4 byte float    the X value for the Node
/// 14      varuint         Y  property for the Node
/// 22.0    4 byte float    the Y value for the Node
/// 0       varuint         Null terminator.
pub struct Object { pub type_id: VarUInt, pub props: Vec<(VarUInt, FieldValue)>, }

impl Object {
    pub fn read_with_header<R: Read>(reader: &mut BinaryReader<R>,
        header: &Header) -> Result<Self> {
        let type_id = reader.read_varuint()?;
        let mut props = Vec::new();

        while let Ok(prop_id) = reader.read_varuint() {
            if  prop_id.0 == 0 { break }

            let prop_value = match header.get_prop_type(prop_id) {
                Some(field_type) => FieldValue::read_with_type(reader, field_type)?,
                None => return Err(Error::new(InvalidData, format!(
                    "unknown property id {} of object type {}", prop_id.0, type_id.0))),
            };  props.push((prop_id, prop_value));
        }       Ok(Self { type_id, props })
    }

    pub fn new_simple(type_id: u32) -> Self {
        Self { props: Vec::new(), type_id: VarUInt::new(type_id), }
    }

    pub fn add_prop(&mut self, prop_id: VarUInt, value: FieldValue) {
        self.props.push((prop_id, value));
    }

    pub fn get_prop(&self, prop_id: u32) -> Option<&FieldValue> {
        self.props.iter().find(|(id, _)|
            id.0 == prop_id).map(|(_, value)| value)
    }
}

/// ## Core:
/// All objects and properties are defined in a set of files we call core defs for
/// [Core Definitions](https://github.com/rive-app/rive-runtime/tree/master/dev/defs).
/// These are defined in a series of JSON objects and help Rive generate serialization,
/// deserialization, and animation property code. The C++ and Flutter runtimes both have
/// helpers to read and generate a lot of the boilerplate code for these types.
///
/// ### Object:
/// A core object is represented by its Core type key. For example, a Shape has core type key 3.
/// Similarly you can see the generated code for the C++ runtime also identifies a Shape with
/// the same key.
///
/// ### Properties:
/// Properties are similarly represented by a Core type key. These are unique across all objects,
/// so property key 13 will always be the X value of a Node object, and it matches in the
/// runtime. A Node's X value is known to be a floating point value so when it is encountered
/// it will be decoded as such. Property key 0 is reserved as a null terminator (meaning we are
/// done reading properties for the current object).
//
/// ## Context:
/// Objects are always provided in context of each other. A Shape will always be provided after
/// an Artboard. The Node's artboard can always be determined by finding the latest read
/// Artboard. This concept is used extensively to provide the context for objects that require
/// it. Another example, a KeyFrame will always be provided after a LinearAnimation, meaning
/// you can always determine which LinearAnimation a KeyFrame belongs to by simply tracking
/// that last read LinearAnimation.
///
/// ## Hierarchy:
/// Objects inside the Artboard can be parented to other objects in the Artboard. This mapping
/// is more complex and requires identifiers to find the parent. The identifiers are provided
/// as a core def property. The value is always an unsigned integer representing the index
/// within the Artboard of the ContainerComponent derived object that makes a valid parent.
///
/// https://github.com/rive-app/rive-runtime/src
pub struct RiveFile { pub header: Header, pub ocoll: Vec<Object>, }

impl RiveFile {
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self> {
        let mut binary_reader = BinaryReader::new(reader);
        let mut ocoll = Vec::new();

        binary_reader.read_magic()?;
        let header = Header::read(&mut binary_reader)?;
        loop {
            match Object::read_with_header(&mut binary_reader, &header) {
                Ok(object) => ocoll.push(object),
                Err(e) => match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => break,
                    _ => return Err(Error::new(e.kind(), format!("failed to read object: {e}"))),
                }
            }
        }       Ok(Self { header, ocoll })
    }
}

include!("defs.rs");    // cargo r --bin parse_rive_defs
