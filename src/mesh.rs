

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rand::prelude::*;

use crate::des::core::*;
use crate::des::resource::*;
use crate::des::fifobuf::*;

type Coords = (u32, u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    North,
    East,
    South,
    West,
    Inject,
    Eject
}

const IN_DIRS: [Direction; 5] = [
    Direction::Inject,
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West
];


const OUT_DIRS: [Direction; 5] = [
    Direction::Eject,
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West
];

impl Direction {
    pub fn flip(d : Self) -> Self {
        match d {
            Direction::North => Direction::South,
            Direction::East => Direction::West,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::Inject => Direction::Eject,
            Direction::Eject => Direction::Inject
        }
    }
}

#[derive(Debug)]
struct Packet {
    dest: Coords,
    payload : u64
}

type PacketBuffer = FifoBuf<Packet>;

pub struct RouterNeighbors {
    north : RefCell<Option<Rc<MeshRouter>>>,
    east  : RefCell<Option<Rc<MeshRouter>>>,
    south : RefCell<Option<Rc<MeshRouter>>>,
    west  : RefCell<Option<Rc<MeshRouter>>>
}

impl RouterNeighbors {
    fn new() -> Self {
        Self {
            north: RefCell::new(None),
            east:  RefCell::new(None),
            south: RefCell::new(None),
            west:  RefCell::new(None)
        }
    }
}

pub struct RouterBuffers {
    inject : Rc<PacketBuffer>,
    north  : Rc<PacketBuffer>,
    east   : Rc<PacketBuffer>,
    south  : Rc<PacketBuffer>,
    west   : Rc<PacketBuffer>
}

impl RouterBuffers {
    fn new(sim : &Rc<Simulation>, buf_size : usize) -> Self {
        Self {
            inject : PacketBuffer::new(sim, buf_size),
            north  : PacketBuffer::new(sim, buf_size),
            east   : PacketBuffer::new(sim, buf_size),
            south  : PacketBuffer::new(sim, buf_size),
            west   : PacketBuffer::new(sim, buf_size),
        }
    }
}

pub struct InputLinks {
    inject : Rc<PacketBuffer>,
    north  : Rc<PacketBuffer>,
    east   : Rc<PacketBuffer>,
    south  : Rc<PacketBuffer>,
    west   : Rc<PacketBuffer>
}

impl InputLinks {
    fn new(sim : &Rc<Simulation>) -> Self {
        Self {
            inject : PacketBuffer::new(sim, 1),
            north  : PacketBuffer::new(sim, 1),
            east   : PacketBuffer::new(sim, 1),
            south  : PacketBuffer::new(sim, 1),
            west   : PacketBuffer::new(sim, 1)
        }
    }
}

struct RoundRobinArbiter {
    max : usize,
    i : Cell<usize>
}

impl RoundRobinArbiter {
    fn new(max : usize) -> Self {
        Self {
            max,
            i: Cell::new(0)
        }
    }

    fn get(&self) -> usize { self.i.get() }

    fn inc(&self) {
        self.i.set((self.i.get() + 1) % self.max);
    }
}

struct Arbiters {
    north : RoundRobinArbiter,
    east  : RoundRobinArbiter,
    south : RoundRobinArbiter,
    west  : RoundRobinArbiter,
    eject : RoundRobinArbiter,
}

impl Arbiters {
    fn new() -> Self {
        Arbiters {
            north : RoundRobinArbiter::new(5),
            east  : RoundRobinArbiter::new(5),
            south : RoundRobinArbiter::new(5),
            west  : RoundRobinArbiter::new(5),
            eject : RoundRobinArbiter::new(5),
        }
    }
}

pub struct MeshRouter {
    sim : Rc<Simulation>,
    coords : Coords,
    ns : RouterNeighbors,
    bufs : RouterBuffers,
    arbs : Arbiters,
    links : InputLinks,
    proc_delay : f32,
    scheduled : Cell<bool>,
    sent : Cell<usize>,
    received : Cell<usize>,
}


impl MeshRouter {
    pub fn new(
        sim : &Rc<Simulation>,
        coords : Coords,
        buf_size : usize,
        proc_delay : f32
    ) -> Rc<Self> {
        Rc::new(Self {
            sim: sim.clone(),
            coords,
            ns : RouterNeighbors::new(),
            bufs : RouterBuffers::new(sim, buf_size),
            arbs : Arbiters::new(),
            links : InputLinks::new(sim),
            proc_delay,
            scheduled: Cell::new(false),
            sent: Cell::new(0),
            received : Cell::new(0)
        })
    }

    fn empty(self : &Rc<Self>) -> bool {
        self.bufs.inject.empty() &&
        self.bufs.north.empty() &&
        self.bufs.east.empty() &&
        self.bufs.south.empty() &&
        self.bufs.west.empty() &&
        self.links.inject.empty() &&
        self.links.north.empty() &&
        self.links.east.empty() &&
        self.links.south.empty() &&
        self.links.west.empty()
    }

    fn route(self : &Rc<Self>, p : &Rc<Packet>) -> Direction {
        if      p.dest.1 > self.coords.1 { Direction::East }
        else if p.dest.1 < self.coords.1 { Direction::West }
        else if p.dest.0 > self.coords.0 { Direction::North }
        else if p.dest.0 < self.coords.0 { Direction::South }
        else { Direction::Eject }
    }

    fn get_link(self: &Rc<Self>, dir : Direction) -> Rc<PacketBuffer> {
        match dir {
            Direction::Inject => self.links.inject.clone(),
            Direction::North => self.links.north.clone(),
            Direction::East => self.links.east.clone(),
            Direction::South => self.links.south.clone(),
            Direction::West => self.links.west.clone(),
            _ => unreachable!()
        }
    }

    fn get_neighbor(self: &Rc<Self>, dir : Direction) -> Rc<Self> {
        let c = match dir {
            Direction::North => &self.ns.north,
            Direction::East => &self.ns.east,
            Direction::South => &self.ns.south,
            Direction::West => &self.ns.west,
            _ => unreachable!()
        };

        c.borrow()
            .as_ref()
            .expect("No neighbor?")
            .clone()
    }

    fn get_buf(self: &Rc<Self>, dir : Direction) -> Rc<PacketBuffer> {
        match dir {
            Direction::Inject => self.bufs.inject.clone(),
            Direction::North => self.bufs.north.clone(),
            Direction::East => self.bufs.east.clone(),
            Direction::South => self.bufs.south.clone(),
            Direction::West => self.bufs.west.clone(),
            _ => unreachable!()
        }
    }

    fn get_arb<'a>(self: &'a Rc<Self>, dir : Direction) -> &'a RoundRobinArbiter {
        match dir {
            Direction::North => &self.arbs.north,
            Direction::East => &self.arbs.east,
            Direction::South => &self.arbs.south,
            Direction::West => &self.arbs.west,
            Direction::Eject => &self.arbs.eject,
            _ => unreachable!()
        }
    }

    fn schedule_proc(self : &Rc<Self>) {
        if !self.scheduled.get() {
            let r = self.clone();
            self.sim.event(Some(self.proc_delay)).callback(move |sim| {
                r.proc();
            });

            self.scheduled.set(true);
        }
    }

    fn receive(
        self : &Rc<Self>,
        from_dir : Direction,
        p : &Rc<Packet>
    ) -> Rc<Event> {
        self.schedule_proc();
        let link = self.get_link(from_dir);
        link.push(p.clone()).delay(1.0)
    }

    fn proc(self : &Rc<Self>) {

        for dir in IN_DIRS {
            let link = self.get_link(dir);
            let buf = self.get_buf(dir);
            if let Some(p) = link.peek() {
                if dir == Direction::Inject {
                    self.sent.set(self.sent.get() + 1);
                }

                link.pend();
                buf.push(p).callback(move |sim| { link.pop(); });
            }
        }

        for odir in OUT_DIRS {
            let arb = self.get_arb(odir);

            for off in 0..IN_DIRS.len() {
                let i = (arb.get() + off) % 5;
                let idir = IN_DIRS
                    .get(i).expect("Out of bounds??").clone();

                let ib = self.get_buf(idir);

                if let Some(p) = ib.peek() {
                    if self.route(&p) == odir {
                        ib.pend();

                        if odir == Direction::Eject {
                            self.received.set(self.received.get() + 1);
                            ib.pop();
                        }
                        else {
                            let or = self.get_neighbor(odir);
                            or.receive(Direction::flip(odir), &p)
                                .callback(move |sim| { ib.pop(); });
                        }
                        break;
                    }
                }
            }

            arb.inc();
        }

        if self.empty() {
            self.scheduled.set(false);
        }
        else {
            let r = self.clone();
            self.sim.event(Some(self.proc_delay)).callback(move |sim| {
                r.proc();
            });
        }
    }
}

pub struct Mesh {
    size : Coords,
    rs : Vec<Rc<MeshRouter>>
}

impl Mesh {
    pub fn new(
        sim : &Rc<Simulation>,
        size : Coords,
        buf_size : usize,
        proc_delay : f32
    ) -> Self {
        let mut rs = Vec::new();

        for r in 0..size.0 {
            for c in 0..size.1 {
                rs.push(MeshRouter::new(
                    sim,
                    (r, c),
                    buf_size,
                    proc_delay));
            }
        }


        for r in 0..size.0 {
            for c in 0..size.1 {
                let router =
                    rs.get_mut((r * size.1 + c) as usize)
                        .unwrap()
                        .clone();

                router.ns.north.replace(if r < size.0 - 1 {
                        Some(rs.get(((r + 1) * size.1 + c) as usize)
                            .unwrap().clone())
                    }
                    else {
                        None
                    });

                router.ns.east.replace(if c < size.1 - 1 {
                        Some(rs.get((r * size.1 + (c + 1)) as usize)
                            .unwrap().clone())
                    }
                    else {
                        None
                    });

                router.ns.south.replace(if r > 0 {
                        Some(rs.get(((r - 1) * size.1 + c) as usize)
                            .unwrap().clone())
                    }
                    else {
                        None
                    });

                router.ns.west.replace(if c > 0 {
                        Some(rs.get((r * size.1 + (c - 1)) as usize)
                            .unwrap().clone())
                    }
                    else {
                        None
                    });

            }
        }

        Mesh { size, rs }
    }

    pub fn get_router(&mut self, r : u32, c : u32) -> Rc<MeshRouter> {
        self.rs.get_mut((r * self.size.1 + c) as usize).unwrap().clone()
    }
}



pub fn test_mesh() {
    println!("Setting up...");

    let sim = Simulation::new();

    let mut m =
        Mesh::new(&sim, (32, 32), 4, 1.0);

    let mut rng = rand::thread_rng();

    for r in 0..m.size.0 {
        for c in 0..m.size.1 {
            let r = m.get_router(r, c);

            for _ in 0..100 {
                let dr : u32 = rng.gen_range(0..m.size.0);
                let dc : u32 = rng.gen_range(0..m.size.1);

                r.receive(
                    Direction::Inject,
                    &Rc::new(Packet { dest: (dr, dc), payload: (dr + dc).into()}));
            }
        }
    }

    println!("Running...");


    let now = SystemTime::now();
    sim.run(None);
    if let Ok(elapsed) = now.elapsed() {
        let secs : f32 = (elapsed.as_millis() as f32) / 1000.0;
        println!("Took {} secs", secs);
        println!("Took {} ticks", sim.now());
        println!("{} ticks/secs", sim.now() / secs);
        println!("{} events/secs", (sim.num_events() as f32) / secs);
    }
}
