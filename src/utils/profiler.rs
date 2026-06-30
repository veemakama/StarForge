use std::time::{Duration, Instant};
use std::mem::size_of;

#[cfg(feature = "memory-profiling")]
use std::alloc::{GlobalAlloc, Layout, System};

#[cfg(feature = "memory-profiling")]
#[derive(Debug)]
struct MemoryProfiler;

#[cfg(feature = "memory-profiling")]
impl GlobalAlloc for MemoryProfiler {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            if let Some(alloc_tracker) = &mut ALLOC_TRACKER {
                alloc_tracker.allocations.push((layout.size(), ptr as usize));
            }
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        if let Some(alloc_tracker) = &mut ALLOC_TRACKER {
            alloc_tracker.allocations.retain(|(size, addr)| ptr as usize != *addr);
        }
    }
}

pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[derive(Debug, Clone)]
pub struct ProfilePoint {
    pub label: String,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct MemoryPoint {
    pub label: String,
    pub timestamp: Duration,
    pub allocated_bytes: usize,
    pub deallocated_bytes: usize,
    pub current_bytes: usize,
    pub peak_bytes: usize,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryMetrics {
    pub allocated: usize,
    pub deallocated: usize,
    pub current: usize,
    pub peak: usize,
    pub samples: Vec<MemoryPoint>,
}

#[derive(Debug)]
pub struct Profiler {
    start: Instant,
    marks: Vec<(String, Instant)>,
    #[cfg(feature = "memory-profiling")]
    memory_tracker: Option<MemoryTracker>,
}

#[cfg(feature = "memory-profiling")]
struct MemoryTracker {
    start: Instant,
    current_memory: usize,
    peak_memory: usize,
    samples: Vec<(String, Instant, usize, usize, usize, usize)>,
}

#[cfg(not(feature = "memory-profiling"))]
struct MemoryTracker;

impl Profiler {
    pub fn start() -> Self {
        #[cfg(feature = "memory-profiling")]
        let memory_tracker: Option<MemoryTracker> = Some(MemoryTracker {
            start: Instant::now(),
            current_memory: 0,
            peak_memory: 0,
            samples: Vec::new(),
        });
        #[cfg(not(feature = "memory-profiling"))]
        let memory_tracker: Option<MemoryTracker> = None;

        Self {
            start: Instant::now(),
            marks: Vec::new(),
            #[cfg(feature = "memory-profiling")]
            memory_tracker,
        }
    }

    pub fn mark(&mut self, label: impl Into<String>) {
        self.marks.push((label.into(), Instant::now()));
        #[cfg(feature = "memory-profiling")]
        if let Some(tracker) = &mut self.memory_tracker {
            tracker.record_sample(label.into(), self.start.elapsed());
        }
    }

    pub fn get_memory_metrics(&self) -> MemoryMetrics {
        let mut metrics = MemoryMetrics::default();
        for (label, at) in &self.marks {
            metrics.samples.push(MemoryPoint {
                label: label.clone(),
                timestamp: at.duration_since(self.start),
                allocated_bytes: 0,
                deallocated_bytes: 0,
                current_bytes: 0,
                peak_bytes: 0,
            });
        }
        metrics
    }

    pub fn points(&self) -> Vec<ProfilePoint> {
        let mut last = self.start;
        let mut points = Vec::with_capacity(self.marks.len());
        for (label, at) in &self.marks {
            points.push(ProfilePoint {
                label: label.clone(),
                elapsed: at.duration_since(last),
            });
            last = *at;
        }
        points
    }

    pub fn total_elapsed(&self) -> Duration {
        match self.marks.last() {
            Some((_, at)) => at.duration_since(self.start),
            None => Duration::from_millis(0),
        }
    }
}
