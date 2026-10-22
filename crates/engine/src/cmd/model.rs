//! Construction tools shared by the UI, CLI and integrated agent.
use super::*;
use cadcraft_doc::model::{Model, Object, Shape};
use cadcraft_doc::{EntityKind, Handle};
use serde_json::json;
use std::sync::Arc;

fn enabled(s: &Session) -> std::result::Result<(), String> {
    let d = s.doc().map_err(|e| e.to_string())?;
    if d.mesh.as_ref().is_some_and(|m| m.model.is_some()) || (d.mesh.is_none() && d.entity_count() == 0) {
        Ok(())
    } else {
        Err("Use an empty drawing for a new construction, or an editable CAD3D project. Existing 2D/STL geometry is never overwritten.".into())
    }
}
fn commit(s: &mut Session, recipe: Model, preserve: bool) -> Result<Value> {
    let mut m = recipe.build().map_err(EngineError::Other)?;
    if preserve && let Some(old) = s.doc()?.mesh.as_ref() {
        m.apply(old.transformation()).map_err(EngineError::Other)?;
        m.unit = old.unit;
    }
    s.doc_mut()?.mesh = Some(Arc::new(m));
    s.execute("mesh.inspect", &json!({}))
}
fn recipe(s: &Session) -> Result<Model> {
    s.doc()?.mesh.as_ref().and_then(|m| m.model.as_deref()).cloned().ok_or_else(|| bad("model", "This is not an editable CAD3D construction"))
}
fn build(s: &mut Session, p: &Value) -> Result<Value> {
    enabled(s).map_err(EngineError::Other)?;
    let recipe: Model = serde_json::from_value(p.clone()).map_err(|e| bad("model.build", e.to_string()))?;
    commit(s, recipe, false)
}
fn edit(s: &mut Session, p: &Value) -> Result<Value> {
    let mut r = recipe(s)?;
    let name = p.get("name").and_then(Value::as_str).ok_or_else(|| bad("model.edit", "name required"))?;
    let changes =
        p.get("changes").and_then(Value::as_object).ok_or_else(|| bad("model.edit", "changes must be an object with complete replacement fields"))?;
    if changes.contains_key("name") || changes.contains_key("kind") {
        return Err(bad("model.edit", "Name and kind cannot be changed; rebuild the recipe for structural changes"));
    }
    let object = r.nodes.iter_mut().find(|n| n.name == name).ok_or_else(|| bad("model.edit", "Object not found"))?;
    let mut v = serde_json::to_value(&*object).map_err(|e| bad("model.edit", e.to_string()))?;
    let map = v.as_object_mut().ok_or_else(|| bad("model.edit", "Invalid object"))?;
    for (k, v) in changes {
        map.insert(k.clone(), v.clone());
    }
    *object = serde_json::from_value(v).map_err(|e| bad("model.edit", e.to_string()))?;
    commit(s, r, true)
}
fn add(s: &mut Session, p: &Value) -> Result<Value> {
    let mut r = recipe(s)?;
    let n: Object = serde_json::from_value(p.clone()).map_err(|e| bad("model.add", e.to_string()))?;
    r.nodes.push(n);
    commit(s, r, true)
}
fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    let mut r = recipe(s)?;
    let name = p.get("name").and_then(Value::as_str).ok_or_else(|| bad("model.remove", "name required"))?;
    let n = r.nodes.len();
    r.nodes.retain(|n| n.name != name);
    if r.nodes.len() == n {
        return Err(bad("model.remove", "Object not found"));
    }
    commit(s, r, true)
}
/// Conversion keeps source 2D entities in the same drawing, hidden while in 3D mode. Undo returns to drafting.
fn from2d(s: &mut Session, p: &Value) -> Result<Value> {
    if s.doc()?.mesh.is_some() {
        return Err(bad("model.from2d", "Activate a 2D drawing first"));
    }
    let height = p.get("height").and_then(Value::as_f64).ok_or_else(|| bad("model.from2d", "height required"))?;
    let unit = p.get("unit").and_then(Value::as_str).ok_or_else(|| bad("model.from2d", "Explicit unit required"))?;
    let selected = if let Some(v) = p.get("handles") {
        let a = v
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 64)
            .ok_or_else(|| bad("model.from2d", "handles must contain 1 to 64 hexadecimal entity handles"))?;
        a.iter()
            .map(|v| {
                v.as_str().and_then(|s| u64::from_str_radix(s, 16).ok()).map(Handle).ok_or_else(|| bad("model.from2d", "Invalid hexadecimal handle"))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        s.selection().to_vec()
    };
    if selected.is_empty() || selected.len() > 64 {
        return Err(bad("model.from2d", "Select 1 to 64 closed circles or polylines, or supply handles"));
    }
    let mut nodes = Vec::new();
    for h in selected {
        let e = s.doc()?.entity(h).ok_or_else(|| bad("model.from2d", "Entity not found"))?;
        if s.doc()?.space_of(h) != Some(cadcraft_doc::Space::Model) {
            return Err(bad("model.from2d", "Only model-space contours are supported"));
        }
        let mut points = Vec::new();
        match &e.kind {
            EntityKind::Circle(c) => {
                if e.common.extrusion != cadcraft_geom::Vec3::Z || c.center.z.abs() > 1e-8 {
                    return Err(bad("model.from2d", "Contour must lie in the world XY plane at Z=0"));
                }
                for i in 0..64 {
                    let a = cadcraft_geom::TAU * i as f64 / 64.0;
                    points.push([c.center.x + c.radius * a.cos(), c.center.y + c.radius * a.sin()]);
                }
            }
            EntityKind::LwPolyline(poly) if poly.closed => {
                if poly.elevation.abs() > 1e-8 || e.common.extrusion != cadcraft_geom::Vec3::Z {
                    return Err(bad("model.from2d", "Contour must lie in world XY at Z=0"));
                }
                if poly.vertices.len() > 256 {
                    return Err(bad("model.from2d", "Contour exceeds 256 source vertices; simplify it first"));
                }
                let pl = cadcraft_geom::Polyline { vertices: poly.vertices.clone(), closed: true };
                let bounds = cadcraft_doc::entity_bounds(s.doc()?, e, 0);
                let tol = bounds.width().max(bounds.height()) * 0.001;
                let mut tessellated = Vec::new();
                for segment in pl.segments() {
                    segment.tessellate(tol.max(1e-5), &mut tessellated);
                    if tessellated.len() > 257 {
                        return Err(bad("model.from2d", "Tessellated contour exceeds 256 vertices; simplify it first"));
                    }
                }
                tessellated.dedup_by(|a, b| a.dist(*b) < 1e-8);
                points = tessellated.into_iter().map(|p| [p.x, p.y]).collect();
            }
            _ => return Err(bad("model.from2d", "Only closed XY circles and polylines can be extruded; open curves/text/images are unsupported")),
        }
        nodes.push(Object {
            name: format!("contour_{:X}", h.0),
            position: [0.0; 3],
            rotation: [0.0; 3],
            shape: Shape::Extrude { profile: points, height, holes: Vec::new() },
        });
    }
    commit(s, Model { version: 1, unit: unit.into(), nodes }, false)
}
fn to2d(s: &mut Session, _: &Value) -> Result<Value> {
    if s.doc()?.mesh.is_none() || s.doc()?.entity_count() == 0 {
        return Err(bad("model.to2d", "No retained 2D source in this model"));
    }
    s.doc_mut()?.mesh = None;
    Ok(json!({"mode":"2d","entities":s.doc()?.entity_count()}))
}
pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
    CommandSpec::new("model.to2d","Return to Source 2D",to2d).params("{}: clear the 3D view and return to retained source entities. Undo restores the 3D construction."),
    CommandSpec::new("model.build","Build 3D Project",build).enabled(enabled).params("{version:1,unit:mm|cm|m|in,nodes:[{name:unique,kind:box,size:[x,y,z],position?:[x,y,z],rotation?:[degreesX,degreesY,degreesZ]} | {name,kind:cylinder,radius,height,segments?:48} | {name,kind:cone,radius,top_radius?:0,height,segments?:48} | {name,kind:sphere,radius,segments?:48} | {name,kind:extrude,profile:[[x,y],...],height,holes?:[[[x,y],...],...]} | {name,kind:revolve,profile:[[radius,z],...],segments?:48} | {name,kind:boolean,operation:union|subtract|intersect,a:earlierName,b:earlierName}]}: build/replace editable construction in an EMPTY drawing or CAD3D project. 1..128 nodes. All nodes support position/rotation. Boxes start at local [0,0,0]; cylinders/cones on Z=0 centered XY; spheres centered XYZ. Rotations XYZ about local origin, then translation. Boolean operands are hidden, leaves displayed. Closed simple profiles <=256 vertices. Curves faceted; segments 12..128. Use model.edit for later changes. No imported STL booleans."),
    CommandSpec::new("model.edit","Edit 3D Object",edit).params("{name:string,changes:{size?:[x,y,z],position?:[x,y,z],rotation?:[x,y,z],radius?:number,height?:number,profile?:[[x,y],...],...}}: replace specified fields of a named construction object and regenerate dependents atomically. Get names/current recipe from mesh.inspect. Cannot change name/kind. Preserves whole-model transforms."),
    CommandSpec::new("model.add","Add 3D Object",add).params("One complete node as in model.build, with a unique name; references earlier nodes. Requires editable CAD3D project."),
    CommandSpec::new("model.remove","Remove 3D Object",remove).params("{name:string}: remove an object; rejected when remaining objects depend on it or project would be empty."),
    CommandSpec::new("model.from2d","Extrude 2D Contours",from2d).params("{height:positive,unit:mm|cm|m|in,handles?:[hexString,...]}: extrude selected or specified closed world-XY circles/polylines at Z=0. Independent volumes, NOT automatic holes. Source 2D geometry stays in memory; Undo returns to drawing. CAD3D retains the original 2D drawing too; model.to2d returns to it."),
]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> Value {
        json!({"version":1,"unit":"mm","nodes":[{"name":"base","kind":"box","size":[100,60,10]}]})
    }
    #[test]
    fn model_create_edit_add_remove_transform_undo_and_rejected_changes_are_atomic() {
        let mut s = Session::new();
        s.execute("model.build", &base()).unwrap();
        let uid = s.state().unwrap().uid;
        s.execute("mesh.move", &json!({"offset":[4,5,6]})).unwrap();
        s.execute("model.edit", &json!({"name":"base","changes":{"size":[120,80,12]}})).unwrap();
        let r = s.execute("mesh.inspect", &json!({})).unwrap();
        assert_eq!(r["size"]["x"], 120.);
        assert_eq!(r["bounds"]["min"]["x"], 4.);
        assert_eq!(s.state().unwrap().uid, uid);
        let before = s.doc().unwrap().mesh.clone();
        let count = s.state().unwrap().undo.len();
        for p in [
            json!({"name":"base","changes":{"size":[-1,2,3]}}),
            json!({"name":"base","changes":{"nonsense":3}}),
            json!({"name":"base","changes":{"kind":"sphere"}}),
        ] {
            assert!(s.execute("model.edit", &p).is_err());
            assert_eq!(before, s.doc().unwrap().mesh);
            assert_eq!(count, s.state().unwrap().undo.len());
        }
        s.execute("model.add", &json!({"name":"hole","kind":"cylinder","radius":5,"height":20,"position":[20,20,-1],"segments":24})).unwrap();
        s.execute("model.add", &json!({"name":"cut","kind":"boolean","operation":"subtract","a":"base","b":"hole"})).unwrap();
        assert!(s.execute("model.remove", &json!({"name":"base"})).is_err());
        s.execute("model.remove", &json!({"name":"cut"})).unwrap();
        s.execute("model.remove", &json!({"name":"hole"})).unwrap();
        for _ in 0..6 {
            s.execute("undo", &json!({})).unwrap();
        }
        assert_eq!(s.doc().unwrap().mesh.as_ref().unwrap().size().x, 100.);
        s.execute("undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().mesh.is_none());
        s.execute("redo", &json!({})).unwrap();
        assert!(s.doc().unwrap().mesh.is_some());
    }
    #[test]
    fn model_extrudes_2d_without_erasing_source_and_refuses_replacing_existing_drawing() {
        let mut s = Session::new();
        s.execute("circle", &json!({"center":[0,0],"radius":10})).unwrap();
        let h = s.doc().unwrap().model.iter().next().unwrap().handle;
        assert!(s.execute("model.build", &base()).is_err());
        s.execute("model.from2d", &json!({"height":20,"unit":"mm","handles":[format!("{:X}",h.0)]})).unwrap();
        assert_eq!(s.doc().unwrap().entity_count(), 1);
        assert_eq!(s.doc().unwrap().mesh.as_ref().unwrap().size().z, 20.);
        s.execute("undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().mesh.is_none());
        s.execute("redo", &json!({})).unwrap();
        s.execute("model.to2d", &json!({})).unwrap();
        assert!(s.doc().unwrap().mesh.is_none());
        assert_eq!(s.doc().unwrap().entity_count(), 1);
        s.execute("undo", &json!({})).unwrap();
        assert!(s.doc().unwrap().mesh.is_some());
    }
}
