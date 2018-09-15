//! 1D 函数采样（宿主侧绘图输入；不依赖 Apollo）。

mod sample;
mod types;

pub use sample::sample_1d;
pub use types::{SampleDomain, SamplePoint, SampledCurve, SamplingPolicy};
