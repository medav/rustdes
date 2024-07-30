

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rand::prelude::*;

use crate::des::core::*;
use crate::des::resource::*;
use crate::des::fifobuf::*;
use crate::des::core::*;



#[derive(Debug)]
pub struct CacheParams {
    pub laddrbits : usize,
    pub capacity : usize,
    pub assoc : usize,
}

pub trait Cache {
    fn new(p : &CacheParams) -> Self;
    fn lookup(&self, addr : u64) -> bool;
    fn insert(&mut self, addr : u64) -> ();
    fn access(&mut self, addr : u64) -> ();
}


#[derive(Debug)]
pub struct NmruCache {
    nset : usize,
    nway : usize,
    laddrbits : usize,
    tags : Vec<Vec<(bool, u64)>>,
    mru : Vec<usize>
}


impl Cache for NmruCache {

    fn new(p : &CacheParams) -> Self {
        let nset = p.capacity / p.assoc;
        let nway = p.assoc;
        Self {
            nset,
            nway,
            laddrbits: p.laddrbits,
            tags: (0..nset)
                .map(|_| (0..nway).map(|_| (false, 0)).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            mru: (0..nset).map(|_| 0).collect::<Vec<_>>()
        }
    }

    fn lookup(&self, addr : u64) -> bool {
        if self.nset == 0 { return false; }
        let line = addr >> self.laddrbits;
        let set = (line % (self.nset as u64)) as usize;
        let tag = line;
        for wi in 0..self.nway {
            if self.tags[set][wi].0 && self.tags[set][wi].1 == tag {
                return true;
            }
        }
        return false;
    }

    fn insert(&mut self, addr : u64) -> () {
        if self.nset == 0 { return; }
        let line = addr >> self.laddrbits;
        let set = (line % (self.nset as u64)) as usize;
        let tag = line;
        let mut way : isize = -1;

        for wi in 0..self.nway {
            if !self.tags[set][wi].0 {
                way = wi as isize;
                break;
            }
        }

        if way == -1 {
            way = ((self.mru[set] + 1) % self.nway) as isize;
        }

        self.tags[set][way as usize] = (true, tag);
    }

    fn access(&mut self, addr : u64) -> () {
        if self.nset == 0 { return; }
        assert!(self.lookup(addr));

        let line = addr >> self.laddrbits;
        let set = (line % (self.nset as u64)) as usize;
        let tag = line;

        for wi in 0..self.nway {
            if self.tags[set][wi].0 && self.tags[set][wi].1 == tag {
                self.mru[set] = wi;
                break;
            }
        }
    }

}



#[test]
fn test_nmru_cache_1() -> () {
    let p = CacheParams {
        laddrbits: 6,
        capacity: 128,
        assoc: 4
    };
    let mut c = NmruCache::new(&p);

    assert!(!c.lookup(0));
    c.insert(0xDEAD0000);
    c.access(0xDEAD0000);

    assert!(c.lookup(0xDEAD0000));

    c.insert(0x1EAD0000);
    c.access(0x1EAD0000);

    c.insert(0x2EAD0000);
    c.access(0x2EAD0000);

    c.insert(0x3EAD0000);
    c.access(0x3EAD0000);

    c.insert(0x4EAD0000);
    c.access(0x4EAD0000);

    assert!(!c.lookup(0xDEAD0000));
    assert!(c.lookup(0x1EAD0000));
    assert!(c.lookup(0x2EAD0000));
    assert!(c.lookup(0x3EAD0000));
    assert!(c.lookup(0x4EAD0000));
}

#[derive(Debug)]
pub enum MemRequest {
    Read(u64),
    Write(u64)
}

type MemRequestBuffer = FifoBuf<MemRequest>;

pub trait CacheClient {
    fn cache_resp(&self) -> Rc<Event>;
}


pub struct TimingCache<T: Cache> {
    sim : Rc<Simulation>,
    cache : RefCell<T>,
    client : Rc<dyn CacheClient>,
    req_queue : Rc<MemRequestBuffer>,
    proc_delay : f32
}

impl<T: Cache + 'static> TimingCache<T>  {
    pub fn new<C>(
        sim : &Rc<Simulation>,
        p: &CacheParams, client : Rc<C>,
        proc_delay : f32
    ) -> Rc<Self> where C: CacheClient + 'static {
        Rc::new(Self {
            sim: sim.clone(),
            cache: RefCell::new(T::new(p)),
            client: client.clone(),
            req_queue: MemRequestBuffer::new(sim, 1),
            proc_delay
        })
    }

    fn schedule_proc(self: &Rc<Self>, trigger : Option<&Event>) {
        let self_inner = self.clone();
        if let Some(ev) = trigger {
            ev.delay(1.0).callback(move |sim| {
                self_inner.proc();
            });
        }
        else {
            self.sim.event(Some(self.proc_delay)).callback(move |sim| {
                self_inner.proc();
            });
        }
    }

    pub fn request(self: &Rc<Self>, req: &Rc<MemRequest>) -> Rc<Event> {
        let ev = self.req_queue.push(req.clone());
        self.schedule_proc(Some(&ev));
        ev
    }

    fn proc(self: &Rc<Self>) {

        if self.empty() {
            self.scheduled.set(false);
        }
        else {
            self.schedule_proc(None);
        }
    }
}




// impl Event for CacheReq {
//     fn process(&mut self, sim: &mut Simulation) {

//     }
// }
