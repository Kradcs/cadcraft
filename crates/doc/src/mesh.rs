//! A standalone triangle-mesh document. Geometry is shared by undo snapshots.
use cadcraft_geom::{Mat4, Vec3};
use std::sync::Arc;

pub const MAX_TRIANGLES: usize = 1_000_000;
pub const MAX_COORD: f64 = 1.0e9;

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    triangles: Arc<Vec<[Vec3; 3]>>,
    transform: Mat4,
    bounds: [Vec3; 2],
    pub unit: Unit,
    pub model: Option<Arc<crate::model::Model>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Unit {
    #[default]
    Unknown,
    Mm,
    Cm,
    M,
    Inch,
}
impl Unit {
    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unitless",
            Self::Mm => "mm",
            Self::Cm => "cm",
            Self::M => "m",
            Self::Inch => "in",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "unitless" => Some(Self::Unknown),
            "mm" => Some(Self::Mm),
            "cm" => Some(Self::Cm),
            "m" => Some(Self::M),
            "in" => Some(Self::Inch),
            _ => None,
        }
    }
    pub fn millimeters(self) -> Option<f64> {
        match self {
            Self::Unknown => None,
            Self::Mm => Some(1.0),
            Self::Cm => Some(10.0),
            Self::M => Some(1000.0),
            Self::Inch => Some(25.4),
        }
    }
}
impl Mesh {
    pub fn new(triangles: Vec<[Vec3; 3]>) -> Result<Self, String> {
        if triangles.is_empty() || triangles.len() > MAX_TRIANGLES {
            return Err("STL: expected 1 to 1,000,000 triangles".into());
        }
        let mut mesh = Self { triangles: Arc::new(triangles), transform: Mat4::IDENTITY, bounds: [Vec3::ZERO; 2], unit: Unit::Unknown, model: None };
        mesh.update_bounds()?;
        Ok(mesh)
    }
    pub fn transformation(&self) -> Mat4 {
        self.transform
    }
    pub fn count(&self) -> usize {
        self.triangles.len()
    }
    pub fn triangles(&self) -> impl Iterator<Item = [Vec3; 3]> + '_ {
        self.triangles.iter().map(|t| t.map(|p| self.transform.apply(p)))
    }
    pub fn bounds(&self) -> [Vec3; 2] {
        self.bounds
    }
    pub fn size(&self) -> Vec3 {
        let [a, b] = self.bounds;
        b - a
    }
    pub fn center(&self) -> Vec3 {
        let [a, b] = self.bounds;
        (a + b) * 0.5
    }
    fn update_bounds(&mut self) -> Result<(), String> {
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = -lo;
        for t in self.triangles() {
            for p in t {
                if !p.is_finite() || p.x.abs().max(p.y.abs()).max(p.z.abs()) > MAX_COORD {
                    return Err("Mesh coordinates must be finite and within ±1e9 units".into());
                }
                lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
                hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
            }
        }
        let s = hi - lo;
        if s.x.max(s.y).max(s.z) < 1.0e-8 {
            return Err("Mesh is empty or too small to display".into());
        }
        self.bounds = [lo, hi];
        Ok(())
    }
    /// Atomic even when the requested transform would overflow or collapse the model.
    pub fn apply(&mut self, t: Mat4) -> Result<(), String> {
        let before = self.transform;
        self.transform = t.mul(&before);
        if let Err(e) = self.update_bounds() {
            self.transform = before;
            return Err(e);
        }
        Ok(())
    }
    pub fn translated(&mut self, offset: Vec3) -> Result<(), String> {
        self.apply(translation(offset))
    }
    pub fn scaled(&mut self, factor: f64) -> Result<(), String> {
        if !factor.is_finite() || !(1.0e-9..=1.0e9).contains(&factor) {
            return Err("Scale factor must be between 1e-9 and 1e9".into());
        }
        let c = self.center();
        let mut s = Mat4::IDENTITY;
        for i in 0..3 {
            s.m[i][i] = factor;
        }
        self.apply(translation(c).mul(&s).mul(&translation(-c)))
    }
    pub fn rotated(&mut self, axis: &str, degrees: f64) -> Result<(), String> {
        if !degrees.is_finite() {
            return Err("Rotation angle must be finite".into());
        }
        let (s, c) = degrees.rem_euclid(360.0).to_radians().sin_cos();
        let mut r = Mat4::IDENTITY;
        match axis {
            "x" => {
                r.m[1][1] = c;
                r.m[1][2] = -s;
                r.m[2][1] = s;
                r.m[2][2] = c;
            }
            "y" => {
                r.m[0][0] = c;
                r.m[0][2] = s;
                r.m[2][0] = -s;
                r.m[2][2] = c;
            }
            "z" => {
                r.m[0][0] = c;
                r.m[0][1] = -s;
                r.m[1][0] = s;
                r.m[1][1] = c;
            }
            _ => return Err("Rotation axis must be x, y or z".into()),
        }
        let center = self.center();
        self.apply(translation(center).mul(&r).mul(&translation(-center)))
    }
    pub fn set_unit(&mut self, unit: Unit, convert: bool) -> Result<(), String> {
        if self.model.is_some() && unit == Unit::Unknown {
            return Err("Editable projects require a physical unit".into());
        }
        if convert {
            let from = self.unit.millimeters().ok_or("Define the original STL unit before converting units")?;
            let to = unit.millimeters().ok_or("Choose mm, cm, m or in for conversion")?;
            let mut s = Mat4::IDENTITY;
            for i in 0..3 {
                s.m[i][i] = from / to;
            }
            self.apply(s)?;
        }
        if !convert && let Some(recipe) = self.model.as_mut() {
            Arc::make_mut(recipe).unit = unit.name().into();
        }
        self.unit = unit;
        Ok(())
    }
}
pub fn translation(p: Vec3) -> Mat4 {
    let mut t = Mat4::IDENTITY;
    t.m[0][3] = p.x;
    t.m[1][3] = p.y;
    t.m[2][3] = p.z;
    t
}

/// STL normals must be scale independent, including facets smaller than the 2D drafting EPS.
pub fn triangle_normal([a, b, c]: [Vec3; 3]) -> Vec3 {
    let n = (b - a).cross(c - a);
    let length = n.len();
    if length > 0.0 && length.is_finite() { Vec3::new(n.x / length, n.y / length, n.z / length) } else { Vec3::ZERO }
}
