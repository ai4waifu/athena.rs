//! 显式十进制文本 wire（仅宿主 / 调试 — 非规范二进制 schema）。

use athena_types::{Diagnostic, DiagnosticCode, NumericKind};

use crate::{integer::Integer, number::NumericValue, rational::Rational, serialization::NumericValueWire};

impl NumericValueWire {
    /// 编码为可读十进制文本（显式文本格式，非规范 wire）。
    pub fn encode_text(value: &NumericValue) -> Result<String, Diagnostic> {
        match value {
            NumericValue::Integer(n) => Ok(n.to_decimal_string()),
            NumericValue::Rational(r) => Ok(r.to_wire_string()),
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "numeric")
                .detail("operation", "encode_text")),
        }
    }

    /// 将十进制文本解码为 wire 记录（文本载荷、精确精度）。
    pub fn decode_text(kind: NumericKind, text: &str) -> Result<NumericValueWire, Diagnostic> {
        if text.len() as u32 > crate::dispatch::PORTABLE_WIRE_PAYLOAD_LIMIT_BYTES {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "numeric")
                .detail("operation", "text_payload_limit"));
        }
        let value = match kind {
            NumericKind::Integer => {
                let n = Integer::from_str(text).map_err(|_| text_err("integer"))?;
                NumericValue::integer(n)
            }
            NumericKind::Rational => {
                let r = Rational::decode_wire_text(text)?;
                NumericValue::rational(r)
            }
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "numeric")
                    .detail("operation", "decode_text_kind"));
            }
        };
        Self::encode(&value)
    }
}

fn text_err(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", op)
}

use std::str::FromStr;

impl Rational {
    /// 解码十进制文本 `numer` 或 `numer/denom`（仅文本格式）。
    pub(crate) fn decode_wire_text(s: &str) -> Result<Self, Diagnostic> {
        if let Some((n, d)) = s.split_once('/') {
            let numer = Integer::from_str(n).map_err(|_| text_err("rational_numer"))?;
            let denom = Integer::from_str(d).map_err(|_| text_err("rational_denom"))?;
            Self::try_new(numer, denom)
        }
        else {
            let n = Integer::from_str(s).map_err(|_| text_err("rational_integer"))?;
            Ok(Self::from_integer(n))
        }
    }
}
