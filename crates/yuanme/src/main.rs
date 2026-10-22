use yuanme_engine::{Cache, Inputs, Mode};

fn main() {
    let build = yuanme_engine::build(&Inputs::default(), &mut Cache::memory(""), Mode::Build);
    assert!(!build.failed());
}
