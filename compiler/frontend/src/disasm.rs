use std::cmp::Ordering;
use std::ops::Range;

use classfile::instructions::{Disassembler, Instr};
use failure::{bail, Fallible};

pub struct InstructionWithRange {
    pub range: Range<u32>,
    pub instr: Instr,
}

pub struct InstructionBlock {
    pub range: Range<u32>,
    pub instrs: Vec<InstructionWithRange>,
}

impl InstructionBlock {
    fn split(&mut self, addr: u32) -> InstructionBlock {
        let index = self
            .instrs
            .binary_search_by_key(&addr, |instr| instr.range.start)
            .unwrap();
        let tail_instrs = self.instrs.split_off(index);
        let tail_block = InstructionBlock {
            range: Range {
                start: addr,
                end: self.range.end,
            },
            instrs: tail_instrs,
        };
        self.range.end = addr;
        tail_block
    }

    fn build(disasm: &mut Disassembler, start_addrs: &mut Vec<u32>) -> Fallible<Self> {
        let start_addr = disasm.position();
        let mut instrs = vec![];
        while let Some((curr_addr, instr)) = disasm.decode_next()? {
            log::trace!("decoded instruction {:?} at address {}", instr, curr_addr);
            let next_addr = disasm.position();
            let should_break = match instr {
                Instr::Return | Instr::IReturn | Instr::AReturn | Instr::AThrow => true,
                Instr::Goto(offset) => {
                    let addr = (i64::from(curr_addr) + i64::from(offset)) as u32;
                    start_addrs.push(addr);
                    true
                }
                Instr::IfLe(offset)
                | Instr::IfLt(offset)
                | Instr::IfEq(offset)
                | Instr::IfNe(offset)
                | Instr::IfGe(offset)
                | Instr::IfGt(offset)
                | Instr::IfICmpGe(offset)
                | Instr::IfICmpGt(offset)
                | Instr::IfICmpLe(offset)
                | Instr::IfACmpNe(offset) => {
                    let if_addr = (i64::from(curr_addr) + i64::from(offset)) as u32;
                    start_addrs.extend_from_slice(&[next_addr, if_addr]);
                    true
                }
                Instr::TableSwitch(ref table_switch) => {
                    start_addrs
                        .push((i64::from(curr_addr) + i64::from(table_switch.default)) as u32);
                    for offset in table_switch.offsets.iter() {
                        start_addrs.push((i64::from(curr_addr) + i64::from(*offset)) as u32);
                    }
                    true
                }
                Instr::LookupSwitch(ref lookup_switch) => {
                    start_addrs
                        .push((i64::from(curr_addr) + i64::from(lookup_switch.default)) as u32);
                    for (_, offset) in lookup_switch.pairs.iter() {
                        start_addrs.push((i64::from(curr_addr) + i64::from(*offset)) as u32);
                    }
                    true
                }
                _ => false,
            };
            let instr_range = Range {
                start: curr_addr,
                end: next_addr,
            };
            instrs.push(InstructionWithRange {
                range: instr_range,
                instr,
            });
            if should_break {
                let block_range = Range {
                    start: start_addr,
                    end: next_addr,
                };
                return Ok(InstructionBlock {
                    range: block_range,
                    instrs,
                });
            }
        }
        bail!("unexpected end of instruction stream")
    }
}

pub struct InstructionBlockMap {
    blocks: Vec<InstructionBlock>,
}

impl InstructionBlockMap {
    pub fn block_starting_at(&self, addr: u32) -> &InstructionBlock {
        let index_opt = self
            .blocks
            .binary_search_by_key(&addr, |block| block.range.start);
        if let Ok(index) = index_opt {
            &self.blocks[index]
        } else {
            panic!("could not find block starting at addr {}", addr)
        }
    }

    pub fn build(mut disasm: Disassembler) -> Fallible<Self> {
        let mut blocks = vec![];

        let mut start_addrs = vec![0u32];
        while let Some(start_addr) = start_addrs.pop() {
            let search_result = blocks.binary_search_by(|block: &InstructionBlock| {
                if block.range.end <= start_addr {
                    Ordering::Less
                } else if block.range.start > start_addr {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            });

            match search_result {
                Ok(index) => {
                    let next_block_opt = {
                        let block = &mut blocks[index];
                        if start_addr > block.range.start {
                            Some(block.split(start_addr))
                        } else {
                            None
                        }
                    };
                    if let Some(next_block) = next_block_opt {
                        blocks.insert(index + 1, next_block);
                    }
                }
                Err(index) => {
                    disasm.set_position(start_addr);
                    let block = InstructionBlock::build(&mut disasm, &mut start_addrs)?;
                    blocks.insert(index, block);
                }
            };
        }

        Ok(InstructionBlockMap { blocks })
    }
}
