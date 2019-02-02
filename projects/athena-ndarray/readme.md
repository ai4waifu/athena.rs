# athena-ndarray

`athena-ndarray` 是 CAS 值语义 N 维数组与 out-of-core 分块存储库。逻辑 shape 使用 checked `u64`，数组可由文件、mmap、对象存储或自定义
store 承载；计算按显式内存预算分块，不要求全部驻留 RAM。

本 crate 不定义 Titan Tensor 的 dtype、device、backend、kernel、stream/event、Autograd、执行图或异步设备生命周期。需要设备执行时必须显式适配
Titan。
