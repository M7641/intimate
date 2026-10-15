//! Print which zones touch. `cargo run -p atlantis-core --example adj`
fn main() {
    let b = atlantis_core::Board::single_lane();
    for (x, y) in b.zone_adjacency() {
        println!("{} - {}", b.zone(x).name, b.zone(y).name);
    }
}
