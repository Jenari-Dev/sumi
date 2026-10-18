//! Print an ONNX node and the producers of its inputs (test aid).
use tract_onnx::prelude::Framework;

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("model path");
    let name = std::env::args().nth(2).expect("node name");
    let proto = tract_onnx::onnx().proto_model_for_path(&path)?;
    let g = proto.graph.as_ref().unwrap();
    if name == "--op" {
        for n in g.node.iter().filter(|n| n.op_type == std::env::args().nth(3).unwrap()) {
            println!("{} inputs={:?}", n.name, n.input);
        }
        return Ok(());
    }
    let node = g.node.iter().find(|n| n.name == name).expect("no such node");
    println!("{} op={} inputs={:?} outputs={:?}", node.name, node.op_type, node.input, node.output);
    for i in &node.input {
        if let Some(p) = g.node.iter().find(|n| n.output.contains(i)) {
            println!("  {i} <- {} op={} inputs={:?}", p.name, p.op_type, p.input);
        } else if g.initializer.iter().any(|t| &t.name == i) {
            println!("  {i} <- initializer");
        }
    }
    for c in g.node.iter().filter(|n| n.input.iter().any(|x| node.output.contains(x))) {
        println!("  consumer {} op={} inputs={:?}", c.name, c.op_type, c.input);
    }
    Ok(())
}
