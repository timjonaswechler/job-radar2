use super::{TransformDescriptor, TransformErrorKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UrlDecode {}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UrlDecodePlan;
pub const DESCRIPTOR: TransformDescriptor = TransformDescriptor { key: "url_decode" };
pub(super) const fn compile(_: &UrlDecode) -> UrlDecodePlan {
    UrlDecodePlan
}
pub(super) fn execute(_: &UrlDecodePlan, value: String) -> Result<String, TransformErrorKind> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len() {
            return Err(TransformErrorKind::InvalidPercentEncoding);
        }
        let Some(high) = hex(bytes[index + 1]) else {
            return Err(TransformErrorKind::InvalidPercentEncoding);
        };
        let Some(low) = hex(bytes[index + 2]) else {
            return Err(TransformErrorKind::InvalidPercentEncoding);
        };
        decoded.push((high << 4) | low);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|_| TransformErrorKind::InvalidUtf8)
}
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
