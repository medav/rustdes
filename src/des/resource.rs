use std::fmt::Debug;
use std::marker::PhantomData;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use crate::des::core::*;
// use crate::des::funcevent::*;


pub struct Resource {
    sim : Rc<Simulation>,
    max : usize,
    val : Cell<usize>,
    q : RefCell<VecDeque<Rc<Event>>>
}

impl Resource {
    pub fn new(sim : &Rc<Simulation>, max : usize) -> Rc<Self> {
        Rc::new(Self {
            sim: sim.clone(),
            max,
            val: Cell::new(0),
            q: RefCell::new(VecDeque::new())
        })
    }

    pub fn full(&self) -> bool { self.val.get() >= self.max }

    pub fn acquire(self : &Rc<Self>) -> Rc<Event> {
        let ev = self.sim.event(None);

        let r = self.clone();
        ev.callback(move |sim| {
            r.val.set(r.val.get() + 1);
        });

        if self.full() {
            let mut q = self.q.borrow_mut();
            q.push_back(ev.clone());
        }
        else {
            self.sim.schedule(&ev, 0.0);
        }

        ev.clone()
    }

    pub fn release(self : &Rc<Self>) {
        assert!(self.val.get() > 0);

        let mut q = self.q.borrow_mut();
        if let Some(ev) = q.pop_front() {
            let r = self.clone();
            ev.callback(move |sim| { r.val.set(r.val.get() - 1); });
            self.sim.schedule(&ev, 0.0);
        }
        else {
            assert!(self.val.get() > 0);
            self.val.set(self.val.get() - 1);
        }
    }

    pub fn debug(&self) {
        print!("[{}/{} ({})]", self.val.get(), self.max, self.q.borrow().len());
    }
}


#[test]
fn proc_test_1() {
    let sim = Simulation::new();
    let r = Resource::new(&sim, 1);

    for i in 1..5 {
        let r_1 = r.clone();
        r.acquire().callback(move |sim : Rc<Simulation>| {
            println!("Acquire [{}] @ {}", i, sim.now());

            let ev = sim.event(Some(10.0));

            let r_2 = r_1.clone();
            ev.callback(move |sim| {
                println!("    Release [{}] @ {}", i, sim.now());
                r_2.release();
            });

        });
    }

    sim.run(None);
}


#[test]
fn foo() {
    let mut v = VecDeque::new();

    v.push_back(1);
    v.push_back(2);
    v.push_back(3);

    println!("{:?}", v.pop_front());
    println!("{:?}", v.pop_front());
    println!("{:?}", v.pop_front());

}

