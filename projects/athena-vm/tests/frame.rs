//! 执行帧合同。

use athena_vm::{Frame, FrameStack};

#[test]
fn frame_absolute_and_stack() {
    let frame = Frame::new(4, 3);
    assert_eq!(frame.absolute(0), Some(4));
    assert_eq!(frame.absolute(2), Some(6));
    assert_eq!(frame.absolute(3), None);
    assert_eq!(frame.end(), 7);

    let mut stack = FrameStack::new();
    stack.push(frame);
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.current().map(|f| f.base), Some(4));
    assert!(stack.pop().is_some());
    assert!(stack.is_empty());
}
