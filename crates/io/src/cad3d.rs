//! Versioned, editable construction file; no executable expressions or external file references.
use cadcraft_doc::{Drawing, mesh::Unit, model::Model};
use cadcraft_geom::Mat4;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
const MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    format: String,
    version: u32,
    recipe: Model,
    transform: Mat4,
    display_unit: String,
    #[serde(default)]
    source_dxf: Option<String>,
}
pub fn is_ours(b: &[u8]) -> bool {
    b.starts_with(b"{\"format\":\"CADCraft3D\"")
}
pub fn read(bytes: &[u8]) -> Result<Drawing, String> {
    if bytes.len() > MAX_BYTES {
        return Err("CAD3D exceeds 16 MiB".into());
    }
    let p: Project = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if p.format != "CADCraft3D" || p.version != 1 {
        return Err("Unsupported CAD3D format/version".into());
    }
    let t = &p.transform.m;
    if t.iter().flatten().any(|v| !v.is_finite()) || t[3] != [0.0, 0.0, 0.0, 1.0] {
        return Err("Invalid CAD3D affine transform".into());
    }
    // Only proper uniform transforms are supported by the mesh tools.
    let cols = [
        cadcraft_geom::Vec3::new(t[0][0], t[1][0], t[2][0]),
        cadcraft_geom::Vec3::new(t[0][1], t[1][1], t[2][1]),
        cadcraft_geom::Vec3::new(t[0][2], t[1][2], t[2][2]),
    ];
    let lengths = cols.map(|c| c.len());
    let k = lengths[0];
    if (!k.is_finite() || k <= 0.0)
        || lengths.iter().any(|v| (*v - k).abs() > k * 1e-7)
        || cols[0].dot(cols[1]).abs() > k * k * 1e-7
        || cols[0].dot(cols[2]).abs() > k * k * 1e-7
        || cols[1].dot(cols[2]).abs() > k * k * 1e-7
        || cols[0].dot(cols[1].cross(cols[2])) <= 0.0
    {
        return Err("CAD3D transform must be a proper uniform scale/rotation/translation".into());
    }
    let mut mesh = p.recipe.build()?;
    mesh.apply(p.transform)?;
    mesh.unit = Unit::parse(&p.display_unit).filter(|u| *u != Unit::Unknown).ok_or("Invalid project display unit")?;
    let mut d =
        if let Some(source) = p.source_dxf { super::dxf_read::read(source.as_bytes()).map_err(|e| e.to_string())? } else { Drawing::new_metric() };
    d.mesh = Some(Arc::new(mesh));
    Ok(d)
}
pub fn write(d: &Drawing) -> Result<Vec<u8>, String> {
    let mesh = d.mesh.as_ref().ok_or("No 3D construction")?;
    let recipe = mesh.model.as_deref().ok_or("Imported STL has no editable recipe; save it as STL")?.clone();
    let source_dxf =
        if d.entity_count() > 0 { Some(cadcraft_dxf::write_ascii_as(&super::dxf_write::tags(d, super::save_version(d)), true)) } else { None };
    let p = Project {
        format: "CADCraft3D".into(),
        version: 1,
        recipe,
        transform: mesh.transformation(),
        display_unit: mesh.unit.name().into(),
        source_dxf,
    };
    let b = serde_json::to_vec(&p).map_err(|e| e.to_string())?;
    if b.len() > MAX_BYTES {
        return Err("CAD3D exceeds 16 MiB".into());
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn volume(m: &Mesh) -> f64 {
        m.triangles().map(|[a, b, c]| a.dot(b.cross(c)) / 6.).sum()
    }
    use cadcraft_doc::mesh::Mesh;
    #[test]
    fn model_flange_roundtrip_has_five_real_holes_and_exports_step() {
        let bytes = include_bytes!("../../../docs/examples/flange.cad3d");
        let d = read(bytes).unwrap();
        let m = d.mesh.as_ref().unwrap();
        let v = volume(m);
        // Reference volume: regular 48-sided outer/central cylinders and four 24-sided bolt bores.
        let area = |r: f64, n: f64| n * 0.5 * r * r * (cadcraft_geom::TAU / n).sin();
        let expected = (area(50., 48.) - area(15., 48.) - 4. * area(4., 24.)) * 10.;
        assert!((v - expected).abs() < 0.02, "volume {v}, expected {expected}");
        cadcraft_geom::solid::closed_components(&m.triangles().collect::<Vec<_>>()).unwrap();
        let reopened = read(&write(&d).unwrap()).unwrap();
        assert_eq!(m.model, reopened.mesh.as_ref().unwrap().model);
        let stl = crate::write(&d, "flange.stl").unwrap();
        let exported = crate::read(&stl, "flange.stl").unwrap();
        assert!((volume(exported.mesh.as_ref().unwrap()) - v).abs() < 0.1);
        let step = crate::write(&d, "flange.step").unwrap();
        let step = String::from_utf8(step).unwrap();
        assert!(step.starts_with("ISO-10303-21;"));
        assert!(step.contains("FACETED_BREP("));
        assert!(step.contains("CLOSED_SHELL("));
        assert!(step.ends_with("END-ISO-10303-21;\n"));
    }
    #[test]
    fn model_office_has_independent_editable_objects_and_step_assembly() {
        let bytes = include_bytes!("../../../docs/examples/office.cad3d");
        let mut d = read(bytes).unwrap();
        let m = Arc::make_mut(d.mesh.as_mut().unwrap());
        assert_eq!(m.unit, Unit::M);
        assert_eq!(m.size(), cadcraft_geom::Vec3::new(8., 6., 3.1));
        assert_eq!(m.model.as_ref().unwrap().nodes.len(), 26);
        m.rotated("z", 35.).unwrap();
        m.translated(cadcraft_geom::Vec3::new(1., 2., 3.)).unwrap();
        m.set_unit(Unit::Mm, true).unwrap();
        let copy = read(&write(&d).unwrap()).unwrap();
        assert_eq!(copy.mesh, d.mesh);
        let step = String::from_utf8(crate::write(&d, "office.step").unwrap()).unwrap();
        assert_eq!(step.matches("FACETED_BREP(").count(), 24);
        assert!(crate::read(step.as_bytes(), "office.step").is_err()); // Import is intentionally unsupported.
    }
    #[test]
    fn model_project_preserves_source_2d_and_rejects_hostile_content() {
        let mut d = read(include_bytes!("../../../docs/examples/flange.cad3d")).unwrap();
        d.add(
            &cadcraft_doc::Space::Model,
            Default::default(),
            cadcraft_doc::EntityKind::Circle(cadcraft_doc::Circle { center: cadcraft_geom::Vec3::ZERO, radius: 5. }),
        )
        .unwrap();
        let bytes = write(&d).unwrap();
        let recovered = crate::read(&bytes, "drawing.sv$").unwrap();
        assert_eq!(recovered.entity_count(), 1);
        assert!(recovered.mesh.as_ref().unwrap().model.is_some());
        let mut hostile: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        hostile["transform"]["m"][0][0] = serde_json::json!(-1.);
        assert!(read(&serde_json::to_vec(&hostile).unwrap()).is_err());
        let mut hostile: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        hostile["recipe"]["nodes"][0]["segments"] = serde_json::json!(10000000);
        assert!(read(&serde_json::to_vec(&hostile).unwrap()).is_err());
        assert!(read(br#"{"format":"CADCraft3D","version":999}"#).is_err());
    }
}
