//! ONNX metadata extraction and Triton config.pbtxt generation.

use crate::errors::AppError;
use std::collections::HashSet;
use std::path::Path;
use tokio::fs;

const MODEL_GRAPH_FIELD: u32 = 7;
const GRAPH_INITIALIZER_FIELD: u32 = 5;
const GRAPH_INPUT_FIELD: u32 = 11;
const GRAPH_OUTPUT_FIELD: u32 = 12;
const VALUE_NAME_FIELD: u32 = 1;
const VALUE_TYPE_FIELD: u32 = 2;
const TYPE_TENSOR_FIELD: u32 = 1;
const TENSOR_ELEM_TYPE_FIELD: u32 = 1;
const TENSOR_SHAPE_FIELD: u32 = 2;
const SHAPE_DIM_FIELD: u32 = 1;
const DIM_VALUE_FIELD: u32 = 1;
const DIM_PARAM_FIELD: u32 = 2;

/// Generates `config.pbtxt` from `templates/config.pbtxt` and ONNX graph metadata.
pub async fn generate_config_pbtxt(
    model_path: &Path,
    model_name: &str,
) -> Result<String, AppError> {
    let model_bytes = fs::read(model_path).await?;
    let metadata = parse_onnx_metadata(&model_bytes)?;
    let template = read_config_template().await?;
    fill_template(&template, model_name, &metadata)
}

async fn read_config_template() -> Result<String, AppError> {
    let dir = std::env::var("TEMPLATES_DIR").unwrap_or_else(|_| "./templates".into());
    fs::read_to_string(Path::new(&dir).join("config.pbtxt"))
        .await
        .map_err(AppError::Io)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TensorMetadata {
    name: String,
    triton_type: String,
    dims: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OnnxMetadata {
    input: TensorMetadata,
    output: TensorMetadata,
}

fn parse_onnx_metadata(bytes: &[u8]) -> Result<OnnxMetadata, AppError> {
    let graph = first_message_field(bytes, MODEL_GRAPH_FIELD)
        .ok_or_else(|| AppError::Validation("ONNX model has no graph".into()))?;
    let initializers = initializer_names(graph);
    let input = parse_first_non_initializer_input(graph, &initializers)?;
    let output = parse_first_value_info(graph, GRAPH_OUTPUT_FIELD, "output")?;
    Ok(OnnxMetadata { input, output })
}

fn parse_first_non_initializer_input(
    graph: &[u8],
    initializers: &HashSet<String>,
) -> Result<TensorMetadata, AppError> {
    fields(graph)
        .filter(|field| field.number == GRAPH_INPUT_FIELD && field.wire_type == 2)
        .filter_map(|field| field.data)
        .filter_map(|value| parse_value_info(value, "input").ok())
        .find(|input| !initializers.contains(&input.name))
        .ok_or_else(|| {
            AppError::Validation("ONNX graph has no non-initializer input tensors".into())
        })
}

fn initializer_names(graph: &[u8]) -> HashSet<String> {
    fields(graph)
        .filter(|field| field.number == GRAPH_INITIALIZER_FIELD && field.wire_type == 2)
        .filter_map(|field| field.data)
        .filter_map(|initializer| first_string_field(initializer, VALUE_NAME_FIELD))
        .collect()
}

fn parse_first_value_info(
    graph: &[u8],
    field_number: u32,
    label: &str,
) -> Result<TensorMetadata, AppError> {
    let value = first_message_field(graph, field_number)
        .ok_or_else(|| AppError::Validation(format!("ONNX graph has no {label} tensors")))?;
    parse_value_info(value, label)
}

fn parse_value_info(value: &[u8], label: &str) -> Result<TensorMetadata, AppError> {
    let name = first_string_field(value, VALUE_NAME_FIELD)
        .ok_or_else(|| AppError::Validation(format!("ONNX {label} tensor has no name")))?;
    let type_proto = first_message_field(value, VALUE_TYPE_FIELD)
        .ok_or_else(|| AppError::Validation(format!("ONNX {label} tensor has no type")))?;
    let tensor_type = first_message_field(type_proto, TYPE_TENSOR_FIELD)
        .ok_or_else(|| AppError::Validation(format!("ONNX {label} is not a tensor")))?;
    let elem_type = first_varint_field(tensor_type, TENSOR_ELEM_TYPE_FIELD)
        .ok_or_else(|| AppError::Validation(format!("ONNX {label} tensor has no element type")))?;
    let shape = first_message_field(tensor_type, TENSOR_SHAPE_FIELD)
        .ok_or_else(|| AppError::Validation(format!("ONNX {label} tensor has no shape")))?;

    Ok(TensorMetadata {
        name,
        triton_type: onnx_elem_type_to_triton(elem_type)?,
        dims: parse_shape_dims(shape),
    })
}

fn fill_template(
    template: &str,
    model_name: &str,
    metadata: &OnnxMetadata,
) -> Result<String, AppError> {
    let mut rendered = template.to_owned();
    let replacements = [
        ("$MODEL_NAME", model_name.to_owned()),
        ("$INPUT_NAME", metadata.input.name.clone()),
        ("$INPUT_DATA_TYPE", metadata.input.triton_type.clone()),
        ("$INPUT_DIMENTIONS", format_dims(&metadata.input.dims)),
        ("$INPUT_DIMENSIONS", format_dims(&metadata.input.dims)),
        ("$OUTPUT_NAME", metadata.output.name.clone()),
        ("$OUTPUT_DATA_TYPE", metadata.output.triton_type.clone()),
        ("$OUTPUT_DIMENTIONS", format_dims(&metadata.output.dims)),
        ("$OUTPUT_DIMENSIONS", format_dims(&metadata.output.dims)),
    ];

    for (placeholder, value) in replacements {
        rendered = rendered.replace(placeholder, &value);
    }

    if rendered.contains('$') {
        return Err(AppError::Validation(
            "config template contains unresolved $ placeholders".into(),
        ));
    }

    Ok(rendered)
}

fn first_message_field(bytes: &[u8], field_number: u32) -> Option<&[u8]> {
    fields(bytes)
        .find(|field| field.number == field_number && field.wire_type == 2)
        .and_then(|field| field.data)
}

fn first_string_field(bytes: &[u8], field_number: u32) -> Option<String> {
    let raw = first_message_field(bytes, field_number)?;
    std::str::from_utf8(raw).ok().map(str::to_owned)
}

fn first_varint_field(bytes: &[u8], field_number: u32) -> Option<u64> {
    fields(bytes)
        .find(|field| field.number == field_number && field.wire_type == 0)
        .and_then(|field| field.varint)
}

fn parse_shape_dims(shape: &[u8]) -> Vec<i64> {
    fields(shape)
        .filter(|field| field.number == SHAPE_DIM_FIELD && field.wire_type == 2)
        .filter_map(|field| field.data)
        .map(parse_dim)
        .collect()
}

fn parse_dim(dim: &[u8]) -> i64 {
    if let Some(value) = first_varint_field(dim, DIM_VALUE_FIELD) {
        return i64::try_from(value).unwrap_or(-1);
    }
    if first_message_field(dim, DIM_PARAM_FIELD).is_some() {
        return -1;
    }
    -1
}

fn format_dims(dims: &[i64]) -> String {
    let values: Vec<String> = triton_dims(dims).iter().map(ToString::to_string).collect();
    format!("[ {} ]", values.join(", "))
}

fn triton_dims(dims: &[i64]) -> &[i64] {
    if dims.first() == Some(&-1) {
        &dims[1..]
    } else {
        dims
    }
}

fn onnx_elem_type_to_triton(elem_type: u64) -> Result<String, AppError> {
    let triton_type = match elem_type {
        1 => "TYPE_FP32",
        2 => "TYPE_UINT8",
        3 => "TYPE_INT8",
        4 => "TYPE_UINT16",
        5 => "TYPE_INT16",
        6 => "TYPE_INT32",
        7 => "TYPE_INT64",
        9 => "TYPE_BOOL",
        10 => "TYPE_FP16",
        11 => "TYPE_FP64",
        12 => "TYPE_UINT32",
        13 => "TYPE_UINT64",
        16 => "TYPE_BF16",
        other => {
            return Err(AppError::Validation(format!(
                "unsupported ONNX tensor element type: {other}"
            )));
        }
    };
    Ok(triton_type.to_owned())
}

struct ProtoField<'a> {
    number: u32,
    wire_type: u8,
    data: Option<&'a [u8]>,
    varint: Option<u64>,
}

fn fields(bytes: &[u8]) -> ProtoFields<'_> {
    ProtoFields { bytes, offset: 0 }
}

struct ProtoFields<'a> {
    bytes: &'a [u8],
    offset: usize,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_onnx_float_to_triton_type() {
        let triton_type = onnx_elem_type_to_triton(1).expect("type mapping");
        assert_eq!(triton_type, "TYPE_FP32");
    }

    #[test]
    fn formats_dynamic_dims() {
        assert_eq!(format_dims(&[-1, 224, 224, 3]), "[ 224, 224, 3 ]");
    }

    #[test]
    fn keeps_unbatched_dims() {
        assert_eq!(format_dims(&[224, 224, 3]), "[ 224, 224, 3 ]");
    }

    #[test]
    fn keeps_non_leading_dynamic_dims() {
        assert_eq!(format_dims(&[-1, 224, -1, 3]), "[ 224, -1, 3 ]");
    }

    #[test]
    fn renders_template_with_triton_dims() {
        let metadata = OnnxMetadata {
            input: TensorMetadata {
                name: "images".to_string(),
                triton_type: "TYPE_FP32".to_string(),
                dims: vec![-1, 224, 224, 3],
            },
            output: TensorMetadata {
                name: "scores".to_string(),
                triton_type: "TYPE_FP32".to_string(),
                dims: vec![-1, 1000],
            },
        };
        let template =
            "name: \"$MODEL_NAME\"\ninput: $INPUT_DIMENTIONS\noutput: $OUTPUT_DIMENSIONS";

        let rendered = fill_template(template, "resnet", &metadata).expect("render template");

        assert!(rendered.contains("input: [ 224, 224, 3 ]"));
        assert!(rendered.contains("output: [ 1000 ]"));
    }
}
