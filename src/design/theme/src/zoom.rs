//! Interface zoom steps (Ctrl + / Ctrl − / Ctrl 0).

/// Smallest zoom.
pub const MIN: f32 = 0.5;
/// Largest zoom.
pub const MAX: f32 = 2.0;
/// One Ctrl + / Ctrl − step.
pub const STEP: f32 = 0.1;
/// The size of the mockups.
pub const DEFAULT: f32 = 1.0;

/// `zoom` within [`MIN`]..=[`MAX`], rounded to hundredths so repeated
/// steps do not accumulate float noise. Non-finite values reset to
/// [`DEFAULT`].
pub fn clamp(zoom: f32) -> f32 {
    if !zoom.is_finite() {
        return DEFAULT;
    }
    (zoom.clamp(MIN, MAX) * 100.0).round() / 100.0
}

/// One step larger.
pub fn zoom_in(zoom: f32) -> f32 {
    clamp(zoom + STEP)
}

/// One step smaller.
pub fn zoom_out(zoom: f32) -> f32 {
    clamp(zoom - STEP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_stay_in_range_and_round() {
        assert!((zoom_in(1.0) - 1.1).abs() < f32::EPSILON);
        assert!((zoom_out(zoom_in(1.0)) - 1.0).abs() < f32::EPSILON);
        assert!((zoom_in(MAX) - MAX).abs() < f32::EPSILON);
        assert!((zoom_out(MIN) - MIN).abs() < f32::EPSILON);
        assert!((clamp(f32::NAN) - DEFAULT).abs() < f32::EPSILON);
    }
}
