
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::fmt::Debug;

use crate::des::core::*;
use crate::des::resource::*;

pub struct FifoBuf<T> {
    sim : Rc<Simulation>,
    res : Rc<Resource>,
    q : RefCell<VecDeque<Rc<T>>>,
    pending : Cell<bool>
}

impl<T: 'static + Debug> FifoBuf<T> {
    pub fn new(sim : &Rc<Simulation>, capacity : usize) -> Rc<Self> {
        Rc::new(Self {
            sim: sim.clone(),
            res: Resource::new(sim, capacity),
            q: RefCell::new(VecDeque::new()),
            pending: Cell::new(false)
        })
    }

    pub fn empty(&self) -> bool {
        self.q.borrow().is_empty()
    }

    pub fn push(self: &Rc<Self>, x : Rc<T>) -> Rc<Event> {
        let b = self.clone();
        let ev = self.res.acquire();
        ev.callback(move |sim : Rc<Simulation>| {
            let mut q = b.q.borrow_mut();
            q.push_back(x.clone());
        });
        ev
    }

    pub fn peek(&self) -> Option<Rc<T>> {
        let q = self.q.borrow();
        if self.pending.get() { return None }

        if let Some(head) = q.front() {
            Some(head.clone())
        }
        else {
            None
        }
    }

    pub fn pend(&self) {
        assert!(!self.pending.get());
        self.pending.set(true);
    }

    pub fn pop(&self) {
        let mut q = self.q.borrow_mut();
        assert!(self.pending.get());

        self.res.release();
        q.pop_front();
        self.pending.set(false);
    }

    pub fn debug(&self) {
        print!("[{}/{}]", self.pending.get(), self.q.borrow().len());
        self.res.debug();
        print!("[");
        for p in self.q.borrow().iter() {
            print!("{:?}, ", p);
        }
        print!("]");
    }
}
