use std::fmt::Debug;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use crate::des::core::*;

struct Process<T> {
    sim : Rc<Simulation>,
    gen : T
}

impl<T> Process<T> {
    pub fn new(sim : &Rc<Simulation>, gen : T) -> Rc<Self> {
        let p = Rc::new(Self {
            sim: sim.clone(),
            gen
        });

        let p_inner = p.clone();
        sim.event(Some(0.0)).callback(move |sim| {
            let ret = p_inner.gen.resume(sim);
        });
        p
    }


}

#[test]
fn test_proc_1() {
    let sim = Simulation::new();

    let p = Process::new(sim, |sim| {
        yield 1;
    });


}
