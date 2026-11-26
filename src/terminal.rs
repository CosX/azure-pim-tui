use std::io::{self, Write};

use crossterm::{
    event::{DisableFocusChange, EnableFocusChange},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[cfg(unix)]
const ENABLE_CLICK_TRACKING: &str = "\x1b[?1000h\x1b[?1006h";
#[cfg(unix)]
const DISABLE_CLICK_TRACKING: &str = "\x1b[?1006l\x1b[?1000l";

#[cfg(unix)]
fn enable_mouse(out: &mut impl Write) -> io::Result<()> {
    out.write_all(ENABLE_CLICK_TRACKING.as_bytes())?;
    out.flush()
}

#[cfg(unix)]
fn disable_mouse(out: &mut impl Write) -> io::Result<()> {
    out.write_all(DISABLE_CLICK_TRACKING.as_bytes())?;
    out.flush()
}

#[cfg(not(unix))]
fn enable_mouse(out: &mut impl Write) -> io::Result<()> {
    execute!(out, crossterm::event::EnableMouseCapture)
}

#[cfg(not(unix))]
fn disable_mouse(out: &mut impl Write) -> io::Result<()> {
    execute!(out, crossterm::event::DisableMouseCapture)
}

pub fn enter(mouse: bool) -> io::Result<()> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen, EnableFocusChange)?;
    if mouse {
        enable_mouse(&mut out)?;
    }
    Ok(())
}

pub fn leave(mouse: bool) -> io::Result<()> {
    let mut out = io::stdout();
    if mouse {
        disable_mouse(&mut out)?;
    }
    execute!(out, DisableFocusChange, LeaveAlternateScreen)?;
    disable_raw_mode()
}

pub fn install_panic_hook(mouse: bool) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = leave(mouse);
        let _ = execute!(io::stdout(), crossterm::cursor::Show);
        previous(info);
    }));
}
