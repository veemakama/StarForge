use std::time::{Duration, Instant};

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

#[derive(Debug)]
pub struct Profiler {
    start: Instant,
    marks: Vec<(String, Instant)>,
}

impl Profiler {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
            marks: Vec::new(),
        }
    }

    pub fn mark(&mut self, label: impl Into<String>) {
        self.marks.push((label.into(), Instant::now()));
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
