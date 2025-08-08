// Bring the helper function into scope
mod utils;

struct User {
    id: u32,
}

fn main() {
    let _ = User { id: 1 };
    do_stuff();
    utils::helper();
}

fn do_stuff() {
    println!("Doing stuff!");
}
