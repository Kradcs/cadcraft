//! Bounded, faceted solid construction. Original implementation; no external CAD kernel.
use crate::{Vec2, Vec3};
pub type Tri = [Vec3; 3];
const EPS: f64 = 1e-8;
const LIMIT: usize = 60_000;

fn positive(v: f64) -> Result<(), String> {
    if v.is_finite() && (1e-5..=1e7).contains(&v) { Ok(()) } else { Err("Dimensions must be finite, between 0.00001 and 10,000,000 units".into()) }
}
pub fn segments(n: usize) -> Result<(), String> {
    if (12..=128).contains(&n) { Ok(()) } else { Err("segments must be between 12 and 128".into()) }
}
/// Validate a simple, nondegenerate closed profile (last point need not repeat first).
pub fn profile(points: &[[f64; 2]]) -> Result<Vec<Vec2>, String> {
    if !(3..=256).contains(&points.len()) {
        return Err("Profile requires 3 to 256 vertices".into());
    }
    let mut p: Vec<_> = points.iter().map(|a| Vec2::new(a[0], a[1])).collect();
    if p.iter().any(|v| !v.is_finite() || v.x.abs().max(v.y.abs()) > 1e7) {
        return Err("Profile coordinates must be finite and within ±1e7".into());
    }
    if p.first() == p.last() {
        p.pop();
    }
    let n = p.len();
    if n < 3 {
        return Err("Profile needs three distinct vertices".into());
    }
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        if a.dist(b) < 1e-6 {
            return Err("Profile contains duplicate vertices or tiny edges".into());
        }
        for j in i + 1..n {
            if j == (i + 1) % n || (j + 1) % n == i {
                continue;
            }
            let (c, d) = (p[j], p[(j + 1) % n]);
            let orient = |a: Vec2, b: Vec2, c: Vec2| (b - a).cross(c - a);
            let (ab_c, ab_d, cd_a, cd_b) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
            let overlap = a.x.min(b.x) <= c.x.max(d.x) + EPS
                && c.x.min(d.x) <= a.x.max(b.x) + EPS
                && a.y.min(b.y) <= c.y.max(d.y) + EPS
                && c.y.min(d.y) <= a.y.max(b.y) + EPS;
            if overlap && ab_c * ab_d <= EPS * EPS && cd_a * cd_b <= EPS * EPS {
                return Err("Profile intersects or touches itself".into());
            }
        }
    }
    let area = crate::shoelace(&p);
    if area.abs() < 1e-10 {
        return Err("Profile has zero area".into());
    }
    if area < 0.0 {
        p.reverse();
    }
    Ok(p)
}
/// Capped extrusion with concave-profile ear triangulation, outward winding.
pub fn extrude(points: &[[f64; 2]], height: f64) -> Result<Vec<Tri>, String> {
    positive(height)?;
    let p = profile(points)?;
    let mut ids: Vec<usize> = (0..p.len()).collect();
    let mut caps = Vec::new();
    while ids.len() > 3 {
        let mut ear = None;
        for i in 0..ids.len() {
            let (a, b, c) = (ids[(i + ids.len() - 1) % ids.len()], ids[i], ids[(i + 1) % ids.len()]);
            let (pa, pb, pc) = (p[a], p[b], p[c]);
            if (pb - pa).cross(pc - pb) <= 1e-12 {
                continue;
            }
            let inside = ids.iter().copied().any(|j| {
                j != a
                    && j != b
                    && j != c
                    && (pb - pa).cross(p[j] - pa) >= -1e-12
                    && (pc - pb).cross(p[j] - pb) >= -1e-12
                    && (pa - pc).cross(p[j] - pc) >= -1e-12
            });
            if !inside {
                ear = Some((i, [a, b, c]));
                break;
            }
        }
        let (i, t) = ear.ok_or("Profile cannot be triangulated; remove collinear or near-coincident vertices")?;
        caps.push(t);
        ids.remove(i);
    }
    caps.push([ids[0], ids[1], ids[2]]);
    let mut out = Vec::new();
    for [a, b, c] in caps {
        out.push([p[c].to3(0.0), p[b].to3(0.0), p[a].to3(0.0)]);
        out.push([p[a].to3(height), p[b].to3(height), p[c].to3(height)]);
    }
    for i in 0..p.len() {
        let a = p[i];
        let b = p[(i + 1) % p.len()];
        out.push([a.to3(0.0), b.to3(0.0), b.to3(height)]);
        out.push([a.to3(0.0), b.to3(height), a.to3(height)]);
    }
    Ok(out)
}
pub fn box_mesh(size: [f64; 3]) -> Result<Vec<Tri>, String> {
    for v in size {
        positive(v)?;
    }
    extrude(&[[0.0, 0.0], [size[0], 0.0], [size[0], size[1]], [0.0, size[1]]], size[2])
}
/// Full revolution around Z of a closed profile in [radius,z].
pub fn revolve(points: &[[f64; 2]], n: usize) -> Result<Vec<Tri>, String> {
    segments(n)?;
    let p = profile(points)?;
    if p.iter().any(|p| p.x < 0.0) {
        return Err("Revolution radius must not be negative".into());
    }
    let mut out = Vec::new();
    let point = |p: Vec2, i: usize| {
        let a = crate::TAU * (i % n) as f64 / n as f64;
        Vec3::new(p.x * a.cos(), p.x * a.sin(), p.y)
    };
    for k in 0..p.len() {
        let a = p[k];
        let b = p[(k + 1) % p.len()];
        for i in 0..n {
            let quad = [point(a, i), point(a, i + 1), point(b, i + 1), point(b, i)];
            for t in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
                if (t[1] - t[0]).cross(t[2] - t[0]).len() > 1e-14 {
                    out.push(t);
                }
            }
        }
    }
    if out.is_empty() {
        return Err("Revolution has no volume".into());
    }
    Ok(out)
}
pub fn cylinder(radius: f64, top: f64, height: f64, n: usize) -> Result<Vec<Tri>, String> {
    positive(radius)?;
    positive(height)?;
    if !top.is_finite() || !(0.0..=1e7).contains(&top) {
        return Err("Top radius must be finite and nonnegative".into());
    }
    let mut p = vec![[0.0, 0.0], [radius, 0.0]];
    if top > 0.0 {
        p.push([top, height]);
    }
    p.push([0.0, height]);
    revolve(&p, n)
}
pub fn sphere(radius: f64, n: usize) -> Result<Vec<Tri>, String> {
    positive(radius)?;
    segments(n)?;
    let mut p = vec![[0.0, -radius]];
    for i in 1..n / 2 {
        let a = crate::PI * i as f64 / (n / 2) as f64;
        p.push([radius * a.sin(), -radius * a.cos()]);
    }
    p.push([0.0, radius]);
    revolve(&p, n)
}

#[derive(Clone)]
struct Polygon {
    v: Vec<Vec3>,
    n: Vec3,
    w: f64,
}
impl Polygon {
    fn new(v: Vec<Vec3>) -> Option<Self> {
        let a = *v.first()?;
        let mut normal = Vec3::ZERO;
        for pair in v.get(1..)?.windows(2) {
            normal = (pair[0] - a).cross(pair[1] - a);
            let l = normal.len();
            if l > 1e-14 {
                normal = normal * (1.0 / l);
                break;
            }
        }
        if normal.len() < 0.5 {
            return None;
        }
        Some(Self { v, n: normal, w: normal.dot(a) })
    }
    fn flip(&mut self) {
        self.v.reverse();
        self.n = -self.n;
        self.w = -self.w;
    }
}
#[derive(Default)]
struct Budget {
    work: usize,
}
impl Budget {
    fn tick(&mut self, n: usize) -> Result<(), String> {
        self.work = self.work.saturating_add(n);
        if self.work > 3_000_000 { Err("Boolean complexity limit exceeded; simplify shapes or reduce segments".into()) } else { Ok(()) }
    }
}
fn split(
    p: Polygon,
    n: Vec3,
    w: f64,
    front: &mut Vec<Polygon>,
    back: &mut Vec<Polygon>,
    same: &mut Vec<Polygon>,
    opposite: &mut Vec<Polygon>,
    budget: &mut Budget,
) -> Result<(), String> {
    budget.tick(p.v.len())?;
    let ds: Vec<f64> = p.v.iter().map(|v| n.dot(*v) - w).collect();
    let has_front = ds.iter().any(|d| *d > EPS);
    let has_back = ds.iter().any(|d| *d < -EPS);
    match (has_front, has_back) {
        (false, false) => {
            if n.dot(p.n) >= 0.0 {
                same.push(p)
            } else {
                opposite.push(p)
            }
        }
        (true, false) => front.push(p),
        (false, true) => back.push(p),
        (true, true) => {
            let mut f = Vec::new();
            let mut b = Vec::new();
            for i in 0..p.v.len() {
                let j = (i + 1) % p.v.len();
                let (v, u, d, e) = (p.v[i], p.v[j], ds[i], ds[j]);
                if d >= -EPS {
                    f.push(v);
                }
                if d <= EPS {
                    b.push(v);
                }
                if (d > EPS && e < -EPS) || (d < -EPS && e > EPS) {
                    let q = v + (u - v) * (d / (d - e));
                    f.push(q);
                    b.push(q);
                }
            }
            if let Some(p) = Polygon::new(f) {
                front.push(p);
            }
            if let Some(p) = Polygon::new(b) {
                back.push(p);
            }
        }
    }
    if front.len() + back.len() + same.len() + opposite.len() > LIMIT {
        return Err("Boolean polygon limit exceeded".into());
    }
    Ok(())
}
#[derive(Default)]
struct Node {
    plane: Option<(Vec3, f64)>,
    p: Vec<Polygon>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
}
impl Node {
    fn depth(d: usize) -> Result<(), String> {
        if d > 192 { Err("Boolean tree depth limit exceeded; reduce segments".into()) } else { Ok(()) }
    }
    fn build(&mut self, p: Vec<Polygon>, b: &mut Budget, d: usize) -> Result<(), String> {
        Self::depth(d)?;
        if p.is_empty() {
            return Ok(());
        }
        if self.plane.is_none() {
            self.plane = p.first().map(|p| (p.n, p.w));
        }
        let Some((n, w)) = self.plane else {
            return Ok(());
        };
        let (mut f, mut k, mut same, mut opposite) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for p in p {
            split(p, n, w, &mut f, &mut k, &mut same, &mut opposite, b)?;
        }
        self.p.extend(same);
        self.p.extend(opposite);
        if !f.is_empty() {
            self.front.get_or_insert_with(Default::default).build(f, b, d + 1)?;
        }
        if !k.is_empty() {
            self.back.get_or_insert_with(Default::default).build(k, b, d + 1)?;
        }
        Ok(())
    }
    fn clip(&self, p: Vec<Polygon>, b: &mut Budget, d: usize) -> Result<Vec<Polygon>, String> {
        Self::depth(d)?;
        let Some((n, w)) = self.plane else {
            return Ok(p);
        };
        let (mut f, mut k, mut same, mut opposite) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for p in p {
            split(p, n, w, &mut f, &mut k, &mut same, &mut opposite, b)?;
        }
        f.extend(same);
        k.extend(opposite);
        if let Some(node) = &self.front {
            f = node.clip(f, b, d + 1)?;
        }
        if let Some(node) = &self.back {
            k = node.clip(k, b, d + 1)?;
        } else {
            k.clear();
        }
        f.extend(k);
        if f.len() > LIMIT {
            return Err("Boolean polygon limit exceeded".into());
        }
        Ok(f)
    }
    fn clip_to(&mut self, other: &Node, b: &mut Budget) -> Result<(), String> {
        self.p = other.clip(std::mem::take(&mut self.p), b, 0)?;
        if let Some(n) = &mut self.front {
            n.clip_to(other, b)?;
        }
        if let Some(n) = &mut self.back {
            n.clip_to(other, b)?;
        }
        Ok(())
    }
    fn invert(&mut self) {
        for p in &mut self.p {
            p.flip();
        }
        if let Some((n, w)) = &mut self.plane {
            *n = -*n;
            *w = -*w;
        }
        if let Some(n) = &mut self.front {
            n.invert();
        }
        if let Some(n) = &mut self.back {
            n.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }
    fn collect(self, out: &mut Vec<Polygon>) -> Result<(), String> {
        out.extend(self.p);
        if out.len() > LIMIT {
            return Err("Boolean polygon limit exceeded".into());
        }
        if let Some(n) = self.front {
            n.collect(out)?;
        }
        if let Some(n) = self.back {
            n.collect(out)?;
        }
        Ok(())
    }
}
/// Boolean on outward-oriented closed triangle surfaces. Work is normalized to the pair's bounds.
/// Convex boundary polygons are retained between operations, avoiding repeated tessellation.
#[derive(Clone)]
pub struct Solid {
    p: Vec<Polygon>,
    refine: bool,
}
impl Solid {
    pub fn new(triangles: Vec<Tri>) -> Result<Self, String> {
        if triangles.len() > 60_000 || triangles.iter().flatten().any(|p| !p.is_finite()) {
            return Err("Invalid or oversized solid".into());
        }
        Ok(Self { p: triangles.into_iter().filter_map(|t| Polygon::new(t.to_vec())).collect(), refine: false })
    }
    pub fn count(&self) -> usize {
        self.p.len()
    }
    pub fn boolean(&self, other: &Self, op: &str) -> Result<Self, String> {
        Ok(Self { p: boolean_polygons(&self.p, &other.p, op)?, refine: true })
    }
    pub fn transformed(self, mut f: impl FnMut(Vec3) -> Vec3) -> Self {
        Self { p: self.p.into_iter().filter_map(|p| Polygon::new(p.v.into_iter().map(&mut f).collect())).collect(), refine: self.refine }
    }
    pub fn triangles(self) -> Result<Vec<Tri>, String> {
        if self.refine {
            refine_triangles(self.p)
        } else {
            Ok(self.p.into_iter().filter_map(|p| Some([*p.v.first()?, *p.v.get(1)?, *p.v.get(2)?])).collect())
        }
    }
}
pub fn boolean(a: &[Tri], b: &[Tri], op: &str) -> Result<Vec<Tri>, String> {
    Solid::new(a.to_vec())?.boolean(&Solid::new(b.to_vec())?, op)?.triangles()
}
fn boolean_polygons(a: &[Polygon], b: &[Polygon], op: &str) -> Result<Vec<Polygon>, String> {
    if !matches!(op, "union" | "subtract" | "intersect") {
        return Err("operation must be union, subtract or intersect".into());
    }
    if a.len() + b.len() > 20_000 {
        return Err("Boolean inputs exceed 20,000 polygons; reduce segments".into());
    }
    if a.is_empty() || b.is_empty() {
        return Ok(match op {
            "union" => a.iter().chain(b).cloned().collect(),
            "subtract" => a.to_vec(),
            _ => Vec::new(),
        });
    }
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = -lo;
    for p in a.iter().chain(b).flat_map(|p| p.v.iter()) {
        if !p.is_finite() {
            return Err("Nonfinite solid coordinates".into());
        }
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let center = (lo + hi) * 0.5;
    let s = hi - lo;
    let scale = s.x.max(s.y).max(s.z);
    if scale < 1e-8 {
        return Err("Solid is too small".into());
    }
    let polygons =
        |polys: &[Polygon]| polys.iter().filter_map(|t| Polygon::new(t.v.iter().map(|p| (*p - center) * (1.0 / scale)).collect())).collect();
    let mut budget = Budget::default();
    let mut aa = Node::default();
    let mut bb = Node::default();
    aa.build(polygons(a), &mut budget, 0)?;
    bb.build(polygons(b), &mut budget, 0)?;
    match op {
        "union" => {
            aa.clip_to(&bb, &mut budget)?;
            bb.clip_to(&aa, &mut budget)?;
            bb.invert();
            bb.clip_to(&aa, &mut budget)?;
            bb.invert();
            let mut p = Vec::new();
            bb.collect(&mut p)?;
            aa.build(p, &mut budget, 0)?;
        }
        "subtract" => {
            aa.invert();
            aa.clip_to(&bb, &mut budget)?;
            bb.clip_to(&aa, &mut budget)?;
            bb.invert();
            bb.clip_to(&aa, &mut budget)?;
            bb.invert();
            let mut p = Vec::new();
            bb.collect(&mut p)?;
            aa.build(p, &mut budget, 0)?;
            aa.invert();
        }
        _ => {
            aa.invert();
            bb.clip_to(&aa, &mut budget)?;
            bb.invert();
            aa.clip_to(&bb, &mut budget)?;
            bb.clip_to(&aa, &mut budget)?;
            let mut p = Vec::new();
            bb.collect(&mut p)?;
            aa.build(p, &mut budget, 0)?;
            aa.invert();
        }
    }
    let mut p = Vec::new();
    aa.collect(&mut p)?;
    Ok(p.into_iter().filter_map(|p| Polygon::new(p.v.into_iter().map(|v| v * scale + center).collect())).collect())
}
fn refine_triangles(polys: Vec<Polygon>) -> Result<Vec<Tri>, String> {
    if polys.is_empty() {
        return Ok(Vec::new());
    }
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = -lo;
    for v in polys.iter().flat_map(|p| p.v.iter()) {
        lo = Vec3::new(lo.x.min(v.x), lo.y.min(v.y), lo.z.min(v.z));
        hi = Vec3::new(hi.x.max(v.x), hi.y.max(v.y), hi.z.max(v.z));
    }
    let center = (lo + hi) * 0.5;
    let size = hi - lo;
    let scale = size.x.max(size.y).max(size.z);
    if scale < 1e-8 {
        return Err("Solid is too small".into());
    }
    let p: Vec<_> = polys.into_iter().filter_map(|p| Polygon::new(p.v.into_iter().map(|v| (v - center) * (1.0 / scale)).collect())).collect();
    let center_world = center;
    // Make adjacent polygon boundaries conform before triangulation. BSP cuts can leave
    // a vertex in the middle of a neighbor's edge; exporting those T junctions is unsafe.
    let mut unique = std::collections::BTreeMap::new();
    let key = |v: Vec3| [(v.x / EPS).round() as i64, (v.y / EPS).round() as i64, (v.z / EPS).round() as i64];
    for poly in &p {
        for v in &poly.v {
            unique.entry(key(*v)).or_insert(*v);
        }
    }
    let mut vertices: Vec<Vec3> = unique.values().copied().collect();
    vertices.sort_by(|a, b| a.x.total_cmp(&b.x));
    let canonical = |v: Vec3| unique.get(&key(v)).copied().unwrap_or(v);
    let mut out = Vec::new();
    let mut work = 0usize;
    for poly in p {
        let mut boundary = Vec::new();
        for i in 0..poly.v.len() {
            let a = canonical(poly.v[i]);
            let b = canonical(poly.v[(i + 1) % poly.v.len()]);
            let edge = b - a;
            let len = edge.dot(edge);
            if len < 1e-20 {
                continue;
            }
            let start = vertices.partition_point(|v| v.x < a.x.min(b.x) - EPS * 4.0);
            let end = vertices.partition_point(|v| v.x <= a.x.max(b.x) + EPS * 4.0);
            work = work.saturating_add(end - start);
            if work > 12_000_000 {
                return Err("Boolean boundary refinement limit exceeded; reduce segments".into());
            }
            let mut cuts = vec![(0.0, a)];
            for v in &vertices[start..end] {
                let t = (*v - a).dot(edge) / len;
                if t > EPS && t < 1.0 - EPS && (*v - (a + edge * t)).len() < EPS * 4.0 {
                    cuts.push((t, *v));
                }
            }
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
            cuts.dedup_by(|a, b| (a.0 - b.0).abs() < EPS);
            boundary.extend(cuts.into_iter().map(|(_, v)| v));
        }
        if boundary.len() < 3 {
            continue;
        }
        if boundary.len() == 3 {
            out.push([boundary[0] * scale + center_world, boundary[1] * scale + center_world, boundary[2] * scale + center_world]);
            continue;
        }
        let center = boundary.iter().copied().fold(Vec3::ZERO, |a, b| a + b) * (1.0 / boundary.len() as f64);
        for i in 0..boundary.len() {
            let a = boundary[i];
            let b = boundary[(i + 1) % boundary.len()];
            if (a - center).cross(b - center).len() > 1e-14 {
                out.push([center * scale + center_world, a * scale + center_world, b * scale + center_world]);
            }
        }
    }
    if out.len() > LIMIT {
        return Err("Boolean triangle limit exceeded".into());
    }
    Ok(out)
}

/// Watertightness of faceted exports, welded at a scale-relative tolerance.
/// Separate components may share no edges; touching/overlapping parts need a boolean union.
pub fn closed_components(tris: &[Tri]) -> Result<Vec<Vec<usize>>, String> {
    use std::collections::BTreeMap;
    if tris.is_empty() || tris.len() > 250_000 {
        return Err("Expected 1 to 250,000 faces".into());
    }
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = -lo;
    for p in tris.iter().flatten() {
        if !p.is_finite() {
            return Err("Nonfinite mesh vertex".into());
        }
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let size = hi - lo;
    let tol = size.x.max(size.y).max(size.z).max(1.0) * 1e-8;
    let key = |p: Vec3| [((p.x - lo.x) / tol).round() as i64, ((p.y - lo.y) / tol).round() as i64, ((p.z - lo.z) / tol).round() as i64];
    let mut vertices = BTreeMap::new();
    let mut edges: BTreeMap<(usize, usize), Vec<(usize, bool)>> = BTreeMap::new();
    for (face, t) in tris.iter().enumerate() {
        let mut ids = [0usize; 3];
        for (id, p) in ids.iter_mut().zip(t) {
            let len = vertices.len();
            *id = *vertices.entry(key(*p)).or_insert(len);
        }
        if ids[0] == ids[1] || ids[1] == ids[2] || ids[2] == ids[0] {
            return Err("Degenerate faces at export tolerance; enlarge tiny details".into());
        }
        for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[0])] {
            edges.entry((a.min(b), a.max(b))).or_default().push((face, a < b));
        }
    }
    let mut adjacent = vec![Vec::new(); tris.len()];
    for edge in edges.values() {
        if edge.len() != 2 || edge[0].1 == edge[1].1 {
            return Err("Mesh is not a closed, consistently oriented manifold; unite touching parts or simplify the construction".into());
        }
        adjacent[edge[0].0].push(edge[1].0);
        adjacent[edge[1].0].push(edge[0].0);
    }
    let mut seen = vec![false; tris.len()];
    let mut components = Vec::new();
    for i in 0..tris.len() {
        if seen[i] {
            continue;
        }
        let mut stack = vec![i];
        let mut group = Vec::new();
        seen[i] = true;
        while let Some(i) = stack.pop() {
            group.push(i);
            for &j in &adjacent[i] {
                if !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        components.push(group);
    }
    Ok(components)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn volume(t: &[Tri]) -> f64 {
        t.iter().map(|[a, b, c]| a.dot(b.cross(*c)) / 6.0).sum()
    }
    #[test]
    fn model_solid_primitives_profiles_and_boolean_volumes() {
        let a = box_mesh([10.0, 10.0, 10.0]).unwrap();
        assert!((volume(&a) - 1000.0).abs() < 1e-8);
        closed_components(&a).unwrap();
        let mut b = a.clone();
        for p in b.iter_mut().flatten() {
            p.x += 5.0;
        }
        for (op, want) in [("union", 1500.0), ("subtract", 500.0), ("intersect", 500.0)] {
            let t = boolean(&a, &b, op).unwrap();
            assert!((volume(&t) - want).abs() < 1e-6, "{op} {}", volume(&t));
            closed_components(&t).unwrap();
        }
        let c = cylinder(2.0, 2.0, 12.0, 48).unwrap();
        let hole = boolean(&a, &c, "subtract").unwrap();
        assert!(volume(&hole) < 1000.0 && volume(&hole) > 950.0);
        let concave = extrude(&[[0., 0.], [3., 0.], [3., 1.], [1., 1.], [1., 3.], [0., 3.]], 2.).unwrap();
        assert!((volume(&concave) - 10.).abs() < 1e-8);
        assert!(volume(&sphere(5., 48).unwrap()) > 510.0);
        assert!((volume(&cylinder(5., 0., 10., 48).unwrap()) - 260.8).abs() < 1.0);
        assert!(extrude(&[[0., 0.], [2., 2.], [0., 2.], [2., 0.]], 1.).is_err());
        assert!(sphere(f64::NAN, 48).is_err());
        assert!(cylinder(1., 1., 1., 100000).is_err());
    }
}
