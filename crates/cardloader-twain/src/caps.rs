//! Capability containers (`TW_ONEVALUE`, `TW_ENUMERATION`, `TW_ARRAY`,
//! `TW_RANGE`) <-> JSON. These functions work on the raw little-endian bytes
//! of a locked container so they are platform independent and testable; the
//! session code only locks/unlocks handles around them.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::twain_ffi::*;
use serde_json::{json, Value};

/// Names of the capabilities this tool reports or negotiates.
pub const KNOWN_CAPS: &[(u16, &str)] = &[
    (CAP_XFERCOUNT, "CAP_XFERCOUNT"),
    (ICAP_COMPRESSION, "ICAP_COMPRESSION"),
    (ICAP_PIXELTYPE, "ICAP_PIXELTYPE"),
    (ICAP_UNITS, "ICAP_UNITS"),
    (ICAP_XFERMECH, "ICAP_XFERMECH"),
    (CAP_FEEDERENABLED, "CAP_FEEDERENABLED"),
    (CAP_FEEDERLOADED, "CAP_FEEDERLOADED"),
    (CAP_SUPPORTEDCAPS, "CAP_SUPPORTEDCAPS"),
    (CAP_AUTOFEED, "CAP_AUTOFEED"),
    (CAP_INDICATORS, "CAP_INDICATORS"),
    (CAP_PAPERDETECTABLE, "CAP_PAPERDETECTABLE"),
    (CAP_UICONTROLLABLE, "CAP_UICONTROLLABLE"),
    (CAP_DEVICEONLINE, "CAP_DEVICEONLINE"),
    (CAP_DUPLEX, "CAP_DUPLEX"),
    (CAP_DUPLEXENABLED, "CAP_DUPLEXENABLED"),
    (CAP_CUSTOMDSDATA, "CAP_CUSTOMDSDATA"),
    (CAP_DOUBLEFEEDDETECTION, "CAP_DOUBLEFEEDDETECTION"),
    (ICAP_FRAMES, "ICAP_FRAMES"),
    (ICAP_XRESOLUTION, "ICAP_XRESOLUTION"),
    (ICAP_YRESOLUTION, "ICAP_YRESOLUTION"),
    (ICAP_SUPPORTEDSIZES, "ICAP_SUPPORTEDSIZES"),
    (ICAP_BITDEPTH, "ICAP_BITDEPTH"),
    (
        ICAP_AUTOMATICBORDERDETECTION,
        "ICAP_AUTOMATICBORDERDETECTION",
    ),
    (ICAP_AUTOMATICDESKEW, "ICAP_AUTOMATICDESKEW"),
];

/// Caps queried by `probe`, after `CAP_SUPPORTEDCAPS`.
pub const PROBE_CAPS: &[u16] = &[
    CAP_DUPLEX,
    CAP_DUPLEXENABLED,
    CAP_FEEDERENABLED,
    CAP_FEEDERLOADED,
    CAP_AUTOFEED,
    CAP_XFERCOUNT,
    CAP_INDICATORS,
    CAP_UICONTROLLABLE,
    CAP_CUSTOMDSDATA,
    CAP_DOUBLEFEEDDETECTION,
    ICAP_AUTOMATICBORDERDETECTION,
    ICAP_AUTOMATICDESKEW,
    ICAP_PIXELTYPE,
    ICAP_BITDEPTH,
    ICAP_XRESOLUTION,
    ICAP_YRESOLUTION,
    ICAP_SUPPORTEDSIZES,
    ICAP_XFERMECH,
];

/// Human readable cap name; unknown ids become `0x1234` (vendor caps are >= 0x8000).
pub fn cap_name(id: u16) -> String {
    KNOWN_CAPS
        .iter()
        .find(|(c, _)| *c == id)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| format!("0x{id:04x}"))
}

pub fn type_name(ty: u16) -> &'static str {
    match ty {
        TWTY_INT8 => "INT8",
        TWTY_INT16 => "INT16",
        TWTY_INT32 => "INT32",
        TWTY_UINT8 => "UINT8",
        TWTY_UINT16 => "UINT16",
        TWTY_UINT32 => "UINT32",
        TWTY_BOOL => "BOOL",
        TWTY_FIX32 => "FIX32",
        TWTY_FRAME => "FRAME",
        TWTY_STR32 => "STR32",
        TWTY_STR64 => "STR64",
        TWTY_STR128 => "STR128",
        TWTY_STR255 => "STR255",
        TWTY_STR1024 => "STR1024",
        TWTY_UNI512 => "UNI512",
        TWTY_HANDLE => "HANDLE",
        _ => "UNKNOWN",
    }
}

/// Size in bytes of one item of type `ty` inside an array/enumeration.
pub fn item_size(ty: u16) -> Option<usize> {
    Some(match ty {
        TWTY_INT8 | TWTY_UINT8 => 1,
        TWTY_INT16 | TWTY_UINT16 | TWTY_BOOL => 2,
        TWTY_INT32 | TWTY_UINT32 | TWTY_FIX32 => 4,
        TWTY_FRAME => 16,
        TWTY_STR32 => 34,
        TWTY_STR64 => 66,
        TWTY_STR128 => 130,
        TWTY_STR255 => 256,
        TWTY_STR1024 => 1026,
        TWTY_UNI512 => 1024,
        TWTY_HANDLE => std::mem::size_of::<usize>(),
        _ => return None,
    })
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn fix32_at(b: &[u8], o: usize) -> f64 {
    TW_FIX32 {
        Whole: u16_at(b, o) as i16,
        Frac: u16_at(b, o + 2),
    }
    .to_f64()
}

/// Decode one item. `b` must hold at least `item_size(ty)` bytes (4 for 32-bit "slots").
pub fn decode_item(ty: u16, b: &[u8]) -> Value {
    match ty {
        TWTY_INT8 => json!(b[0] as i8),
        TWTY_UINT8 => json!(b[0]),
        TWTY_INT16 => json!(u16_at(b, 0) as i16),
        TWTY_UINT16 => json!(u16_at(b, 0)),
        TWTY_BOOL => json!(u16_at(b, 0) != 0),
        TWTY_INT32 => json!(u32_at(b, 0) as i32),
        TWTY_UINT32 => json!(u32_at(b, 0)),
        TWTY_FIX32 => json!(fix32_at(b, 0)),
        TWTY_FRAME => json!({
            "left": fix32_at(b, 0), "top": fix32_at(b, 4),
            "right": fix32_at(b, 8), "bottom": fix32_at(b, 12),
        }),
        TWTY_STR32 | TWTY_STR64 | TWTY_STR128 | TWTY_STR255 | TWTY_STR1024 => {
            let n = item_size(ty).unwrap_or(0).min(b.len());
            let end = b[..n].iter().position(|&c| c == 0).unwrap_or(n);
            json!(String::from_utf8_lossy(&b[..end]))
        }
        _ => Value::Null, // handles / UNI512: not meaningful outside the process
    }
}

/// Total byte length of a container, computed from its header. `header` must hold
/// at least the first 14 bytes (or the whole container if it is shorter).
pub fn container_len(con_type: u16, header: &[u8]) -> Option<usize> {
    if header.len() < 6 {
        return None;
    }
    let ty = u16_at(header, 0);
    let isz = item_size(ty)?;
    Some(match con_type {
        // Values larger than 32 bits (frames, strings) are stored inline from offset 2.
        TWON_ONEVALUE => 2 + isz.max(4),
        TWON_RANGE => 22,
        TWON_ARRAY => 6 + isz * (u32_at(header, 2) as usize).min(MAX_ITEMS),
        TWON_ENUMERATION => 14 + isz * (u32_at(header, 2) as usize).min(MAX_ITEMS),
        _ => return None,
    })
}

/// Upper bound to protect against garbage `NumItems` from a buggy source.
const MAX_ITEMS: usize = 4096;

/// Decode a whole container into JSON.
pub fn decode_container(con_type: u16, b: &[u8]) -> Result<Value, String> {
    let need = container_len(con_type, b).ok_or("unknown or truncated container")?;
    if b.len() < need {
        return Err(format!("container truncated ({} < {need})", b.len()));
    }
    let ty = u16_at(b, 0);
    let isz = item_size(ty).unwrap_or(4);
    let items = |start: usize, n: usize| -> Vec<Value> {
        (0..n.min(MAX_ITEMS))
            .map(|i| decode_item(ty, &b[start + i * isz..]))
            .collect()
    };
    Ok(match con_type {
        TWON_ONEVALUE => {
            json!({"con": "onevalue", "type": type_name(ty), "value": decode_item(ty, &b[2..])})
        }
        TWON_ARRAY => {
            json!({"con": "array", "type": type_name(ty), "values": items(6, u32_at(b, 2) as usize)})
        }
        TWON_ENUMERATION => {
            let values = items(14, u32_at(b, 2) as usize);
            let pick = |i: u32| values.get(i as usize).cloned().unwrap_or(Value::Null);
            json!({
                "con": "enumeration", "type": type_name(ty),
                "current": pick(u32_at(b, 6)), "default": pick(u32_at(b, 10)), "values": values,
            })
        }
        TWON_RANGE => {
            let slot = |o: usize| decode_item(ty, &b[o..o + 4]);
            json!({
                "con": "range", "type": type_name(ty),
                "min": slot(2), "max": slot(6), "step": slot(10), "default": slot(14), "current": slot(18),
            })
        }
        _ => unreachable!(),
    })
}

/// The "current" value of a decoded container (array -> list of values).
pub fn current_value(container: &Value) -> Value {
    match container["con"].as_str() {
        Some("onevalue") => container["value"].clone(),
        Some("enumeration") | Some("range") => container["current"].clone(),
        Some("array") => container["values"].clone(),
        _ => Value::Null,
    }
}

/// Encode a scalar JSON value into a 32-bit item slot.
pub fn encode_slot(ty: u16, v: &Value) -> Result<u32, String> {
    let bad = || format!("cannot encode {v} as {}", type_name(ty));
    Ok(match ty {
        TWTY_BOOL => match v {
            Value::Bool(b) => *b as u32,
            _ => (v.as_i64().ok_or_else(bad)? != 0) as u32,
        },
        TWTY_FIX32 => TW_FIX32::from_f64(v.as_f64().ok_or_else(bad)?).to_u32(),
        TWTY_INT8 => v.as_i64().ok_or_else(bad)? as i8 as u8 as u32,
        TWTY_INT16 => v.as_i64().ok_or_else(bad)? as i16 as u16 as u32,
        TWTY_INT32 | TWTY_UINT8 | TWTY_UINT16 | TWTY_UINT32 => v.as_i64().ok_or_else(bad)? as u32,
        _ => return Err(bad()),
    })
}

/// Bytes of a `TW_ONEVALUE` container.
pub fn encode_onevalue(ty: u16, v: &Value) -> Result<Vec<u8>, String> {
    let mut out = ty.to_le_bytes().to_vec();
    out.extend_from_slice(&encode_slot(ty, v)?.to_le_bytes());
    Ok(out)
}

/// Bytes of a `TW_ARRAY` container (scalar item types only).
pub fn encode_array(ty: u16, values: &[Value]) -> Result<Vec<u8>, String> {
    let isz = item_size(ty)
        .filter(|s| *s <= 4)
        .ok_or("array item type not supported")?;
    let mut out = ty.to_le_bytes().to_vec();
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for v in values {
        out.extend_from_slice(&encode_slot(ty, v)?.to_le_bytes()[..isz]);
    }
    Ok(out)
}

/// Loose comparison of requested vs. actual cap values (FIX32 rounding, int vs float).
pub fn values_match(requested: &Value, actual: &Value) -> bool {
    match (requested, actual) {
        (Value::Number(a), Value::Number(b)) => {
            (a.as_f64().unwrap_or(f64::NAN) - b.as_f64().unwrap_or(f64::NAN)).abs() < 0.01
        }
        (Value::Bool(a), Value::Number(b)) | (Value::Number(b), Value::Bool(a)) => {
            (b.as_f64() != Some(0.0)) == *a
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| values_match(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, x)| b.get(k).is_some_and(|y| values_match(x, y)))
        }
        _ => requested == actual,
    }
}

/// `MSG_QUERYSUPPORT` bits as a list of operation names.
pub fn query_support_names(bits: u32) -> Vec<&'static str> {
    [
        (TWQC_GET, "get"),
        (TWQC_SET, "set"),
        (TWQC_GETDEFAULT, "getdefault"),
        (TWQC_GETCURRENT, "getcurrent"),
        (TWQC_RESET, "reset"),
    ]
    .iter()
    .filter(|(b, _)| bits & b != 0)
    .map(|(_, n)| *n)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le16(v: u16) -> [u8; 2] {
        v.to_le_bytes()
    }
    fn le32(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }

    #[test]
    fn onevalue_roundtrip() {
        let b = encode_onevalue(TWTY_INT16, &json!(-1)).unwrap();
        assert_eq!(b.len(), 6);
        assert_eq!(container_len(TWON_ONEVALUE, &b), Some(6));
        let c = decode_container(TWON_ONEVALUE, &b).unwrap();
        assert_eq!(c, json!({"con":"onevalue","type":"INT16","value":-1}));

        let b = encode_onevalue(TWTY_FIX32, &json!(300)).unwrap();
        assert_eq!(
            current_value(&decode_container(TWON_ONEVALUE, &b).unwrap()),
            json!(300.0)
        );

        let b = encode_onevalue(TWTY_BOOL, &json!(true)).unwrap();
        assert_eq!(b, vec![6, 0, 1, 0, 0, 0]);
        assert_eq!(
            current_value(&decode_container(TWON_ONEVALUE, &b).unwrap()),
            json!(true)
        );
    }

    #[test]
    fn enumeration_decode() {
        let mut b = Vec::new();
        b.extend_from_slice(&le16(TWTY_UINT16));
        b.extend_from_slice(&le32(3));
        b.extend_from_slice(&le32(2)); // current index
        b.extend_from_slice(&le32(0)); // default index
        for v in [0u16, 1, 2] {
            b.extend_from_slice(&le16(v));
        }
        assert_eq!(container_len(TWON_ENUMERATION, &b), Some(20));
        let c = decode_container(TWON_ENUMERATION, &b).unwrap();
        assert_eq!(c["values"], json!([0, 1, 2]));
        assert_eq!(c["current"], json!(2));
        assert_eq!(c["default"], json!(0));
        assert_eq!(current_value(&c), json!(2));
    }

    #[test]
    fn range_fix32_decode() {
        let mut b = le16(TWTY_FIX32).to_vec();
        for v in [50.0, 600.0, 1.0, 300.0, 400.5] {
            b.extend_from_slice(&le32(TW_FIX32::from_f64(v).to_u32()));
        }
        let c = decode_container(TWON_RANGE, &b).unwrap();
        assert_eq!(c["min"], json!(50.0));
        assert_eq!(c["max"], json!(600.0));
        assert_eq!(current_value(&c), json!(400.5));
    }

    #[test]
    fn array_roundtrip_and_empty() {
        let b = encode_array(TWTY_UINT16, &[json!(0), json!(1)]).unwrap();
        assert_eq!(b.len(), 10);
        let c = decode_container(TWON_ARRAY, &b).unwrap();
        assert_eq!(current_value(&c), json!([0, 1]));
        let empty = encode_array(TWTY_UINT16, &[]).unwrap();
        assert_eq!(
            decode_container(TWON_ARRAY, &empty).unwrap()["values"],
            json!([])
        );
    }

    #[test]
    fn onevalue_frame_and_string_inline() {
        let mut b = le16(TWTY_FRAME).to_vec();
        for v in [0.0, 0.0, 2.6, 3.62] {
            b.extend_from_slice(&le32(TW_FIX32::from_f64(v).to_u32()));
        }
        let c = decode_container(TWON_ONEVALUE, &b).unwrap();
        assert!((c["value"]["bottom"].as_f64().unwrap() - 3.62).abs() < 1e-4);

        let mut s = le16(TWTY_STR32).to_vec();
        s.extend_from_slice(&str32("PaperStream"));
        assert_eq!(
            decode_container(TWON_ONEVALUE, &s).unwrap()["value"],
            json!("PaperStream")
        );
    }

    #[test]
    fn truncated_and_unknown() {
        assert!(decode_container(TWON_ONEVALUE, &[4, 0, 1]).is_err());
        assert!(decode_container(99, &[4, 0, 1, 0, 0, 0]).is_err());
        assert_eq!(cap_name(0x8123), "0x8123");
        assert_eq!(cap_name(ICAP_XRESOLUTION), "ICAP_XRESOLUTION");
    }

    #[test]
    fn matching() {
        assert!(values_match(&json!(300), &json!(300.0)));
        assert!(!values_match(&json!(300), &json!(600.0)));
        assert!(values_match(&json!(true), &json!(1)));
        assert!(values_match(&json!([]), &json!([])));
        assert!(!values_match(&json!(true), &json!(false)));
        assert_eq!(query_support_names(TWQC_GET | TWQC_SET), vec!["get", "set"]);
    }
}
