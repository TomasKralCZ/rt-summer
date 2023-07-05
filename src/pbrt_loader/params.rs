use eyre::{eyre, Result};
use glam::{Vec2, Vec3};
use smallvec::SmallVec;

use super::Int;

// TODO: investigate if HashMap should be used insted. But it needs to be ordered...
pub struct ParamList<'t> {
    params: SmallVec<[ListParam<'t>; 4]>,
    index: usize,
}

impl<'t> ParamList<'t> {
    pub fn new(params: SmallVec<[ListParam<'t>; 4]>) -> Self {
        Self { params, index: 0 }
    }

    pub fn expect_simple(&mut self) -> Result<&'t str> {
        let param = self
            .params
            .get(self.index)
            .ok_or_else(|| eyre!("Param not in param list"))?;

        self.index += 1;

        match param.value {
            ListParamValue::Empty => Ok(param.name),
            _ => return Err(eyre!("Expected simple param, got '{:?}'", param)),
        }
    }

    pub fn next_param(&mut self) -> Result<&ListParam<'t>> {
        let p = self
            .params
            .get(self.index)
            .ok_or_else(|| eyre!("Param not in param list"));

        self.index += 1;
        p
    }

    pub fn params(&self) -> &[ListParam<'t>] {
        &self.params[self.index..]
    }

    pub fn get(&self, name: &str) -> Option<&ListParam<'t>> {
        self.params[self.index..].iter().find(|p| p.name == name)
    }
}

#[derive(Debug)]
pub struct ListParam<'t> {
    pub name: &'t str,
    pub value: ListParamValue<'t>,
}

impl<'t> ListParam<'t> {
    pub fn new(name: &'t str, value: ListParamValue<'t>) -> Self {
        Self { name, value }
    }

    pub fn expect_empty(&self) -> Result<&'t str> {
        match &self.value {
            ListParamValue::Empty => Ok(self.name),
            var => Err(eyre!("Expected empty param, got '{:?}'", var)),
        }
    }

    pub fn expect_empty_named(&self, name: &str) -> Result<()> {
        match &self.value {
            ListParamValue::Empty if self.name == name => Ok(()),
            var => Err(eyre!("Expected empty param, got '{:?}'", var)),
        }
    }

    pub fn expect_single(&self) -> Result<&Value<'t>> {
        match &self.value {
            ListParamValue::Single(value) => Ok(value),
            var => Err(eyre!("Expected single value param, got '{:?}'", var)),
        }
    }

    pub fn expect_single_named(&self, name: &str) -> Result<&Value<'t>> {
        match &self.value {
            ListParamValue::Single(value) if self.name == name => Ok(value),
            var => Err(eyre!("Expected single value param, got '{:?}'", var)),
        }
    }

    pub fn expect_list(&self) -> Result<&ValueList> {
        match &self.value {
            ListParamValue::List(values) => Ok(values),
            var => Err(eyre!("Expected list value param, got '{:?}'", var)),
        }
    }
}

#[derive(Debug)]
pub enum ListParamValue<'t> {
    /// One-word parameter
    Empty,
    /// Param with a type, name and a single value
    Single(Value<'t>),
    /// Param with a type, name and a list of values
    List(ValueList),
}

#[derive(Debug, Clone)]
pub enum Value<'t> {
    Integer(Int),
    Float(f32),
    Point2(Vec2),
    Vector2(Vec2),
    Point3(Vec3),
    Vector3(Vec3),
    Normal3(Vec3),
    Spectrum((Int, f32)),
    Rgb(Vec3),
    Blackbody(Int),
    Bool(bool),
    String(&'t str),
    Texture(&'t str),
}

impl<'t> Value<'t> {
    pub fn expect_integer(&self) -> Result<Int> {
        todo!()
    }
    pub fn expect_float(&self) -> Result<f32> {
        match self {
            Value::Float(f) => Ok(*f),
            _ => Err(eyre!("Expected float value, got '{:?}'", self)),
        }
    }
    pub fn expect_point2(&self) -> Result<Vec2> {
        todo!()
    }
    pub fn expect_vector2(&self) -> Result<Vec2> {
        todo!()
    }
    pub fn expect_point3(&self) -> Result<Vec3> {
        todo!()
    }
    pub fn expect_vector3(&self) -> Result<Vec3> {
        todo!()
    }
    pub fn expect_normal3(&self) -> Result<Vec3> {
        todo!()
    }
    pub fn expect_spectrum(&self) -> Result<(Int, f32)> {
        todo!()
    }
    pub fn expect_rgb(&self) -> Result<Vec3> {
        match self {
            Value::Rgb(rgb) => Ok(*rgb),
            _ => Err(eyre!("Expected RGB value, got '{:?}'", self)),
        }
    }
    pub fn expect_blackbody(&self) -> Result<Int> {
        todo!()
    }
    pub fn expect_bool(&self) -> Result<bool> {
        todo!()
    }
    pub fn expect_string(&self) -> Result<&'t str> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err(eyre!("Expected string value, got '{:?}'", self)),
        }
    }
    pub fn expect_texture(&self) -> Result<&'t str> {
        todo!()
    }
}

pub type ValueVec<T> = SmallVec<[T; 4]>;

#[derive(Debug, Clone)]
pub enum ValueList {
    Integer(ValueVec<Int>),
    Float(ValueVec<f32>),
    Point2(ValueVec<Vec2>),
    Vector2(ValueVec<Vec2>),
    Point3(ValueVec<Vec3>),
    Vector3(ValueVec<Vec3>),
    Normal3(ValueVec<Vec3>),
    Spectrum(ValueVec<(Int, f32)>),
}

pub enum SingleValueOrList<'t> {
    Value(Value<'t>),
    List(ValueList),
}

#[derive(PartialEq, Eq)]
pub enum Directive {
    AttributeBegin,
    AttributeEnd,
    ObjectBegin,
    ObjectEnd,
}
