// A bare string is not a context tag. Before the CapabilityTag
// newtype, `ctx.add_present("rust")` compiled and the `stack:rust`
// probe silently never matched; now the wrong call fails cargo check.

use vibe_resolver::ActivationContext;

fn main() {
    let mut ctx = ActivationContext::default();
    ctx.add_present("rust");
}
