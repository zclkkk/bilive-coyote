use base64::Engine;
use qrcode::QrCode;

pub fn generate_qr_data_url(content: &str) -> Result<String, String> {
    let code = QrCode::new(content).map_err(|e| format!("QR generation failed: {e}"))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(200, 200)
        .build();
    let b64 = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
    Ok(format!("data:image/svg+xml;base64,{b64}"))
}
