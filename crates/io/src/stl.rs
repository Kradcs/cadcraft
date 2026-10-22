//! Bounded ASCII/binary STL interchange. Input normals are replaced by geometric normals.
use cadcraft_doc::{
    Drawing,
    mesh::{MAX_TRIANGLES, Mesh, Unit},
};
use cadcraft_geom::Vec3;
use std::sync::Arc;
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
const HEADER: &[u8] = b"CADCraft STL units=";

pub fn is_ours(bytes: &[u8]) -> bool {
    bytes.starts_with(HEADER)
}
pub fn read(bytes: &[u8]) -> Result<Drawing, String> {
    if bytes.len() > MAX_BYTES {
        return Err("STL exceeds 64 MiB".into());
    }
    let count = bytes.get(80..84).and_then(|b| <[u8; 4]>::try_from(b).ok()).map(u32::from_le_bytes).map(|v| v as usize);
    let binary = count.is_some_and(|n| n.checked_mul(50).and_then(|v| v.checked_add(84)) == Some(bytes.len()));
    let mut triangles = Vec::new();
    let mut attributes = false;
    if binary {
        let count = count.unwrap_or_default();
        if count == 0 || count > MAX_TRIANGLES {
            return Err("STL: expected 1 to 1,000,000 triangles".into());
        }
        triangles.reserve(count);
        for facet in bytes.get(84..).unwrap_or_default().chunks_exact(50) {
            let mut t = [Vec3::ZERO; 3];
            for (i, vertex) in t.iter_mut().enumerate() {
                let mut coords = [0.0; 3];
                for (axis, c) in coords.iter_mut().enumerate() {
                    let at = 12 + i * 12 + axis * 4;
                    let raw = facet.get(at..at + 4).and_then(|v| <[u8; 4]>::try_from(v).ok()).ok_or("Truncated STL vertex")?;
                    *c = f64::from(f32::from_le_bytes(raw));
                }
                *vertex = Vec3::new(coords[0], coords[1], coords[2]);
            }
            attributes |= facet.get(48..50).is_some_and(|v| v != [0, 0]);
            triangles.push(t);
        }
    } else {
        let text = std::str::from_utf8(bytes).map_err(|_| "Invalid binary or ASCII STL")?;
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        if lines.next().is_none_or(|l| l.split_ascii_whitespace().next() != Some("solid")) {
            return Err("STL must contain a binary header or an ASCII solid".into());
        }
        let mut state = 0usize;
        let mut t = [Vec3::ZERO; 3];
        let mut ended = false;
        for line in lines {
            let mut words = line.split_ascii_whitespace();
            let first = words.next().unwrap_or("");
            match (state, first) {
                (0, "solid") => {
                    ended = false;
                }
                (0, "endsolid") => {
                    ended = true;
                }
                (0, "facet") if !ended && words.next() == Some("normal") => {
                    for _ in 0..3 {
                        number(words.next())?;
                    }
                    if words.next().is_some() {
                        return Err("Invalid STL normal".into());
                    }
                    state = 1;
                }
                (1, "outer") if words.next() == Some("loop") && words.next().is_none() => state = 2,
                (2..=4, "vertex") => {
                    let p = Vec3::new(number(words.next())?, number(words.next())?, number(words.next())?);
                    if words.next().is_some() {
                        return Err("Invalid STL vertex".into());
                    }
                    if let Some(v) = t.get_mut(state - 2) {
                        *v = p;
                    }
                    state += 1;
                }
                (5, "endloop") if words.next().is_none() => state = 6,
                (6, "endfacet") if words.next().is_none() => {
                    if triangles.len() >= MAX_TRIANGLES {
                        return Err("STL exceeds 1,000,000 triangles".into());
                    }
                    triangles.push(t);
                    state = 0;
                }
                _ => return Err("Invalid or truncated ASCII STL facet".into()),
            }
        }
        if state != 0 || !ended {
            return Err("Truncated ASCII STL: missing endsolid".into());
        }
    }
    let mut mesh = Mesh::new(triangles)?;
    if is_ours(bytes)
        && let Some(header) = bytes.get(HEADER.len()..80)
    {
        let label = std::str::from_utf8(header).unwrap_or("").trim_matches(|c| c == '\0' || c == ' ');
        if let Some(unit) = Unit::parse(label) {
            mesh.unit = unit;
        }
    }
    let mut d = Drawing::new_metric();
    d.mesh = Some(Arc::new(mesh));
    if attributes {
        d.read_notes.push("STL facet attributes/colors are not supported; geometry was preserved.".into());
    }
    Ok(d)
}
fn number(v: Option<&str>) -> Result<f64, String> {
    let n = v.and_then(|s| s.parse::<f64>().ok()).filter(|v| v.is_finite()).ok_or("Invalid STL number")?;
    Ok(n)
}
pub fn write(d: &Drawing) -> Result<Vec<u8>, String> {
    let mesh = d.mesh.as_ref().ok_or("This drawing has no STL mesh")?;
    let mut bytes = vec![0; 80];
    let text = format!("CADCraft STL units={}", mesh.unit.name());
    for (to, from) in bytes.iter_mut().zip(text.bytes()) {
        *to = from;
    }
    let count = u32::try_from(mesh.count()).map_err(|_| "Too many STL triangles")?;
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.reserve(mesh.count().saturating_mul(50));
    for t in mesh.triangles() {
        let [a, b, c] = t;
        let normal = (b - a).cross(c - a).normalized();
        for v in [normal, a, b, c] {
            for n in [v.x, v.y, v.z] {
                if !n.is_finite() || n.abs() > f64::from(f32::MAX) {
                    return Err("STL coordinate overflow".into());
                }
                bytes.extend_from_slice(&(n as f32).to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[0, 0]);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ascii() -> Vec<u8> {
        b"solid test\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 10 0 0\nvertex 0 20 3\nendloop\nendfacet\nendsolid test\n".to_vec()
    }
    #[test]
    fn ascii_binary_and_autosave_roundtrip_preserve_geometry_and_unit() {
        let mut d = read(&ascii()).unwrap();
        Arc::make_mut(d.mesh.as_mut().unwrap()).unit = Unit::Mm;
        let bytes = write(&d).unwrap();
        assert_eq!(bytes.len(), 134);
        let restored = crate::read(&bytes, "recovery.sv$").unwrap();
        assert_eq!(restored.mesh.as_ref().unwrap().unit, Unit::Mm);
        assert_eq!(restored.mesh.as_ref().unwrap().bounds(), d.mesh.as_ref().unwrap().bounds());
        assert!(crate::write(&d, "lose.dxf").is_err());
        assert!(crate::write(&d, "lose.dwg").is_err());
        assert!(crate::write(&d, "lose.png").is_err());
        let bytes = crate::write(&d, "keep.stl").unwrap();
        assert_eq!(read(&bytes).unwrap().mesh.as_ref().unwrap().count(), 1);
    }
    #[test]
    fn malformed_counts_missing_records_non_finite_and_extreme_coordinates_are_rejected() {
        let d = read(&ascii()).unwrap();
        let bytes = write(&d).unwrap();
        for end in [0, 4, 80, 83, 84, 100, 133] {
            assert!(read(&bytes[..end]).is_err(), "{end}");
        }
        let mut forged = bytes.clone();
        forged[80..84].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read(&forged).is_err());
        for value in [f32::NAN, f32::INFINITY, 1.0e30] {
            let mut damaged = bytes.clone();
            damaged[96..100].copy_from_slice(&value.to_le_bytes());
            assert!(read(&damaged).is_err());
        }
        let text = String::from_utf8(ascii()).unwrap();
        for damaged in [
            text.replace("endfacet", ""),
            text.replace("endsolid test", ""),
            text.replace("vertex 10", "vertex NaN"),
            text.replace("outer loop", "vertex 0 0 0"),
        ] {
            assert!(read(damaged.as_bytes()).is_err());
        }
        // A binary file can legitimately start with the ASCII word solid.
        let mut solid = bytes.clone();
        solid[..5].copy_from_slice(b"solid");
        assert_eq!(read(&solid).unwrap().mesh.as_ref().unwrap().count(), 1);
    }
    #[test]
    fn supplied_stl_is_read_transformed_exported_and_reread_when_requested() {
        let Some(path) = std::env::var_os("CADCRAFT_TEST_STL") else { return };
        let bytes = std::fs::read(path).unwrap();
        let mut d = read(&bytes).unwrap();
        let original = d.mesh.as_ref().unwrap().count();
        assert!(original > 0);
        let mesh = Arc::make_mut(d.mesh.as_mut().unwrap());
        mesh.unit = Unit::Mm;
        mesh.scaled(100.0 / mesh.size().z).unwrap();
        mesh.rotated("z", 35.0).unwrap();
        let center = mesh.center();
        let min = mesh.bounds()[0];
        mesh.translated(Vec3::new(-center.x, -center.y, -min.z)).unwrap();
        let result = read(&write(&d).unwrap()).unwrap();
        let m = result.mesh.as_ref().unwrap();
        assert_eq!(m.count(), original);
        assert!((m.size().z - 100.0).abs() < 1e-3);
        assert!(m.bounds()[0].z.abs() < 1e-4);
        eprintln!("Actual STL roundtrip: {original} triangles, height {:.6}, minimum Z {:.6}", m.size().z, m.bounds()[0].z);
    }
}
