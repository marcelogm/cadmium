//! How real time turns into simulation quanta, and where a loop is.

/// Where a loop is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The pen is down, or about to be.
    Stroke,
    /// The pen is up and the bristles are settling.
    Settle,
    /// The finished stroke is on show; nothing is simulated.
    Pause,
}

/// How many quanta are due after `real_dt_ms` more at `speed`, and the time left
/// over. Past `cap` quanta the excess is dropped, keeping less than a quantum.
/// Negative or non-finite inputs count as 0, so the remainder stays finite.
pub(crate) fn quanta_due(
    accumulated_ms: f64,
    real_dt_ms: f64,
    speed: f64,
    quantum_ms: f64,
    cap: u32,
) -> (u32, f64) {
    let total_ms = time_or_zero(accumulated_ms) + time_or_zero(real_dt_ms) * time_or_zero(speed);
    if !total_ms.is_finite() {
        return (cap, 0.0);
    }
    let due = (total_ms / quantum_ms).floor();
    if due > f64::from(cap) {
        return (cap, total_ms.rem_euclid(quantum_ms));
    }
    (due as u32, total_ms - due * quantum_ms)
}

fn time_or_zero(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
