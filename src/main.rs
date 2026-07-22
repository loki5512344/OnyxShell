#![no_std]
#![no_main]
#![allow(
    unsafe_op_in_unsafe_fn,
    non_snake_case,
    clippy::missing_safety_doc,
    static_mut_refs
)]

mod commands;
mod eval;
mod features;
mod io;
mod path;
mod pipeline;
mod repl;
mod syscalls;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start(argc: usize, argv: *const *const u8) -> ! {
    if argc > 1 && !argv.is_null() {
        let script_ptr = *argv.add(1);
        if !script_ptr.is_null() {
            let mut path = [0u8; 256];
            let mut i = 0;
            while *script_ptr.add(i) != 0 && i < 255 {
                path[i] = *script_ptr.add(i);
                i += 1;
            }
            commands::do_script(&path[..i]);
        }
        syscalls::exit(0);
    }

    syscalls::write(1, repl::VERSION_BANNER.as_ptr(), repl::VERSION_BANNER.len());
    features::env_init();

    let raw_ok = syscalls::enable_raw_mode() == 0;
    if raw_ok {
        repl::raw::raw_mode_repl();
    } else {
        repl::cooked::cooked_mode_repl();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        let _ = syscalls::disable_raw_mode();
        io::write_str("osh: internal panic — halting\n");
        syscalls::exit(101);
    }
}
