#![no_main]

use libfuzzer_sys::fuzz_target;
use libguix::parse_channels_list;

fuzz_target!(|s: &str| {
    let _ = parse_channels_list(s);
});
