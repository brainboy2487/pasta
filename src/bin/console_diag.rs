fn main() {
    #[cfg(windows)] {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::System::Console::{
            GetStdHandle, GetConsoleMode, PeekConsoleInputW, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, INPUT_RECORD,
        };
        use std::mem::size_of;
        use std::io::{self, Write};

        unsafe {
            let stdin = GetStdHandle(STD_INPUT_HANDLE);
            let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
            println!("stdin handle: 0x{:x}", stdin);
            println!("stdout handle: 0x{:x}", stdout);
            if stdin == INVALID_HANDLE_VALUE || stdout == INVALID_HANDLE_VALUE {
                println!("One of the standard handles is INVALID_HANDLE_VALUE");
            }
            let mut in_mode: u32 = 0;
            let mut out_mode: u32 = 0;
            let gm_in = GetConsoleMode(stdin, &mut in_mode as *mut u32);
            let gm_out = GetConsoleMode(stdout, &mut out_mode as *mut u32);
            println!("GetConsoleMode(stdin) returned {} mode=0x{:08x}", gm_in, in_mode);
            println!("GetConsoleMode(stdout) returned {} mode=0x{:08x}", gm_out, out_mode);

            let mut records: [INPUT_RECORD; 16] = std::mem::zeroed();
            let mut read = 0u32;
            let res = PeekConsoleInputW(stdin, records.as_mut_ptr(), records.len() as u32, &mut read as *mut u32);
            println!("PeekConsoleInputW returned {} read={} record_size={} bytes", res, read, size_of::<INPUT_RECORD>());
            for i in 0..(read as usize) {
                let ev = &records[i];
                println!("event[{}] type={} (EventType)", i, ev.EventType);
            }
            println!("Press Enter to exit diagnostic...");
            let _ = io::stdout().flush();
            let mut s = String::new();
            let _ = io::stdin().read_line(&mut s);
        }
    }
    #[cfg(not(windows))] {
        println!("Console diagnostic only available on Windows");
    }
}
