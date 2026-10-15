fn main() {
    println!("Mathcraft {}", env!("CARGO_PKG_VERSION"));
    println!(
        "Platform: {} / {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("Development scaffold; the calculation engine is not implemented yet.");
}
