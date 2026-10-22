use super::*;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) struct Files(pub PathBuf);
impl Files {
    pub(crate) fn new() -> Self {
        // Windows clocks can return the same timestamp to concurrent tests.
        // Keep each live fixture distinct without relying on clock resolution.
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("cadcraft-attachments-{}-{id}-{serial}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(crate) fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
    pub(crate) fn image(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        let image = image::RgbImage::from_fn(32, 24, |x, y| image::Rgb([255, (x * 5) as u8, (y * 5) as u8]));
        image.save(&path).unwrap();
        path
    }
    pub(crate) fn pdf(&self, name: &str, pages: usize) -> PathBuf {
        self.write(name, &pdf(pages))
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Entirely synthetic PDF: vector rectangle, standard-font text and an embedded RGB scan.
fn pdf(pages: usize) -> Vec<u8> {
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Count {pages} /Kids [{}] >>", (0..pages).map(|i| format!("{} 0 R", 5 + 2 * i)).collect::<Vec<_>>().join(" "))
            .into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        [
            b"<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 12 >>\nstream\n".as_slice(),
            &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
            b"\nendstream",
        ]
        .concat(),
    ];
    for i in 0..pages {
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 120] /Resources << /Font << /F1 3 0 R >> /XObject << /Im0 4 0 R >> >> /Contents {} 0 R >>", 6+2*i).into_bytes());
        let content =
            format!("1 0 0 rg 10 10 60 20 re f\n0 0 0 rg BT /F1 12 Tf 10 95 Td (CADCraft page {}) Tj ET\nq 40 0 0 40 120 20 cm /Im0 Do Q\n", i + 1);
        objects.push(format!("<< /Length {} >>\nstream\n{content}endstream", content.len()).into_bytes());
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend(object);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
    for offset in offsets.iter().skip(1) {
        out.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len()).as_bytes());
    out
}

pub(crate) fn selection(paths: &[PathBuf]) -> Selection {
    Selection { items: paths.iter().map(|p| load_file(p, &AtomicBool::new(false)).unwrap()).collect(), ..Default::default() }
}

fn wait(selection: &mut Selection) {
    let start = Instant::now();
    while selection.is_loading() && start.elapsed() < Duration::from_secs(15) {
        selection.poll();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!selection.is_loading(), "attachment worker stalled");
}

#[test]
fn pdf_text_vectors_and_scan_are_rasterized_in_page_order() {
    let files = Files::new();
    let a = load_file(&files.pdf("planta.pdf", 2), &AtomicBool::new(false)).unwrap();
    assert_eq!(a.images, 2);
    for (index, parts) in a.input.chunks_exact(2).enumerate() {
        let label = parts[0]["text"].as_str().unwrap();
        assert!(label.contains(&format!("\"page\":{}", index + 1)));
        let url = parts[1]["url"].as_str().unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(url.strip_prefix("data:image/png;base64,").unwrap()).unwrap();
        let pixels = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(pixels.dimensions(), (400, 240));
        assert!(pixels.pixels().filter(|p| p[0] > 200 && p[1] < 50 && p[2] < 50).count() > 1000);
        assert!(pixels.pixels().any(|p| p[2] > 200 && p[0] < 50), "embedded scan is missing");
        assert!(pixels.pixels().any(|p| p[0] < 50 && p[1] < 50 && p[2] < 50), "standard font text is missing");
    }
}

#[test]
fn png_jpeg_and_unicode_text_prepare_the_codex_input_without_local_paths() {
    let files = Files::new();
    let txt = files.write("medidas.txt", "Parede: 5 m; pé-direito: 2,8 m".as_bytes());
    let selected = selection(&[files.image("foto.JPG"), files.image("planta.png"), txt]);
    let input = selected.input("Desenhe as paredes").unwrap();
    assert_eq!(input.iter().filter(|v| v["type"] == "image").count(), 2);
    assert!(input.last().unwrap()["text"].as_str().unwrap().contains("pé-direito"));
    assert!(!serde_json::to_string(&input).unwrap().contains(&files.0.to_string_lossy().to_string()));
    let utf16: Vec<u8> = [vec![0xff, 0xfe], "Cota: 3,5 m – revisão".encode_utf16().flat_map(u16::to_le_bytes).collect()].concat();
    assert_eq!(decode_text(&utf16).unwrap(), "Cota: 3,5 m – revisão");
    assert_eq!(decode_text(b"\xef\xbb\xbfBOM").unwrap(), "BOM");
}

#[test]
fn corrupt_oversize_unsupported_and_excess_pages_are_rejected_without_truncation() {
    let files = Files::new();
    for (name, bytes) in [
        ("bad.pdf", b"not a pdf".as_slice()),
        ("bad.png", b"garbage"),
        ("binary.txt", b"a\0b"),
        ("empty.txt", b""),
        ("report.docx", b"zip"),
        ("badutf.txt", b"\xff"),
    ] {
        assert!(load_file(&files.write(name, bytes), &AtomicBool::new(false)).is_err(), "{name}");
    }
    assert!(load_file(&files.pdf("too-many.pdf", 13), &AtomicBool::new(false)).err().unwrap().contains("12 páginas"));
    let huge = files.0.join("huge.png");
    std::fs::File::create(&huge).unwrap().set_len(MAX_FILE_BYTES + 1).unwrap();
    assert!(load_file(&huge, &AtomicBool::new(false)).is_err());
    assert!(load_file(&files.write("long.txt", &vec![b'a'; MAX_TEXT_BYTES + 1]), &AtomicBool::new(false)).is_err());
    let mut a = load_file(&files.image("one.png"), &AtomicBool::new(false)).unwrap();
    a.images = MAX_IMAGES + 1;
    assert!(validate_budget(&[a]).is_err());
}

#[test]
fn asynchronous_selection_is_atomic_cancellable_and_retains_content_snapshot() {
    let files = Files::new();
    let txt = files.write("saved.txt", b"original dimensions");
    let ctx = egui::Context::default();
    let mut selected = Selection::default();
    selected.load(vec![txt.clone(), txt.clone()], &ctx);
    assert!(selected.input("prompt").is_err(), "loading must block Send");
    wait(&mut selected);
    assert_eq!(selected.items.len(), 1);
    std::fs::write(&txt, b"changed externally").unwrap();
    assert!(selected.input("prompt").unwrap()[1]["text"].as_str().unwrap().contains("original dimensions"));
    selected.load(vec![files.image("okay.png"), files.write("broken.pdf", b"bad")], &ctx);
    wait(&mut selected);
    assert!(selected.error.is_some());
    assert_eq!(selected.items.len(), 1, "failed batch must preserve the prior selection");
    selected.load(vec![files.pdf("cancel.pdf", 3)], &ctx);
    selected.clear();
    selected.poll();
    assert!(selected.items.is_empty());
    assert!(!selected.is_loading());
}
