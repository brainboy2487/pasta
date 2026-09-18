use std::io::{self, Write};

#[cfg(windows)]
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
#[cfg(windows)]
use windows_sys::Win32::System::Console::{
    GetStdHandle, SetConsoleMode, GetConsoleMode, ReadConsoleInputW, 
    ENABLE_VIRTUAL_TERMINAL_PROCESSING, ENABLE_PROCESSED_INPUT,
    ENABLE_LINE_INPUT, ENABLE_ECHO_INPUT,
    STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, INPUT_RECORD,
    KEY_EVENT, KEY_EVENT_RECORD,
    RIGHT_CTRL_PRESSED, LEFT_CTRL_PRESSED,
    RIGHT_ALT_PRESSED, LEFT_ALT_PRESSED,
    SHIFT_PRESSED,
};

#[derive(Debug, Default)]
pub struct TerminalState {
    raw_enabled: bool,
    alt_screen_enabled: bool,
    cursor_hidden: bool,
    #[cfg(unix)]
    original_termios: Option<libc::termios>,
    #[cfg(windows)]
    original_input_mode: Option<u32>,
    #[cfg(windows)]
    original_output_mode: Option<u32>,
}

impl TerminalState {
    pub fn raw_enable(&mut self) -> io::Result<()> {
        if self.raw_enabled {
            return Ok(());
        }
        #[cfg(unix)]
        {
            if unsafe { libc::isatty(libc::STDIN_FILENO) } == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "term.raw_enable requires a tty on stdin",
                ));
            }
            let mut termios: libc::termios = unsafe { std::mem::zeroed() };
            if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut termios) } != 0 {
                return Err(io::Error::last_os_error());
            }
            let original = termios;
            unsafe { libc::cfmakeraw(&mut termios) };
            termios.c_oflag |= libc::OPOST;
            if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &termios) } != 0 {
                return Err(io::Error::last_os_error());
            }
            self.original_termios = Some(original);
            self.raw_enabled = true;
            return Ok(());
        }
        #[cfg(windows)]
        {
            // Get both stdin and stdout handles and their modes
            let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
            if stdin_handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "term.raw_enable failed to get stdin handle",
                ));
            }
            let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
            if stdout_handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "term.raw_enable failed to get stdout handle",
                ));
            }

            let mut in_mode = 0u32;
            if unsafe { GetConsoleMode(stdin_handle, &mut in_mode) } == 0 {
                // Try allocating a console in case the process wasn't started from one
                #[cfg(windows)]
                {
                    use windows_sys::Win32::System::Console::AllocConsole;
                    let _ = unsafe { AllocConsole() };
                }
                // retry
                if unsafe { GetConsoleMode(stdin_handle, &mut in_mode) } == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "term.raw_enable: stdin is not a console",
                    ));
                }
            }
            let mut out_mode = 0u32;
            if unsafe { GetConsoleMode(stdout_handle, &mut out_mode) } == 0 {
                // Try allocating a console and retry
                #[cfg(windows)]
                {
                    use windows_sys::Win32::System::Console::AllocConsole;
                    let _ = unsafe { AllocConsole() };
                }
                if unsafe { GetConsoleMode(stdout_handle, &mut out_mode) } == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "term.raw_enable: stdout is not a console",
                    ));
                }
            }

            // Save original modes for restoration
            self.original_input_mode = Some(in_mode);
            self.original_output_mode = Some(out_mode);

            // For input: disable line input, echo, and processed input to receive raw key events
            let new_in = in_mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT);
            if unsafe { SetConsoleMode(stdin_handle, new_in) } == 0 {
                return Err(io::Error::last_os_error());
            }

            // For output: enable virtual terminal processing so ANSI sequences work
            let new_out = out_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
            // If enabling VT fails, we don't treat it as fatal; continue if possible
            let _ = unsafe { SetConsoleMode(stdout_handle, new_out) };

            self.raw_enabled = true;
            return Ok(());
        }
        #[allow(unreachable_code)]
        Err(io::Error::new(
            io::ErrorKind::Other,
            "term.raw_enable is not supported on this platform",
        ))
    }

    pub fn raw_disable(&mut self) -> io::Result<()> {
        if !self.raw_enabled {
            return Ok(());
        }
        #[cfg(unix)]
        {
            if let Some(original) = self.original_termios.take() {
                if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &original) } != 0 {
                    self.original_termios = Some(original);
                    return Err(io::Error::last_os_error());
                }
            }
            self.raw_enabled = false;
            return Ok(());
        }
        #[cfg(windows)]
        {
            // Restore input mode if present
            if let Some(orig_in) = self.original_input_mode.take() {
                let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
                if stdin_handle != INVALID_HANDLE_VALUE {
                    unsafe { SetConsoleMode(stdin_handle, orig_in) };
                }
            }
            // Restore output mode if present
            if let Some(orig_out) = self.original_output_mode.take() {
                let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
                if stdout_handle != INVALID_HANDLE_VALUE {
                    unsafe { SetConsoleMode(stdout_handle, orig_out) };
                }
            }
            self.raw_enabled = false;
            return Ok(());
        }
        #[allow(unreachable_code)]
        Err(io::Error::new(
            io::ErrorKind::Other,
            "term.raw_disable is not supported on this platform",
        ))
    }

    pub fn read_key(&self, timeout_ms: Option<i32>) -> io::Result<String> {
        if !self.raw_enabled {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "term.read_key requires term.raw_enable() first",
            ));
        }
        #[cfg(unix)]
        {
            if unsafe { libc::isatty(libc::STDIN_FILENO) } == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "term.read_key requires a tty on stdin",
                ));
            }
            let Some(first) = read_byte(libc::STDIN_FILENO, timeout_ms.unwrap_or(-1))? else {
                return Ok(String::new());
            };
            return decode_key_from_fd(libc::STDIN_FILENO, first);
        }
        #[cfg(windows)]
        {
            return read_key_windows(timeout_ms);
        }
        #[allow(unreachable_code)]
        Err(io::Error::new(
            io::ErrorKind::Other,
            "term.read_key is not supported on this platform",
        ))
    }

    pub fn enter_alt_screen(&mut self) -> io::Result<()> {
        if self.alt_screen_enabled {
            return Ok(());
        }
        write_stdout("\x1b[?1049h")?;
        self.alt_screen_enabled = true;
        Ok(())
    }

    pub fn leave_alt_screen(&mut self) -> io::Result<()> {
        if !self.alt_screen_enabled {
            return Ok(());
        }
        write_stdout("\x1b[?1049l")?;
        self.alt_screen_enabled = false;
        Ok(())
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        if self.cursor_hidden {
            return Ok(());
        }
        write_stdout("\x1b[?25l")?;
        self.cursor_hidden = true;
        Ok(())
    }

    pub fn show_cursor(&mut self) -> io::Result<()> {
        if !self.cursor_hidden {
            return Ok(());
        }
        write_stdout("\x1b[?25h")?;
        self.cursor_hidden = false;
        Ok(())
    }

    pub fn cleanup(&mut self) {
        let _ = self.show_cursor();
        let _ = self.leave_alt_screen();
        let _ = self.raw_disable();
    }
}

pub fn is_tty() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::isatty(libc::STDIN_FILENO) != 0 && libc::isatty(libc::STDOUT_FILENO) != 0 }
    }
    #[cfg(windows)]
    {
        let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        stdin_handle != INVALID_HANDLE_VALUE && stdout_handle != INVALID_HANDLE_VALUE
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

pub fn terminal_size() -> io::Result<(usize, usize)> {
    #[cfg(unix)]
    {
        let mut winsize: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) } == 0
            && winsize.ws_col > 0
            && winsize.ws_row > 0
        {
            return Ok((winsize.ws_col as usize, winsize.ws_row as usize));
        }
    }
    #[cfg(windows)]
    {
        if let Ok((cols, rows)) = get_console_size() {
            return Ok((cols as usize, rows as usize));
        }
    }
    let cols = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(80);
    let rows = std::env::var("LINES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(24);
    Ok((cols, rows))
}

pub fn write_stdout(text: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.write_all(text.as_bytes())?;
    Ok(())
}

pub fn flush_stdout() -> io::Result<()> {
    io::stdout().flush()
}

pub fn draw_frame(rows: &[String], cursor_row: usize, cursor_col: usize) -> io::Result<()> {
    let mut output = String::new();
    for (idx, row) in rows.iter().enumerate() {
        output.push_str("\x1b[");
        output.push_str(&(idx + 1).to_string());
        output.push_str(";1H\x1b[2K");
        output.push_str(row);
    }
    output.push_str(&format!("\x1b[{};{}H", cursor_row.max(1), cursor_col.max(1)));
    write_stdout(&output)?;
    flush_stdout()
}

pub fn draw_buffer_view(
    lines: &[String],
    scroll_row: usize,
    scroll_col: usize,
    gutter: usize,
    status_line: &str,
    cursor_row: usize,
    cursor_col: usize,
) -> io::Result<()> {
    let (cols, rows) = terminal_size()?;
    let body_rows = rows.saturating_sub(1);
    let text_width = cols.saturating_sub(gutter).max(1);
    let mut output = String::new();

    for screen_y in 0..body_rows {
        let file_row = scroll_row + screen_y;
        output.push_str("\x1b[");
        output.push_str(&(screen_y + 1).to_string());
        output.push_str(";1H\x1b[2K");
        if let Some(line) = lines.get(file_row) {
            output.push_str(&(file_row + 1).to_string());
            output.push(' ');
            output.push_str(&visible_slice(line, scroll_col, text_width));
        } else {
            output.push('~');
        }
    }

    output.push_str("\x1b[");
    output.push_str(&rows.max(1).to_string());
    output.push_str(";1H\x1b[2K");
    output.push_str(status_line);
    output.push_str(&format!("\x1b[{};{}H", cursor_row.max(1), cursor_col.max(1)));
    write_stdout(&output)?;
    flush_stdout()
}

pub fn clear_screen() -> io::Result<()> {
    write_stdout("\x1b[2J\x1b[H")
}

pub fn move_cursor(row: usize, col: usize) -> io::Result<()> {
    write_stdout(&format!("\x1b[{};{}H", row.max(1), col.max(1)))
}

pub fn clear_line() -> io::Result<()> {
    write_stdout("\r\x1b[2K")
}

pub fn clear_to_end() -> io::Result<()> {
    write_stdout("\x1b[K")
}

#[cfg(unix)]
fn read_byte(fd: i32, timeout_ms: i32) -> io::Result<Option<u8>> {
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    if rc == 0 {
        return Ok(None);
    }
    let mut byte = [0u8; 1];
    let n = unsafe { libc::read(fd, byte.as_mut_ptr() as *mut libc::c_void, 1) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    if n == 0 {
        return Ok(None);
    }
    Ok(Some(byte[0]))
}

#[cfg(unix)]
fn decode_key_from_fd(fd: i32, first: u8) -> io::Result<String> {
    match first {
        b'\r' | b'\n' => Ok("Enter".to_string()),
        b'\t' => Ok("Tab".to_string()),
        0x7f | 0x08 => Ok("Backspace".to_string()),
        0x1b => {
            let mut seq = Vec::new();
            while let Some(byte) = read_byte(fd, 1)? {
                seq.push(byte);
                if seq.len() >= 8 {
                    break;
                }
            }
            Ok(decode_escape_sequence(&seq).unwrap_or_else(|| "Escape".to_string()))
        }
        0x01..=0x1a => Ok(format!("Ctrl+{}", ((first - 1) + b'A') as char)),
        0x1c => Ok("Ctrl+\\".to_string()),
        0x1d => Ok("Ctrl+]".to_string()),
        0x1e => Ok("Ctrl+^".to_string()),
        0x1f => Ok("Ctrl+_".to_string()),
        0x20..=0x7e => Ok((first as char).to_string()),
        0x80..=0xff => decode_utf8_bytes(fd, first),
        _ => Ok(String::new()),
    }
}

fn visible_slice(text: &str, scroll_col: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    text.chars().skip(scroll_col).take(width).collect()
}

#[cfg(windows)]
fn read_key_windows(timeout_ms: Option<i32>) -> io::Result<String> {
    let stdin_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if stdin_handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to get stdin handle",
        ));
    }

    let mut records: [INPUT_RECORD; 32] = unsafe { std::mem::zeroed() };
    let mut records_read = 0u32;

    if unsafe { ReadConsoleInputW(stdin_handle, records.as_mut_ptr(), 32, &mut records_read) } == 0 {
        return Err(io::Error::last_os_error());
    }

    for i in 0..records_read as usize {
        let record = unsafe { records[i] };
        if record.EventType as u32 == KEY_EVENT {
            let key_record = unsafe { record.Event.KeyEvent };
            if key_record.bKeyDown != 0 {
                return decode_windows_key(&key_record);
            }
        }
    }

    Ok(String::new())
}

#[cfg(windows)]
fn get_console_size() -> io::Result<(i32, i32)> {
    use windows_sys::Win32::System::Console::{
        GetConsoleScreenBufferInfo, CONSOLE_SCREEN_BUFFER_INFO,
    };

    let stdout_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if stdout_handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to get stdout handle",
        ));
    }

    let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
    if unsafe { GetConsoleScreenBufferInfo(stdout_handle, &mut csbi) } == 0 {
        return Err(io::Error::last_os_error());
    }

    let cols = (csbi.srWindow.Right - csbi.srWindow.Left + 1) as i32;
    let rows = (csbi.srWindow.Bottom - csbi.srWindow.Top + 1) as i32;

    if cols > 0 && rows > 0 {
        Ok((cols, rows))
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Invalid console size",
        ))
    }
}

#[cfg(windows)]
fn decode_windows_key(key_record: &KEY_EVENT_RECORD) -> io::Result<String> {
    let is_ctrl = (key_record.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED)) != 0;
    let is_alt = (key_record.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED)) != 0;
    let is_shift = (key_record.dwControlKeyState & SHIFT_PRESSED) != 0;
    let vkey = key_record.wVirtualKeyCode as u32;

    let key_name = match vkey {
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x1B => "Escape".to_string(),
        0x20 => " ".to_string(),
        0x21 => "PageUp".to_string(),
        0x22 => "PageDown".to_string(),
        0x23 => "End".to_string(),
        0x24 => "Home".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0x2C => "PrintScreen".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x30..=0x39 => ((vkey as u8) as char).to_string(),
        0x41..=0x5A => ((vkey as u8) as char).to_string().to_lowercase(),
        0x60..=0x69 => format!("F{}", vkey - 0x60 + 1),
        0x70..=0x7B => format!("F{}", vkey - 0x70 + 1),
        _ => {
            // Prefer UnicodeChar (ReadConsoleInputW delivers wide chars); fall back to AsciiChar
            let unicode_char = unsafe { key_record.uChar.UnicodeChar };
            let ascii_char = unsafe { key_record.uChar.AsciiChar };
            if unicode_char != 0 {
                if let Some(ch) = std::char::from_u32(unicode_char as u32) {
                    if ch.is_control() {
                        // Map common control letters to Ctrl+X
                        if (unicode_char as u32) <= 26 {
                            format!("Ctrl+{}", (unicode_char as u8 + b'A' - 1) as char)
                        } else {
                            ch.to_string()
                        }
                    } else {
                        ch.to_string()
                    }
                } else {
                    "Unknown".to_string()
                }
            } else if ascii_char != 0 {
                let ch = (ascii_char as u8) as char;
                if ch.is_control() {
                    if (ascii_char as u8) <= 26 {
                        format!("Ctrl+{}", (ascii_char as u8 + b'A' - 1) as char)
                    } else {
                        ch.to_string()
                    }
                } else {
                    ch.to_string()
                }
            } else {
                "Unknown".to_string()
            }
        }
    };

    // Determine ascii byte if available for legacy checks
    let ascii_byte_opt = unsafe {
        let uc = key_record.uChar.UnicodeChar;
        let ac = key_record.uChar.AsciiChar;
        if uc != 0 {
            Some(uc as u8)
        } else if ac != 0 {
            Some(ac as u8)
        } else {
            None
        }
    };

    if is_ctrl && !matches!(ascii_byte_opt.unwrap_or(0), b'a'..=b'z' | b'A'..=b'Z') {
        Ok(format!("Ctrl+{}", key_name))
    } else if is_alt {
        Ok(format!("Alt+{}", key_name))
    } else if is_shift && (vkey == 0x09) {
        Ok("Shift+Tab".to_string())
    } else {
        Ok(key_name)
    }
}

#[cfg(unix)]
fn decode_utf8_bytes(fd: i32, first: u8) -> io::Result<String> {
    let expected = utf8_expected_len(first).unwrap_or(1);
    let mut bytes = vec![first];
    while bytes.len() < expected {
        let Some(next) = read_byte(fd, 1)? else {
            break;
        };
        bytes.push(next);
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(_) => Ok(String::new()),
    }
}

fn utf8_expected_len(first: u8) -> Option<usize> {
    match first {
        0x00..=0x7f => Some(1),
        0xc0..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf7 => Some(4),
        _ => None,
    }
}

fn decode_escape_sequence(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return Some("Escape".to_string());
    }
    match bytes[0] {
        b'[' => decode_csi_sequence(&bytes[1..]),
        b'O' => match bytes.get(1).copied() {
            Some(b'H') => Some("Home".to_string()),
            Some(b'F') => Some("End".to_string()),
            _ => Some("Escape".to_string()),
        },
        b if (0x20..=0x7e).contains(&b) => Some(format!("Alt+{}", b as char)),
        _ => Some("Escape".to_string()),
    }
}

fn decode_csi_sequence(bytes: &[u8]) -> Option<String> {
    let first = *bytes.first()?;
    match first {
        b'A' => Some("Up".to_string()),
        b'B' => Some("Down".to_string()),
        b'C' => Some("Right".to_string()),
        b'D' => Some("Left".to_string()),
        b'H' => Some("Home".to_string()),
        b'F' => Some("End".to_string()),
        b'Z' => Some("Shift+Tab".to_string()),
        b'1'..=b'9' => {
            let digits: String = bytes
                .iter()
                .take_while(|b| b.is_ascii_digit() || **b == b';')
                .map(|b| *b as char)
                .collect();
            if bytes.last().copied() != Some(b'~') {
                return Some("Escape".to_string());
            }
            let code = digits.split(';').next().unwrap_or("");
            match code {
                "1" | "7" => Some("Home".to_string()),
                "2" => Some("Insert".to_string()),
                "3" => Some("Delete".to_string()),
                "4" | "8" => Some("End".to_string()),
                "5" => Some("PageUp".to_string()),
                "6" => Some("PageDown".to_string()),
                _ => Some("Escape".to_string()),
            }
        }
        _ => Some("Escape".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_escape_sequence, terminal_size, utf8_expected_len};

    #[test]
    fn decode_escape_keys_matches_window_conventions() {
        assert_eq!(decode_escape_sequence(b"[A").as_deref(), Some("Up"));
        assert_eq!(decode_escape_sequence(b"[B").as_deref(), Some("Down"));
        assert_eq!(decode_escape_sequence(b"[C").as_deref(), Some("Right"));
        assert_eq!(decode_escape_sequence(b"[D").as_deref(), Some("Left"));
        assert_eq!(decode_escape_sequence(b"[3~").as_deref(), Some("Delete"));
        assert_eq!(decode_escape_sequence(b"[5~").as_deref(), Some("PageUp"));
        assert_eq!(decode_escape_sequence(b"[6~").as_deref(), Some("PageDown"));
        assert_eq!(decode_escape_sequence(b"[H").as_deref(), Some("Home"));
        assert_eq!(decode_escape_sequence(b"[F").as_deref(), Some("End"));
    }

    #[test]
    fn decode_alt_and_function_style_sequences() {
        assert_eq!(decode_escape_sequence(b"OH").as_deref(), Some("Home"));
        assert_eq!(decode_escape_sequence(b"f").as_deref(), Some("Alt+f"));
    }

    #[test]
    fn utf8_length_helper_recognizes_multibyte_leads() {
        assert_eq!(utf8_expected_len(b'a'), Some(1));
        assert_eq!(utf8_expected_len(0xc3), Some(2));
        assert_eq!(utf8_expected_len(0xe2), Some(3));
        assert_eq!(utf8_expected_len(0xf0), Some(4));
        assert_eq!(utf8_expected_len(0x80), None);
    }

    #[test]
    fn terminal_size_falls_back_to_environment() {
        std::env::set_var("COLUMNS", "132");
        std::env::set_var("LINES", "44");
        let (cols, rows) = terminal_size().unwrap();
        assert!(cols > 0);
        assert!(rows > 0);
    }
}
