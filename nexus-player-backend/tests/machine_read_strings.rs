//! R10, R11: the frozen frontend parses StreamError::ChallengeRequired with a regex, so the
//! message must keep the text that regex expects. The expected text is read from the frontend
//! source at run time, so it stays in one place.

use nexus_player_backend::streaming::error::StreamError;
use std::path::Path;

/// The text of CHALLENGE_RE before its first capture group.
fn challenge_regex_prefix() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../nexus-player-frontend/src/utils/providerAuth.ts");
    let source = std::fs::read_to_string(&path).expect("providerAuth.ts must be readable");
    let declaration = source
        .lines()
        .find(|line| line.contains("const CHALLENGE_RE"))
        .expect("providerAuth.ts must declare CHALLENGE_RE");
    let (_, pattern) = declaration
        .split_once('/')
        .expect("CHALLENGE_RE must be a regex literal");
    let (prefix, _) = pattern
        .split_once('(')
        .expect("CHALLENGE_RE must have a capture group");
    prefix.to_string()
}

#[test]
fn challenge_message_matches_the_frontend_regex() {
    let prefix = challenge_regex_prefix();
    assert!(
        !prefix.trim().is_empty(),
        "the regex prefix must not be empty"
    );

    let message = StreamError::ChallengeRequired {
        provider: "soundcloud".to_string(),
        url: "https://example.test/challenge".to_string(),
    }
    .to_string();

    assert!(
        message.starts_with(&prefix),
        "{message:?} must start with the CHALLENGE_RE prefix {prefix:?}"
    );
    assert!(
        message.ends_with("soundcloud: https://example.test/challenge"),
        "{message:?} must end with `provider: url` so that the regex captures both"
    );
}

/// R6: the message is English (ASCII) and the Russian exception comment is gone.
#[test]
fn challenge_message_is_ascii_and_exception_comment_removed() {
    let message = StreamError::ChallengeRequired {
        provider: "soundcloud".to_string(),
        url: "https://example.test/challenge".to_string(),
    }
    .to_string();
    assert!(message.is_ascii(), "{message:?} must be ASCII");

    let source = include_str!("../src/streaming/error.rs");
    assert!(
        !source.contains("Stays in Russian"),
        "error.rs must not keep the `Stays in Russian` comment"
    );
}
