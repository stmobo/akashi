use tcnati_rust::player::Player;
use tcnati_rust::resources::{LocalResourceStore, ResourceCount, ResourceType};

fn main() {
    let rsc_store = LocalResourceStore::new(vec![ResourceType::new(0, "points")]);
}
