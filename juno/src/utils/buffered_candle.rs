use std::borrow::Cow;

use crate::{Candle, Interval};

pub struct BufferedCandle {
    interval: Interval,
    buffer_interval: Interval,
    buffer_candle: Option<Candle>,
    enabled: bool,
}

impl BufferedCandle {
    pub fn new(interval: Interval, buffer_interval: Option<Interval>) -> Self {
        if interval == 0 {
            panic!("interval 0")
        }

        let enabled = if let Some(buffer_interval) = buffer_interval {
            if interval > buffer_interval {
                panic!("interval larger than buffer interval")
            }
            buffer_interval > interval
        } else {
            false
        };

        let buffer_interval = buffer_interval.unwrap_or_default();
        Self {
            interval,
            buffer_interval,
            enabled,
            buffer_candle: None,
        }
    }

    pub fn buffer<'a>(&'a mut self, candle: &'a Candle) -> Option<Cow<'a, Candle>> {
        if !self.enabled {
            return Some(Cow::Borrowed(candle));
        }

        let ret = match self.buffer_candle {
            None => {
                self.buffer_candle = Some(Candle {
                    time: candle.time.floor(self.buffer_interval),
                    open: candle.open,
                    high: candle.high,
                    low: candle.low,
                    close: candle.close,
                    volume: candle.volume,
                });
                None
            }
            Some(ref mut buffer_candle) => {
                if candle.time >= buffer_candle.time + self.buffer_interval {
                    Some(*buffer_candle)
                } else {
                    *buffer_candle += candle;
                    None
                }
            }
        };

        let is_last = (candle.time + self.interval) % self.buffer_interval == 0;

        if let Some(ret) = ret {
            if is_last {
                panic!("too many missing candles");
            }
            self.buffer_candle = Some(*candle);
            Some(Cow::Owned(ret))
        } else if is_last {
            Some(Cow::Owned(self.buffer_candle.take().unwrap()))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Timestamp;

    use super::*;

    #[test]
    fn test_disabled_without_buffer_interval() {
        let input = Candle {
            time: 0.into(),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        };
        let expected_output = Some(Cow::Borrowed(&input));
        let mut target = BufferedCandle::new(2.into(), None);

        let output = target.buffer(&input);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_disabled_with_same_interval_and_buffer_interval() {
        let input = Candle {
            time: 0.into(),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        };
        let expected_output = Some(Cow::Borrowed(&input));
        let mut target = BufferedCandle::new(2.into(), Some(2.into()));

        let output = target.buffer(&input);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_buffered_simple() {
        let input1 = Candle {
            time: 0.into(),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        };
        let input2 = Candle {
            time: 1.into(),
            open: 4.0,
            high: 8.0,
            low: 2.0,
            close: 6.0,
            volume: 20.0,
        };
        let expected_output1 = None;
        let expected_output2 = Some(Cow::Borrowed(&Candle {
            time: Timestamp(0),
            open: 2.0,
            high: 8.0,
            low: 1.0,
            close: 6.0,
            volume: 30.0,
        }));
        let mut target = BufferedCandle::new(1.into(), Some(2.into()));

        let output1 = target.buffer(&input1);
        assert_eq!(output1, expected_output1);

        let output2 = target.buffer(&input2);
        assert_eq!(output2, expected_output2);
    }

    #[test]
    fn test_buffered_missing_beginning() {
        let input = Candle {
            time: 1.into(),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        };
        let expected_output = Some(Cow::Borrowed(&Candle {
            time: Timestamp(0),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        }));
        let mut target = BufferedCandle::new(1.into(), Some(2.into()));

        let output = target.buffer(&input);
        assert_eq!(output, expected_output);
    }

    #[test]
    fn test_buffered_missing_end() {
        let input1 = Candle {
            time: 0.into(),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        };
        let input2 = Candle {
            time: 2.into(),
            open: 4.0,
            high: 8.0,
            low: 2.0,
            close: 6.0,
            volume: 20.0,
        };
        let input3 = Candle {
            time: 3.into(),
            open: 8.0,
            high: 16.0,
            low: 4.0,
            close: 12.0,
            volume: 40.0,
        };
        let expected_output1 = None;
        let expected_output2 = Some(Cow::Borrowed(&Candle {
            time: Timestamp(0),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        }));
        let expected_output3 = Some(Cow::Borrowed(&Candle {
            time: Timestamp(2),
            open: 4.0,
            high: 16.0,
            low: 2.0,
            close: 12.0,
            volume: 60.0,
        }));
        let mut target = BufferedCandle::new(1.into(), Some(2.into()));

        let output1 = target.buffer(&input1);
        assert_eq!(output1, expected_output1);

        let output2 = target.buffer(&input2);
        assert_eq!(output2, expected_output2);

        let output3 = target.buffer(&input3);
        assert_eq!(output3, expected_output3);
    }

    #[test]
    fn test_buffered_missing_end_and_beginning() {
        let input1 = Candle {
            time: 0.into(),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        };
        let input2 = Candle {
            time: 4.into(),
            open: 4.0,
            high: 8.0,
            low: 2.0,
            close: 6.0,
            volume: 20.0,
        };
        let expected_output1 = None;
        let expected_output2 = Some(Cow::Borrowed(&Candle {
            time: Timestamp(0),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        }));
        let mut target = BufferedCandle::new(1.into(), Some(3.into()));

        let output1 = target.buffer(&input1);
        assert_eq!(output1, expected_output1);

        let output2 = target.buffer(&input2);
        assert_eq!(output2, expected_output2);
    }

    #[test]
    #[should_panic]
    fn test_buffered_missing_end_and_beginning_unsolvable_panics() {
        let input1 = Candle {
            time: 0.into(),
            open: 2.0,
            high: 4.0,
            low: 1.0,
            close: 3.0,
            volume: 10.0,
        };
        let input2 = Candle {
            time: 3.into(),
            open: 4.0,
            high: 8.0,
            low: 2.0,
            close: 6.0,
            volume: 20.0,
        };
        let expected_output1 = None;

        let mut target = BufferedCandle::new(1.into(), Some(2.into()));

        let output1 = target.buffer(&input1);
        assert_eq!(output1, expected_output1);

        target.buffer(&input2);
    }

    #[test]
    #[should_panic]
    fn test_zero_interval_panics() {
        BufferedCandle::new(0.into(), None);
    }

    #[test]
    #[should_panic]
    fn test_interval_greater_than_buffer_interval_panics() {
        BufferedCandle::new(2.into(), Some(1.into()));
    }
}
