//! readline.rs — raw-mode line editor for PASTA
//!
//! Features:
//!   Up / Down arrows  — scroll shared 50-entry history ring
//!   Left / Right      — move cursor within current line
//!   Home / Ctrl-A     — jump to start of line
//!   End  / Ctrl-E     — jump to end of line
//!   Backspace         — delete char before cursor
//!   Delete (ESC[3~)   — delete char at cursor
//!   Ctrl-K            — kill from cursor to end
//!   Ctrl-U            — kill from start to cursor
//!   Ctrl-C            — clear current line
//!   Ctrl-D (empty)    — EOF / exit

use std::io::{self, Write};
use std::sync::Mutex;

// ── Shared history ────────────────────────────────────────────────────────────

const MAX_HISTORY: usize = 50;

static HISTORY: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Push a line into the interactive history.
pub fn history_push(line: &str) {
    let s = line.trim().to_string();
    if s.is_empty() { return; }
    let mut h = HISTORY.lock().unwrap();
    if h.last().map(|x: &String| x.as_str()) == Some(s.as_str()) { return; }
    if h.len() >= MAX_HISTORY { h.remove(0); }
    h.push(s);
}

/// Return a copy of the interactive history.
pub fn history_get() -> Vec<String> {
    HISTORY.lock().unwrap().clone()
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Read a line using the raw-mode editor; returns None on EOF.
pub fn read_line_with_history(prompt: &str) -> io::Result<Option<String>> {
    #[cfg(unix)]
    {
        if unsafe { libc::isatty(libc::STDIN_FILENO) } != 0 {
            return unix::read_line_raw(prompt);
        }
    }
    fallback(prompt)
}

fn fallback(prompt: &str) -> io::Result<Option<String>> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut line = String::new();
    let n = io::stdin().read_line(&mut line)?;
    if n == 0 { return Ok(None); }
    Ok(Some(line.trim_end_matches(&['\n', '\r'][..]).to_string()))
}

// ── Unix raw-mode implementation ──────────────────────────────────────────────

#[cfg(unix)]
mod unix {
    use super::*;
    use std::io::Read;
    use std::os::unix::io::FromRawFd;

    // termios helpers
    fn get_termios(fd: i32) -> io::Result<libc::termios> {
        let mut t: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut t) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(t)
    }

    fn set_termios(fd: i32, t: &libc::termios) -> io::Result<()> {
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, t) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn make_raw(orig: &libc::termios) -> libc::termios {
        let mut raw = *orig;
        unsafe { libc::cfmakeraw(&mut raw); }
        // Keep output processing so \r\n renders correctly
        raw.c_oflag |= libc::OPOST;
        raw
    }

    // RAII terminal restore
    struct RawGuard { fd: i32, orig: libc::termios }
    impl Drop for RawGuard {
        fn drop(&mut self) { let _ = set_termios(self.fd, &self.orig); }
    }

    // ── Repaint helpers ───────────────────────────────────────────────────────

    /// Full repaint: go to column 0, clear line, print prompt + buffer,
    /// then reposition cursor.
    fn repaint(out: &mut impl Write, prompt: &str, buf: &[char], cursor: usize) -> io::Result<()> {
        write!(out, "\r\x1b[K{}", prompt)?;
        let s: String = buf.iter().collect();
        write!(out, "{}", s)?;
        // Move cursor back from end of buffer to cursor position
        let tail = buf.len() - cursor;
        if tail > 0 { write!(out, "\x1b[{}D", tail)?; }
        out.flush()
    }

    // ── Main read loop ────────────────────────────────────────────────────────

    pub fn read_line_raw(prompt: &str) -> io::Result<Option<String>> {
        let fd = libc::STDIN_FILENO;

        let orig = get_termios(fd)?;
        let raw  = make_raw(&orig);
        set_termios(fd, &raw)?;
        let _guard = RawGuard { fd, orig };

        let mut stdout = io::stdout();
        // We need a separate reader that won't close fd on drop
        let mut stdin  = unsafe { std::fs::File::from_raw_fd(fd) };

        // Print initial prompt
        write!(stdout, "{}", prompt)?;
        stdout.flush()?;

        let mut buf: Vec<char>          = Vec::new();
        let mut cursor: usize           = 0;

        let history                     = history_get();
        let hist_len                    = history.len();
        let mut hist_idx: Option<usize> = None;
        let mut saved_buf: Vec<char>    = Vec::new();

        let mut byte = [0u8; 1];

        loop {
            if stdin.read(&mut byte)? == 0 { break; }

            match byte[0] {

                // Enter
                b'\r' | b'\n' => {
                    write!(stdout, "\r\n")?;
                    stdout.flush()?;
                    let result: String = buf.iter().collect();
                    std::mem::forget(stdin);
                    return Ok(Some(result));
                }

                // Ctrl-D on empty line = EOF
                4 => {
                    if buf.is_empty() {
                        write!(stdout, "\r\n")?;
                        stdout.flush()?;
                        std::mem::forget(stdin);
                        return Ok(None);
                    }
                }

                // Ctrl-C = clear line
                3 => {
                    buf.clear();
                    cursor = 0;
                    repaint(&mut stdout, prompt, &buf, cursor)?;
                }

                // Backspace
                127 | 8 => {
                    if cursor > 0 {
                        cursor -= 1;
                        buf.remove(cursor);
                        repaint(&mut stdout, prompt, &buf, cursor)?;
                    }
                }

                // Ctrl-A = start of line
                1 => {
                    cursor = 0;
                    repaint(&mut stdout, prompt, &buf, cursor)?;
                }

                // Ctrl-E = end of line
                5 => {
                    cursor = buf.len();
                    repaint(&mut stdout, prompt, &buf, cursor)?;
                }

                // Ctrl-K = kill to end
                11 => {
                    buf.truncate(cursor);
                    repaint(&mut stdout, prompt, &buf, cursor)?;
                }

                // Ctrl-U = kill to start
                21 => {
                    buf.drain(..cursor);
                    cursor = 0;
                    repaint(&mut stdout, prompt, &buf, cursor)?;
                }

                // ESC sequence
                0x1b => {
                    let mut seq = [0u8; 1];
                    if stdin.read(&mut seq)? == 0 { continue; }
                    if seq[0] != b'[' { continue; }
                    if stdin.read(&mut seq)? == 0 { continue; }

                    match seq[0] {
                        // Up arrow — history back
                        b'A' => {
                            if hist_len == 0 { continue; }
                            if hist_idx.is_none() {
                                saved_buf = buf.clone();
                            }
                            hist_idx = Some(match hist_idx {
                                None    => hist_len - 1,
                                Some(0) => 0,
                                Some(i) => i - 1,
                            });
                            buf    = history[hist_idx.unwrap()].chars().collect();
                            cursor = buf.len();
                            repaint(&mut stdout, prompt, &buf, cursor)?;
                        }

                        // Down arrow — history forward
                        b'B' => {
                            if let Some(idx) = hist_idx {
                                if idx + 1 < hist_len {
                                    hist_idx = Some(idx + 1);
                                    buf    = history[hist_idx.unwrap()].chars().collect();
                                } else {
                                    hist_idx = None;
                                    buf    = saved_buf.clone();
                                }
                                cursor = buf.len();
                                repaint(&mut stdout, prompt, &buf, cursor)?;
                            }
                        }

                        // Right arrow
                        b'C' => {
                            if cursor < buf.len() {
                                cursor += 1;
                                repaint(&mut stdout, prompt, &buf, cursor)?;
                            }
                        }

                        // Left arrow
                        b'D' => {
                            if cursor > 0 {
                                cursor -= 1;
                                repaint(&mut stdout, prompt, &buf, cursor)?;
                            }
                        }

                        // Extended: Home, End, Delete (ESC [ N ~)
                        b'1'..=b'9' => {
                            let code = seq[0];
                            let mut tilde = [0u8; 1];
                            let _ = stdin.read(&mut tilde);
                            match code {
                                b'1' | b'7' => { cursor = 0;          repaint(&mut stdout, prompt, &buf, cursor)?; }
                                b'4' | b'8' => { cursor = buf.len();  repaint(&mut stdout, prompt, &buf, cursor)?; }
                                b'3' if cursor < buf.len() => {
                                    buf.remove(cursor);
                                    repaint(&mut stdout, prompt, &buf, cursor)?;
                                }
                                _ => {}
                            }
                        }

                        _ => {}
                    }
                }

                // Printable ASCII
                c @ 0x20..=0x7e => {
                    hist_idx = None;
                    buf.insert(cursor, c as char);
                    cursor += 1;
                    repaint(&mut stdout, prompt, &buf, cursor)?;
                }

                // UTF-8 continuation / lead — treat as opaque char
                c @ 0x80..=0xff => {
                    hist_idx = None;
                    buf.insert(cursor, c as char);
                    cursor += 1;
                    repaint(&mut stdout, prompt, &buf, cursor)?;
                }

                _ => {}
            }
        }

        write!(stdout, "\r\n")?;
        stdout.flush()?;
        let result: String = buf.iter().collect();
        std::mem::forget(stdin);
        Ok(Some(result))
    }
}
