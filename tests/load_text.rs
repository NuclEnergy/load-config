use std::path::PathBuf;

use load_config::Loader;
use serde::Deserialize;

const PEM_PATH: &str = "tests/text/nested/certs/server.pem";

#[derive(Debug, Deserialize)]
struct Config {
    service: Service,
    port_text: String,
    directive_text: String,
}

#[derive(Debug, Deserialize)]
struct Service {
    tls: Tls,
}

#[derive(Debug, Deserialize)]
struct Tls {
    cert: String,
}

fn temp_file(name: &str, content: &[u8]) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn text_directive_resolves_relative_to_containing_file() {
    let config = Loader::load_path::<Config>("tests/text/root.json").unwrap();

    assert_eq!(
        config.service.tls.cert,
        std::fs::read_to_string(PEM_PATH).unwrap()
    );
}

#[test]
fn text_content_is_not_parsed() {
    let config = Loader::load_path::<Config>("tests/text/root.json").unwrap();

    assert_eq!(config.port_text.trim_end(), "3306");
    assert_eq!(
        config.directive_text.trim_end(),
        r#"{"$path":"does-not-exist.json"}"#
    );
}

#[test]
fn loader_text_entry_points_return_the_file_content() {
    let expected = std::fs::read_to_string(PEM_PATH).unwrap();

    assert_eq!(Loader::load_text(PEM_PATH).unwrap(), expected);
    assert_eq!(
        Loader::new().text(&PEM_PATH).load::<String>().unwrap(),
        expected
    );
}

#[test]
fn text_keeps_line_endings_and_trailing_newline() {
    let content = "first\r\nsecond\n\n";
    let path = temp_file("line_endings.txt", content.as_bytes());

    assert_eq!(Loader::load_text(&path).unwrap(), content);
}

#[test]
fn text_rejects_invalid_utf8() {
    let path = temp_file("binary.der", &[0x30, 0x82, 0xff, 0xfe]);

    let msg = Loader::load_text(&path).unwrap_err().to_string();

    assert!(msg.contains("failed to read"), "unexpected error: {msg}");
}

#[test]
fn text_missing_file_returns_error() {
    let msg = Loader::load_text("tests/text/missing.pem")
        .unwrap_err()
        .to_string();

    assert!(
        msg.contains("failed to read") && msg.contains("tests/text/missing.pem"),
        "unexpected error: {msg}"
    );
}

#[test]
fn text_directive_non_string_returns_error() {
    let msg = Loader::load_value_from_path("tests/text/non_string.json")
        .unwrap_err()
        .to_string();

    assert!(
        msg.contains("special member $text expects a string value"),
        "unexpected error: {msg}"
    );
}

#[test]
fn path_directive_still_rejects_unknown_extension_and_suggests_text() {
    let msg = Loader::load_value_from_path(PEM_PATH)
        .unwrap_err()
        .to_string();

    assert!(
        msg.contains("unsupported file extension .pem") && msg.contains("$text"),
        "unexpected error: {msg}"
    );
}
