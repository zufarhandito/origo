//! Generated fixtures only. No customer documents or external assets.
/// Image-only pages exercise PDFium image decoding. RunLengthDecode keeps the
/// generated fixture small; these are simulated scans, not real scanner output.
pub fn scanned_book(count: usize) -> Vec<u8> {
    let kids = (0..count)
        .map(|i| format!("{} 0 R", 3 + i * 3))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {count} >>").into_bytes(),
    ];
    for index in 0..count {
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 840] /Resources << /XObject << /Scan {} 0 R >> >> /Contents {} 0 R >>", 5 + index * 3, 4 + index * 3).into_bytes());
        let content = b"q 600 0 0 840 0 0 cm /Scan Do Q\n";
        objects.push(stream("", content));
        let mut image = Vec::new();
        // 1500x2100 grayscale, white margins and simulated text lines.
        for y in 0..2100 {
            for block in 0..12 {
                let length = if block == 11 { 92 } else { 128 };
                let ink = (120..1980).contains(&y)
                    && y % 32 < 5
                    && (1..11).contains(&block)
                    && (block + index) % 7 != 0;
                image.extend_from_slice(&[(257 - length) as u8, if ink { 32 } else { 248 }]);
            }
        }
        image.push(128);
        objects.push(stream("/Type /XObject /Subtype /Image /Width 1500 /Height 2100 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /RunLengthDecode", &image));
    }
    assemble(&objects)
}
fn stream(attributes: &str, bytes: &[u8]) -> Vec<u8> {
    let mut object = format!("<< {attributes} /Length {} >>\nstream\n", bytes.len()).into_bytes();
    object.extend_from_slice(bytes);
    object.extend_from_slice(b"\nendstream");
    object
}
fn assemble(objects: &[Vec<u8>]) -> Vec<u8> {
    assemble_trailer(objects, "")
}
fn assemble_trailer(objects: &[Vec<u8>], trailer: &str) -> Vec<u8> {
    let mut document = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(document.len());
        document.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        document.extend_from_slice(object);
        document.extend_from_slice(b"\nendobj\n");
    }
    let xref = document.len();
    document.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        document.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    document.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R {trailer} >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    document
}

pub fn pdf(count: usize, mixed: bool) -> Vec<u8> {
    vector_pdf(count, mixed, false, "PAGE")
}
pub fn selection_pdf() -> Vec<u8> {
    vector_pdf(4, false, true, "PAGE")
}
pub fn selection_pdf_named(prefix: &str) -> Vec<u8> {
    vector_pdf(4, false, true, prefix)
}
fn vector_pdf(count: usize, mixed: bool, selection: bool, prefix: &str) -> Vec<u8> {
    let kids = (0..count)
        .map(|i| format!("{} 0 R", 4 + i * 2))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {count} >>"),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    ];
    for index in 0..count {
        let (width, height) = if mixed && index % 3 == 1 {
            (420, 300)
        } else {
            (300, 420)
        };
        let rotation = if selection {
            format!("/Rotate {}", index * 90)
        } else if mixed && index % 4 == 3 {
            "/Rotate 90".into()
        } else {
            String::new()
        };
        let color = if index % 2 == 0 {
            "0.95 0.2 0.15"
        } else {
            "0.1 0.25 0.95"
        };
        let extra = if selection {
            "0 -40 Td (Second line) Tj\n"
        } else {
            ""
        };
        let stream = format!(
            "{color} rg 0 0 {width} {height} re f\n1 1 1 rg BT /F1 24 Tf 25 180 Td ({prefix} {}) Tj {extra} ET\n",
            index + 1
        );
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] {rotation} /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>", 5 + index * 2));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{stream}endstream",
            stream.len()
        ));
    }
    let mut document = "%PDF-1.4\n".to_string();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(document.len());
        document.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
    }
    let xref = document.len();
    document.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for offset in offsets {
        document.push_str(&format!("{offset:010} 00000 n \n"));
    }
    document.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    ));
    document.into_bytes()
}

/// Structural markers exercise rejection; the signature marker is not a valid cryptographic signature.
pub fn export_safety_pdf(feature: &str) -> Vec<u8> {
    let (catalog, page, extra, trailer): (&str, &str, Vec<Vec<u8>>, &str) = match feature {
        "info" => (
            "",
            "",
            vec![b"<< /Producer (PDF Studio tests) /Title (Synthetic metadata) >>".to_vec()],
            "/Info 6 0 R",
        ),
        "metadata" => (
            "/Metadata 6 0 R",
            "",
            vec![stream(
                "/Type /Metadata /Subtype /XML",
                b"<metadata>test</metadata>",
            )],
            "",
        ),
        "form" => (
            "/AcroForm 6 0 R",
            "",
            vec![b"<< /Fields [] >>".to_vec()],
            "",
        ),
        "signature" => (
            "/Perms << /DocMDP 6 0 R >>",
            "",
            vec![b"<< /Type /Sig /ByteRange [0 1 2 3] /Contents <00> >>".to_vec()],
            "",
        ),
        "tagged" => (
            "/StructTreeRoot 6 0 R /MarkInfo << /Marked true >>",
            "",
            vec![b"<< /Type /StructTreeRoot /K [] >>".to_vec()],
            "",
        ),
        "bookmark" => (
            "/Outlines 6 0 R",
            "",
            vec![
                b"<< /Type /Outlines /First 7 0 R /Last 7 0 R /Count 1 >>".to_vec(),
                b"<< /Title (Part) /Parent 6 0 R /Dest [4 0 R /Fit] >>".to_vec(),
            ],
            "",
        ),
        "annotation" => (
            "",
            "/Annots [6 0 R]",
            vec![
                b"<< /Type /Annot /Subtype /Text /Rect [10 10 30 30] /Contents (Note) >>".to_vec(),
            ],
            "",
        ),
        "javascript" => (
            "/OpenAction 6 0 R",
            "",
            vec![b"<< /S /JavaScript /JS (void 0) >>".to_vec()],
            "",
        ),
        "attachment" => (
            "/Names << /EmbeddedFiles << /Names [(test) 6 0 R] >> >>",
            "",
            vec![
                b"<< /Type /Filespec /F (test.txt) /EF << /F 7 0 R >> >>".to_vec(),
                stream("/Type /EmbeddedFile", b"test"),
            ],
            "",
        ),
        "labels" => ("/PageLabels << /Nums [0 << /S /r >>] >>", "", vec![], ""),
        "unknown" => ("/FutureFeature true", "", vec![], ""),
        _ => ("", "", vec![], ""),
    };
    let mut objects=vec![format!("<< /Type /Catalog /Pages 2 0 R {catalog} >>").into_bytes(),b"<< /Type /Pages /Kids [4 0 R] /Count 1 >>".to_vec(),b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 420] /Resources << /Font << /F1 3 0 R >> >> /Contents 5 0 R {page} >>").into_bytes(),stream("",b"BT /F1 12 Tf 20 80 Td (SAFE) Tj ET")];
    objects.extend(extra);
    assemble_trailer(&objects, trailer)
}
