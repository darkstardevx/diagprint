#![no_main]

use diagprint_axum::{MAX_REQUEST_ID_LEN, RequestId};
use libfuzzer_sys::fuzz_target;

fn expected(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fuzz_target!(|data: &[u8]| {
    let value = String::from_utf8_lossy(data);

    let result = RequestId::new(value.to_string());

    assert_eq!(result.is_ok(), expected(&value),);

    if let Ok(request_id) = result {
        assert_eq!(request_id.as_str(), value.as_ref(),);
    }
});
