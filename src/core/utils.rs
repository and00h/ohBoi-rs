pub(crate) struct Counter {
    cnt: u32,
    pub limit: u32,
    period: u32,
    period_cnt: u32,
}

impl Counter {
    pub fn new(limit: u32, period: u32) -> Self {
        Self { cnt: 0, limit, period, period_cnt: 0 }
    }

    pub fn step(&mut self) -> bool {
        if self.cnt < self.limit {
            self.period_cnt += 1;
            if self.period_cnt == self.period {
                self.period_cnt = 0;
                self.cnt += 1;
            }
        }

        self.cnt == self.limit
    }

    #[inline]
    pub fn reset(&mut self) {
        self.cnt = 0;
        self.period_cnt = 0;
    }

    #[inline]
    pub fn expired(&self) -> bool {
        self.cnt == self.limit
    }
}

pub(crate) struct FallingEdgeDetector {
    old: bool
}

impl FallingEdgeDetector {
    //pub fn new() -> Self {
    //    Self { old: false }
    //}

    pub fn with_initial_value(initial: bool) -> Self {
        Self { old: initial }
    }

    #[inline]
    pub fn detect(&mut self, new: bool) -> bool {
        let ret = self.old & !new;
        self.old = new;
        ret
    }
}