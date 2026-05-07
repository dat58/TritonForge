//! ONNX metadata extraction and Triton config.pbtxt generation.

use crate::errors::AppError;
use crate::models::job::WarmupInput;
use crate::onnx::{
    GRAPH_INITIALIZER_FIELD, GRAPH_INPUT_FIELD, GRAPH_OUTPUT_FIELD, MODEL_GRAPH_FIELD,
    TENSOR_ELEM_TYPE_FIELD, TENSOR_SHAPE_FIELD, TYPE_TENSOR_FIELD, VALUE_NAME_FIELD,
    VALUE_TYPE_FIELD, fields, first_message_field, first_string_field, first_varint_field,
    parse_shape_dims,
};
use std::collections::HashSet;
use std::path::Path;
use tokio::fs;

const CONFIG_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/config.pbtxt"
));

/// Generates `config.pbtxt` from `templates/config.pbtxt` and ONNX graph metadata.
///
/// `warmup_inputs` is rendered into the `model_warmup` block. When empty, the
/// entire `model_warmup { … }` block is stripped from the output so the
/// generated config doesn't ship with stub warmup data.
pub async fn generate_config_pbtxt(
    model_path: &Path,
    model_name: &str,
    warmup_inputs: &[WarmupInput],
) -> Result<String, AppError> {
    let model_bytes = fs::read(model_path).await?;
    let metadata = parse_onnx_metadata(&model_bytes)?;
    fill_template(CONFIG_TEMPLATE, model_name, &metadata, warmup_inputs)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TensorMetadata {
    name: String,
    triton_type: String,
    dims: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OnnxMetadata {
    inputs: Vec<TensorMetadata>,
    outputs: Vec<TensorMetadata>,
}

fn parse_onnx_metadata(bytes: &[u8]) -> Result<OnnxMetadata, AppError> {
    let graph = first_message_field(bytes, MODEL_GRAPH_FIELD)
        .ok_or_else(|| AppError::Validation("ONNX model has no graph".into()))?;
    let initializers = initializer_names(graph);
    let inputs = parse_non_initializer_inputs(graph, &initializers)?;
    let outputs = parse_value_infos(graph, GRAPH_OUTPUT_FIELD, "output")?;
    Ok(OnnxMetadata { inputs, outputs })
}

fn parse_non_initializer_inputs(
    graph: &[u8],
    initializers: &HashSet<String>,
) -> Result<Vec<TensorMetadata>, AppError> {
    let mut inputs = Vec::new();
    for value in graph_values(graph, GRAPH_INPUT_FIELD) {
        let name = first_string_field(value, VALUE_NAME_FIELD)
            .ok_or_else(|| AppError::Validation("ONNX input tensor has no name".into()))?;
        if !initializers.contains(&name) {
            inputs.push(parse_value_info(value, "input")?);
        }
    }
    non_empty_tensors(inputs, "ONNX graph has no non-initializer input tensors")
}

fn initializer_names(graph: &[u8]) -> HashSet<String> {
    fields(graph)
        .filter(|field| field.number == GRAPH_INITIALIZER_FIELD && field.wire_type == 2)
        .filter_map(|field| field.data)
        .filter_map(|initializer| first_string_field(initializer, VALUE_NAME_FIELD))
        .collect()
}

fn parse_value_infos(
    graph: &[u8],
    field_number: u32,
    label: &str,
) -> Result<Vec<TensorMetadata>, AppError> {
    let tensors = graph_values(graph, field_number)
        .map(|value| parse_value_info(value, label))
        .collect::<Result<Vec<_>, _>>()?;
    non_empty_tensors(tensors, &format!("ONNX graph has no {label} tensors"))
}

fn graph_values(graph: &[u8], field_number: u32) -> impl Iterator<Item = &[u8]> {
    fields(graph)
        .filter(move |field| field.number == field_number && field.wire_type == 2)
        .filter_map(|field| field.data)
}

fn non_empty_tensors(
    tensors: Vec<TensorMetadata>,
    message: &str,
) -> Result<Vec<TensorMetadata>, AppError> {
    if tensors.is_empty() {
        Err(AppError::Validation(message.to_owned()))
    } else {
        Ok(tensors)
    }
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
    warmup_inputs: &[WarmupInput],
) -> Result<String, AppError> {
    let pre_substituted = if warmup_inputs.is_empty() {
        strip_model_warmup_block(template)
    } else {
        template.to_owned()
    };

    let mut rendered = pre_substituted;
    let replacements = [
        ("$MODEL_NAME", model_name.to_owned()),
        ("$INPUT_BLOCKS", format_tensor_blocks(&metadata.inputs)),
        ("$OUTPUT_BLOCKS", format_tensor_blocks(&metadata.outputs)),
        ("$INPUT_WARMUP_BLOCKS", format_warmup_blocks(warmup_inputs)),
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

fn format_tensor_blocks(tensors: &[TensorMetadata]) -> String {
    tensors
        .iter()
        .map(format_tensor_block)
        .collect::<Vec<_>>()
        .join(",\n")
}

fn format_tensor_block(tensor: &TensorMetadata) -> String {
    format!(
        "  {{\n    name: \"{}\"\n    data_type: {}\n    dims: {}\n  }}",
        tensor.name,
        tensor.triton_type,
        format_dims(&tensor.dims)
    )
}

/// Formats the `model_warmup.inputs` map literal as `[ { key: ..., value { ... } }, ... ]`.
///
/// Returns `[]` for empty inputs (caller is expected to strip the block instead
/// when the vec is empty so this branch is unused in production).
fn format_warmup_blocks(inputs: &[WarmupInput]) -> String {
    if inputs.is_empty() {
        return "[]".to_owned();
    }
    let entries = inputs
        .iter()
        .map(format_warmup_entry)
        .collect::<Vec<_>>()
        .join(",\n");
    format!("[\n{entries}\n  ]")
}

fn format_warmup_entry(input: &WarmupInput) -> String {
    let dim_values: Vec<String> = input.dims.iter().map(ToString::to_string).collect();
    let data_field = if input.zero_data {
        "zero_data: true"
    } else {
        "random_data: true"
    };
    format!(
        "    {{\n      key: \"{key}\"\n      value: {{\n        data_type: {dt}\n        dims: [ {dims} ]\n        {data_field}\n      }}\n    }}",
        key = input.key,
        dt = input.data_type.as_pbtxt(),
        dims = dim_values.join(", ")
    )
}

/// Removes the entire `model_warmup { … }` block from `template`.
///
/// Brace-aware so a future template with nested messages inside the warmup block
/// still strips correctly. Leaves the template untouched if the block isn't found
/// or braces are unbalanced.
fn strip_model_warmup_block(template: &str) -> String {
    let Some(start) = template.find("model_warmup") else {
        return template.to_owned();
    };
    let after_keyword = start + "model_warmup".len();
    let Some(open_offset) = template[after_keyword..].find('{') else {
        return template.to_owned();
    };
    let scan_start = after_keyword + open_offset + 1;
    let bytes = template.as_bytes();
    let mut depth: i32 = 1;
    let mut idx = scan_start;
    while idx < bytes.len() && depth > 0 {
        match bytes[idx] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        idx += 1;
    }
    if depth != 0 {
        return template.to_owned();
    }
    let mut end = idx;
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }

    let mut out = String::with_capacity(template.len());
    out.push_str(&template[..start]);
    out.push_str(&template[end..]);
    out
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
            inputs: vec![TensorMetadata {
                name: "images".to_string(),
                triton_type: "TYPE_FP32".to_string(),
                dims: vec![-1, 224, 224, 3],
            }],
            outputs: vec![TensorMetadata {
                name: "scores".to_string(),
                triton_type: "TYPE_FP32".to_string(),
                dims: vec![-1, 1000],
            }],
        };
        let template =
            "name: \"$MODEL_NAME\"\ninput [\n$INPUT_BLOCKS\n]\noutput [\n$OUTPUT_BLOCKS\n]";

        let rendered = fill_template(template, "resnet", &metadata, &[]).expect("render template");

        assert!(rendered.contains("name: \"images\""));
        assert!(rendered.contains("dims: [ 224, 224, 3 ]"));
        assert!(rendered.contains("name: \"scores\""));
        assert!(rendered.contains("dims: [ 1000 ]"));
        assert!(!rendered.contains("},\n]"));
    }

    #[test]
    fn renders_multiple_input_and_output_blocks() {
        let metadata = OnnxMetadata {
            inputs: vec![
                tensor("images", "TYPE_FP32", vec![-1, 3, 224, 224]),
                tensor("scale", "TYPE_FP32", vec![1]),
            ],
            outputs: vec![
                tensor("boxes", "TYPE_FP32", vec![-1, 100, 4]),
                tensor("scores", "TYPE_FP32", vec![-1, 100]),
            ],
        };

        let rendered = fill_template(
            "input [\n$INPUT_BLOCKS\n]\noutput [\n$OUTPUT_BLOCKS\n]",
            "detector",
            &metadata,
            &[],
        )
        .expect("render template");

        assert!(rendered.contains("name: \"images\""));
        assert!(rendered.contains("dims: [ 3, 224, 224 ]"));
        assert!(rendered.contains("name: \"scale\""));
        assert!(rendered.contains("dims: [ 1 ]"));
        assert!(rendered.contains("dims: [ 3, 224, 224 ]\n  },\n  {\n    name: \"scale\""));
        assert!(rendered.contains("name: \"boxes\""));
        assert!(rendered.contains("dims: [ 100, 4 ]"));
        assert!(rendered.contains("name: \"scores\""));
        assert!(rendered.contains("dims: [ 100 ]"));
        assert!(rendered.contains("dims: [ 100, 4 ]\n  },\n  {\n    name: \"scores\""));
    }

    #[test]
    fn renders_multiple_warmup_blocks() {
        use crate::models::job::TritonDataType;
        let metadata = OnnxMetadata {
            inputs: vec![tensor("images", "TYPE_FP32", vec![-1, 3, 224, 224])],
            outputs: vec![tensor("scores", "TYPE_FP32", vec![-1, 1000])],
        };
        let template = "model_warmup {\n    name: \"$MODEL_NAME\"\n    inputs: $INPUT_WARMUP_BLOCKS\n}\ninput [\n$INPUT_BLOCKS\n]\noutput [\n$OUTPUT_BLOCKS\n]";
        let warmups = vec![
            WarmupInput {
                key: "INPUT0".to_string(),
                data_type: TritonDataType::Fp32,
                dims: vec![1, 3, 224, 224],
                zero_data: true,
            },
            WarmupInput {
                key: "INPUT1".to_string(),
                data_type: TritonDataType::Int64,
                dims: vec![1],
                zero_data: false,
            },
        ];

        let rendered =
            fill_template(template, "resnet", &metadata, &warmups).expect("render template");

        assert!(rendered.contains("model_warmup"));
        assert!(rendered.contains("key: \"INPUT0\""));
        assert!(rendered.contains("key: \"INPUT1\""));
        assert!(rendered.contains("data_type: TYPE_FP32"));
        assert!(rendered.contains("data_type: TYPE_INT64"));
        assert!(rendered.contains("dims: [ 1, 3, 224, 224 ]"));
        assert!(rendered.contains("zero_data: true"));
        assert!(rendered.contains("random_data: true"));
        assert!(!rendered.contains("$INPUT_WARMUP_BLOCKS"));
    }

    #[test]
    fn omits_model_warmup_block_when_inputs_empty() {
        let metadata = OnnxMetadata {
            inputs: vec![tensor("images", "TYPE_FP32", vec![-1, 3, 224, 224])],
            outputs: vec![tensor("scores", "TYPE_FP32", vec![-1, 1000])],
        };
        let template = "name: \"$MODEL_NAME\"\nmodel_warmup {\n    name: \"$MODEL_NAME\"\n    inputs: $INPUT_WARMUP_BLOCKS\n}\ninput [\n$INPUT_BLOCKS\n]\noutput [\n$OUTPUT_BLOCKS\n]";

        let rendered = fill_template(template, "resnet", &metadata, &[]).expect("render template");

        assert!(!rendered.contains("model_warmup"));
        assert!(!rendered.contains("$INPUT_WARMUP_BLOCKS"));
        assert!(rendered.contains("name: \"resnet\""));
        assert!(rendered.contains("name: \"images\""));
    }

    #[test]
    fn parses_all_non_initializer_inputs_and_outputs() {
        let graph = graph(
            &[
                value_info("weights", 1, &[64, 3, 7, 7]),
                value_info("images", 1, &[-1, 3, 224, 224]),
                value_info("scale", 1, &[1]),
            ],
            &[
                value_info("boxes", 1, &[-1, 100, 4]),
                value_info("scores", 1, &[-1, 100]),
            ],
            &["weights"],
        );
        let model = message_field(MODEL_GRAPH_FIELD, &graph);

        let metadata = parse_onnx_metadata(&model).expect("parse metadata");

        assert_eq!(metadata.inputs.len(), 2);
        assert_eq!(metadata.inputs[0].name, "images");
        assert_eq!(metadata.inputs[1].name, "scale");
        assert_eq!(metadata.outputs.len(), 2);
        assert_eq!(metadata.outputs[0].name, "boxes");
        assert_eq!(metadata.outputs[1].name, "scores");
    }

    #[test]
    fn parse_metadata_fails_without_non_initializer_inputs() {
        let graph = graph(
            &[value_info("weights", 1, &[64, 3, 7, 7])],
            &[value_info("scores", 1, &[-1, 100])],
            &["weights"],
        );
        let model = message_field(MODEL_GRAPH_FIELD, &graph);

        let result = parse_onnx_metadata(&model);

        assert!(result.is_err());
    }

    #[test]
    fn parse_metadata_fails_without_outputs() {
        let graph = graph(&[value_info("images", 1, &[-1, 3, 224, 224])], &[], &[]);
        let model = message_field(MODEL_GRAPH_FIELD, &graph);

        let result = parse_onnx_metadata(&model);

        assert!(result.is_err());
    }

    fn tensor(name: &str, triton_type: &str, dims: Vec<i64>) -> TensorMetadata {
        TensorMetadata {
            name: name.to_string(),
            triton_type: triton_type.to_string(),
            dims,
        }
    }

    fn graph(inputs: &[Vec<u8>], outputs: &[Vec<u8>], initializers: &[&str]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for initializer in initializers {
            bytes.extend(message_field(
                GRAPH_INITIALIZER_FIELD,
                &string_field(VALUE_NAME_FIELD, initializer),
            ));
        }
        for input in inputs {
            bytes.extend(message_field(GRAPH_INPUT_FIELD, input));
        }
        for output in outputs {
            bytes.extend(message_field(GRAPH_OUTPUT_FIELD, output));
        }
        bytes
    }

    fn value_info(name: &str, elem_type: u64, dims: &[i64]) -> Vec<u8> {
        let tensor = [varint_field(TENSOR_ELEM_TYPE_FIELD, elem_type), shape(dims)].concat();
        let type_proto = message_field(TYPE_TENSOR_FIELD, &tensor);
        [
            string_field(VALUE_NAME_FIELD, name),
            message_field(VALUE_TYPE_FIELD, &type_proto),
        ]
        .concat()
    }

    fn shape(dims: &[i64]) -> Vec<u8> {
        dims.iter()
            .map(|dim| {
                let value = u64::try_from(*dim).ok();
                let dim_bytes = value.map_or_else(
                    || string_field(crate::onnx::DIM_PARAM_FIELD, "N"),
                    |static_dim| varint_field(crate::onnx::DIM_VALUE_FIELD, static_dim),
                );
                message_field(TENSOR_SHAPE_FIELD, &dim_bytes)
            })
            .collect::<Vec<_>>()
            .concat()
    }

    fn string_field(field_number: u32, value: &str) -> Vec<u8> {
        message_field(field_number, value.as_bytes())
    }

    fn message_field(field_number: u32, value: &[u8]) -> Vec<u8> {
        let mut bytes = key(field_number, 2);
        push_varint(
            u64::try_from(value.len()).expect("fixture length"),
            &mut bytes,
        );
        bytes.extend_from_slice(value);
        bytes
    }

    fn varint_field(field_number: u32, value: u64) -> Vec<u8> {
        let mut bytes = key(field_number, 0);
        push_varint(value, &mut bytes);
        bytes
    }

    fn key(field_number: u32, wire_type: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_varint(
            u64::from((field_number << 3) | u32::from(wire_type)),
            &mut bytes,
        );
        bytes
    }

    fn push_varint(mut value: u64, bytes: &mut Vec<u8>) {
        while value >= 0x80 {
            bytes.push((value as u8) | 0x80);
            value >>= 7;
        }
        bytes.push(value as u8);
    }
}
