use clap::Parser;
use rspirv::binary::{Assemble, Disassemble};
use std::collections::{HashMap, HashSet};

use rspirv::dr::{Instruction, Operand};
use rspirv::spirv::Op;
use std::path::PathBuf;
use std::fs;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(index = 1)]
    file: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(short, long, action)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    let data: Vec<u8> = fs::read(args.file.clone()).unwrap();

    let mut loader = rspirv::dr::Loader::new();
    rspirv::binary::parse_bytes(&data, &mut loader).unwrap();
    let mut module = loader.module();


    // move OpTypeSampledImage
    for i in 0..module.types_global_values.len() {
        let inst = module.types_global_values.get(i).unwrap().clone();
        if inst.class.opcode == Op::TypeSampledImage {
            let image_id = inst.operands.get(0).unwrap().id_ref_any().unwrap();


            for j in (1..=i).rev() {
                // until correct OpTypeImage is reached
                let prev = module.types_global_values.get(j - 1).unwrap();
                if prev.class.opcode == Op::TypeImage && prev.result_id.unwrap() == image_id {
                    if args.verbose { eprintln!("Moving \x1b[7m{}\x1b[0m just after related \x1b[7m{}\x1b[0m", inst.disassemble(), prev.disassemble()); }
                    break;
                }

                // move the OpSampledImage instruction farther back
                module.types_global_values.swap(j - 1, j);
            }
        }
    }


    let mut replace_op = HashMap::new();
    let mut replace_type = HashMap::new();

    let mut possibly_unused = HashSet::new();

    module.all_inst_iter().for_each(|inst| {
        _ = match inst.class.opcode {
            Op::TypeSampledImage => {
                replace_type.insert(inst.operands.get(0).unwrap().id_ref_any().unwrap(), inst.result_id.unwrap());
                if args.verbose { eprintln!("Promoting all uses of %{} (Image) to %{} (SampledImage) via \x1b[7m{}\x1b[0m", inst.operands.get(0).unwrap().id_ref_any().unwrap(), inst.result_id.unwrap(), inst.disassemble()); }
            }
            Op::SampledImage => {
                possibly_unused.insert(inst.result_id.unwrap());
                replace_op.insert(inst.result_id.unwrap(), inst.operands.get(0).unwrap().id_ref_any().unwrap());
                if args.verbose { eprintln!("Replacing usage of %{} with %{} based on \x1b[7m{}\x1b[0m (the Image parameter should be promoted to SampledImage already)", inst.result_id.unwrap(), inst.operands.get(0).unwrap().id_ref_any().unwrap(), inst.disassemble()); }
            }
            _ => (),
        };
    });


    // replace usage of OpSampledImage with the Image directly
    module.all_inst_iter_mut().for_each(|inst| {
        inst.operands.iter_mut().for_each(|op| {
            match op {
                Operand::IdRef(ref mut word) => *word = *replace_op.get(&word).unwrap_or(&word),
                _ => (),
            }
        })
    });

    // replace all uses of OpTypeImage with OpTypeSampledImage
    module.all_inst_iter_mut().for_each(|inst| {
        if inst.class.opcode != Op::TypeSampledImage {
            // replace result types
            if let Some(ref mut result_type) = inst.result_type {
                *result_type = *replace_type.get(result_type).unwrap_or(result_type);
            }
            // replace op codes
            inst.operands.iter_mut().for_each(|op| {
                match op {
                    Operand::IdRef(ref mut word) => *word = *replace_type.get(&word).unwrap_or(&word),
                    _ => (),
                }
            })
        }
    });

    while possibly_unused.len() > 0 {
        let current_id = possibly_unused.iter().next().unwrap().clone();
        possibly_unused.remove(&current_id);

        // check if the current instruction is used by any other instruction
        let used = module.all_inst_iter().any(|inst| {
            inst.operands.iter().any(|op| op.id_ref_any().is_some_and(|id_ref| id_ref == current_id))
        });

        if !used {
            module.all_inst_iter_mut().for_each(|inst| {
                if inst.result_id.is_some_and(|result_id| result_id == current_id) {

                    // add any referenced id_refs to the possibly_unused list
                    inst.operands.iter().for_each(|op| {
                        if let Some(id_ref) = op.id_ref_any() {
                            possibly_unused.insert(id_ref);
                        }
                    });
                    if let Some(result_type) = inst.result_type {
                        possibly_unused.insert(result_type);
                    }

                    if args.verbose { eprintln!("Replacing now unused instruction \x1b[7m{}\x1b[0m with OpNop(no-op)", inst.disassemble()); }

                    // replace with no-op
                    *inst = Instruction::new(Op::Nop, None, None, vec![]);
                }
            })
        }
    }

    fs::write(
        args.output.unwrap_or(args.file.with_extension("modified.spv")),
        module.assemble().iter().flat_map(|word| { word.to_le_bytes() }).collect::<Vec<_>>(),
    ).unwrap()
}