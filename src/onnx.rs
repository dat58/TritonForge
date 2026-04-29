//! Shared ONNX protobuf parsing utilities — compiled for both native and WASM.
//!
//! Contains a minimal hand-written protobuf walker and the public
//! [`parse_onnx_inputs`] function used by the upload form to derive
//! placeholder text for the trtexec shape arguments.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ── ONNX / protobuf field numbers ────────────────────────────────────────────

pub(crate) const MODEL_GRAPH_FIELD: u32 = 7;
pub(crate) const GRAPH_INITIALIZER_FIELD: u32 = 5;
pub(crate) const GRAPH_INPUT_FIELD: u32 = 11;
pub(crate) const GRAPH_OUTPUT_FIELD: u32 = 12;
pub(crate) const VALUE_NAME_FIELD: u32 = 1;
pub(crate) const VALUE_TYPE_FIELD: u32 = 2;
pub(crate) const TYPE_TENSOR_FIELD: u32 = 1;
pub(crate) const TENSOR_ELEM_TYPE_FIELD: u32 = 1;
pub(crate) const TENSOR_SHAPE_FIELD: u32 = 2;
pub(crate) const SHAPE_DIM_FIELD: u32 = 1;
pub(crate) const DIM_VALUE_FIELD: u32 = 1;
pub(crate) const DIM_PARAM_FIELD: u32 = 2;

// ── Public types ──────────────────────────────────────────────────────────────

/// Name and shape dimensions for one ONNX input tensor.
///
/// Dynamic axes are represented as `−1` (matching ONNX convention).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnnxTensorInfo {
    /// Tensor name as declared in the ONNX graph.
    pub name: String,
    /// Shape dimensions; `−1` indicates a dynamic (symbolic) axis.
    pub dims: Vec<i64>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parses all non-initializer input tensors from a raw ONNX model protobuf.
///
/// Returns an empty `Vec` (via `Result::Err`) when the bytes do not contain a
/// valid ONNX graph — callers typically `.unwrap_or_default()` for UI hints.
pub fn parse_onnx_inputs(bytes: &[u8]) -> Result<Vec<OnnxTensorInfo>, crate::errors::AppError> {
    let graph = first_message_field(bytes, MODEL_GRAPH_FIELD)
        .ok_or_else(|| crate::errors::AppError::Validation("ONNX model has no graph".into()))?;
    let initializers = initializer_names(graph);
    Ok(all_non_initializer_inputs(graph, &initializers))
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn all_non_initializer_inputs(graph: &[u8], initializers: &HashSet<String>) -> Vec<OnnxTensorInfo> {
    fields(graph)
        .filter(|f| f.number == GRAPH_INPUT_FIELD && f.wire_type == 2)
        .filter_map(|f| f.data)
        .filter_map(|v| {
            let name = first_string_field(v, VALUE_NAME_FIELD)?;
            let type_proto = first_message_field(v, VALUE_TYPE_FIELD)?;
            let tensor_type = first_message_field(type_proto, TYPE_TENSOR_FIELD)?;
            let shape = first_message_field(tensor_type, TENSOR_SHAPE_FIELD)?;
            Some(OnnxTensorInfo {
                name,
                dims: parse_shape_dims(shape),
            })
        })
        .filter(|t| !initializers.contains(&t.name))
        .collect()
}

fn initializer_names(graph: &[u8]) -> HashSet<String> {
    fields(graph)
        .filter(|f| f.number == GRAPH_INITIALIZER_FIELD && f.wire_type == 2)
        .filter_map(|f| f.data)
        .filter_map(|init| first_string_field(init, VALUE_NAME_FIELD))
        .collect()
}

// ── Protobuf field iterator ───────────────────────────────────────────────────

pub(crate) struct ProtoField<'a> {
    pub(crate) number: u32,
    pub(crate) wire_type: u8,
    pub(crate) data: Option<&'a [u8]>,
    pub(crate) varint: Option<u64>,
}

pub(crate) struct ProtoFields<'a> {
    bytes: &'a [u8],
    offset: usize,
}

pub(crate) fn fields(bytes: &[u8]) -> ProtoFields<'_> {
    ProtoFields { bytes, offset: 0 }
}

impl<'a> Iterator for ProtoFields<'a> {
    type Item = ProtoField<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let key = read_varint(self.bytes, &mut self.offset)?;
        let number = u32::try_from(key >> 3).ok()?;
        let wire_type = u8::try_from(key & 0x07).ok()?;

        match wire_type {
            0 => read_varint(self.bytes, &mut self.offset).map(|value| ProtoField {
                number,
                wire_type,
                data: None,
                varint: Some(value),
            }),
            1 => skip_bytes(self.bytes, &mut self.offset, 8).map(|data| ProtoField {
                number,
                wire_type,
                data: Some(data),
                varint: None,
            }),
            2 => read_length_delimited(self.bytes, &mut self.offset).map(|data| ProtoField {
                number,
                wire_type,
                data: Some(data),
                varint: None,
            }),
            5 => skip_bytes(self.bytes, &mut self.offset, 4).map(|data| ProtoField {
                number,
                wire_type,
                data: Some(data),
                varint: None,
            }),
            _ => None,
        }
    }
}

// ── Low-level protobuf readers ────────────────────────────────────────────────

pub(crate) fn first_message_field(bytes: &[u8], field_number: u32) -> Option<&[u8]> {
    fields(bytes)
        .find(|field| field.number == field_number && field.wire_type == 2)
        .and_then(|field| field.data)
}

pub(crate) fn first_string_field(bytes: &[u8], field_number: u32) -> Option<String> {
    let raw = first_message_field(bytes, field_number)?;
    std::str::from_utf8(raw).ok().map(str::to_owned)
}

pub(crate) fn first_varint_field(bytes: &[u8], field_number: u32) -> Option<u64> {
    fields(bytes)
        .find(|field| field.number == field_number && field.wire_type == 0)
        .and_then(|field| field.varint)
}

pub(crate) fn parse_shape_dims(shape: &[u8]) -> Vec<i64> {
    fields(shape)
        .filter(|field| field.number == SHAPE_DIM_FIELD && field.wire_type == 2)
        .filter_map(|field| field.data)
        .map(parse_dim)
        .collect()
}

pub(crate) fn parse_dim(dim: &[u8]) -> i64 {
    if let Some(value) = first_varint_field(dim, DIM_VALUE_FIELD) {
        return i64::try_from(value).unwrap_or(-1);
    }
    if first_message_field(dim, DIM_PARAM_FIELD).is_some() {
        return -1;
    }
    -1
}

fn read_length_delimited<'a>(bytes: &'a [u8], offset: &mut usize) -> Option<&'a [u8]> {
    let len = usize::try_from(read_varint(bytes, offset)?).ok()?;
    skip_bytes(bytes, offset, len)
}

fn skip_bytes<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = offset.checked_add(len)?;
    let data = bytes.get(*offset..end)?;
    *offset = end;
    Some(data)
}

fn read_varint(bytes: &[u8], offset: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(*offset)?;
        *offset += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_onnx_inputs_returns_error_on_empty_bytes() {
        assert!(parse_onnx_inputs(&[]).is_err());
    }

    #[test]
    fn parse_dim_returns_negative_one_for_dynamic_axis() {
        // A dim proto that has field 2 (DIM_PARAM) instead of field 1 (DIM_VALUE)
        // field tag for field 2, wire_type 2: (2 << 3) | 2 = 18 = 0x12
        // followed by length 3, then "N" (any string)
        let dim_bytes: &[u8] = &[0x12, 0x01, b'N'];
        assert_eq!(parse_dim(dim_bytes), -1);
    }

    #[test]
    fn parse_dim_returns_value_for_static_axis() {
        // field tag for field 1 (DIM_VALUE), wire_type 0: (1 << 3) | 0 = 8 = 0x08
        // followed by varint 224
        let dim_bytes: &[u8] = &[0x08, 0xe0, 0x01]; // 0xe0 | 0x01<<7 = 224
        assert_eq!(parse_dim(dim_bytes), 224);
    }
}
