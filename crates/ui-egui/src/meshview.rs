//! Standalone STL workspace. Geometry changes always go through engine commands.
use crate::CadApp;
use cadcraft_render::mesh::Camera;
#[cfg(not(target_arch = "wasm32"))]
use cadcraft_render::mesh::Frame;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq)]
struct Key {
    uid: u64,
    revision: u64,
    camera: Camera,
    width: u32,
    height: u32,
}
#[derive(Default)]
pub struct State {
    uid: Option<u64>,
    camera: Option<Camera>,
    displayed: Option<Key>,
    texture: Option<egui::TextureHandle>,
    error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pending: Option<std::sync::mpsc::Receiver<(Key, Result<Frame, String>)>>,
    axis: usize,
    degrees: f64,
    factor: f64,
    size: f64,
    offset: [f64; 3],
}

impl State {
    fn open(&mut self, uid: u64, mesh: &cadcraft_doc::mesh::Mesh) {
        if self.uid != Some(uid) {
            self.uid = Some(uid);
            self.camera = Some(Camera::fit(mesh));
            self.error = None;
            self.texture = None;
            self.displayed = None;
            self.factor = 1.0;
            self.size = mesh.size().z;
            self.degrees = 90.0;
            self.axis = 2;
            self.offset = [0.0; 3];
        }
    }
}

pub fn command(app: &mut CadApp, p: &Value) -> Result<Value, String> {
    let mesh = app.session.doc().map_err(|e| e.to_string())?.mesh.as_ref().ok_or("Open an STL model first")?;
    let uid = app.session.state().map_err(|e| e.to_string())?.uid;
    let state = &mut app.meshview;
    state.open(uid, mesh);
    let mut camera = state.camera.unwrap_or_else(|| Camera::fit(mesh));
    match p.get("view").and_then(Value::as_str).unwrap_or("fit") {
        "fit" => camera = Camera::fit(mesh),
        "front" => {
            camera.yaw = -90.0;
            camera.pitch = 0.0;
        }
        "back" => {
            camera.yaw = 90.0;
            camera.pitch = 0.0;
        }
        "right" => {
            camera.yaw = 0.0;
            camera.pitch = 0.0;
        }
        "top" => {
            camera.yaw = -90.0;
            camera.pitch = 90.0;
        }
        "isometric" => {
            camera.yaw = -45.0;
            camera.pitch = 35.264389682754654;
        }
        _ => return Err("view must be fit, front, back, right, top or isometric".into()),
    }
    state.camera = Some(camera);
    Ok(json!({"yaw":camera.yaw,"pitch":camera.pitch,"zoom":camera.zoom}))
}

pub fn show(app: &mut CadApp, ui: &mut egui::Ui) {
    let Some((uid, revision, mesh, title, undo, redo)) = app
        .session
        .state()
        .ok()
        .and_then(|s| s.doc.mesh.clone().map(|m| (s.uid, s.revision, m, s.title.clone(), !s.undo.is_empty(), !s.redo.is_empty())))
    else {
        return;
    };
    let mut state = std::mem::take(&mut app.meshview);
    state.open(uid, &mesh);
    let mut request = None;
    ui.horizontal_wrapped(|ui| {
        ui.heading(crate::tl!("STL 3D Model"));
        ui.label(&title);
        for (label, id, enabled) in [
            ("Open STL Model...", "ui.mesh.open", true),
            ("Codex Assistant", "ui.agent", true),
            ("Save STL As...", "ui.saveas", true),
            ("Undo", "undo", undo),
            ("Redo", "redo", redo),
        ] {
            if ui.add_enabled(enabled, egui::Button::new(crate::i18n::t(label))).clicked() {
                request = Some((id, Value::Null));
            }
        }
    });
    let dims = mesh.size();
    let unit_label = if mesh.unit == cadcraft_doc::mesh::Unit::Unknown { crate::tl!("Not specified") } else { mesh.unit.name() };
    ui.label(format!("{}: {}   ·   X {:.3} × Y {:.3} × Z {:.3} {}", crate::tl!("Triangles"), mesh.count(), dims.x, dims.y, dims.z, unit_label));
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::tl!("Original STL unit"));
        let mut unit = mesh.unit;
        egui::ComboBox::from_id_salt("mesh_unit")
            .selected_text(if unit == cadcraft_doc::mesh::Unit::Unknown { crate::tl!("Not specified") } else { unit.name() })
            .show_ui(ui, |ui| {
                for u in [
                    cadcraft_doc::mesh::Unit::Unknown,
                    cadcraft_doc::mesh::Unit::Mm,
                    cadcraft_doc::mesh::Unit::Cm,
                    cadcraft_doc::mesh::Unit::M,
                    cadcraft_doc::mesh::Unit::Inch,
                ] {
                    ui.selectable_value(&mut unit, u, if u == cadcraft_doc::mesh::Unit::Unknown { crate::tl!("Not specified") } else { u.name() });
                }
            });
        if unit != mesh.unit {
            request = Some(("mesh.units", json!({"unit":unit.name(),"convert":false})));
        }
        if mesh.unit == cadcraft_doc::mesh::Unit::Unknown {
            ui.small(crate::tl!("STL has no unit. Choose the original unit before physical measurements."));
        }
    });
    egui::CollapsingHeader::new(crate::tl!("Transform 3D Model")).default_open(true).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::tl!("Axis"));
            for (index, name) in ["X", "Y", "Z"].iter().enumerate() {
                ui.selectable_value(&mut state.axis, index, *name);
            }
            let axis = match state.axis {
                0 => "x",
                1 => "y",
                _ => "z",
            };
            ui.add(egui::DragValue::new(&mut state.degrees).speed(1.0).suffix("°"));
            if ui.button(crate::tl!("Rotate model")).clicked() {
                request = Some(("mesh.rotate", json!({"axis":axis,"degrees":state.degrees})));
            }
            ui.separator();
            ui.label(crate::tl!("Target size"));
            ui.add(egui::DragValue::new(&mut state.size).speed(0.5).range(1e-6..=1e8));
            if ui.button(crate::tl!("Resize proportionally")).clicked() {
                request = Some(("mesh.resize", json!({"axis":axis,"size":state.size})));
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::tl!("Scale factor"));
            ui.add(egui::DragValue::new(&mut state.factor).speed(0.01).range(1e-6..=1e6));
            if ui.button(crate::tl!("Scale model")).clicked() {
                request = Some(("mesh.scale", json!({"factor":state.factor})));
            }
            for (index, name) in ["X", "Y", "Z"].iter().enumerate() {
                if let Some(v) = state.offset.get_mut(index) {
                    ui.add(egui::DragValue::new(v).prefix(format!("{name}: ")).speed(0.5));
                }
            }
            if ui.button(crate::tl!("Move model")).clicked() {
                request = Some(("mesh.move", json!({"offset":state.offset})));
            }
            if ui.button(crate::tl!("Center on ground")).clicked() {
                request = Some(("mesh.center", json!({"ground":true})));
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        for (label, view) in
            [("Fit model", "fit"), ("Front", "front"), ("Back", "back"), ("Top", "top"), ("Right", "right"), ("Isometric", "isometric")]
        {
            if ui.button(crate::i18n::t(label)).clicked() {
                request = Some(("ui.mesh.view", json!({"view":view})));
            }
        }
        ui.small(crate::tl!("Drag to orbit · right-drag to pan · wheel to zoom"));
    });
    if let Some(error) = &state.error {
        ui.add(egui::Label::new(error).wrap());
    }
    let available = ui.available_size().max(egui::vec2(20.0, 20.0));
    let (rect, response) = ui.allocate_exact_size(available, egui::Sense::drag());
    let mut camera = state.camera.unwrap_or_else(|| Camera::fit(&mesh));
    if response.dragged_by(egui::PointerButton::Primary) {
        let delta = ui.input(|i| i.pointer.delta());
        camera.yaw = (camera.yaw - f64::from(delta.x) * 0.5).rem_euclid(360.0);
        camera.pitch = (camera.pitch + f64::from(delta.y) * 0.5).clamp(-89.0, 89.0);
    }
    if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
        let delta = ui.input(|i| i.pointer.delta());
        camera.pan[0] = (camera.pan[0] + f64::from(delta.x / rect.width())).clamp(-5.0, 5.0);
        camera.pan[1] = (camera.pan[1] + f64::from(delta.y / rect.height())).clamp(-5.0, 5.0);
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        camera.zoom = (camera.zoom * (f64::from(scroll) * 0.002).exp()).clamp(0.05, 20.0);
    }
    state.camera = Some(camera);
    let ratio = (1280.0 / rect.width()).min(1024.0 / rect.height()).min(1.0);
    let key = Key { uid, revision, camera, width: (rect.width() * ratio).max(1.0) as u32, height: (rect.height() * ratio).max(1.0) as u32 };
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(result) = state.pending.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(v) => Some(v),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(_) => Some((key, Err("3D renderer stopped. Use Fit model to retry.".into()))),
        }) {
            state.pending = None;
            let (done, result) = result;
            if done.uid == uid {
                match result {
                    Ok(frame) => {
                        state.texture = Some(ui.ctx().load_texture(
                            "mesh_3d_frame",
                            egui::ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba),
                            egui::TextureOptions::LINEAR,
                        ));
                        state.error = None;
                    }
                    Err(e) => state.error = Some(e),
                }
                state.displayed = Some(done);
            }
        }
        if state.pending.is_none() && state.displayed != Some(key) {
            let (tx, rx) = std::sync::mpsc::channel();
            let ctx = ui.ctx().clone();
            match std::thread::Builder::new().name("cadcraft-stl-view".into()).spawn(move || {
                let result = std::panic::catch_unwind(|| cadcraft_render::mesh::render(&mesh, key.camera, key.width, key.height))
                    .unwrap_or_else(|_| Err("Unable to render STL model".into()));
                let _ = tx.send((key, result));
                ctx.request_repaint();
            }) {
                Ok(_) => state.pending = Some(rx),
                Err(e) => {
                    state.error = Some(e.to_string());
                    state.displayed = Some(key);
                }
            }
        }
        if state.pending.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(30));
        }
    }
    #[cfg(target_arch = "wasm32")]
    if state.displayed != Some(key) {
        match cadcraft_render::mesh::render(&mesh, key.camera, key.width, key.height) {
            Ok(frame) => {
                state.texture = Some(ui.ctx().load_texture(
                    "mesh_3d_frame",
                    egui::ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba),
                    egui::TextureOptions::LINEAR,
                ));
                state.error = None;
            }
            Err(e) => state.error = Some(e),
        }
        state.displayed = Some(key);
    }
    ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(31, 37, 46));
    if let Some(texture) = &state.texture {
        ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            crate::tl!("Preparing 3D view…"),
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
    }
    let origin = rect.left_bottom() + egui::vec2(48.0, -65.0);
    let [right, up, _] = camera.basis();
    for (axis, label, color) in [
        (cadcraft_geom::Vec3::new(1.0, 0.0, 0.0), "X", egui::Color32::from_rgb(240, 95, 95)),
        (cadcraft_geom::Vec3::new(0.0, 1.0, 0.0), "Y", egui::Color32::from_rgb(95, 220, 120)),
        (cadcraft_geom::Vec3::Z, "Z", egui::Color32::from_rgb(95, 155, 245)),
    ] {
        let end = origin + egui::vec2(axis.dot(right) as f32 * 30.0, -axis.dot(up) as f32 * 30.0);
        ui.painter().line_segment([origin, end], egui::Stroke::new(2.0, color));
        ui.painter().text(end, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.0), color);
    }
    app.meshview = state;
    crate::cmdline::keyboard(app, ui.ctx());
    if app.ui.show_command_line {
        crate::cmdline::show(app, ui, rect);
    }
    if let Some((id, params)) = request
        && let Err(e) = app.run(id, params)
    {
        app.meshview.error = Some(e);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use cadcraft_doc::mesh::Mesh;
    use cadcraft_geom::Vec3;
    use egui_kittest::{Harness, kittest::Queryable};
    use std::sync::Arc;
    fn app() -> CadApp {
        let mut session = cadcraft_engine::Session::new();
        let mesh = Mesh::new(vec![
            [Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 30.0)],
            [Vec3::ZERO, Vec3::new(0.0, 10.0, 30.0), Vec3::new(-12.0, 0.0, 0.0)],
        ])
        .unwrap();
        session.doc_mut().unwrap().mesh = Some(Arc::new(mesh));
        CadApp::new(session, crate::Services::default())
    }
    #[test]
    fn mesh_ui_changes_geometry_with_undo_and_camera_without_geometry_changes() {
        let mut a = app();
        a.run("ui.mesh.view", json!({"view":"front"})).unwrap();
        let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
            |ui, a: &mut CadApp| {
                a.logic(&ui.ctx().clone());
                a.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        assert_eq!(h.state().meshview.camera.unwrap().yaw, -90.0);
        assert!(h.query_by_label("STL 3D Model").is_some());
        let size = h.state().session.doc().unwrap().mesh.as_ref().unwrap().size();
        h.state_mut().meshview.factor = 2.0;
        h.get_by_label("Scale model").click();
        h.run_steps(3);
        assert_eq!(h.state().session.doc().unwrap().mesh.as_ref().unwrap().size(), size * 2.0);
        h.state_mut().run("undo", json!({})).unwrap();
        h.run_steps(2);
        assert_eq!(h.state().session.doc().unwrap().mesh.as_ref().unwrap().size(), size);
        let original_camera = h.state().meshview.camera.unwrap();
        for (event, steps) in [
            (egui::Event::PointerMoved(egui::pos2(500.0, 500.0)), 1),
            (
                egui::Event::PointerButton {
                    pos: egui::pos2(500.0, 500.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                1,
            ),
            (egui::Event::PointerMoved(egui::pos2(560.0, 530.0)), 2),
            (
                egui::Event::PointerButton {
                    pos: egui::pos2(560.0, 530.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                1,
            ),
        ] {
            h.event(event);
            h.run_steps(steps);
        }
        assert_ne!(h.state().meshview.camera.unwrap().yaw, original_camera.yaw);
        let before_pan = h.state().meshview.camera.unwrap().pan;
        for (event, steps) in [
            (
                egui::Event::PointerButton {
                    pos: egui::pos2(560.0, 530.0),
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                1,
            ),
            (egui::Event::PointerMoved(egui::pos2(590.0, 550.0)), 2),
            (
                egui::Event::PointerButton {
                    pos: egui::pos2(590.0, 550.0),
                    button: egui::PointerButton::Secondary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                1,
            ),
        ] {
            h.event(event);
            h.run_steps(steps);
        }
        assert_ne!(h.state().meshview.camera.unwrap().pan, before_pan);
        h.get_by_label("Right").click();
        h.run_steps(3);
        assert_eq!(h.state().meshview.camera.unwrap().yaw, 0.0);
        assert_eq!(h.state().session.doc().unwrap().mesh.as_ref().unwrap().size(), size);
        h.state_mut().cmd.buffer = "mesh.center".into();
        h.event(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        h.run_steps(3);
        assert!(h.state().cmd.buffer.is_empty());
        assert!(h.state().session.doc().unwrap().mesh.as_ref().unwrap().center().x.abs() < 1e-8);
        h.state_mut().run("new", json!({})).unwrap();
        h.run_steps(3);
        assert!(h.query_by_label("STL 3D Model").is_none());
        h.state_mut().run("document.switch", json!({"index":0})).unwrap();
        h.run_steps(3);
        assert!(h.query_by_label("STL 3D Model").is_some());
    }
    #[test]
    fn agent_inspects_transforms_verifies_and_undoes_meshes_and_refuses_wrong_document() {
        let mut a = app();
        let target = a.session.state().unwrap().uid;
        let report = crate::agent::execute(&mut a, target, "cadcraft_inspect", &json!({})).unwrap();
        assert_eq!(report["type"], "stlMesh");
        assert_eq!(report["triangles"], 2);
        for (command, params) in
            [("mesh.units", json!({"unit":"mm"})), ("mesh.resize", json!({"axis":"z","size":100})), ("mesh.center", json!({"ground":true}))]
        {
            crate::agent::execute(&mut a, target, "cadcraft_execute", &json!({"command":command,"params":params})).unwrap();
        }
        let result = crate::agent::execute(&mut a, target, "cadcraft_inspect", &json!({})).unwrap();
        assert!((result["size"]["z"].as_f64().unwrap() - 100.0).abs() < 1e-8);
        assert_eq!(result["bounds"]["min"]["z"], 0.0);
        crate::agent::execute(&mut a, target, "cadcraft_execute", &json!({"command":"ui.mesh.view","params":{"view":"front"}})).unwrap();
        assert_eq!(a.meshview.camera.unwrap().yaw, -90.0);
        let commands = crate::agent::execute(&mut a, target, "cadcraft_commands", &json!({})).unwrap();
        assert!(commands.as_array().unwrap().iter().any(|v| v["id"] == "mesh.resize" && v["enabled"] == true));
        assert!(
            crate::agent::execute(&mut a, target, "cadcraft_execute", &json!({"command":"circle","params":{"center":[0,0],"radius":1}})).is_err()
        );
        a.run("undo", json!({})).unwrap();
        a.run("undo", json!({})).unwrap();
        assert_eq!(a.session.doc().unwrap().mesh.as_ref().unwrap().size().z, 30.0);
        a.run("new", json!({})).unwrap();
        assert!(crate::agent::execute(&mut a, target, "cadcraft_execute", &json!({"command":"mesh.scale","params":{"factor":2}})).is_err());
    }
    #[test]
    fn mesh_workspace_visual_when_requested() {
        let Some(path) = std::env::var_os("CADCRAFT_MESH_UI_SHOT") else { return };
        let mut a = app();
        if let Some(data) = std::env::var_os("CADCRAFT_MESH_PREVIEW_DATA") {
            let bytes = std::fs::read(data).unwrap();
            let mut triangles = Vec::new();
            for raw in bytes.chunks_exact(72) {
                let mut t = [Vec3::ZERO; 3];
                for (v, r) in t.iter_mut().zip(raw.chunks_exact(24)) {
                    let mut c = [0.0; 3];
                    for (to, n) in c.iter_mut().zip(r.chunks_exact(8)) {
                        *to = f64::from_le_bytes(n.try_into().unwrap());
                    }
                    *v = Vec3::new(c[0], c[1], c[2]);
                }
                triangles.push(t);
            }
            a.session.doc_mut().unwrap().mesh = Some(Arc::new(Mesh::new(triangles).unwrap()));
            a.session.state_mut().unwrap().title = "Model.stl".into();
        }
        a.ui.interface_language = crate::i18n::Preference::BrazilianPortuguese;
        a.agent.open = true;
        let mut h = Harness::builder().with_size(egui::vec2(1500.0, 950.0)).wgpu().build_ui_state(
            |ui, a: &mut CadApp| {
                a.logic(&ui.ctx().clone());
                a.ui(ui);
            },
            a,
        );
        h.run_steps(4);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while h.state().meshview.texture.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            h.run_steps(2);
        }
        assert!(h.state().meshview.texture.is_some());
        h.run_steps(5);
        h.render().unwrap().save(std::path::PathBuf::from(path)).unwrap();
    }
}
