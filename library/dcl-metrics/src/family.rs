//! Declared families bind label names once. Callers supply the fixed number of values.
use crate::{Counter, Descriptor, Gauge, Histogram, Label, SharedString, Unit};
use std::marker::PhantomData;

pub struct Family<H, const N: usize> {
    pub descriptor: Descriptor,
    keys: &'static [&'static str; N],
    handle: PhantomData<H>,
}

pub type CounterFamily<const N: usize> = Family<Counter, N>;
pub type GaugeFamily<const N: usize> = Family<Gauge, N>;
pub type HistogramFamily<const N: usize> = Family<Histogram, N>;

impl<H, const N: usize> Family<H, N> {
    const fn declared(descriptor: Descriptor, keys: &'static [&'static str; N]) -> Self {
        Self {
            descriptor,
            keys,
            handle: PhantomData,
        }
    }

    fn labels(&self, values: [SharedString; N]) -> Vec<Label> {
        self.keys
            .iter()
            .zip(values)
            .map(|(&key, value)| Label::new(key, value))
            .collect()
    }
}

impl<const N: usize> CounterFamily<N> {
    pub const fn new(
        name: &'static str,
        help: &'static str,
        unit: Option<Unit>,
        keys: &'static [&'static str; N],
    ) -> Self {
        Self::declared(Descriptor::counter(name, help, unit, keys), keys)
    }

    pub fn handle(&self, values: [&'static str; N]) -> Counter {
        self.handle_owned(values.map(SharedString::const_str))
    }

    /// Accept runtime values from a bounded source, such as a matched route or configured job.
    pub fn handle_owned(&self, values: [SharedString; N]) -> Counter {
        metrics::counter!(self.descriptor.name, self.labels(values))
    }
}

impl<const N: usize> GaugeFamily<N> {
    pub const fn new(
        name: &'static str,
        help: &'static str,
        unit: Option<Unit>,
        keys: &'static [&'static str; N],
    ) -> Self {
        Self::declared(Descriptor::gauge(name, help, unit, keys), keys)
    }

    pub fn handle(&self, values: [&'static str; N]) -> Gauge {
        self.handle_owned(values.map(SharedString::const_str))
    }

    /// Accept runtime values from a bounded source, such as a matched route or configured job.
    pub fn handle_owned(&self, values: [SharedString; N]) -> Gauge {
        metrics::gauge!(self.descriptor.name, self.labels(values))
    }
}

impl<const N: usize> HistogramFamily<N> {
    pub const fn new(
        name: &'static str,
        help: &'static str,
        unit: Option<Unit>,
        keys: &'static [&'static str; N],
        buckets: &'static [f64],
    ) -> Self {
        Self::declared(Descriptor::histogram(name, help, unit, keys, buckets), keys)
    }

    pub fn handle(&self, values: [&'static str; N]) -> Histogram {
        self.handle_owned(values.map(SharedString::const_str))
    }

    /// Accept runtime values from a bounded source, such as a matched route or configured job.
    pub fn handle_owned(&self, values: [SharedString; N]) -> Histogram {
        metrics::histogram!(self.descriptor.name, self.labels(values))
    }
}
