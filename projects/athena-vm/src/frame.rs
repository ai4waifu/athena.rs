//! 调用 / 作用域执行帧（非引擎词法 `ScopeFrame`）。

/// 单层执行帧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// 本帧在 [`super::SlotTable`] 中的槽基址。
    pub base: u32,
    /// 本帧局部槽数量。
    pub locals: u32,
    /// 指令指针（线性骨架）或块内 PC。
    pub pc: u32,
}

impl Frame {
    /// 构造帧。
    #[inline]
    pub const fn new(base: u32, locals: u32) -> Self {
        Self { base, locals, pc: 0 }
    }

    /// 本帧槽区间结束（不含）。
    #[inline]
    pub const fn end(self) -> u32 {
        self.base.saturating_add(self.locals)
    }

    /// 将帧内局部下标映射为绝对槽下标。
    #[inline]
    pub const fn absolute(self, local: u32) -> Option<u32> {
        if local >= self.locals { None } else { Some(self.base.saturating_add(local)) }
    }
}

/// 帧栈（支持后续 region / 调用嵌套）。
#[derive(Debug, Clone, Default)]
pub struct FrameStack {
    frames: Vec<Frame>,
}

impl FrameStack {
    /// 空栈。
    #[inline]
    pub const fn new() -> Self {
        Self { frames: Vec::new() }
    }

    /// 压入帧。
    #[inline]
    pub fn push(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    /// 弹出帧。
    #[inline]
    pub fn pop(&mut self) -> Option<Frame> {
        self.frames.pop()
    }

    /// 当前帧。
    #[inline]
    pub fn current(&self) -> Option<&Frame> {
        self.frames.last()
    }

    /// 当前帧（可变）。
    #[inline]
    pub fn current_mut(&mut self) -> Option<&mut Frame> {
        self.frames.last_mut()
    }

    /// 深度。
    #[inline]
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// 是否为空。
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}
