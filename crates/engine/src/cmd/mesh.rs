//! Standalone mesh commands; normal document snapshots provide Undo/Redo.
use super::*;
use cadcraft_doc::mesh::{Mesh, Unit};
use cadcraft_geom::Vec3;
use serde_json::json;
use std::sync::Arc;

pub(crate) fn allowed(id: &str) -> bool {
    id.starts_with("mesh.")
        || matches!(
            id,
            "open"
                | "new"
                | "close"
                | "closeall"
                | "save"
                | "qsave"
                | "saveas"
                | "undo"
                | "redo"
                | "u"
                | "mredo"
                | "document.switch"
                | "document.bytes"
                | "commands"
                | "recover"
        )
}
fn enabled(s: &Session) -> std::result::Result<(), String> {
    s.doc().ok().and_then(|d| d.mesh.as_ref()).map(|_| ()).ok_or_else(|| "Open an STL model first".into())
}
fn mesh(s: &Session) -> Result<&Mesh> {
    s.doc()?.mesh.as_deref().ok_or_else(|| EngineError::Other("Open an STL model first".into()))
}
fn edit(s: &mut Session, change: impl FnOnce(&mut Mesh) -> std::result::Result<(), String>) -> Result<Value> {
    let mut next = mesh(s)?.clone();
    change(&mut next).map_err(EngineError::Other)?;
    s.doc_mut()?.mesh = Some(Arc::new(next));
    inspect(s, &Value::Null)
}
fn finite(p: &Value, key: &str) -> Result<f64> {
    p.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| bad("mesh", format!("{key} must be a finite number")))
}
fn axis(p: &Value) -> Result<&str> {
    match p.get("axis").and_then(Value::as_str) {
        Some(v @ ("x" | "y" | "z")) => Ok(v),
        _ => Err(bad("mesh", "axis must be x, y or z")),
    }
}
fn inspect(s: &mut Session, _: &Value) -> Result<Value> {
    let m = mesh(s)?;
    let [min, max] = m.bounds();
    let size = m.size();
    Ok(
        json!({"type":"stlMesh","triangles":m.count(),"unit":m.unit.name(),"bounds":{"min":min,"max":max},"size":size,"note":"STL is a triangle mesh, without textures or parametric CAD features. Set its original unit explicitly before physical measurements."}),
    )
}
pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
    CommandSpec::new("mesh.inspect","Inspect 3D Model",inspect).enabled(enabled).noundo().params("{} -> triangle count, unit, exact transformed bounds and size"),
    CommandSpec::new("mesh.move","Move 3D Model",|s,p| {
        let v=p.get("offset").and_then(Value::as_array).filter(|a|a.len()==3).ok_or_else(||bad("mesh.move","offset must be [x,y,z]"))?;
        let mut n=[0.0;3]; for (to,from) in n.iter_mut().zip(v) { *to=from.as_f64().filter(|v|v.is_finite()).ok_or_else(||bad("mesh.move","offset must be finite"))?; }
        edit(s,|m|m.translated(Vec3::new(n[0],n[1],n[2])))
    }).enabled(enabled).params("{offset:[x,y,z]}: translate in the model's current unit"),
    CommandSpec::new("mesh.rotate","Rotate 3D Model",|s,p| { let axis=axis(p)?;let angle=finite(p,"degrees")?; edit(s,|m|m.rotated(axis,angle)) }).enabled(enabled).params("{axis:x|y|z,degrees:number}: right-hand rotation about model center"),
    CommandSpec::new("mesh.scale","Scale 3D Model",|s,p| { let k=finite(p,"factor")?;edit(s,|m|m.scaled(k)) }).enabled(enabled).params("{factor:positive number}: uniform scale about model center"),
    CommandSpec::new("mesh.resize","Resize 3D Model",|s,p| {
        let axis=axis(p)?;let size=finite(p,"size")?;if size<=0.0{return Err(bad("mesh.resize","size must be positive"));}
        let dims=mesh(s)?.size();let current=match axis{"x"=>dims.x,"y"=>dims.y,_=>dims.z};
        if current<1e-9 {return Err(bad("mesh.resize","The selected axis has zero thickness"));}
        edit(s,|m|m.scaled(size/current))
    }).enabled(enabled).params("{axis:x|y|z,size:positive number}: set bounding size in current unit, preserve proportions; Z is height. Inspect units first."),
    CommandSpec::new("mesh.center","Center 3D Model",|s,p| {
        let center=mesh(s)?.center();let min=mesh(s)?.bounds()[0];
        let ground=p.get("ground").and_then(Value::as_bool).unwrap_or(true);
        edit(s,|m|m.translated(Vec3::new(-center.x,-center.y,if ground {-min.z} else {-center.z})))
    }).enabled(enabled).params("{ground?:true}: center X/Y at origin and put lowest Z on ground; ground:false centers all axes"),
    CommandSpec::new("mesh.units","3D Model Units",|s,p| {
        let unit=p.get("unit").and_then(Value::as_str).and_then(Unit::parse).ok_or_else(||bad("mesh.units","unit must be unitless, mm, cm, m or in"))?;
        let convert=p.get("convert").and_then(Value::as_bool).unwrap_or(false);
        edit(s,|m|m.set_unit(unit,convert))
    }).enabled(enabled).params("{unit:unitless|mm|cm|m|in,convert?:false}: false labels original unit without changing coordinates; true converts coordinates preserving physical size. NEVER guess original STL units."),
]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        let mut s = Session::empty();
        s.execute("new", &json!({})).unwrap();
        s.doc_mut().unwrap().mesh = Some(Arc::new(Mesh::new(vec![[Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 30.0)]]).unwrap()));
        s
    }
    #[test]
    fn transform_units_and_document_undo_work_without_touching_other_drawings() {
        let mut s = session();
        let original = s.doc().unwrap().mesh.clone().unwrap();
        s.execute("mesh.units", &json!({"unit":"mm"})).unwrap();
        s.execute("mesh.resize", &json!({"axis":"z","size":100.0})).unwrap();
        s.execute("mesh.rotate", &json!({"axis":"z","degrees":90.0})).unwrap();
        s.execute("mesh.move", &json!({"offset":[17.0,25.0,-4.0]})).unwrap();
        s.execute("mesh.center", &json!({"ground":true})).unwrap();
        let report = s.execute("mesh.inspect", &json!({})).unwrap();
        assert_eq!(report["unit"], "mm");
        assert!((report["size"]["z"].as_f64().unwrap() - 100.0).abs() < 1e-8);
        assert_eq!(report["bounds"]["min"]["z"], 0.0);
        for _ in 0..5 {
            s.execute("undo", &json!({})).unwrap();
        }
        assert_eq!(s.doc().unwrap().mesh.as_deref(), Some(original.as_ref()));
        for _ in 0..5 {
            s.execute("redo", &json!({})).unwrap();
        }
        s.execute("mesh.units", &json!({"unit":"cm","convert":true})).unwrap();
        assert!((s.doc().unwrap().mesh.as_ref().unwrap().size().z - 10.0).abs() < 1e-8);
        assert!(s.execute("line", &json!({"points":[[0,0],[1,1]]})).is_err());
        assert!(s.start("circle").is_err());
        s.execute("new", &json!({})).unwrap();
        assert!(s.doc().unwrap().mesh.is_none());
        s.execute("document.switch", &json!({"index":0})).unwrap();
        assert!(s.doc().unwrap().mesh.is_some());
    }
    #[test]
    fn rejected_transforms_leave_geometry_and_undo_history_unchanged() {
        let mut s = session();
        let before = s.doc().unwrap().mesh.clone().unwrap();
        let steps = s.state().unwrap().undo.len();
        for (id, p) in [
            ("mesh.scale", json!({"factor":0})),
            ("mesh.scale", json!({"factor":1e300})),
            ("mesh.rotate", json!({"axis":"bad","degrees":90})),
            ("mesh.resize", json!({"axis":"z","size":-4})),
            ("mesh.move", json!({"offset":[1e100,0,0]})),
            ("mesh.units", json!({"unit":"cm","convert":true})),
        ] {
            assert!(s.execute(id, &p).is_err(), "{id}");
            assert_eq!(s.doc().unwrap().mesh.as_deref(), Some(before.as_ref()));
            assert_eq!(s.state().unwrap().undo.len(), steps);
        }
    }
}
