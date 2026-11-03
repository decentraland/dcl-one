#[derive(Default)]
pub struct BatchSizes {
    samples: u64,
    items: u64,
    min: usize,
    max: usize,
}

impl BatchSizes {
    pub fn record(&mut self, size: usize) {
        self.min = if self.samples == 0 {
            size
        } else {
            self.min.min(size)
        };
        self.max = self.max.max(size);
        self.items += size as u64;
        self.samples += 1;
    }

    pub fn range(&self) -> Option<(usize, f64, usize)> {
        (self.samples > 0).then(|| (self.min, self.items as f64 / self.samples as f64, self.max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_batches_count_but_no_samples_have_no_range() {
        let mut sizes = BatchSizes::default();
        assert_eq!(sizes.range(), None);
        sizes.record(4);
        sizes.record(0);
        sizes.record(2);
        assert_eq!(sizes.range(), Some((0, 2.0, 4)));
    }
}
