//! Fonts embedded in the binary (licenses next to the files in `fonts/`).

use std::borrow::Cow;

use crate::ThemeError;

/// Inter (interface) and `JetBrains Mono` (code) in the weights the mockups use.
pub const EMBEDDED_FONTS: &[&[u8]] = &[
    include_bytes!("../fonts/inter/Inter-Regular.ttf"),
    include_bytes!("../fonts/inter/Inter-Medium.ttf"),
    include_bytes!("../fonts/inter/Inter-SemiBold.ttf"),
    include_bytes!("../fonts/jetbrains-mono/JetBrainsMono-Regular.ttf"),
    include_bytes!("../fonts/jetbrains-mono/JetBrainsMono-Medium.ttf"),
];

/// Registers the embedded fonts with the platform text system. Call once at
/// startup, before opening windows.
///
/// # Errors
///
/// Fails if the text system rejects a font file.
pub fn register_fonts(cx: &gpui::App) -> Result<(), ThemeError> {
    let fonts = EMBEDDED_FONTS
        .iter()
        .map(|bytes| Cow::Borrowed(*bytes))
        .collect();
    cx.text_system()
        .add_fonts(fonts)
        .map_err(|error| ThemeError::Fonts(error.to_string()))
}
