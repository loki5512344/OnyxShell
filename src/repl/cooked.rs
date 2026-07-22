use super::{LINE_MAX, PROMPT};
use crate::{eval, io, syscalls};

static mut G_LINE: [u8; LINE_MAX] = [0u8; LINE_MAX];

pub unsafe fn cooked_mode_repl() -> ! {
    loop {
        syscalls::write(1, PROMPT.as_ptr(), PROMPT.len());
        let n = io::read_line(&mut G_LINE);
        if n == 0 {
            continue;
        }
        let mut end = n;
        while end > 0
            && (G_LINE[end - 1] == b'\n' || G_LINE[end - 1] == b'\r' || G_LINE[end - 1] == 0)
        {
            end -= 1;
        }
        if end == 0 {
            continue;
        }
        eval::eval_line(&G_LINE[..end]);
    }
}
