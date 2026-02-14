// Wait flags
pub const WNOHANG: usize = 1;
pub const WUNTRACED: usize = 2;
pub const WCONTINUED: usize = 8;

// Exit status macros (simplified for now)
// In C:
// WIFEXITED(status)    -> WTERMSIG(status) == 0
// WEXITSTATUS(status)  -> (status & 0xff00) >> 8
// WIFSIGNALED(status)  -> (((signed char) (((status) & 0x7f) + 1) >> 1) > 0)
// WTERMSIG(status)     -> (status & 0x7f)

// We will construct the status word as:
// (exit_code << 8) | termination_signal
