use pretty_assertions::assert_eq;

const MAX_SIZE_MB: u64 = 2048;
const MAX_SIZE_BYTES: u64 = MAX_SIZE_MB * 1024 * 1024;

fn allowed_extension(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.ends_with(".onnx")
}

fn within_size_limit(bytes: u64) -> bool {
    bytes <= MAX_SIZE_BYTES
}

#[test]
fn valid_onnx_extension_accepted() {
    assert!(allowed_extension("resnet50.onnx"));
    assert!(allowed_extension("MODEL.ONNX"));
}

#[test]
fn tensorflow_extensions_rejected() {
    assert!(!allowed_extension("model.pb"));
    assert!(!allowed_extension("saved.savedmodel"));
}

#[test]
fn invalid_extensions_rejected() {
    assert!(!allowed_extension("model.pt"));
    assert!(!allowed_extension("model.bin"));
    assert!(!allowed_extension("model.h5"));
    assert!(!allowed_extension("model"));
}

#[test]
fn size_within_limit_accepted() {
    assert!(within_size_limit(0));
    assert!(within_size_limit(1024 * 1024)); // 1 MB
    assert!(within_size_limit(MAX_SIZE_BYTES));
}

#[test]
fn size_over_limit_rejected() {
    assert!(!within_size_limit(MAX_SIZE_BYTES + 1));
    assert!(!within_size_limit(u64::MAX));
}

#[test]
fn empty_filename_rejected() {
    assert_eq!(allowed_extension(""), false);
}
