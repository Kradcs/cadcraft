//! ISO 10303-21 / AP203 faceted B-rep export. No analytic surfaces or editable feature tree.
//! Each visible construction object is exported separately, preserving assembly volumes.
use cadcraft_doc::{Drawing, mesh::Mesh};
use cadcraft_geom::solid::{Tri, closed_components};
use std::fmt::Write;

struct Writer {
    text: String,
    next: usize,
}
impl Writer {
    fn entity(&mut self, v: String) -> usize {
        let id = self.next;
        self.next += 1;
        let _ = writeln!(self.text, "#{id}={v};");
        id
    }
}
fn refs(ids: &[usize]) -> String {
    ids.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(",")
}
fn real(v: f64) -> String {
    let s = format!("{v:.12E}");
    s.replace('e', "E")
}
pub fn write(d: &Drawing) -> Result<Vec<u8>, String> {
    let mesh = d.mesh.as_ref().ok_or("No 3D model")?;
    let unit = mesh.unit.millimeters().ok_or("Define physical units before STEP export")?;
    let parts: Vec<(String, Vec<Tri>)> = if let Some(recipe) = mesh.model.as_ref() {
        let transform = mesh.transformation();
        recipe.parts()?.into_iter().map(|(name, t)| (name, t.into_iter().map(|tri| tri.map(|p| transform.apply(p))).collect())).collect()
    } else {
        vec![("STL model".into(), mesh.triangles().collect())]
    };
    // Validate before producing any output, including on imported or non-manifold STL.
    let mut checked = Vec::new();
    for (name, t) in parts {
        if !t.is_empty() {
            let components = closed_components(&t)?;
            checked.push((name, t, components));
        }
    }
    let mut w=Writer{text:"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('CADCraft faceted geometry; curves tessellated'),'2;1');\nFILE_NAME('model.step','2026-10-22T00:00:00',(''),(''),'CADCraft','CADCraft','');\nFILE_SCHEMA(('CONFIG_CONTROL_DESIGN'));\nENDSEC;\nDATA;\n".into(),next:1};
    let app = w.entity("APPLICATION_CONTEXT('configuration controlled 3d designs of mechanical parts and assemblies')".into());
    w.entity(format!("APPLICATION_PROTOCOL_DEFINITION('international standard','config_control_design',1994,#{app})"));
    let product_context = w.entity(format!("PRODUCT_CONTEXT('',#{app},'mechanical')"));
    let product = w.entity(format!("PRODUCT('CADCraft','CADCraft','faceted construction',(#{product_context}))"));
    let formation = w.entity(format!("PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE('','',#{product},.NOT_KNOWN.)"));
    let def_context = w.entity(format!("PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')"));
    let definition = w.entity(format!("PRODUCT_DEFINITION('design','',#{formation},#{def_context})"));
    let shape = w.entity(format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
    let mm = w.entity("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.))".into());
    let angle = w.entity("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))".into());
    let solid_angle = w.entity("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())".into());
    let size = mesh.size();
    let tolerance = size.x.max(size.y).max(size.z).max(1.0) * unit * 1e-8;
    let uncertainty = w.entity(format!("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE({}),#{mm},'distance_accuracy_value','')", real(tolerance)));
    let context=w.entity(format!("(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{mm},#{angle},#{solid_angle})) REPRESENTATION_CONTEXT('','3D'))"));
    let origin_id = w.entity("CARTESIAN_POINT('',(0.,0.,0.))".into());
    let z_axis = w.entity("DIRECTION('',(0.,0.,1.))".into());
    let x_axis = w.entity("DIRECTION('',(1.,0.,0.))".into());
    let placement = w.entity(format!("AXIS2_PLACEMENT_3D('',#{origin_id},#{z_axis},#{x_axis})"));
    let mut breps = vec![placement];
    for (name, triangles, components) in checked {
        let mut point_ids = std::collections::BTreeMap::new();
        let mut faces = Vec::new();
        let origin = triangles.first().map(|t| t[0]).unwrap_or_default();
        let size = Mesh::new(triangles.clone())?.size();
        let tol = size.x.max(size.y).max(size.z).max(1.0) * 1e-8;
        for t in &triangles {
            let mut ids = Vec::new();
            for p in t {
                let key = [((p.x - origin.x) / tol).round() as i64, ((p.y - origin.y) / tol).round() as i64, ((p.z - origin.z) / tol).round() as i64];
                let id = *point_ids
                    .entry(key)
                    .or_insert_with(|| w.entity(format!("CARTESIAN_POINT('',({},{},{}))", real(p.x * unit), real(p.y * unit), real(p.z * unit))));
                ids.push(id);
            }
            let lp = w.entity(format!("POLY_LOOP('',({}))", refs(&ids)));
            let bound = w.entity(format!("FACE_OUTER_BOUND('',#{lp},.T.)"));
            faces.push(w.entity(format!("FACE('',(#{bound}))")));
        }
        for group in components {
            let volume: f64 =
                group.iter().filter_map(|i| triangles.get(*i)).map(|[a, b, c]| (*a - origin).dot((*b - origin).cross(*c - origin)) / 6.0).sum();
            if volume <= 0.0 {
                return Err("STEP requires outward closed shells; inverted meshes or enclosed cavity shells are unsupported".into());
            }
            let ids: Vec<_> = group.iter().filter_map(|i| faces.get(*i).copied()).collect();
            let shell = w.entity(format!("CLOSED_SHELL('',({}))", refs(&ids)));
            let name: String = name.chars().map(|c| if c.is_ascii_graphic() || c == ' ' { c } else { '_' }).collect();
            let name = name.replace('\'', "''");
            breps.push(w.entity(format!("FACETED_BREP('{name}',#{shell})")));
        }
    }
    let rep = w.entity(format!("FACETED_BREP_SHAPE_REPRESENTATION('',({}),#{context})", refs(&breps)));
    w.entity(format!("SHAPE_DEFINITION_REPRESENTATION(#{shape},#{rep})"));
    w.text.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(w.text.into_bytes())
}
