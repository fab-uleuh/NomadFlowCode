use qrcode::QrCode;

/// Render a QR code as a compact Unicode string using half-block characters.
/// Each character represents two vertical modules, giving a compact output.
fn render_qr_unicode(code: &QrCode) -> String {
    let colors = code.to_colors();
    let width = code.width();
    let height = colors.len() / width;
    let mut output = String::new();

    let pixel = |x: usize, y: usize| -> qrcode::Color { colors[y * width + x] };

    // Add quiet zone top
    let blank_line: String = " ".repeat(width + 4);

    output.push_str(&blank_line);
    output.push('\n');

    // Process two rows at a time using half-block characters
    let mut y = 0;
    while y < height {
        output.push_str("  "); // quiet zone left
        for x in 0..width {
            let top = pixel(x, y);
            let bottom = if y + 1 < height {
                pixel(x, y + 1)
            } else {
                qrcode::Color::Light
            };

            match (top, bottom) {
                (qrcode::Color::Dark, qrcode::Color::Dark) => output.push('\u{2588}'), // █
                (qrcode::Color::Dark, qrcode::Color::Light) => output.push('\u{2580}'), // ▀
                (qrcode::Color::Light, qrcode::Color::Dark) => output.push('\u{2584}'), // ▄
                (qrcode::Color::Light, qrcode::Color::Light) => output.push(' '),
            }
        }
        output.push_str("  "); // quiet zone right
        output.push('\n');
        y += 2;
    }

    output.push_str(&blank_line);
    output.push('\n');

    output
}

/// Display connection info with QR code in the terminal.
pub fn print_connection_info(connect_url: &str, secret: &str, public: bool) {
    let encoded_url = urlencoding::encode(connect_url);
    let deep_link = if secret.is_empty() {
        format!("nomadflowcode://add-server?url={encoded_url}")
    } else {
        let encoded_secret = urlencoding::encode(secret);
        format!("nomadflowcode://add-server?url={encoded_url}&secret={encoded_secret}")
    };

    let qr_block = match QrCode::new(&deep_link) {
        Ok(code) => render_qr_unicode(&code),
        Err(_) => "  [QR code generation failed]\n".to_string(),
    };

    let qr_lines: Vec<&str> = qr_block.lines().collect();
    let qr_width = qr_lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    // Compute box width: at least as wide as QR + some padding, or URL line
    let url_line = format!("  URL      : {connect_url}");
    let secret_line = if secret.is_empty() {
        String::new()
    } else {
        format!("  Secret   : {secret}")
    };

    let content_width = [
        qr_width + 2,
        url_line.chars().count() + 2,
        secret_line.chars().count() + 2,
        "  NomadFlow Server Ready  ".len(),
        "  Scan this QR code from the app  ".len(),
    ]
    .into_iter()
    .max()
    .unwrap_or(40);

    let box_width = content_width + 2; // +2 for ║ borders

    let top = format!("  ╔{}╗", "═".repeat(box_width));
    let bottom = format!("  ╚{}╝", "═".repeat(box_width));
    let sep = format!("  ╠{}╣", "═".repeat(box_width));
    let empty = format!("  ║{}║", " ".repeat(box_width));

    let center = |s: &str| -> String {
        let len = s.chars().count();
        if len >= box_width {
            return format!("  ║{s}║");
        }
        let pad = box_width - len;
        let left = pad / 2;
        let right = pad - left;
        format!("  ║{}{}{}║", " ".repeat(left), s, " ".repeat(right))
    };

    let left_align = |s: &str| -> String {
        let len = s.chars().count();
        if len >= box_width {
            return format!("  ║{s}║");
        }
        format!("  ║{}{}║", s, " ".repeat(box_width - len))
    };

    eprintln!();
    eprintln!("{top}");
    eprintln!("{}", center("NomadFlow Server Ready"));
    eprintln!("{sep}");
    eprintln!("{empty}");

    for line in &qr_lines {
        eprintln!("{}", center(line));
    }

    eprintln!("{empty}");
    eprintln!("{}", center("Scan this QR code from the app"));
    eprintln!("{}", center("or enter manually:"));
    eprintln!("{empty}");
    eprintln!("{}", left_align(&url_line));
    if !secret.is_empty() {
        eprintln!("{}", left_align(&secret_line));
    }
    eprintln!("{empty}");
    eprintln!("{bottom}");
    if public {
        eprintln!();
        eprintln!("  Public tunnel provided by fab_uleuh — free during beta.");
        eprintln!("  This may become a paid option in the future.");
        eprintln!("  You can always self-host via VPN or your own relay.");
    }
    eprintln!();
}
