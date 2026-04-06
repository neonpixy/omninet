// Import the existing divinity_ffi.h — all 990 C functions available here.
pub const c = @cImport({
    @cInclude("divinity_ffi.h");
});
