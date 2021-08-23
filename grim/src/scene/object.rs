use crate::{SystemInfo};
use crate::io::MemoryStream;
use crate::scene::*;

pub enum Object {
    Draw(DrawObject),
    Group(GroupObject),
    Mat(MatObject),
    Mesh(MeshObject),
    Tex(Tex),
    Trans(TransObject),
    Packed(PackedObject),
}

#[derive(Debug)]
pub struct PackedObject {
    pub name: String,
    pub object_type: String,
    pub data: Vec<u8>
}

impl Object {
    pub fn get_name(&self) -> &str {
        match self {
            Object::Draw(draw) => &draw.name,
            Object::Group(grp) => &grp.name,
            Object::Mat(mat) => &mat.name,
            Object::Mesh(mesh) => &mesh.name,
            Object::Tex(tex) => &tex.name,
            Object::Trans(trans) => &trans.name,
            Object::Packed(packed) => &packed.name,
        }
    }

    pub fn get_type(&self) -> &str {
        match self {
            Object::Draw(_) => "Draw",
            Object::Group(_) => "Group",
            Object::Mat(_) => "Mat",
            Object::Mesh(_) => "Mesh",
            Object::Tex(_) => "Tex",
            Object::Trans(_) => "Trans",
            Object::Packed(packed) => &packed.object_type,
        }
    }

    pub fn unpack(&self, info: &SystemInfo) -> Option<Object> {
        match self {
            Object::Packed(packed) => {
                let mut stream = MemoryStream::from_slice_as_read(packed.data.as_slice());

                match packed.object_type.as_str() {
                    "Draw" => {
                        let mut draw = DrawObject::default();

                        if draw.load(&mut stream, info).is_ok() {
                            draw.name = packed.name.to_owned();
                            Some(Object::Draw(draw))
                        } else {
                            None
                        }
                    },
                    "Mat" => {
                        let mut mat = MatObject::default();

                        if mat.load(&mut stream, info).is_ok() {
                            mat.name = packed.name.to_owned();
                            Some(Object::Mat(mat))
                        } else {
                            None
                        }
                    },
                    "Tex" => {
                        match Tex::from_stream(&mut stream, info) {
                            Ok(mut tex) => {
                                tex.name = packed.name.to_owned();
                                Some(Object::Tex(tex))
                            },
                            Err(_) => None,
                        }
                    },
                    "Trans" => {
                        let mut trans = TransObject::default();

                        if trans.load(&mut stream, info).is_ok() {
                            trans.name = packed.name.to_owned();
                            Some(Object::Trans(trans))
                        } else {
                            None
                        }
                    },
                    _ => None
                }
            },
            _ => None
        }
    }
}