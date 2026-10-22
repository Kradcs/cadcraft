//! Bounded orthographic software rasterizer with a depth buffer and flat lighting.
//! No GPU requirement; callers can render away from the UI thread.
use cadcraft_doc::mesh::Mesh;
use cadcraft_geom::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: f64,
    pub pan: [f64; 2],
    pub target: Vec3,
    pub radius: f64,
}
impl Camera {
    pub fn fit(mesh: &Mesh) -> Self {
        Self { yaw: -65.0, pitch: 20.0, zoom: 1.0, pan: [0.0; 2], target: mesh.center(), radius: mesh.size().len() * 0.5 }
    }
    pub fn basis(self) -> [Vec3; 3] {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let eye = Vec3::new(cy * cp, sy * cp, sp);
        let right = Vec3::new(-sy, cy, 0.0);
        let up = eye.cross(right);
        [right, up, eye]
    }
}
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub triangles: usize,
}
pub fn render(mesh: &Mesh, camera: Camera, width: u32, height: u32) -> Result<Frame, String> {
    if width == 0
        || height == 0
        || width > 1280
        || height > 1024
        || !camera.target.is_finite()
        || ![camera.yaw, camera.pitch, camera.zoom, camera.radius, camera.pan[0], camera.pan[1]].iter().all(|v| v.is_finite())
        || !(0.05..=20.0).contains(&camera.zoom)
        || camera.radius < 1e-9
    {
        return Err("Invalid 3D viewport".into());
    }
    let w = width as usize;
    let h = height as usize;
    let mut rgba = vec![0u8; w * h * 4];
    for (row, pixels) in rgba.chunks_exact_mut(w * 4).enumerate() {
        let k = (row as f32 / h as f32 * 9.0) as u8;
        for p in pixels.chunks_exact_mut(4) {
            p.copy_from_slice(&[31 + k, 37 + k, 46 + k, 255]);
        }
    }
    let mut depth = vec![f64::NEG_INFINITY; w * h];
    let [right, up, eye] = camera.basis();
    let scale = 0.82 * f64::from(width.min(height)) * camera.zoom / (2.0 * camera.radius);
    let light = (eye + up * 0.6 + right * (-0.4)).normalized();
    let mut work = 0u64;
    for t in mesh.triangles() {
        let projected = t.map(|v| {
            let p = v - camera.target;
            Vec3::new(
                f64::from(width) * (0.5 + camera.pan[0]) + p.dot(right) * scale,
                f64::from(height) * (0.5 + camera.pan[1]) - p.dot(up) * scale,
                p.dot(eye),
            )
        });
        let [a, b, c] = projected;
        let area = edge(a, b, c.x, c.y);
        if !area.is_finite() || area.abs() < 1e-8 {
            continue;
        }
        let x0 = a.x.min(b.x).min(c.x).floor().clamp(0.0, f64::from(width)) as usize;
        let x1 = a.x.max(b.x).max(c.x).ceil().clamp(0.0, f64::from(width)) as usize;
        let y0 = a.y.min(b.y).min(c.y).floor().clamp(0.0, f64::from(height)) as usize;
        let y1 = a.y.max(b.y).max(c.y).ceil().clamp(0.0, f64::from(height)) as usize;
        work = work.saturating_add(((x1 - x0) * (y1 - y0)) as u64);
        if work > 48_000_000 {
            return Err("3D view too expensive to render. Use Fit model or zoom out.".into());
        }
        let normal = cadcraft_doc::mesh::triangle_normal(t);
        let shade = 0.30 + 0.70 * normal.dot(light).abs();
        let color = [(230.0 * shade) as u8, (190.0 * shade) as u8, (71.0 * shade) as u8, 255];
        for y in y0..y1 {
            for x in x0..x1 {
                let px = x as f64 + 0.5;
                let py = y as f64 + 0.5;
                let u = edge(b, c, px, py) / area;
                let v = edge(c, a, px, py) / area;
                let z = 1.0 - u - v;
                if u < -1e-9 || v < -1e-9 || z < -1e-9 {
                    continue;
                }
                let d = u * a.z + v * b.z + z * c.z;
                let index = y * w + x;
                if let Some(old) = depth.get_mut(index)
                    && d > *old
                {
                    *old = d;
                    if let Some(pixel) = rgba.get_mut(index * 4..index * 4 + 4) {
                        pixel.copy_from_slice(&color);
                    }
                }
            }
        }
    }
    Ok(Frame { width, height, rgba, triangles: mesh.count() })
}
fn edge(a: Vec3, b: Vec3, x: f64, y: f64) -> f64 {
    (x - a.x) * (b.y - a.y) - (y - a.y) * (b.x - a.x)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depth_order_is_independent_of_triangle_order_and_camera_changes_the_view() {
        let front = [Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0)];
        let back = front.map(|p| p + Vec3::new(0.5, 1.0, 0.0));
        let a = Mesh::new(vec![front, back]).unwrap();
        let b = Mesh::new(vec![back, front]).unwrap();
        let mut camera = Camera::fit(&a);
        camera.yaw = -90.0;
        camera.pitch = 0.0;
        let first = render(&a, camera, 320, 240).unwrap();
        let second = render(&b, camera, 320, 240).unwrap();
        assert_eq!(first.rgba, second.rgba);
        assert!(first.rgba.chunks_exact(4).filter(|p| p[0] > 100 && p[1] > 80).count() > 1000);
        camera.yaw = 20.0;
        let other = render(&a, camera, 320, 240).unwrap();
        assert_ne!(first.rgba, other.rgba);
        assert!(render(&a, camera, 100_000, 100_000).is_err());
        camera.zoom = f64::NAN;
        assert!(render(&a, camera, 320, 240).is_err());
    }
    #[test]
    fn lighting_is_invariant_under_unit_scale_changes() {
        let triangles = vec![[Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0)]];
        let large = Mesh::new(triangles).unwrap();
        let mut small = large.clone();
        small.scaled(0.000001).unwrap();
        let mut a = Camera::fit(&large);
        a.yaw = -90.0;
        a.pitch = 0.0;
        let mut b = Camera::fit(&small);
        b.yaw = -90.0;
        b.pitch = 0.0;
        let first = render(&large, a, 320, 240).unwrap();
        let second = render(&small, b, 320, 240).unwrap();
        let index = (120 * 320 + 160) * 4;
        assert_eq!(&first.rgba[index..index + 4], &second.rgba[index..index + 4]);
    }

    #[test]
    fn supplied_model_render_when_requested() {
        // The user's file is deliberately not a repository fixture or a release asset.
        let Some(path) = std::env::var_os("CADCRAFT_MESH_PREVIEW_DATA") else { return };
        let data = std::fs::read(path).unwrap();
        let mut triangles = Vec::new();
        for facet in data.chunks_exact(72) {
            let mut t = [Vec3::ZERO; 3];
            for (v, raw) in t.iter_mut().zip(facet.chunks_exact(24)) {
                let mut c = [0.0; 3];
                for (to, bytes) in c.iter_mut().zip(raw.chunks_exact(8)) {
                    *to = f64::from_le_bytes(bytes.try_into().unwrap());
                }
                *v = Vec3::new(c[0], c[1], c[2]);
            }
            triangles.push(t);
        }
        let mesh = Mesh::new(triangles).unwrap();
        let start = std::time::Instant::now();
        let frame = render(&mesh, Camera::fit(&mesh), 1000, 850).unwrap();
        if let Some(path) = std::env::var_os("CADCRAFT_MESH_PREVIEW_PNG") {
            tiny_skia::Pixmap::from_vec(frame.rgba.clone(), tiny_skia::IntSize::from_wh(frame.width, frame.height).unwrap())
                .unwrap()
                .save_png(path)
                .unwrap();
        }
        eprintln!("Rendered {} triangles at 1000×850 in {:?}", frame.triangles, start.elapsed());
    }
}
