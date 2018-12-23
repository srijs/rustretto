use std::collections::BTreeMap;

use crate::translate::{VarId, VarIdGen};
use crate::types::Type;

#[derive(Clone, Debug)]
pub(crate) struct StackAndLocals {
    pub stack: Vec<VarId>,
    pub locals: BTreeMap<usize, VarId>,
}

impl StackAndLocals {
    pub fn new(max_stack: u16, _max_locals: u16, args: &[VarId]) -> StackAndLocals {
        let stack = Vec::with_capacity(max_stack as usize);
        let mut locals = BTreeMap::new();
        let mut next_local_idx = 0;
        for arg in args.iter() {
            locals.insert(next_local_idx, arg.clone());
            // long and double occupy two local slots
            if arg.0 == Type::Long || arg.0 == Type::Double {
                next_local_idx += 2;
            } else {
                next_local_idx += 1;
            }
        }
        StackAndLocals { stack, locals }
    }

    pub fn new_with_same_shape(&self, var_id_gen: &mut VarIdGen) -> Self {
        let stack = self
            .stack
            .iter()
            .map(|v| var_id_gen.gen(v.0.clone()))
            .collect();
        let locals = self
            .locals
            .iter()
            .map(|(i, v)| (*i, var_id_gen.gen(v.0.clone())))
            .collect();
        StackAndLocals { stack, locals }
    }

    pub fn pop(&mut self) -> VarId {
        self.stack.pop().unwrap()
    }

    pub fn pop_n(&mut self, n: usize) -> Vec<VarId> {
        let index = self.stack.len() - n;
        self.stack.split_off(index)
    }

    pub fn push(&mut self, var: VarId) {
        self.stack.push(var);
    }

    pub fn load(&mut self, idx: usize) {
        match self.locals.get(&idx) {
            Some(var) => self.stack.push(var.clone()),
            None => panic!("local slot {} is empty ({:?})", idx, self.locals),
        }
    }

    pub fn store(&mut self, idx: usize) {
        self.locals.insert(idx, self.stack.pop().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate::VarIdGen;
    use crate::types::Type;

    #[test]
    fn new_long_and_double_occupy_wide_slots() {
        let mut gen = VarIdGen::new();
        let args = vec![
            gen.gen(Type::Long),
            gen.gen(Type::Integer),
            gen.gen(Type::Double),
            gen.gen(Type::Float),
        ];
        let frame = StackAndLocals::new(0, 6, &args);

        assert_eq!(frame.locals[&0], args[0]);
        assert_eq!(frame.locals[&2], args[1]);
        assert_eq!(frame.locals[&3], args[2]);
        assert_eq!(frame.locals[&5], args[3]);
    }
}
