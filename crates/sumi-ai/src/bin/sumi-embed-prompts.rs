//! Development tool: compute CLIP text embeddings for `prompts::PROMPTS` and
//! write `data/prompt-embeddings.json` (shipped with Sumi, so users only
//! download the vision encoder).
//!
//!     cargo run -p sumi-ai --features embed --bin sumi-embed-prompts -- <dir with text_model.onnx + tokenizer.json>

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use sumi_ai::clip::{PromptEmbedding, PromptFile};
use sumi_ai::model::MODEL_ID;
use sumi_ai::prompts::PROMPTS;
use tract_onnx::prelude::*;
use tract_onnx::prelude::Framework;

/// CLIP's context length and end-of-text token (also its padding token).
const CONTEXT: usize = 77;
const EOT: i64 = 49407;

fn main() -> Result<()> {
    let dir = PathBuf::from(std::env::args().nth(1).context("usage: sumi-embed-prompts <model dir>")?);
    let out = std::env::args().nth(2).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data").join("prompt-embeddings.json")
    });
    let tok = tokenizers::Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| anyhow::anyhow!("tokenizer: {e}"))?;
    // tract cannot type the text model's `Range` nodes; with batch 1 and a
    // fixed context length both are constants: `Range` = arange(seq_len) for
    // the causal mask, `Range_1` = arange(batch) for picking the EOT token.
    let onnx = tract_onnx::onnx();
    let mut proto = onnx.proto_model_for_path(dir.join("text_model.onnx"))?;
    let graph = proto.graph.as_mut().context("ONNX model has no graph")?;
    let mut patched = 0;
    for node in graph.node.iter_mut().filter(|n| n.op_type == "Range") {
        let len: i64 = match node.name.as_str() {
            "/text_model/Range" => CONTEXT as i64,
            "/text_model/Range_1" => 1,
            other => bail!("unexpected Range node {other:?} in the text model"),
        };
        node.op_type = "Constant".into();
        node.input.clear();
        node.attribute = vec![tract_onnx::pb::AttributeProto {
            name: "value".into(),
            r#type: tract_onnx::pb::attribute_proto::AttributeType::Tensor as i32,
            t: Some(tract_onnx::pb::TensorProto {
                dims: vec![len],
                data_type: tract_onnx::pb::tensor_proto::DataType::Int64 as i32,
                int64_data: (0..len).collect(),
                ..Default::default()
            }),
            ..Default::default()
        }];
        patched += 1;
    }
    println!("patched {patched} Range node(s) to constants");
    let model = onnx.model_for_proto_model(&proto)?;
    let inputs = model.input_outlets()?.len();
    let mut model = model.with_input_fact(0, i64::fact([1, CONTEXT]).into())?;
    if inputs > 1 {
        model = model.with_input_fact(1, i64::fact([1, CONTEXT]).into())?;
    }
    let plan = model.with_output_names(["text_embeds"])?.into_optimized()?.into_runnable()?;
    let mut prompts = Vec::new();
    for (_, text) in PROMPTS {
        let enc = tok.encode(*text, true).map_err(|e| anyhow::anyhow!("tokenizing {text:?}: {e}"))?;
        let mut ids: Vec<i64> = enc.get_ids().iter().map(|&i| i as i64).collect();
        if ids.len() > CONTEXT {
            bail!("prompt {text:?} is longer than {CONTEXT} tokens");
        }
        let used = ids.len();
        // Pad with end-of-text: CLIP pools at the first EOT and its mask is causal.
        ids.resize(CONTEXT, EOT);
        let mut feed = tvec!(Tensor::from_shape(&[1, CONTEXT], &ids)?.into());
        if inputs > 1 {
            let mask: Vec<i64> = (0..CONTEXT).map(|i| i64::from(i < used)).collect();
            feed.push(Tensor::from_shape(&[1, CONTEXT], &mask)?.into());
        }
        let res = plan.run(feed)?;
        let mut v: Vec<f32> = res[0].to_array_view::<f32>()?.iter().copied().collect();
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter_mut().for_each(|x| *x /= n);
        println!("{used:>2} tokens  {text}");
        prompts.push(PromptEmbedding { text: text.to_string(), embedding: v });
    }
    let file = PromptFile { model: MODEL_ID.to_string(), prompts };
    std::fs::write(&out, serde_json::to_string(&file)?)?;
    println!("wrote {} ({} prompts)", out.display(), file.prompts.len());
    Ok(())
}
