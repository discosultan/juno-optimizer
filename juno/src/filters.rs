use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Price {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl Price {
    pub fn round_down(&self, mut price: f64) -> f64 {
        if price < self.min {
            return 0.0;
        }
        if self.max > 0.0 {
            price = f64::min(price, self.max);
        }
        if self.step > 0.0 {
            price = (price / self.step).floor() * self.step;
        }

        price
    }

    pub fn valid(&self, price: f64) -> bool {
        (self.min == 0.0 || price >= self.min)
            && (self.max == 0.0 || price <= self.max)
            && (self.step == 0.0 || (price - self.min) % self.step == 0.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Size {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl Size {
    pub fn round_down(&self, mut size: f64) -> f64 {
        if size < self.min {
            return 0.0;
        }

        if self.max > 0.0 {
            size = f64::min(size, self.max);
        }
        if self.step > 0.0 {
            size = (size / self.step).floor() * self.step;
        }

        size
    }

    pub fn round_up(&self, size: f64) -> f64 {
        let mut size = size;
        if size < self.min {
            return 0.0;
        }
        size = f64::min(size, self.max);
        (size / self.step).ceil() * self.step
    }

    pub fn valid(&self, size: f64) -> bool {
        size >= self.min && size <= self.max && (size - self.min) % self.step == 0.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Filters {
    pub price: Price,
    pub size: Size,

    pub base_precision: u32,
    pub quote_precision: u32,
}
