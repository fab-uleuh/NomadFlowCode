use std::io::Write;

use alacritty_terminal::event::EventListener;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};

/// Preamble: hide cursor, clear screen, home, SGR reset.
pub const PREAMBLE: &[u8] = b"\x1b[?25l\x1b[2J\x1b[H\x1b[0m";

/// Postamble: show cursor.
pub const POSTAMBLE_SHOW_CURSOR: &[u8] = b"\x1b[?25h";

/// Serialize the current terminal screen state to ANSI escape sequences.
///
/// Produces a self-contained snapshot: preamble (hide cursor + clear + home + SGR reset),
/// grid cells (with trailing space optimization), postamble (SGR reset + cursor position +
/// DECSCUSR cursor shape + show cursor).
pub fn snapshot<L: EventListener>(term: &Term<L>) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8192);
    let content = term.renderable_content();

    // 1. Preamble: hide cursor, clear screen, home, SGR reset
    buf.extend_from_slice(PREAMBLE);

    // 2. Collect cells per row, then emit with trailing-space trimming
    let mut row_cells: Vec<CellInfo> = Vec::with_capacity(256);
    let mut current_row: Option<usize> = None;
    let mut prev_fg: Option<Color> = None;
    let mut prev_bg: Option<Color> = None;
    let mut prev_flags = Flags::empty();
    let mut prev_emit_row: Option<usize> = None;
    let mut prev_emit_col: Option<usize> = None;

    for cell in content.display_iter {
        // Skip spacer cells for wide characters (CJK, emoji)
        if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
            continue;
        }

        let row = cell.point.line.0.max(0) as usize + 1; // 1-based for ANSI
        let col = cell.point.column.0 + 1; // 1-based for ANSI

        // When row changes, flush the buffered row
        if current_row != Some(row) {
            if !row_cells.is_empty() {
                emit_row(
                    &mut buf,
                    &row_cells,
                    &mut prev_fg,
                    &mut prev_bg,
                    &mut prev_flags,
                    &mut prev_emit_row,
                    &mut prev_emit_col,
                );
                row_cells.clear();
            }
            current_row = Some(row);
        }

        row_cells.push(CellInfo {
            row,
            col,
            c: cell.c,
            fg: cell.fg,
            bg: cell.bg,
            flags: cell.flags,
        });
    }

    // Flush last row
    if !row_cells.is_empty() {
        emit_row(
            &mut buf,
            &row_cells,
            &mut prev_fg,
            &mut prev_bg,
            &mut prev_flags,
            &mut prev_emit_row,
            &mut prev_emit_col,
        );
    }

    // 3. Postamble: reset SGR, position cursor, set cursor shape, show cursor
    buf.extend_from_slice(b"\x1b[0m");

    let cursor = content.cursor;
    let cursor_row = cursor.point.line.0.max(0) as usize + 1;
    let cursor_col = cursor.point.column.0 + 1;
    let _ = write!(buf, "\x1b[{cursor_row};{cursor_col}H");

    // Cursor shape (DECSCUSR) and visibility
    match cursor.shape {
        CursorShape::Block => buf.extend_from_slice(b"\x1b[2 q"),
        CursorShape::Underline => buf.extend_from_slice(b"\x1b[4 q"),
        CursorShape::Beam => buf.extend_from_slice(b"\x1b[6 q"),
        // DECSCUSR has no hollow block variant — use steady block as approximation
        CursorShape::HollowBlock => buf.extend_from_slice(b"\x1b[2 q"),
        CursorShape::Hidden => {} // No DECSCUSR for hidden cursor
    }

    // Show cursor — unless the application has hidden it
    if !matches!(cursor.shape, CursorShape::Hidden) {
        buf.extend_from_slice(POSTAMBLE_SHOW_CURSOR);
    }

    buf
}

struct CellInfo {
    row: usize,
    col: usize,
    c: char,
    fg: Color,
    bg: Color,
    flags: Flags,
}

/// Check if a cell is a default-style space (can be safely trimmed).
fn is_default_space(cell: &CellInfo) -> bool {
    cell.c == ' '
        && matches!(cell.fg, Color::Named(NamedColor::Foreground))
        && matches!(cell.bg, Color::Named(NamedColor::Background))
        && cell.flags.is_empty()
}

/// Emit a single row of cells with trailing-space trimming and cursor-advance optimization.
fn emit_row(
    buf: &mut Vec<u8>,
    cells: &[CellInfo],
    prev_fg: &mut Option<Color>,
    prev_bg: &mut Option<Color>,
    prev_flags: &mut Flags,
    prev_row: &mut Option<usize>,
    prev_col: &mut Option<usize>,
) {
    // Find last significant (non-default-space) cell
    let last_sig = cells.iter().rposition(|c| !is_default_space(c));
    let emit_count = match last_sig {
        Some(pos) => pos + 1,
        None => return, // Entirely empty row — skip
    };

    let cells = &cells[..emit_count];
    let mut i = 0;

    while i < cells.len() {
        let cell = &cells[i];

        // Check for runs of >3 consecutive default-style spaces within the row
        if is_default_space(cell) {
            let run_start = i;
            while i < cells.len() && is_default_space(&cells[i]) {
                i += 1;
            }
            let run_len = i - run_start;

            if run_len > 3 {
                // Check if cursor is sequential (same row, right after previous emit)
                let sequential = match (*prev_row, *prev_col) {
                    (Some(pr), Some(pc)) => cell.row == pr && cell.col == pc + 1,
                    _ => false,
                };

                if sequential {
                    // Cursor is at start of space run — relative forward is safe
                    let _ = write!(buf, "\x1b[{run_len}C");
                } else {
                    // Cursor is on wrong row or not sequential — use absolute CUP
                    let target_col = cells[i - 1].col + 1;
                    let _ = write!(buf, "\x1b[{};{}H", cell.row, target_col);
                }
                *prev_row = Some(cell.row);
                *prev_col = Some(cells[i - 1].col);
                continue;
            } else {
                // Short run — emit spaces normally (rewind i)
                i = run_start;
            }
        }

        let cell = &cells[i];

        // Position cursor if not sequential
        let need_position = match (*prev_row, *prev_col) {
            (Some(pr), Some(pc)) => !(cell.row == pr && cell.col == pc + 1),
            _ => true,
        };

        if need_position {
            let _ = write!(buf, "\x1b[{};{}H", cell.row, cell.col);
        }

        // Emit SGR if style changed.
        // Strategy: reset-then-replay — always emit \x1b[0m first, then re-emit all
        // active attributes. This is correct but slightly larger than delta-only emission.
        // Acceptable trade-off: simplicity over minimal bytes; output stays well under 20KB.
        if prev_fg.as_ref() != Some(&cell.fg)
            || prev_bg.as_ref() != Some(&cell.bg)
            || *prev_flags != cell.flags
        {
            buf.extend_from_slice(b"\x1b[0m");
            emit_flags(buf, cell.flags);
            emit_fg_color(buf, &cell.fg);
            emit_bg_color(buf, &cell.bg);

            *prev_fg = Some(cell.fg);
            *prev_bg = Some(cell.bg);
            *prev_flags = cell.flags;
        }

        // Emit character
        let c = cell.c;
        if c == ' ' && cell.flags.is_empty() {
            buf.push(b' ');
        } else {
            let mut char_buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut char_buf);
            buf.extend_from_slice(encoded.as_bytes());
        }

        *prev_row = Some(cell.row);
        *prev_col = Some(cell.col);
        i += 1;
    }
}

fn emit_flags(buf: &mut Vec<u8>, flags: Flags) {
    if flags.contains(Flags::BOLD) {
        buf.extend_from_slice(b"\x1b[1m");
    }
    if flags.contains(Flags::DIM) {
        buf.extend_from_slice(b"\x1b[2m");
    }
    if flags.contains(Flags::ITALIC) {
        buf.extend_from_slice(b"\x1b[3m");
    }
    if flags.contains(Flags::UNDERLINE) {
        buf.extend_from_slice(b"\x1b[4m");
    }
    if flags.contains(Flags::INVERSE) {
        buf.extend_from_slice(b"\x1b[7m");
    }
    if flags.contains(Flags::STRIKEOUT) {
        buf.extend_from_slice(b"\x1b[9m");
    }
}

fn emit_fg_color(buf: &mut Vec<u8>, color: &Color) {
    match color {
        Color::Named(named) => {
            if let Some(code) = named_color_fg_code(named) {
                let _ = write!(buf, "\x1b[{code}m");
            } else {
                // Default foreground or non-standard named color — emit explicit reset
                buf.extend_from_slice(b"\x1b[39m");
            }
        }
        Color::Spec(rgb) => {
            let _ = write!(buf, "\x1b[38;2;{};{};{}m", rgb.r, rgb.g, rgb.b);
        }
        Color::Indexed(idx) => {
            let _ = write!(buf, "\x1b[38;5;{idx}m");
        }
    }
}

fn emit_bg_color(buf: &mut Vec<u8>, color: &Color) {
    match color {
        Color::Named(named) => {
            if let Some(code) = named_color_bg_code(named) {
                let _ = write!(buf, "\x1b[{code}m");
            } else {
                // Default background or non-standard named color — emit explicit reset
                buf.extend_from_slice(b"\x1b[49m");
            }
        }
        Color::Spec(rgb) => {
            let _ = write!(buf, "\x1b[48;2;{};{};{}m", rgb.r, rgb.g, rgb.b);
        }
        Color::Indexed(idx) => {
            let _ = write!(buf, "\x1b[48;5;{idx}m");
        }
    }
}

fn named_color_fg_code(color: &NamedColor) -> Option<u8> {
    match color {
        NamedColor::Black => Some(30),
        NamedColor::Red => Some(31),
        NamedColor::Green => Some(32),
        NamedColor::Yellow => Some(33),
        NamedColor::Blue => Some(34),
        NamedColor::Magenta => Some(35),
        NamedColor::Cyan => Some(36),
        NamedColor::White => Some(37),
        NamedColor::BrightBlack => Some(90),
        NamedColor::BrightRed => Some(91),
        NamedColor::BrightGreen => Some(92),
        NamedColor::BrightYellow => Some(93),
        NamedColor::BrightBlue => Some(94),
        NamedColor::BrightMagenta => Some(95),
        NamedColor::BrightCyan => Some(96),
        NamedColor::BrightWhite => Some(97),
        _ => None, // Foreground/Background/Cursor etc. use defaults
    }
}

fn named_color_bg_code(color: &NamedColor) -> Option<u8> {
    match color {
        NamedColor::Black => Some(40),
        NamedColor::Red => Some(41),
        NamedColor::Green => Some(42),
        NamedColor::Yellow => Some(43),
        NamedColor::Blue => Some(44),
        NamedColor::Magenta => Some(45),
        NamedColor::Cyan => Some(46),
        NamedColor::White => Some(47),
        NamedColor::BrightBlack => Some(100),
        NamedColor::BrightRed => Some(101),
        NamedColor::BrightGreen => Some(102),
        NamedColor::BrightYellow => Some(103),
        NamedColor::BrightBlue => Some(104),
        NamedColor::BrightMagenta => Some(105),
        NamedColor::BrightCyan => Some(106),
        NamedColor::BrightWhite => Some(107),
        _ => None,
    }
}
