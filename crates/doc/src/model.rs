//! Editable, bounded construction recipe. Coordinates are explicit, in a single physical unit.
use crate::mesh::{Mesh, Unit};
use cadcraft_geom::{Vec3, solid};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn default_segments() -> usize {
    48
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub version: u32,
    pub unit: String,
    pub nodes: Vec<Object>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub name: String,
    #[serde(flatten)]
    pub shape: Shape,
    #[serde(default)]
    pub position: [f64; 3],
    #[serde(default)]
    pub rotation: [f64; 3],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Shape {
    Box {
        size: [f64; 3],
    },
    Cylinder {
        radius: f64,
        height: f64,
        #[serde(default = "default_segments")]
        segments: usize,
    },
    Cone {
        radius: f64,
        #[serde(default)]
        top_radius: f64,
        height: f64,
        #[serde(default = "default_segments")]
        segments: usize,
    },
    Sphere {
        radius: f64,
        #[serde(default = "default_segments")]
        segments: usize,
    },
    Extrude {
        profile: Vec<[f64; 2]>,
        height: f64,
        #[serde(default)]
        holes: Vec<Vec<[f64; 2]>>,
    },
    Revolve {
        profile: Vec<[f64; 2]>,
        #[serde(default = "default_segments")]
        segments: usize,
    },
    Boolean {
        operation: String,
        a: String,
        b: String,
    },
}
impl Model {
    pub fn parts(&self) -> Result<Vec<(String, Vec<solid::Tri>)>, String> {
        if self.version != 1 {
            return Err("Unsupported CAD3D version; expected 1".into());
        }
        let _unit = Unit::parse(&self.unit).filter(|u| *u != Unit::Unknown).ok_or("Choose a physical unit: mm, cm, m or in")?;
        if self.nodes.is_empty() || self.nodes.len() > 128 {
            return Err("A project requires 1 to 128 named objects".into());
        }
        let mut objects: BTreeMap<String, solid::Solid> = BTreeMap::new();
        let mut consumed = BTreeSet::new();
        let mut total = 0usize;
        let mut booleans = 0usize;
        let mut profile_vertices = 0usize;
        for node in &self.nodes {
            if node.name.trim().is_empty() || node.name.len() > 80 || objects.contains_key(&node.name) {
                return Err("Object names must be unique, nonempty and at most 80 bytes".into());
            }
            if node.position.iter().any(|v| !v.is_finite() || v.abs() > 1e7) || node.rotation.iter().any(|v| !v.is_finite() || v.abs() > 360000.0) {
                return Err(format!("{}: invalid position or rotation", node.name));
            }
            match &node.shape {
                Shape::Boolean { .. } => booleans += 1,
                Shape::Extrude { profile, holes, .. } => {
                    booleans += holes.len();
                    profile_vertices += profile.len() + holes.iter().map(Vec::len).sum::<usize>();
                }
                Shape::Revolve { profile, .. } => profile_vertices += profile.len(),
                _ => {}
            }
            if booleans > 32 || profile_vertices > 4096 {
                return Err("Recipe exceeds 32 booleans or 4096 profile vertices; simplify the construction".into());
            }
            let mut triangles = match &node.shape {
                Shape::Box { size } => solid::Solid::new(solid::box_mesh(*size)?)?,
                Shape::Cylinder { radius, height, segments } => solid::Solid::new(solid::cylinder(*radius, *radius, *height, *segments)?)?,
                Shape::Cone { radius, top_radius, height, segments } => {
                    solid::Solid::new(solid::cylinder(*radius, *top_radius, *height, *segments)?)?
                }
                Shape::Sphere { radius, segments } => solid::Solid::new(solid::sphere(*radius, *segments)?)?,
                Shape::Extrude { profile, height, holes } => {
                    if holes.len() > 16 {
                        return Err("An extrusion supports at most 16 cutout profiles".into());
                    }
                    let mut out = solid::Solid::new(solid::extrude(profile, *height)?)?;
                    for hole in holes {
                        let cutter = solid::Solid::new(solid::extrude(hole, *height)?)?;
                        out = out.boolean(&cutter, "subtract")?;
                    }
                    out
                }
                Shape::Revolve { profile, segments } => solid::Solid::new(solid::revolve(profile, *segments)?)?,
                Shape::Boolean { operation, a, b } => {
                    if a == b {
                        return Err("Boolean operands must be different objects".into());
                    }
                    let aa = objects.get(a).ok_or_else(|| format!("{a}: operand must appear before its boolean result"))?;
                    let bb = objects.get(b).ok_or_else(|| format!("{b}: operand must appear before its boolean result"))?;
                    let result = aa.boolean(bb, operation)?;
                    consumed.insert(a.clone());
                    consumed.insert(b.clone());
                    result
                }
            };
            triangles = triangles.transformed(|mut p| {
                // Local rotations X, Y, Z about the construction origin, then world translation.
                for (axis, degrees) in node.rotation.iter().enumerate() {
                    let (s, c) = degrees.rem_euclid(360.0).to_radians().sin_cos();
                    p = match axis {
                        0 => Vec3::new(p.x, p.y * c - p.z * s, p.y * s + p.z * c),
                        1 => Vec3::new(p.x * c + p.z * s, p.y, -p.x * s + p.z * c),
                        _ => Vec3::new(p.x * c - p.y * s, p.x * s + p.y * c, p.z),
                    };
                }
                p + Vec3::new(node.position[0], node.position[1], node.position[2])
            });
            total = total.saturating_add(triangles.count());
            if total > 250_000 {
                return Err("Construction exceeds 250,000 intermediate triangles; simplify the recipe".into());
            }
            objects.insert(node.name.clone(), triangles);
        }
        objects
            .into_iter()
            .filter(|(name, _)| !consumed.contains(name))
            .map(|(name, s)| {
                let t = s.triangles()?;
                if !t.is_empty() {
                    solid::closed_components(&t).map_err(|e| format!("{name}: {e}"))?;
                }
                Ok((name, t))
            })
            .collect()
    }
    pub fn build(&self) -> Result<Mesh, String> {
        let triangles = self.parts()?.into_iter().flat_map(|(_, t)| t).collect();
        let mut mesh = Mesh::new(triangles)?;
        mesh.unit = Unit::parse(&self.unit).ok_or("Invalid project unit")?;
        mesh.model = Some(Arc::new(self.clone()));
        Ok(mesh)
    }
}
