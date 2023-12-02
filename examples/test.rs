
use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct Trallocator<A: GlobalAlloc>(pub A, AtomicU64);

unsafe impl<A: GlobalAlloc> GlobalAlloc for Trallocator<A> {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        self.1.fetch_add(l.size() as u64, Ordering::SeqCst);
        self.0.alloc(l)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, l: Layout) {
        self.0.dealloc(ptr, l);
        self.1.fetch_sub(l.size() as u64, Ordering::SeqCst);
    }
}

impl<A: GlobalAlloc> Trallocator<A> {
    pub const fn new(a: A) -> Self {
        Trallocator(a, AtomicU64::new(0))
    }

    pub fn reset(&self) {
        self.1.store(0, Ordering::SeqCst);
    }
    pub fn get(&self) -> u64 {
        self.1.load(Ordering::SeqCst)
    }
}

use memory_stats::memory_stats;
use std::alloc::System;
#[global_allocator]
static GLOBAL: Trallocator<System> = Trallocator::new(System);

fn main() {
    GLOBAL.reset();
    println!("memory used: {} bytes", GLOBAL.get());
    GLOBAL.reset();
    {
        // let mut vec = vec![1, 2, 3, 4];
        // for i in 5..20 {
        //     vec.push(i);
        //     println!("memory used: {} bytes", GLOBAL.get());
        // }
        // for v in vec {
        //     println!("{}", v);
        // }
    }
    
    // for (pid, process) in sys.processes() {
    //     println!("[{pid}] {} {:?}", process.name(), process.disk_usage());
        
    // }

    let x = 1;
    // For some reason this does not print zero =/
    println!("memory used: {} bytes", GLOBAL.get());
    let y = vec![1, 2, 3, 4];
    println!("memory used: {} bytes", GLOBAL.get());
    let z = Box::new(3u64);
    println!("memory used: {} bytes", GLOBAL.get());

    let x = 4_u32;
    println!("Original x-value: {}", x);
    
    if let Some(usage) = memory_stats() {
        println!("Current physical memory usage: {}", usage.physical_mem);
        println!("Current virtual memory usage: {}", usage.virtual_mem);
    } else {
        println!("Couldn't get the current memory usage :(");
    }
    
    let value1 = vec![10;1024000];
    
    std::thread::sleep(std::time::Duration::from_secs(1));

    if let Some(usage) = memory_stats() {
        println!("Current physical memory usage: {}", usage.physical_mem);
        println!("Current virtual memory usage: {}", usage.virtual_mem);
    } else {
        println!("Couldn't get the current memory usage :(");
    }
    
    std::thread::sleep(std::time::Duration::from_secs(1));
    let value = [10;102400];
    
    std::thread::sleep(std::time::Duration::from_secs(1));
    if let Some(usage) = memory_stats() {
        println!("Current physical memory usage: {}", usage.physical_mem);
        println!("Current virtual memory usage: {}", usage.virtual_mem);
    } else {
        println!("Couldn't get the current memory usage :(");
    }

    let val = Box::new(value);
    
    std::thread::sleep(std::time::Duration::from_secs(1));
    if let Some(usage) = memory_stats() {
        println!("Current physical memory usage: {}", usage.physical_mem);
        println!("Current virtual memory usage: {}", usage.virtual_mem);
    } else {
        println!("Couldn't get the current memory usage :(");
    }

    let mut value = vec![];
    for v in 0..100 {
        value.push(Box::new(v));
    }
    
    std::thread::sleep(std::time::Duration::from_secs(1));

    if let Some(usage) = memory_stats() {
        println!("Current physical memory usage: {}", usage.physical_mem);
        println!("Current virtual memory usage: {}", usage.virtual_mem);
    } else {
        println!("Couldn't get the current memory usage :(");
    }
    std::thread::sleep(std::time::Duration::from_secs(1000));
    
}