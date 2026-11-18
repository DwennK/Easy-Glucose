//! Emits RGBA icon pixels for scripts/icons.py using the application's SVG renderer.
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut output = io::stdout().lock();
    for argument in std::env::args().skip(1) {
        let size = argument.parse::<u32>().expect("icon size");
        output.write_all(&easy_glucose::platform::tray::icon_rgba(size, false))?;
    }
    Ok(())
}
