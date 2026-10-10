// SQLx tracks existing included files, but a newly added migration must also
// invalidate every crate that embeds the migration list in an incremental build.
fn main() {
    println!("cargo:rerun-if-changed=../../core/migrations");
}
