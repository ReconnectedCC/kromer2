fn main() {
    // trigger recompilation when a new migration is added
    println!("cargo:rerun-if-changed=migrations");

    built::write_built_file().expect("Failed to acquire build-time information");
}
