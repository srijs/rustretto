use std::collections::BTreeMap;

use super::{VarId, VarIdGen};

#[derive(Clone, Debug)]
pub(crate) struct StackAndLocals {
    pub stack: Vec<VarId>,
    pub locals: BTreeMap<usize, VarId>,
}

impl StackAndLocals {
    pub fn new(max_stack: u16, _max_locals: u16, args: &[VarId]) -> StackAndLocals {
        let stack = Vec::with_capacity(max_stack as usize);
        let mut locals = BTreeMap::new();
        locals.extend(args.into_iter().cloned().enumerate());
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
        self.stack.push(self.locals[&idx].clone());
    }

    pub fn store(&mut self, idx: usize) {
        self.locals.insert(idx, self.stack.pop().unwrap());
    }
}
