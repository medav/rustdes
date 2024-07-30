

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::rc::Rc;

pub trait CallbackFn = Fn(Rc<Simulation>) -> ();
pub type EventCallback = Box<dyn CallbackFn>;

pub struct Event {
    sim : Rc<Simulation>,
    t : Cell<Option<f32>>,
    callbacks : RefCell<Vec<EventCallback>>
}

impl Event {
    pub fn new(sim : &Rc<Simulation>, delay_opt : Option<f32>) -> Rc<Self> {
        Rc::new(Self {
            sim: sim.clone(),
            t: Cell::new(if let Some(delay) = delay_opt {
                Some(sim.now() + delay)
            } else {
                None
            }),
            callbacks: RefCell::new(Vec::new())
        })
    }

    pub fn exec(&self) -> () {
        let callbacks = self.callbacks.borrow();
        for cb in callbacks.iter() {
            cb.call((self.sim.clone(),))
        }
    }

    pub fn callback<T>(&self, f : T) where T: CallbackFn + 'static {
        let mut callbacks = self.callbacks.borrow_mut();
        callbacks.push(Box::new(f))
    }

    pub fn set_time(&self, t : f32) {
        self.t.set(Some(t));
    }

    pub fn delay(&self, delay : f32) -> Rc<Self> {
        let ev = self.sim.event(None);
        let ev_inner = ev.clone();
        self.callback(move |sim : Rc<Simulation>| {
            sim.schedule(&ev_inner, delay);
        });
        ev.clone()
    }

}


impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.t == other.t { Ordering::Equal }
        else if self.t > other.t { Ordering::Less }
        else { Ordering::Greater }
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool { self.t == other.t }
}

impl Eq for Event { }

pub struct Simulation {
    time : Cell<f32>,
    num_events : Cell<u64>,
    q : RefCell<BinaryHeap<Rc<Event>>>
}

impl Simulation {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            time: Cell::new(0.0),
            num_events: Cell::new(0),
            q: RefCell::new(BinaryHeap::new())
        })
    }

    pub fn enqueue(&self, ev : &Rc<Event>) {
        self.q.borrow_mut().push(ev.clone());
    }

    pub fn schedule(&self, ev : &Rc<Event>, delay : f32) -> () {
        ev.set_time(self.now() + delay);
        self.enqueue(ev)
    }

    pub fn event(self: &Rc<Self>, delay : Option<f32>) -> Rc<Event> {
        let ev = Event::new(self, delay);
        if let Some(_) = delay { self.enqueue(&ev) }
        ev
    }

    fn pop(&self) -> Option<Rc<Event>> {
        self.q.borrow_mut().pop()
    }

    pub fn run(&self, limit: Option<f32>) {
        while let Some(entry) = self.pop() {
            self.time.set(entry.t.get().unwrap());

            if let Some(limit_val) = limit {
                if self.now() > limit_val {
                    self.time.set(limit_val);
                    break
                }
            }

            entry.exec();
            self.num_events.set(self.num_events.get() + 1);
        }
    }

    pub fn now(&self) -> f32 { self.time.get() }
    pub fn num_events(&self) -> u64 { self.num_events.get() }
}


#[test]
fn test1() {
    let sim = Simulation::new();

    let ev = sim.event(Some(10.0f32));
    ev.callback(|sim| {
        println!("foo @ {}", sim.now());
    });

    ev.callback(|sim| {
        println!("bar @ {}", sim.now());
    });

    sim.run(None);
}


