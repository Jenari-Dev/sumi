//! Load the CLIP vision encoder with tract and time one inference (test aid).

use tract_onnx::prelude::*;

fn main() -> anyhow::Result<()> {
    let path = sumi_ai::model::vision_path();
    let t = std::time::Instant::now();
    let plan = tract_onnx::onnx()
        .model_for_path(&path)?
        .with_input_fact(0, f32::fact([1, 3, 224, 224]).into())?
        .with_output_names(["image_embeds"])?
        .into_optimized()?
        .into_runnable()?;
    println!("loaded in {:?}", t.elapsed());
    let input = Tensor::from_shape(&[1, 3, 224, 224], &vec![0.1f32; 3 * 224 * 224])?;
    for i in 0..3 {
        let t = std::time::Instant::now();
        let out = plan.run(tvec!(input.clone().into()))?;
        let v = out[0].to_array_view::<f32>()?;
        println!("run {i}: {:?} shape {:?} first {:.4}", t.elapsed(), v.shape(), v.iter().next().unwrap());
    }
    Ok(())
}
