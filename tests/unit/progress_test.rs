use pretty_assertions::assert_eq;

/// Parses a trtexec stdout line and extracts a progress percentage (0–100).
///
/// Recognises lines like:
/// - `[I] [TRT] Timing: 45%`
/// - `[I] ... Building TensorRT engine: 73%`
/// - plain `75%`
fn parse_trtexec_progress(line: &str) -> Option<u8> {
    let pct_str = line.split('%').next()?;
    let last_token = pct_str.split_whitespace().next_back()?;
    let value: u8 = last_token.parse().ok()?;
    Some(value.min(100))
}

/// Aggregates a list of log lines and returns the maximum progress seen.
fn max_progress_from_lines(lines: &[&str]) -> u8 {
    lines
        .iter()
        .filter_map(|l| parse_trtexec_progress(l))
        .max()
        .unwrap_or(0)
}

#[test]
fn parses_bare_percentage() {
    assert_eq!(parse_trtexec_progress("75%"), Some(75));
}

#[test]
fn parses_trt_timing_line() {
    assert_eq!(parse_trtexec_progress("[I] [TRT] Timing: 45%"), Some(45));
}

#[test]
fn parses_building_engine_line() {
    assert_eq!(
        parse_trtexec_progress("[I] Building TensorRT engine: 73%"),
        Some(73)
    );
}

#[test]
fn returns_none_for_no_percentage() {
    assert_eq!(parse_trtexec_progress("[I] Starting conversion..."), None);
    assert_eq!(parse_trtexec_progress(""), None);
}

#[test]
fn clamps_over_100() {
    assert_eq!(parse_trtexec_progress("150%"), Some(100));
}

#[test]
fn aggregates_max_progress() {
    let lines = [
        "[I] Building TensorRT engine: 20%",
        "[I] Timing: 45%",
        "[I] Timing: 80%",
        "[I] Finalization complete",
    ];
    assert_eq!(max_progress_from_lines(&lines), 80);
}

#[test]
fn returns_zero_for_no_matching_lines() {
    let lines = ["[I] Start", "[W] Some warning"];
    assert_eq!(max_progress_from_lines(&lines), 0);
}
