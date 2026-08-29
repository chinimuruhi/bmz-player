// CPAL 0.17+ keeps `LoopbackDevice` drop glue in every macOS output stream. That
// introduces a strong reference to AudioHardwareDestroyProcessTap (macOS 14.2),
// even though BMZ Player only creates output streams and never uses loopback
// capture. Satisfy that unreachable drop path locally so dyld can load the
// output-only player on older macOS versions.
//
// If BMZ adds macOS input/loopback capture in the future, remove this shim and
// load the process-tap API conditionally on macOS 14.2 or newer instead.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
extern "C" fn AudioHardwareDestroyProcessTap(_tap_id: u32) -> i32 {
    // kAudioHardwareUnsupportedOperationError ('unop'). This path cannot be
    // reached by BMZ Player's output-only use of CPAL.
    i32::from_be_bytes(*b"unop")
}
