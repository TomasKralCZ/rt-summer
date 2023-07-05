use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use eyre::{eyre, Result};
use flate2::read::GzDecoder;
use glam::{Vec2, Vec3};
use ply_rs::ply;
use smallvec::SmallVec;

use super::{params::ListParamValue, ListParam, TriMesh, Value};

// Source stolen from:
// https://github.com/beltegeuse/pbrt_rs/blob/master/src/ply.rs
// we need a better PLY library...

pub struct PlyFace {
    pub indices: SmallVec<[i32; 4]>,
}

impl ply::PropertyAccess for PlyFace {
    fn new() -> Self {
        PlyFace {
            indices: SmallVec::new(),
        }
    }
    fn set_property(&mut self, key: String, property: ply::Property) {
        match (key.as_ref(), property) {
            ("vertex_indices", ply::Property::ListInt(vec)) => {
                if vec.len() != 3 && vec.len() != 4 {
                    eprintln!("Weird PLY face lenght: '{}'", vec.len());
                    return;
                }

                for i in vec {
                    self.indices.push(i);
                }
            }
            ("vertex_indices", ply::Property::ListUInt(vec)) => {
                if vec.len() != 3 && vec.len() != 4 {
                    eprintln!("Weird PLY face lenght: '{}'", vec.len());
                    return;
                }

                for i in vec {
                    self.indices.push(i as i32);
                }
            }
            ("vertex_indices", ply::Property::ListUChar(vec)) => {
                if vec.len() != 3 && vec.len() != 4 {
                    eprintln!("Weird PLY face lenght: '{}'", vec.len());
                    return;
                }

                for i in vec {
                    self.indices.push(i as i32);
                }
            }
            (k, _) => eprintln!("Face: Unexpected key/value combination: key: {}", k),
        }
    }
}

pub struct PlyVertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub has_normal: bool,
    pub has_uv: bool,
}

impl ply::PropertyAccess for PlyVertex {
    fn new() -> Self {
        PlyVertex {
            pos: Vec3::new(0.0, 0.0, 0.0),
            normal: Vec3::new(0.0, 0.0, 0.0),
            uv: Vec2::new(0.0, 0.0),
            has_normal: false,
            has_uv: false,
        }
    }

    fn set_property(&mut self, key: String, property: ply::Property) {
        match (key.as_ref(), property) {
            ("x", ply::Property::Float(v)) => self.pos.x = v,
            ("y", ply::Property::Float(v)) => self.pos.y = v,
            ("z", ply::Property::Float(v)) => self.pos.z = v,
            ("nx", ply::Property::Float(v)) => {
                self.has_normal = true;
                self.normal.x = v
            }
            ("ny", ply::Property::Float(v)) => {
                self.has_normal = true;
                self.normal.y = v
            }
            ("nz", ply::Property::Float(v)) => {
                self.has_normal = true;
                self.normal.z = v
            }
            ("u", ply::Property::Float(v)) => {
                self.has_uv = true;
                self.uv.x = v
            }
            ("v", ply::Property::Float(v)) => {
                self.has_uv = true;
                self.uv.y = v
            }
            ("s", ply::Property::Float(v)) => {
                self.has_uv = true;
                self.uv.x = v
            }
            ("t", ply::Property::Float(v)) => {
                self.has_uv = true;
                self.uv.y = v
            }
            ("texture_u", ply::Property::Float(v)) => {
                self.has_uv = true;
                self.uv.x = v
            }
            ("texture_v", ply::Property::Float(v)) => {
                self.has_uv = true;
                self.uv.y = v
            }
            ("texture_s", ply::Property::Float(v)) => {
                self.has_uv = true;
                self.uv.x = v
            }
            ("texture_t", ply::Property::Float(v)) => {
                self.has_uv = true;
                self.uv.y = v
            }
            (k, _) => eprintln!("Face: Unexpected key/value combination: key: {}", k),
        }
    }
}

pub(super) fn parse_plymesh(file_directory: &Path, params: &[ListParam]) -> Result<TriMesh> {
    let mut indices: Vec<i32> = Vec::new();
    let mut points: Option<Vec<Vec3>> = None;
    let mut normals: Option<Vec<Vec3>> = None;
    let mut _tangents: Option<Vec<Vec3>> = None;
    let mut uvs: Option<Vec<Vec2>> = None;

    for p in params {
        match (p.name, &p.value) {
            ("filename", ListParamValue::Single(Value::String(filepath))) => {
                // TODO: couldn't extract this into it's own function becuase read_ply() returns non-exported type
                let mut path = PathBuf::from(file_directory);
                path.push(filepath);

                let ply_file = File::open(&path)?;
                let mut reader = BufReader::new(ply_file);

                let mut ply = Vec::new();

                if let Some(Some("gz")) = &path.extension().map(|ext| ext.to_str()) {
                    let mut decoder = GzDecoder::new(reader);
                    decoder.read_to_end(&mut ply)?;
                } else {
                    reader.read_to_end(&mut ply)?;
                }

                let p = ply_rs::parser::Parser::<ply::DefaultElement>::new();
                let mut reader = ply.as_slice();
                let ply_header = p.read_header(&mut reader)?;

                let vertex_parser = ply_rs::parser::Parser::<PlyVertex>::new();
                let face_parser = ply_rs::parser::Parser::<PlyFace>::new();

                for (_ignore_key, element) in &ply_header.elements {
                    match element.name.as_ref() {
                        "vertex" => {
                            let vertices = vertex_parser.read_payload_for_element(
                                &mut reader,
                                element,
                                &ply_header,
                            )?;

                            if !vertices.is_empty() {
                                points = Some(vertices.iter().map(|v| v.pos).collect());

                                if vertices[0].has_normal {
                                    normals = Some(vertices.iter().map(|v| v.normal).collect())
                                }
                                if vertices[0].has_uv {
                                    uvs = Some(vertices.iter().map(|v| v.uv).collect())
                                }
                            }
                        }
                        "face" => {
                            let faces = face_parser.read_payload_for_element(
                                &mut reader,
                                element,
                                &ply_header,
                            )?;

                            for face in faces {
                                if face.indices.len() == 3 {
                                    indices.extend_from_slice(&face.indices);
                                } else {
                                    eprintln!("PLY face with 4 vertices - not implemented yet");
                                }
                            }

                            for i in &indices {
                                if *i < 0 {
                                    eprintln!("PLY index is less than 0");
                                }
                            }

                            if indices.len() % 3 != 0 {
                                return Err(eyre!("Index buffer length is not a multiple of 3"));
                            }
                        }
                        e => panic!("Enexpeced element: '{}'", e),
                    }
                }
            }
            p => return Err(eyre!("Unexpected PLY mesh param: '{:?}'", p)),
        }
    }

    if let Some(normals) = &normals {
        for n in normals {
            if !n.is_normalized() {
                return Err(eyre!("PLY normal is not normalized"));
            }
        }
    }

    let (indices, vertices) = match (indices.len(), points) {
        (0, Some(vertices)) if vertices.len() == 3 => {
            let indices = vec![1, 2, 3];
            (indices, vertices)
        }
        (len, Some(vertices)) if len >= 3 => (indices, vertices),
        _ => return Err(eyre!("Triangle mesh vertices or indices not specified")),
    };

    Ok(TriMesh {
        indices,
        pos: vertices,
        normals,
        tangents: None,
        uvs,
    })
}
