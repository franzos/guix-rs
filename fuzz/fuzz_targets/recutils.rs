#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|s: &str| {
    libguix::fuzz_api::recutils(s);
});
