
#![feature(fn_traits)]
#![feature(trait_alias)]

extern crate num;
extern crate num_derive;
extern crate libc;
extern crate memmap2;

mod des;
mod cache;
mod mesh;
// mod rvemu;


fn main() {
    mesh::test_mesh()

}
